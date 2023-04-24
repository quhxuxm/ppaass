use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{codec::PpaassMessageCodec, PpaassMessage, RsaCryptoFetcher};

use anyhow::Result;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, Stream, StreamExt,
};

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

type PpaassMessageFramedRead<'a, 'b, T, R> = SplitStream<Framed<T, PpaassMessageCodec<'a, 'b, R>>>;
type PpaassMessageFramedWrite<'a, 'b, T, R> = SplitSink<Framed<T, PpaassMessageCodec<'a, 'b, R>>, PpaassMessage<'a, 'b>>;

pub struct PpaassConnectionParts<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub read: PpaassConnectionRead<'a, 'b, T, R, I>,
    pub write: PpaassConnectionWrite<'a, 'b, T, R, I>,
    pub id: I,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct PpaassConnection<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    framed_read: PpaassMessageFramedRead<'a, 'b, T, R>,
    framed_write: PpaassMessageFramedWrite<'a, 'b, T, R>,
    id: I,
}

impl<'a, 'b, T, R, I> PpaassConnection<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new(id: I, stream: T, rsa_crypto_fetcher: Arc<R>, compress: bool, buffer_size: usize) -> Self {
        let ppaass_message_codec = PpaassMessageCodec::new(compress, rsa_crypto_fetcher);
        let ppaass_message_framed = Framed::with_capacity(stream, ppaass_message_codec, buffer_size);
        let (framed_write, framed_read) = ppaass_message_framed.split();
        Self { framed_write, framed_read, id }
    }

    pub fn split<'c, 'd>(self) -> PpaassConnectionParts<'c, 'd, T, R, I> {
        let read = PpaassConnectionRead::new(self.id.clone(), self.framed_read);
        let write = PpaassConnectionWrite::new(self.id.clone(), self.framed_write);
        let id = self.id;
        PpaassConnectionParts { read, write, id }
    }
}

#[pin_project]
#[derive(Debug)]
pub struct PpaassConnectionWrite<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    framed_write: PpaassMessageFramedWrite<'a, 'b, T, R>,
}

impl<'a, 'b, T, R, I> PpaassConnectionWrite<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn new<'c, 'd>(connection_id: I, framed_write: PpaassMessageFramedWrite<'c, 'd, T, R>) -> Self {
        Self { connection_id, framed_write }
    }
}

impl<'a, 'b, T, R, I> Sink<PpaassMessage<'a, 'b>> for PpaassConnectionWrite<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.framed_write.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: PpaassMessage) -> Result<(), Self::Error> {
        let this = self.project();
        this.framed_write.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.framed_write.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.framed_write.poll_close(cx)
    }
}

#[pin_project]
#[derive(Debug)]
pub struct PpaassConnectionRead<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    framed_read: PpaassMessageFramedRead<'a, 'b, T, R>,
}

impl<'a, 'b, T, R, I> PpaassConnectionRead<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn new(connection_id: I, framed_read: PpaassMessageFramedRead<T, R>) -> Self {
        Self { connection_id, framed_read }
    }
}

impl<'a, 'b, T, R, I> Stream for PpaassConnectionRead<'a, 'b, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    type Item = Result<PpaassMessage<'a, 'b>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.framed_read.poll_next(cx)
    }
}
