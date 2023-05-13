use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{codec::PpaassMessageCodec, CommonError, PpaassMessage, RsaCryptoFetcher};

use futures::{
    stream::{SplitSink, SplitStream},
    Sink, Stream, StreamExt,
};

use pin_project::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

type PpaassMessageFramedRead<T, R> = SplitStream<Framed<T, PpaassMessageCodec<R>>>;
type PpaassMessageFramedWrite<T, R> = SplitSink<Framed<T, PpaassMessageCodec<R>>, PpaassMessage>;

pub struct PpaassConnectionParts<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    pub read_part: PpaassConnectionRead<T, R, I>,
    pub write_part: PpaassConnectionWrite<T, R, I>,
    pub id: I,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct PpaassConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    framed_read: PpaassMessageFramedRead<T, R>,
    framed_write: PpaassMessageFramedWrite<T, R>,
    id: I,
}

impl<T, R, I> PpaassConnection<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    pub fn new(id: I, stream: T, rsa_crypto_fetcher: Arc<R>, compress: bool, buffer_size: usize) -> Self {
        let ppaass_message_codec = PpaassMessageCodec::new(compress, rsa_crypto_fetcher);
        let ppaass_message_framed = Framed::with_capacity(stream, ppaass_message_codec, buffer_size);
        let (framed_write, framed_read) = ppaass_message_framed.split();
        Self { framed_write, framed_read, id }
    }

    pub fn split(self) -> PpaassConnectionParts<T, R, I> {
        let read = PpaassConnectionRead::new(self.id.clone(), self.framed_read);
        let write = PpaassConnectionWrite::new(self.id.clone(), self.framed_write);
        let id = self.id;
        PpaassConnectionParts {
            read_part: read,
            write_part: write,
            id,
        }
    }
}

#[pin_project]
#[derive(Debug)]
pub struct PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    framed_write: PpaassMessageFramedWrite<T, R>,
}

impl<T, R, I> PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    fn new(connection_id: I, framed_write: PpaassMessageFramedWrite<T, R>) -> Self {
        Self { connection_id, framed_write }
    }
}

impl<T, R, I> Sink<PpaassMessage> for PpaassConnectionWrite<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    type Error = CommonError;

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
pub struct PpaassConnectionRead<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    connection_id: I,
    #[pin]
    framed_read: PpaassMessageFramedRead<T, R>,
}

impl<T, R, I> PpaassConnectionRead<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    fn new(connection_id: I, framed_read: PpaassMessageFramedRead<T, R>) -> Self {
        Self { connection_id, framed_read }
    }
}

impl<T, R, I> Stream for PpaassConnectionRead<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    type Item = Result<PpaassMessage, CommonError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.framed_read.poll_next(cx)
    }
}
