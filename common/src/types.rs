use crate::codec::PpaassMessageCodec;
use crate::PpaassMessage;
use futures_util::stream::{SplitSink, SplitStream};
use tokio_util::codec::Framed;

pub(crate) type PpaassMessageFramedRead<T, R> = SplitStream<Framed<T, PpaassMessageCodec<R>>>;
pub(crate) type PpaassMessageFramedWrite<T, R> = SplitSink<Framed<T, PpaassMessageCodec<R>>, PpaassMessage>;
