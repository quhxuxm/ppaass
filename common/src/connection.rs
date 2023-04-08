use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{codec::PpaassMessageCodec, PpaassMessage, RsaCryptoFetcher};
use anyhow::anyhow;
use anyhow::Result;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use log::{debug, error};
use pin_project::{pin_project, pinned_drop};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

type PpaassMessageFramedRead<T, R> = SplitStream<Framed<T, PpaassMessageCodec<R>>>;
type PpaassMessageFramedWrite<T, R> = SplitSink<Framed<T, PpaassMessageCodec<R>>, PpaassMessage>;

pub struct PpaassConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    framed_read: PpaassMessageFramedRead<T, R>,
    framed_write: PpaassMessageFramedWrite<T, R>,
    connection_id: I,
}

impl<T, R, I> PpaassConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new(connection_id: I, stream: T, rsa_crypto_fetcher: Arc<R>, compress: bool, buffer_size: usize) -> Self {
        let ppaass_message_codec = PpaassMessageCodec::new(compress, rsa_crypto_fetcher);
        let ppaass_message_framed = Framed::with_capacity(stream, ppaass_message_codec, buffer_size);
        let (framed_write, framed_read) = ppaass_message_framed.split();
        Self {
            framed_write,
            framed_read,
            connection_id,
        }
    }

    pub fn split(self) -> (PpaassConnectionRead<T, R, I>, PpaassConnectionWrite<T, R, I>) {
        let read_part = PpaassConnectionRead::new(self.connection_id.clone(), self.framed_read);
        let write_part = PpaassConnectionWrite::new(self.connection_id.clone(), self.framed_write);
        (read_part, write_part)
    }
}

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
    fn new(connection_id: I, ppaass_message_framed_write: PpaassMessageFramedWrite<T, R>) -> Self {
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
    fn new(connection_id: I, ppaass_message_framed_read: PpaassMessageFramedRead<T, R>) -> Self {
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
