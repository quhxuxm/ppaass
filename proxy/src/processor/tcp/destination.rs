use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{anyhow, Result};
use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink,
};
use futures_util::{SinkExt, Stream};
use pin_project::{pin_project, pinned_drop};

use tokio::net::TcpStream;
use tokio_util::codec::{BytesCodec, Framed};
use tracing::{debug, error};

type DestBytesFramedRead = SplitStream<Framed<TcpStream, BytesCodec>>;
type DestBytesFramedWrite = SplitSink<Framed<TcpStream, BytesCodec>, BytesMut>;

#[pin_project(PinnedDrop)]
pub(crate) struct DestConnectionWrite {
    agent_connection_id: String,
    tcp_loop_key: String,
    #[pin]
    dest_bytes_framed_write: Option<DestBytesFramedWrite>,
}

impl DestConnectionWrite {
    pub(crate) fn new(agent_connection_id: String, tcp_loop_key: String, dest_bytes_framed_write: DestBytesFramedWrite) -> Self {
        Self {
            agent_connection_id,
            tcp_loop_key,
            dest_bytes_framed_write: Some(dest_bytes_framed_write),
        }
    }
}

#[pinned_drop]
impl PinnedDrop for DestConnectionWrite {
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let agent_connection_id = this.agent_connection_id.clone();
        let tcp_loop_key = this.tcp_loop_key.clone();
        if let Some(mut dest_bytes_framed_write) = this.dest_bytes_framed_write.take() {
            tokio::spawn(async move {
                debug!("Agent connection {agent_connection_id} with tcp loop key {tcp_loop_key} drop dest connection write.");
                if let Err(e) = dest_bytes_framed_write.close().await {
                    error!("Agent connection {agent_connection_id} with tcp loop key {tcp_loop_key}, error happen on drop dest connection write: {e:?}")
                }
            });
        };
    }
}

impl Sink<BytesMut> for DestConnectionWrite {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            match dest_bytes_framed_write.poll_ready(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow!("Dest bytes framed write not exist")))
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            return dest_bytes_framed_write.start_send(item).map_err(|e| anyhow!(e));
        }
        Err(anyhow!("Dest bytes framed write not exist"))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            match dest_bytes_framed_write.poll_flush(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow!("Dest bytes framed write not exist")))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            match dest_bytes_framed_write.poll_close(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow!("Dest bytes framed write not exist")))
    }
}

#[pin_project(PinnedDrop)]
pub(crate) struct DestConnectionRead {
    agent_connection_id: String,
    tcp_loop_key: String,
    #[pin]
    dest_bytes_framed_read: Option<DestBytesFramedRead>,
}

impl DestConnectionRead {
    pub(crate) fn new(agent_connection_id: String, tcp_loop_key: String, dest_bytes_framed_read: DestBytesFramedRead) -> Self {
        Self {
            agent_connection_id,
            tcp_loop_key,
            dest_bytes_framed_read: Some(dest_bytes_framed_read),
        }
    }
}

#[pinned_drop]
impl PinnedDrop for DestConnectionRead {
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let agent_connection_id = this.agent_connection_id.clone();
        let tcp_loop_key = this.tcp_loop_key.clone();
        if let Some(dest_bytes_framed_read) = this.dest_bytes_framed_read.take() {
            tokio::spawn(async move {
                debug!("Agent connection {agent_connection_id} with tcp loop key {tcp_loop_key} drop dest connection read.");
                drop(dest_bytes_framed_read)
            });
        };
    }
}

impl Stream for DestConnectionRead {
    type Item = Result<BytesMut, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if let Some(dest_bytes_framed_read) = this.dest_bytes_framed_read.as_pin_mut() {
            return match dest_bytes_framed_read.poll_next(cx) {
                Poll::Ready(value) => match value {
                    None => Poll::Ready(None),
                    Some(value) => Poll::Ready(Some(value.map_err(|e| anyhow!(e)))),
                },
                Poll::Pending => Poll::Pending,
            };
        }
        Poll::Ready(None)
    }
}
