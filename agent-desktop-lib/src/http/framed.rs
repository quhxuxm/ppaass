use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{ready, Sink, Stream};
use httpcodec::{Request, Response};
use pin_project::pin_project;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedParts};

use crate::error::Error;

use super::codec::HttpCodec;

#[pin_project]
#[derive(Debug)]
pub(crate) struct HttpFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    #[pin]
    concrete_framed: Framed<T, HttpCodec>,
}

impl<T> HttpFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub(crate) fn new(stream: T) -> Self {
        let concrete_framed = Framed::new(stream, Default::default());
        Self { concrete_framed }
    }

    pub(crate) fn split(self) -> FramedParts<T, HttpCodec> {
        self.split()
    }
}

impl<T> Stream for HttpFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Item = Result<Request<Vec<u8>>, Error>;

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

impl<T> Sink<Response<Vec<u8>>> for HttpFramed<T>
where
    T: AsyncRead + AsyncWrite,
{
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.concrete_framed.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Response<Vec<u8>>) -> Result<(), Self::Error> {
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
