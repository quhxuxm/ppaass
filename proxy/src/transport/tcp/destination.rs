use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;

use futures::{Sink, Stream};
use pin_project::pin_project;

use tokio::net::TcpStream;
use tokio_util::codec::{BytesCodec, Framed};

use crate::error::ProxyServerError;

/// The destination connection framed with BytesCodec
#[pin_project]
pub(crate) struct DstConnection {
    #[pin]
    inner: Framed<TcpStream, BytesCodec>,
}

impl DstConnection {
    pub fn new(stream: TcpStream, buffer_size: usize) -> Self {
        let inner = Framed::with_capacity(stream, BytesCodec::new(), buffer_size);
        Self { inner }
    }
}

impl Sink<BytesMut> for DstConnection {
    type Error = ProxyServerError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        Sink::<BytesMut>::poll_ready(this.inner, cx).map_err(ProxyServerError::GeneralIo)
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        Sink::<BytesMut>::start_send(this.inner, item).map_err(ProxyServerError::GeneralIo)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        Sink::<BytesMut>::poll_flush(this.inner, cx).map_err(ProxyServerError::GeneralIo)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        Sink::<BytesMut>::poll_close(this.inner, cx).map_err(ProxyServerError::GeneralIo)
    }
}

impl Stream for DstConnection {
    type Item = Result<BytesMut, ProxyServerError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx).map_err(ProxyServerError::GeneralIo)
    }
}