use std::{
    fmt::{Debug, Display},
    pin::Pin,
    task::{Context, Poll},
};

use crate::{
    types::{PpaassMessageFramedRead, PpaassMessageFramedWrite},
    PpaassMessage, RsaCryptoFetcher,
};
use anyhow::anyhow;
use anyhow::Result;
use futures::{Sink, SinkExt, Stream};
use log::{debug, error};
use pin_project::{pin_project, pinned_drop};
use tokio::io::{AsyncRead, AsyncWrite};

#[pin_project(PinnedDrop)]
#[derive(Debug)]
pub struct PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    ppaass_message_framed_write: Option<PpaassMessageFramedWrite<T, R>>,
}

#[pinned_drop]
impl<T, R, I> PinnedDrop for PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let connection_id = this.connection_id.clone();
        if let Some(mut ppaass_message_framed_write) = this.ppaass_message_framed_write.take() {
            tokio::spawn(async move {
                if let Err(e) = ppaass_message_framed_write.close().await {
                    error!("Fail to close agent connection because of error: {e:?}");
                };
                debug!("Ppaass connection writer [{connection_id}] dropped")
            });
        }
    }
}

impl<T, R, I> PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new(connection_id: I, ppaass_message_framed_write: PpaassMessageFramedWrite<T, R>) -> Self {
        Self {
            connection_id,
            ppaass_message_framed_write: Some(ppaass_message_framed_write),
        }
    }
}

impl<T, R, I> Sink<PpaassMessage> for PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let ppaass_message_framed_write = this.ppaass_message_framed_write.as_pin_mut();
        if let Some(ppaass_message_framed_write) = ppaass_message_framed_write {
            ppaass_message_framed_write.poll_ready(cx)
        } else {
            Poll::Ready(Err(anyhow!("Ppaass message framed write not exist.")))
        }
    }

    fn start_send(self: Pin<&mut Self>, item: PpaassMessage) -> Result<(), Self::Error> {
        let this = self.project();
        let ppaass_message_framed_write = this.ppaass_message_framed_write.as_pin_mut();
        if let Some(ppaass_message_framed_write) = ppaass_message_framed_write {
            ppaass_message_framed_write.start_send(item)
        } else {
            Err(anyhow!("Ppaass message framed write not exist."))
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let ppaass_message_framed_write = this.ppaass_message_framed_write.as_pin_mut();
        if let Some(ppaass_message_framed_write) = ppaass_message_framed_write {
            ppaass_message_framed_write.poll_flush(cx)
        } else {
            Poll::Ready(Err(anyhow!("Ppaass message framed write not exist.")))
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let ppaass_message_framed_write = this.ppaass_message_framed_write.as_pin_mut();
        if let Some(ppaass_message_framed_write) = ppaass_message_framed_write {
            ppaass_message_framed_write.poll_close(cx)
        } else {
            Poll::Ready(Err(anyhow!("Ppaass message framed write not exist.")))
        }
    }
}

#[pin_project]
#[derive(Debug)]
pub struct PpaassConnectionRead<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    ppaass_message_framed_read: PpaassMessageFramedRead<T, R>,
}

impl<T, R, I> PpaassConnectionRead<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new(connection_id: I, ppaass_message_framed_read: PpaassMessageFramedRead<T, R>) -> Self {
        Self {
            connection_id,
            ppaass_message_framed_read,
        }
    }
}

impl<T, R, I> Stream for PpaassConnectionRead<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    type Item = Result<PpaassMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.ppaass_message_framed_read.poll_next(cx)
    }
}
