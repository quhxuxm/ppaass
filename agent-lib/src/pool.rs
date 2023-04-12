use std::sync::Arc;
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};
use std::{net::SocketAddr, time::Duration};

use anyhow::{Context as AnyhowContext, Result};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    time::timeout,
};

use tracing::{debug, error};

use ppaass_common::{generate_uuid, RsaCryptoFetcher};
use ppaass_common::{PpaassConnection, PpaassConnectionRead};
use ppaass_common::{PpaassConnectionParts, PpaassConnectionWrite};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};

type ProxyConnectionSplitResult<T, R, I> = Result<(PpaassConnectionRead<T, R, I>, PpaassConnectionWrite<T, R, I>)>;
pub(crate) struct ProxyConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) connection_id: I,
    read: Option<PpaassConnectionRead<T, R, I>>,
    write: Option<PpaassConnectionWrite<T, R, I>>,
}

impl<T, R, I> ProxyConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn split(mut self) -> ProxyConnectionSplitResult<T, R, I> {
        let connection_id = self.connection_id.clone();
        let read = self.read.take().context(format!("Proxy connection [{connection_id}] can not get read part"))?;
        let write = self
            .write
            .take()
            .context(format!("Proxy connection [{connection_id}] can not get write part"))?;

        Ok((read, write))
    }
}

impl<T, R, I> Debug for ProxyConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConnection").field("connection id", &self.connection_id).finish()
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

    pub(crate) async fn take_connection(&self) -> Result<ProxyConnection<TcpStream, AgentServerRsaCryptoFetcher, String>> {
        debug!("Begin to feed proxy connections");

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

        proxy_tcp_stream.set_nodelay(true)?;
        proxy_tcp_stream.set_linger(Some(Duration::from_secs(20)))?;
        let ppaass_connection = PpaassConnection::new(
            generate_uuid(),
            proxy_tcp_stream,
            self.rsa_crypto_fetcher.clone(),
            self.configuration.get_compress(),
            self.configuration.get_message_framed_buffer_size(),
        );
        let PpaassConnectionParts {
            read,
            write,
            id: connection_id,
        } = ppaass_connection.split();

        Ok(ProxyConnection {
            connection_id,
            read: Some(read),
            write: Some(write),
        })
    }
}
