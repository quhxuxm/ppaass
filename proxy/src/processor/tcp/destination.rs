use std::task::{Context, Poll};
use std::{
    fmt::{Debug, Display},
    pin::Pin,
};

use anyhow::{anyhow, Result};
use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, StreamExt,
};
use futures_util::Stream;
use pin_project::pin_project;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{BytesCodec, Framed};

type DstBytesFramedRead<T> = SplitStream<Framed<T, BytesCodec>>;
type DstBytesFramedWrite<T> = SplitSink<Framed<T, BytesCodec>, BytesMut>;

pub(crate) struct DstConnectionParts<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub read: DstConnectionRead<T, I>,
    pub write: DstConnectionWrite<T, I>,
    pub id: I,
}
pub(crate) struct DstConnection<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    framed_read: DstBytesFramedRead<T>,
    framed_write: DstBytesFramedWrite<T>,
    id: I,
}

impl<T, I> DstConnection<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new(id: I, stream: T, buffer_size: usize) -> Self {
        let dst_bytes_framed = Framed::with_capacity(stream, BytesCodec::new(), buffer_size);
        let (framed_write, framed_read) = dst_bytes_framed.split::<BytesMut>();
        Self { framed_write, framed_read, id }
    }

    pub fn split(self) -> DstConnectionParts<T, I> {
        let read = DstConnectionRead::new(self.id.clone(), self.framed_read);
        let write = DstConnectionWrite::new(self.id.clone(), self.framed_write);
        let id = self.id;
        DstConnectionParts { read, write, id }
    }
}

#[pin_project]
pub(crate) struct DstConnectionWrite<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    dst_bytes_framed_write: DstBytesFramedWrite<T>,
}

impl<T, I> DstConnectionWrite<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn new(connection_id: I, dst_bytes_framed_write: DstBytesFramedWrite<T>) -> Self {
        Self {
            connection_id,
            dst_bytes_framed_write,
        }
    }
}

impl<T, I> Sink<BytesMut> for DstConnectionWrite<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.dst_bytes_framed_write.poll_ready(cx).map_err(|e| anyhow!(e))
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        this.dst_bytes_framed_write.start_send(item).map_err(|e| anyhow!(e))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.dst_bytes_framed_write.poll_flush(cx).map_err(|e| anyhow!(e))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.dst_bytes_framed_write.poll_close(cx).map_err(|e| anyhow!(e))
    }
}

#[pin_project]
pub(crate) struct DstConnectionRead<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    dst_bytes_framed_read: DstBytesFramedRead<T>,
}

impl<T, I> DstConnectionRead<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn new(connection_id: I, dst_bytes_framed_read: DstBytesFramedRead<T>) -> Self {
        Self {
            connection_id,
            dst_bytes_framed_read,
        }
    }
}

impl<T, I> Stream for DstConnectionRead<T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    type Item = Result<BytesMut, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.dst_bytes_framed_read.poll_next(cx).map_err(|e| anyhow!(e))
    }
}
