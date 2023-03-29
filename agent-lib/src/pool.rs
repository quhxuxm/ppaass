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
use tokio::{net::TcpStream, time::timeout};

use tokio_util::codec::Framed;
use tracing::{debug, error};

use ppaass_common::codec::PpaassMessageCodec;
use ppaass_common::generate_uuid;
use ppaass_common::PpaassMessage;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};

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
    pub(crate) fn split(mut self) -> Result<(ProxyConnectionRead, ProxyConnectionWrite)> {
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
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
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

        Ok(Self {
            proxy_addresses: proxy_addresses.clone(),
            configuration: configuration.clone(),
            rsa_crypto_fetcher,
        })
    }

    pub(crate) async fn take_connection(&self) -> Result<ProxyConnection> {
        debug!("Begin to feed proxy connections");
        let message_framed_buffer_size = self.configuration.get_message_framed_buffer_size();

        let proxy_tcp_stream = match timeout(
            Duration::from_secs(self.configuration.get_connect_to_proxy_timeout()),
            TcpStream::connect(self.proxy_addresses.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Fail to feed proxy connection because of timeout.");
                return Err(anyhow::anyhow!("Fail to feed proxy connection because of timeout."));
            },
            Ok(Ok(proxy_tcp_stream)) => proxy_tcp_stream,
            Ok(Err(e)) => {
                error!("Fail to feed proxy connection because of error: {e:?}");
                return Err(anyhow::anyhow!(e));
            },
        };
        debug!("Success connect to proxy when feed connection pool.");
        let proxy_message_codec = PpaassMessageCodec::new(self.configuration.get_compress(), self.rsa_crypto_fetcher.clone());
        let proxy_message_framed = Framed::with_capacity(proxy_tcp_stream, proxy_message_codec, message_framed_buffer_size);

        Ok(ProxyConnection {
            id: generate_uuid(),
            proxy_message_framed: Some(proxy_message_framed),
        })
    }
}
