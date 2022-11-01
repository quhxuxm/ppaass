use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::{ready, Sink, Stream};
use ppaass_common::RsaCryptoFetcher;
use ppaass_protocol::PpaassMessage;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio_util::codec::Framed;

use crate::{codec::PpaassMessageCodec, error::Error};

#[derive(Debug)]
pub struct PpaassMessageFramed<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin,
    R: RsaCryptoFetcher,
{
    inner: Framed<T, PpaassMessageCodec<R>>,
}

impl<T, R> PpaassMessageFramed<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin,
    R: RsaCryptoFetcher,
{
    pub fn new(stream: T, compress: bool, buffer_size: usize, rsa_crypto_fetcher: Arc<R>) -> Result<Self, Error> {
        let framed = Framed::with_capacity(stream, PpaassMessageCodec::new(compress, rsa_crypto_fetcher), buffer_size);
        Ok(Self { inner: framed })
    }
}

impl<T, R> Stream for PpaassMessageFramed<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin,
    R: RsaCryptoFetcher,
{
    type Item = Result<PpaassMessage, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let framed_poll_next_result = ready!(Pin::new(&mut self.inner).poll_next(cx));
        match framed_poll_next_result {
            None => Poll::Ready(None),
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            Some(Ok(v)) => Poll::Ready(Some(Ok(v))),
        }
    }
}

impl<T, R> Sink<PpaassMessage> for PpaassMessageFramed<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin,
    R: RsaCryptoFetcher,
{
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.inner).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: PpaassMessage) -> Result<(), Error> {
        Pin::new(&mut self.inner).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}
