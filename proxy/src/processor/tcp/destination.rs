use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, StreamExt,
};
use futures_util::Stream;
use pin_project::pin_project;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{BytesCodec, Framed};

use crate::error::NetworkError;

type DstBytesFramedRead<T> = SplitStream<Framed<T, BytesCodec>>;
type DstBytesFramedWrite<T> = SplitSink<Framed<T, BytesCodec>, BytesMut>;

/// The parts of the destination connection
pub(crate) struct DstConnectionParts<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    pub read: DstConnectionRead<T>,
    pub write: DstConnectionWrite<T>,
}

/// The destination connection framed with BytesCodec
pub(crate) struct DstConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    framed_read: DstBytesFramedRead<T>,
    framed_write: DstBytesFramedWrite<T>,
}

impl<T> DstConnection<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    pub fn new(stream: T, buffer_size: usize) -> Self {
        let dst_bytes_framed = Framed::with_capacity(stream, BytesCodec::new(), buffer_size);
        let (framed_write, framed_read) = dst_bytes_framed.split::<BytesMut>();
        Self { framed_write, framed_read }
    }

    pub fn split(self) -> DstConnectionParts<T> {
        let read = DstConnectionRead::new(self.framed_read);
        let write = DstConnectionWrite::new(self.framed_write);
        DstConnectionParts { read, write }
    }
}

#[pin_project]
pub(crate) struct DstConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    #[pin]
    framed_write: DstBytesFramedWrite<T>,
}

impl<T> DstConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    fn new(framed_write: DstBytesFramedWrite<T>) -> Self {
        Self { framed_write }
    }
}

impl<T> Sink<BytesMut> for DstConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    type Error = NetworkError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.framed_write.poll_ready(cx).map_err(NetworkError::DestinationWrite)
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        this.framed_write.start_send(item).map_err(NetworkError::DestinationWrite)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.framed_write.poll_flush(cx).map_err(NetworkError::DestinationWrite)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.framed_write.poll_close(cx).map_err(NetworkError::DestinationWrite)
    }
}

#[pin_project]
pub(crate) struct DstConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    #[pin]
    framed_read: DstBytesFramedRead<T>,
}

impl<T> DstConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    fn new(framed_read: DstBytesFramedRead<T>) -> Self {
        Self { framed_read }
    }
}

impl<T> Stream for DstConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    type Item = Result<BytesMut, NetworkError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.framed_read.poll_next(cx).map_err(NetworkError::DestinationRead)
    }
}
