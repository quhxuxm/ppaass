use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::BytesMut;
use futures::{ready, Sink, Stream};
use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedParts};

use crate::{
    error::Error,
    socks::{
        codec::Socks5InitCommandContentCodec,
        message::init::{Socks5InitCommandContent, Socks5InitCommandResultContent},
    },
};

#[pin_project]
#[derive(Debug)]
pub(crate) struct Socks5InitFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    #[pin]
    concrete_framed: Framed<T, Socks5InitCommandContentCodec>,
}

impl<T> Socks5InitFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub(crate) fn new(stream: T) -> Self {
        let framed_parts = FramedParts::new(stream, Socks5InitCommandContentCodec);
        let concrete_framed = Framed::from_parts(framed_parts);
        Self { concrete_framed }
    }

    pub(crate) fn split(self) -> FramedParts<T, Socks5InitCommandContentCodec> {
        self.split()
    }
}

impl<T> Stream for Socks5InitFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Item = Result<Socks5InitCommandContent, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let concrete_poll_result = ready!(this.concrete_framed.poll_next(cx));
        match concrete_poll_result {
            None => Poll::Ready(None),
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            Some(Ok(v)) => Poll::Ready(Some(Ok(v))),
        }
    }
}

impl<T> Sink<Socks5InitCommandResultContent> for Socks5InitFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.concrete_framed.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Socks5InitCommandResultContent) -> Result<(), Self::Error> {
        let this = self.project();
        this.concrete_framed.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.concrete_framed.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.concrete_framed.poll_close(cx)
    }
}
