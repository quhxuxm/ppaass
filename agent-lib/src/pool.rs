use std::{
    fmt::Debug,
    str::FromStr,
    task::{Context, Poll},
};
use std::{net::SocketAddr, time::Duration};
use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use pin_project::{pin_project, pinned_drop};
use tokio::{sync::mpsc::channel, task::JoinHandle};

use tokio::{
    net::TcpStream,
    sync::Mutex,
    time::{interval, timeout},
};
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

use ppaass_common::{codec::PpaassMessageCodec, PpaassMessageParts};
use ppaass_common::{
    generate_uuid, heartbeat::HeartbeatResponsePayload, PpaassMessageGenerator, PpaassMessageProxyPayload, PpaassMessageProxyPayloadParts,
    PpaassMessageProxyPayloadType,
};
use ppaass_common::{PpaassMessage, PpaassMessagePayloadEncryptionSelector};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, AgentServerPayloadEncryptionTypeSelector};

type ProxyMessageFramed = Framed<TcpStream, PpaassMessageCodec<AgentServerRsaCryptoFetcher>>;
type ProxyMessageFramedWrite = SplitSink<ProxyMessageFramed, PpaassMessage>;
type ProxyMessageFramedRead = SplitStream<ProxyMessageFramed>;

#[pin_project]
pub(crate) struct ProxyConnectionRead {
    #[pin]
    proxy_message_framed_read: ProxyMessageFramedRead,
}

impl ProxyConnectionRead {
    pub(crate) fn new(proxy_message_framed_read: ProxyMessageFramedRead) -> Self {
        Self { proxy_message_framed_read }
    }
}

impl Stream for ProxyConnectionRead {
    type Item = Result<PpaassMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.proxy_message_framed_read.poll_next(cx)
    }
}

#[pin_project(PinnedDrop)]
pub(crate) struct ProxyConnectionWrite {
    proxy_connection_id: String,
    #[pin]
    proxy_message_framed_write: Option<ProxyMessageFramedWrite>,
}

impl ProxyConnectionWrite {
    pub(crate) fn new(proxy_message_framed_write: ProxyMessageFramedWrite, proxy_connection_id: impl AsRef<str>) -> Self {
        Self {
            proxy_message_framed_write: Some(proxy_message_framed_write),
            proxy_connection_id: proxy_connection_id.as_ref().to_owned(),
        }
    }
}
#[pinned_drop]
impl PinnedDrop for ProxyConnectionWrite {
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let connection_id = this.proxy_connection_id.clone();

        if let Some(mut proxy_message_framed_write) = this.proxy_message_framed_write.take() {
            tokio::spawn(async move {
                if let Err(e) = proxy_message_framed_write.close().await {
                    error!("Fail to close proxy connection because of error: {e:?}");
                };
                debug!("Proxy connection [{connection_id}] dropped")
            });
        }
    }
}

impl Sink<PpaassMessage> for ProxyConnectionWrite {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let proxy_message_framed_write = this.proxy_message_framed_write.as_pin_mut();
        match proxy_message_framed_write {
            Some(proxy_message_framed_write) => proxy_message_framed_write.poll_ready(cx),
            None => Poll::Ready(Err(anyhow::anyhow!("Proxy message framed not exist"))),
        }
    }

    fn start_send(self: Pin<&mut Self>, item: PpaassMessage) -> Result<(), Self::Error> {
        let this = self.project();
        let proxy_message_framed_write = this.proxy_message_framed_write.as_pin_mut();
        match proxy_message_framed_write {
            Some(proxy_message_framed_write) => proxy_message_framed_write.start_send(item),
            None => Err(anyhow::anyhow!("Proxy message framed not exist")),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let proxy_message_framed_write = this.proxy_message_framed_write.as_pin_mut();
        match proxy_message_framed_write {
            Some(proxy_message_framed_write) => proxy_message_framed_write.poll_flush(cx),
            None => Poll::Ready(Err(anyhow::anyhow!("Proxy message framed not exist"))),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let proxy_message_framed_write = this.proxy_message_framed_write.as_pin_mut();
        match proxy_message_framed_write {
            Some(proxy_message_framed_write) => proxy_message_framed_write.poll_close(cx),
            None => Poll::Ready(Err(anyhow::anyhow!("Proxy message framed not exist"))),
        }
    }
}

pub(crate) struct ProxyConnection {
    pub(crate) id: String,
    proxy_message_framed: Option<ProxyMessageFramed>,
}

impl ProxyConnection {
    pub(crate) fn split_framed(&mut self) -> Result<(ProxyConnectionRead, ProxyConnectionWrite)> {
        let proxy_message_framed = self.proxy_message_framed.take();
        let connection_id = self.id.to_owned();
        match proxy_message_framed {
            Some(proxy_message_framed) => {
                let (proxy_message_framed_write, proxy_message_framed_read) = proxy_message_framed.split();
                Ok((
                    ProxyConnectionRead::new(proxy_message_framed_read),
                    ProxyConnectionWrite::new(proxy_message_framed_write, connection_id),
                ))
            },
            None => Err(anyhow::anyhow!("Proxy connection [{connection_id}] agent message framed not exist.")),
        }
    }
}

impl Debug for ProxyConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConnection").field("id", &self.id).finish()
    }
}

#[derive(Debug)]
pub(crate) struct ProxyConnectionPool {
    proxy_addresses: Vec<SocketAddr>,
    idle_connections: Arc<Mutex<Vec<ProxyConnection>>>,
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    heartbeat_guard: Option<JoinHandle<Result<()>>>,
}

impl ProxyConnectionPool {
    pub(crate) async fn new(configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Result<Self> {
        let proxy_addresses_configuration = configuration
            .get_proxy_addresses()
            .expect("Fail to parse proxy addresses from configuration file");
        let mut proxy_addresses: Vec<SocketAddr> = Vec::new();
        for address in proxy_addresses_configuration {
            match SocketAddr::from_str(address) {
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

        let connections = Arc::new(Mutex::new(Vec::new()));
        let mut pool = Self {
            proxy_addresses: proxy_addresses.clone(),
            idle_connections: connections.clone(),
            configuration: configuration.clone(),
            rsa_crypto_fetcher: rsa_crypto_fetcher.clone(),
            heartbeat_guard: None,
        };
        Self::feed_connections(configuration.clone(), connections.clone(), proxy_addresses, rsa_crypto_fetcher.clone()).await?;
        let heartbeat_guard = pool.start_idle_heartbeat().await;
        pool.heartbeat_guard = Some(heartbeat_guard);
        Ok(pool)
    }

    async fn start_idle_heartbeat(&self) -> JoinHandle<Result<()>> {
        let configuration = self.configuration.clone();
        let idle_connections = self.idle_connections.clone();
        let proxy_addresses = self.proxy_addresses.clone();
        let rsa_crypto_fetcher = self.rsa_crypto_fetcher.clone();
        tokio::spawn(async move {
            let proxy_addresses = proxy_addresses;
            let proxy_connection_pool_size = configuration.get_proxy_connection_pool_size();
            let user_token = configuration.get_user_token().to_owned().expect("User token not configured.");
            let idle_proxy_heartbeat_interval = configuration.get_idle_proxy_heartbeat_interval();
            let mut heartbeat_interval = interval(Duration::from_secs(idle_proxy_heartbeat_interval));
            loop {
                heartbeat_interval.tick().await;
                if let Err(e) = Self::feed_connections(
                    configuration.clone(),
                    idle_connections.clone(),
                    proxy_addresses.clone(),
                    rsa_crypto_fetcher.clone(),
                )
                .await
                {
                    error!("Error happen when feed proxy connection: {e:?}");
                    continue;
                };
                let (tx, mut rx) = channel::<ProxyConnection>(proxy_connection_pool_size);
                let mut idle_connections = idle_connections.lock().await;
                while let Some(idle_connection) = idle_connections.pop() {
                    let user_token = user_token.clone();
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let ProxyConnection { id, proxy_message_framed } = idle_connection;
                        let (mut write, mut read) = match proxy_message_framed {
                            None => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of proxy message framed not exist");
                                return Err(anyhow::anyhow!(
                                    "Fail to do idle heartbeat for proxy connection {id} because of proxy message framed not exist"
                                ));
                            },
                            Some(proxy_message_framed) => proxy_message_framed.split(),
                        };
                        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().as_bytes().to_vec()));
                        let idle_heartbeat_request = match PpaassMessageGenerator::generate_heartbeat_request(user_token.clone(), payload_encryption) {
                            Ok(v) => v,
                            Err(e) => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of error: {e:?}");
                                if let Err(e) = write.close().await {
                                    error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                                }
                                return Err(e);
                            },
                        };
                        if let Err(e) = write.send(idle_heartbeat_request).await {
                            error!("Fail to do idle heartbeat for proxy connection {id} because of error: {e:?}");
                            if let Err(e) = write.close().await {
                                error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                            }
                            return Err(e);
                        };
                        let idle_heartbeat_response = timeout(Duration::from_secs(5), read.next()).await;
                        let idle_heartbeat_response = match idle_heartbeat_response {
                            Err(_) => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of timeout.");
                                if let Err(e) = write.close().await {
                                    error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                                }
                                return Err(anyhow::anyhow!("Fail to do idle heartbeat for proxy connection {id} because of timeout."));
                            },
                            Ok(None) => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of no response.");
                                if let Err(e) = write.close().await {
                                    error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                                }
                                return Err(anyhow::anyhow!("Fail to do idle heartbeat for proxy connection {id} because of no response."));
                            },
                            Ok(Some(Ok(v))) => v,
                            Ok(Some(Err(e))) => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of error: {e:?}");
                                if let Err(e) = write.close().await {
                                    error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                                }
                                return Err(e);
                            },
                        };
                        let PpaassMessageParts { payload_bytes, .. } = idle_heartbeat_response.split();
                        let PpaassMessageProxyPayloadParts { payload_type, data } = match TryInto::<PpaassMessageProxyPayload>::try_into(payload_bytes) {
                            Ok(v) => v.split(),
                            Err(e) => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of error: {e:?}");
                                if let Err(e) = write.close().await {
                                    error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                                }
                                return Err(e);
                            },
                        };
                        if PpaassMessageProxyPayloadType::IdleHeartbeat != payload_type {
                            error!("Fail to do idle heartbeat for proxy connection {id} because of invalid payload type: {payload_type:?}");
                            if let Err(e) = write.close().await {
                                error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                            }
                            return Err(anyhow::anyhow!(
                                "Fail to do idle heartbeat for proxy connection {id} because of invalid payload type: {payload_type:?}"
                            ));
                        }
                        let idle_heartbeat_response: HeartbeatResponsePayload = match data.try_into() {
                            Ok(v) => v,
                            Err(e) => {
                                error!("Fail to do idle heartbeat for proxy connection {id} because of error: {e:?}");
                                if let Err(e) = write.close().await {
                                    error!("Fail to close idle proxy connection {id} because of error: {e:?}");
                                }
                                return Err(e);
                            },
                        };
                        debug!("Success to do idle heartbeat for proxy connection {id}: {idle_heartbeat_response:?}");

                        let proxy_message_framed = match read.reunite(write) {
                            Ok(v) => v,
                            Err(e) => {
                                error!("Fail to rebuild proxy message framed for idle proxy connection {id} because of error: {e:?}");
                                return Err(anyhow::anyhow!(e));
                            },
                        };
                        if let Err(e) = tx
                            .send(ProxyConnection {
                                id,
                                proxy_message_framed: Some(proxy_message_framed),
                            })
                            .await
                        {
                            error!("Fail to send proxy connection because of error: {e:?}")
                        }
                        Ok(())
                    });
                }
                drop(tx);
                while let Some(idle_connection) = rx.recv().await {
                    idle_connections.push(idle_connection);
                }
            }
        })
    }

    async fn feed_connections(
        configuration: Arc<AgentServerConfig>, connections: Arc<Mutex<Vec<ProxyConnection>>>, proxy_addresses: Vec<SocketAddr>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        debug!("Begin to feed proxy connections");
        let proxy_connection_pool_size = configuration.get_proxy_connection_pool_size();
        let message_framed_buffer_size = configuration.get_message_framed_buffer_size();
        let mut connections = connections.lock().await;
        let current_connection_len = connections.len();
        if current_connection_len >= proxy_connection_pool_size {
            return Ok(());
        }
        let (tx, mut rx) = channel::<ProxyConnection>(proxy_connection_pool_size);
        for _ in current_connection_len..proxy_connection_pool_size {
            let configuration = configuration.clone();
            let rsa_crypto_fetcher = rsa_crypto_fetcher.clone();
            let tx = tx.clone();
            let proxy_addresses = proxy_addresses.clone();
            tokio::spawn(async move {
                let proxy_tcp_stream = match TcpStream::connect(proxy_addresses.as_slice()).await {
                    Ok(proxy_tcp_stream) => proxy_tcp_stream,
                    Err(e) => {
                        error!("Fail to feed proxy connection because of error: {e:?}");
                        return Err(e);
                    },
                };
                debug!("Success connect to proxy when feed connection pool.");
                let proxy_message_codec = PpaassMessageCodec::new(configuration.get_compress(), rsa_crypto_fetcher);
                let proxy_message_framed = Framed::with_capacity(proxy_tcp_stream, proxy_message_codec, message_framed_buffer_size);
                let connection = ProxyConnection {
                    id: generate_uuid(),
                    proxy_message_framed: Some(proxy_message_framed),
                };
                if let Err(e) = tx.send(connection).await {
                    error!("Fail to send proxy connection to channel because of error: {e:?}");
                };
                Ok(())
            });
        }
        drop(tx);
        loop {
            let connection = rx.recv().await;
            match connection {
                None => break,
                Some(connection) => {
                    connections.push(connection);
                },
            }
        }

        Ok(())
    }

    pub(crate) async fn take_connection(&self) -> Result<ProxyConnection> {
        let proxy_connection_pool_size = self.configuration.get_proxy_connection_pool_size();
        loop {
            let proxy_addresses = self.proxy_addresses.clone();
            let mut connections_lock = self.idle_connections.lock().await;
            let connection = connections_lock.pop();
            match connection {
                Some(connection) => {
                    debug!("Success to take connection from pool.");
                    let current_connections_number = connections_lock.len();
                    if current_connections_number < (proxy_connection_pool_size / 2) {
                        info!("Proxy connection do not have enough proxy connection, start to feed: {current_connections_number}");
                        tokio::spawn(Self::feed_connections(
                            self.configuration.clone(),
                            self.idle_connections.clone(),
                            proxy_addresses,
                            self.rsa_crypto_fetcher.clone(),
                        ));
                    }
                    return Ok(connection);
                },
                None => {
                    drop(connections_lock);
                    tokio::spawn(Self::feed_connections(
                        self.configuration.clone(),
                        self.idle_connections.clone(),
                        proxy_addresses,
                        self.rsa_crypto_fetcher.clone(),
                    ));
                },
            }
        }
    }
}

impl Drop for ProxyConnectionPool {
    fn drop(&mut self) {
        if let Some(heartbeat_guard) = self.heartbeat_guard.take() {
            heartbeat_guard.abort();
        }
    }
}
