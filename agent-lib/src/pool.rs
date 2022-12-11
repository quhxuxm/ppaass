use std::{
    fmt::Debug,
    str::FromStr,
    task::{Context, Poll},
};
use std::{net::SocketAddr, time::Duration};
use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use futures::{
    future::join_all,
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use pin_project::{pin_project, pinned_drop};
use tokio::{
    net::TcpStream,
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    time::{interval, timeout},
};
use tokio_util::codec::Framed;
use tracing::{debug, error};

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
    pub(crate) guard: Option<OwnedSemaphorePermit>,
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
    connections: Arc<Mutex<Vec<ProxyConnection>>>,
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    connection_number_semaphore: Arc<Semaphore>,
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
        let connection_number_semaphore = Arc::new(Semaphore::new(configuration.get_proxy_connection_number_semaphore()));
        let pool = Self {
            proxy_addresses,
            connections,
            configuration: configuration.clone(),
            rsa_crypto_fetcher: rsa_crypto_fetcher.clone(),
            connection_number_semaphore,
        };
        pool.feed_connections().await?;
        pool.start_idle_heartbeat().await?;
        Ok(pool)
    }

    async fn start_idle_heartbeat(&self) -> Result<()> {
        let connections = self.connections.clone();
        let user_token = self.configuration.get_user_token().to_owned().expect("User token not configured.");
        let idle_proxy_heartbeat_interval = self.configuration.get_idle_proxy_heartbeat_interval();
        tokio::spawn(async move {
            let mut heart_beat_interval = interval(Duration::from_secs(idle_proxy_heartbeat_interval));
            loop {
                heart_beat_interval.tick().await;
                let mut connections = connections.lock().await;

                let mut connection_heartbeat_tasks = Vec::new();
                while let Some(connection) = connections.pop() {
                    let user_token = user_token.clone();
                    let connection_heartbeat_task = tokio::spawn(async move {
                        let ProxyConnection {
                            id,
                            proxy_message_framed,
                            guard,
                        } = connection;
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
                        Ok(ProxyConnection {
                            id,
                            proxy_message_framed: Some(proxy_message_framed),
                            guard,
                        })
                    });
                    connection_heartbeat_tasks.push(connection_heartbeat_task);
                }
                let connection_heartbeat_tasks_result = join_all(connection_heartbeat_tasks).await;
                connection_heartbeat_tasks_result.into_iter().for_each(|heartbeat_task_result| {
                    if let Ok(Ok(connection)) = heartbeat_task_result {
                        connections.push(connection);
                    }
                });
            }
        });
        Ok(())
    }

    async fn feed_connections(&self) -> Result<()> {
        let proxy_connection_number = self.configuration.get_proxy_connection_number();
        let message_framed_buffer_size = self.configuration.get_message_framed_buffer_size();
        let connections_out = self.connections.clone();
        let mut connections = connections_out.lock().await;
        debug!("Begin to feed proxy connections");
        loop {
            let current_connection_len = connections.len();
            if current_connection_len >= proxy_connection_number {
                break;
            }
            for _ in current_connection_len..proxy_connection_number {
                let proxy_addresses = self.proxy_addresses.clone();
                let configuration = self.configuration.clone();
                let rsa_crypto_fetcher = self.rsa_crypto_fetcher.clone();
                let proxy_tcp_stream = match TcpStream::connect(proxy_addresses.as_slice()).await {
                    Ok(proxy_tcp_stream) => proxy_tcp_stream,
                    Err(e) => {
                        error!("Fail to feed proxy connection because of error: {e:?}");
                        continue;
                    },
                };
                debug!("Success connect to proxy when feed connection pool.");
                let proxy_message_codec = PpaassMessageCodec::new(configuration.get_compress(), rsa_crypto_fetcher);
                let proxy_message_framed = Framed::with_capacity(proxy_tcp_stream, proxy_message_codec, message_framed_buffer_size);
                let connection = ProxyConnection {
                    id: generate_uuid(),
                    proxy_message_framed: Some(proxy_message_framed),
                    guard: None,
                };
                connections.push(connection);
            }
        }
        Ok(())
    }

    pub(crate) async fn take_connection(&self) -> Result<ProxyConnection> {
        let connection_number_semaphore = self.connection_number_semaphore.clone();

        let guard = match timeout(
            Duration::from_secs(self.configuration.get_take_proxy_conection_timeout()),
            connection_number_semaphore.acquire_owned(),
        )
        .await
        {
            Ok(Ok(v)) => {
                debug!("Proxy connection semaphore remaining: {}", self.connection_number_semaphore.available_permits());
                v
            },
            Ok(Err(e)) => {
                error!("Fail to take proxy connection from pool because of error: {e:?}");
                return Err(anyhow::anyhow!(e));
            },
            Err(_) => {
                error!("Fail to take proxy connection from pool because of semaphore timeout.");
                return Err(anyhow::anyhow!("Fail to take proxy connection from pool because of timeout."));
            },
        };
        loop {
            let mut connections = self.connections.lock().await;
            let connection = connections.pop();
            match connection {
                Some(mut connection) => {
                    debug!("Success to take connection from pool.");
                    connection.guard = Some(guard);
                    return Ok(connection);
                },
                None => {
                    drop(connections);
                    if let Err(e) = self.feed_connections().await {
                        error!("Error happen when feed proxy connection pool on take connection: {e:?}")
                    };
                },
            }
        }
    }
}
