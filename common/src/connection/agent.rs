use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::{codec::agent::PpaassAgentConnectionCodec, CommonError, PpaassAgentMessage, PpaassProxyMessage, RsaCryptoFetcher};

use futures::{Sink, Stream};
use pin_project::pin_project;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[non_exhaustive]
#[pin_project]
pub struct PpaassAgentConnection<R>
where
    R: RsaCryptoFetcher,
{
    #[pin]
    inner: Framed<TcpStream, PpaassAgentConnectionCodec<R>>,
    connection_id: String,
}

impl<R> PpaassAgentConnection<R>
where
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub fn new(connection_id: String, stream: TcpStream, rsa_crypto_fetcher: R, compress: bool, buffer_size: usize) -> PpaassAgentConnection<R> {
        let agent_connection_codec = PpaassAgentConnectionCodec::new(compress, rsa_crypto_fetcher);
        let inner = Framed::with_capacity(stream, agent_connection_codec, buffer_size);
        Self { inner, connection_id }
    }
}

impl<R> Sink<PpaassProxyMessage> for PpaassAgentConnection<R>
where
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    type Error = CommonError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: PpaassProxyMessage) -> Result<(), Self::Error> {
        let this = self.project();
        this.inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.inner.poll_close(cx)
    }
}

impl<R> Stream for PpaassAgentConnection<R>
where
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    type Item = Result<PpaassAgentMessage, CommonError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx)
    }
}
