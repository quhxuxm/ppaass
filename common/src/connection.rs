use std::{
    fmt::{Debug, Display},
    pin::Pin,
    task::{Context, Poll},
};

use crate::{codec::PpaassMessageCodec, CommonError, PpaassMessage, RsaCryptoFetcher};

use futures::{Sink, Stream};
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

#[non_exhaustive]
#[pin_project]
#[derive(Debug)]
pub struct PpaassConnection<'r, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    #[pin]
    inner: Framed<T, PpaassMessageCodec<'r, R>>,
    connection_id: I,
}

impl<'r, T, R, I> PpaassConnection<'r, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new<'a>(connection_id: I, stream: T, rsa_crypto_fetcher: &'a R, compress: bool, buffer_size: usize) -> PpaassConnection<'r, T, R, I>
    where
        'a: 'r,
    {
        let ppaass_message_codec = PpaassMessageCodec::new(compress, rsa_crypto_fetcher);
        let inner = Framed::with_capacity(stream, ppaass_message_codec, buffer_size);
        Self { inner, connection_id }
    }

    pub fn get_connection_id(&self) -> &I {
        &self.connection_id
    }
}

impl<T, R, I> Sink<PpaassMessage> for PpaassConnection<'_, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    type Error = CommonError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: PpaassMessage) -> Result<(), Self::Error> {
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

impl<T, R, I> Stream for PpaassConnection<'_, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    type Item = Result<PpaassMessage, CommonError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx)
    }
}
