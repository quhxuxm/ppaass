use std::sync::Arc;
use std::{fmt::Debug, str::FromStr};
use std::{net::SocketAddr, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use deadpool::{
    managed::{self, Manager, Object, Pool},
    Status,
};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};

use ppaass_common::generate_uuid;
use ppaass_io::PpaassMessageFramed;

use anyhow::Result;
use ppaass_protocol::{
    heartbeat::HeartbeatResponsePayload, PpaassMessage, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadEncryptionSelector,
    PpaassMessagePayloadParts, PpaassMessagePayloadType, PpaassMessageUtil,
};
use tokio::{net::TcpStream, sync::Mutex, task::JoinHandle};
use tracing::{debug, error, info};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, AgentServerPayloadEncryptionTypeSelector};

pub(crate) type ProxyMessageFramed = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;
pub(crate) type ProxyMessageFramedWriter = SplitSink<ProxyMessageFramed, PpaassMessage>;
pub(crate) type ProxyMessageFramedReader = SplitStream<ProxyMessageFramed>;

pub(crate) struct ProxyConnection {
    id: String,
    reader: Arc<Mutex<ProxyMessageFramedReader>>,
    writer: Arc<Mutex<ProxyMessageFramedWriter>>,
}

impl ProxyConnection {
    pub(crate) fn get_id(&self) -> &String {
        &self.id
    }
    pub(crate) fn get_reader(&self) -> Arc<Mutex<ProxyMessageFramedReader>> {
        self.reader.clone()
    }

    pub(crate) fn get_writer(&self) -> Arc<Mutex<ProxyMessageFramedWriter>> {
        self.writer.clone()
    }
}

impl Debug for ProxyConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConnection").field("id", &self.id).finish()
    }
}

pub(crate) struct ProxyConnectionManager {
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    proxy_addresses: Vec<SocketAddr>,
}

impl ProxyConnectionManager {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Self {
        let proxy_addresses_configuration = configuration
            .get_proxy_addresses()
            .expect("Fail to parse proxy addresses from configuration file");
        let mut proxy_addresses: Vec<SocketAddr> = Vec::new();
        for address in proxy_addresses_configuration {
            match SocketAddr::from_str(&address) {
                Ok(r) => {
                    debug!("Put proxy server address: {:?}", r);
                    proxy_addresses.push(r);
                },
                Err(e) => {
                    error!("Fail to convert proxy address to socket address because of error: {:#?}", e);
                },
            }
        }
        if proxy_addresses.is_empty() {
            error!("No available proxy address for runtime to use.");
            panic!("No available proxy address for runtime to use.")
        }
        Self {
            configuration,
            rsa_crypto_fetcher,
            proxy_addresses,
        }
    }
}

#[async_trait]
impl Manager for ProxyConnectionManager {
    type Type = ProxyConnection;
    type Error = anyhow::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let proxy_tcp_stream = TcpStream::connect(self.proxy_addresses.as_slice()).await?;
        let ppaass_message_framed = PpaassMessageFramed::new(proxy_tcp_stream, self.configuration.get_compress(), 1024 * 64, self.rsa_crypto_fetcher.clone())
            .context("fail to create ppaass message framed")?;
        let (ppaass_message_framed_write, ppaass_message_framed_read) = ppaass_message_framed.split();
        Ok(ProxyConnection {
            id: generate_uuid(),
            reader: Arc::new(Mutex::new(ppaass_message_framed_read)),
            writer: Arc::new(Mutex::new(ppaass_message_framed_write)),
        })
    }

    async fn recycle(&self, _proxy_message_framed: &mut Self::Type) -> managed::RecycleResult<Self::Error> {
        Ok(())
    }
}

pub(crate) struct ProxyConnectionPoolKeepalive {
    proxy_connection_pool: Pool<ProxyConnectionManager>,
    configuration: Arc<AgentServerConfig>,
}

impl ProxyConnectionPoolKeepalive {
    pub(crate) fn new(proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>) -> Self {
        Self {
            proxy_connection_pool,
            configuration,
        }
    }

    pub(crate) async fn start(self) -> Result<JoinHandle<Result<()>>> {
        let connection_pool = self.proxy_connection_pool;
        let configuration = self.configuration;
        let keepalive_guard = tokio::spawn(async move {
            let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                // Heartbeat with proxy server every 5 seconds for every connection in the pool
                heartbeat_interval.tick().await;

                let Status { max_size, .. } = connection_pool.status();
                for _ in 0..max_size {
                    let idle_connection = connection_pool.get().await.map_err(|e| anyhow::anyhow!(e))?;
                    debug!("Heartbeat on proxy connection: [{:?}]", idle_connection.as_ref());

                    let configuration = configuration.clone();
                    tokio::spawn(async move {
                        let user_token = configuration
                            .get_user_token()
                            .as_ref()
                            .context("Can not get user token from configuration file")?;
                        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                        let heartbeat_message = PpaassMessageUtil::create_agent_heartbeat_request(user_token, payload_encryption)?;
                        info!(
                            "Proxy connection [{:?}] send heartbeat request to proxy: {heartbeat_message:?}",
                            idle_connection.as_ref()
                        );
                        {
                            let idle_connection_writer = idle_connection.get_writer();
                            let mut idle_connection_writer = idle_connection_writer.lock().await;
                            idle_connection_writer.send(heartbeat_message).await?;
                        }
                        let heartbeat_response_message = {
                            let idle_connection_reader = idle_connection.get_reader();
                            let mut idle_connection_reader = idle_connection_reader.lock().await;
                            idle_connection_reader.next().await
                        };

                        match heartbeat_response_message {
                            None => {
                                let idle_connection = Object::take(idle_connection);
                                let idle_connection_writer = idle_connection.get_writer();
                                let mut idle_connection_writer = idle_connection_writer.lock().await;
                                idle_connection_writer.close().await?;
                                info!("Proxy connection [{:?}] disconnected already, shutdown it.", idle_connection);
                                Ok::<_, anyhow::Error>(())
                            },
                            Some(Err(e)) => {
                                error!(
                                    "Fail to do idle heartbeat on proxy connection [{:?}] because of error: {e:?}",
                                    idle_connection.as_ref()
                                );
                                let idle_connection = Object::take(idle_connection);
                                let idle_connection_writer = idle_connection.get_writer();
                                let mut idle_connection_writer = idle_connection_writer.lock().await;
                                idle_connection_writer.close().await?;
                                Err(anyhow::anyhow!("Fail to do idele heartbeat on proxy connection."))
                            },
                            Some(Ok(heartbeat_response_message)) => {
                                let PpaassMessageParts { payload_bytes, .. } = heartbeat_response_message.split();
                                let PpaassMessagePayloadParts { payload_type, data } = TryInto::<PpaassMessagePayload>::try_into(payload_bytes)?.split();
                                if let PpaassMessagePayloadType::ProxyPayload(ppaass_protocol::PpaassMessageProxyPayloadTypeValue::IdleHeartbeatSuccess) =
                                    payload_type
                                {
                                    let heartbeat_success_response: HeartbeatResponsePayload = data.try_into()?;
                                    info!(
                                        "Success to do idle heartbeat on proxy connection: {:?}, heartbeat response: {heartbeat_success_response:?}",
                                        idle_connection.as_ref()
                                    );
                                    Ok::<_, anyhow::Error>(())
                                } else {
                                    error!(
                                        "Fail to do idle heartbeat on proxy connection [{:?}] because of invalid payload type: [{payload_type:?}].",
                                        idle_connection.as_ref()
                                    );
                                    let idle_connection = Object::take(idle_connection);
                                    let idle_connection_writer = idle_connection.get_writer();
                                    let mut idle_connection_writer = idle_connection_writer.lock().await;
                                    idle_connection_writer.close().await?;
                                    Err(anyhow::anyhow!("Fail to do idele heartbeat on proxy connection."))
                                }
                            },
                        }
                    });
                }
            }
        });
        Ok(keepalive_guard)
    }
}
