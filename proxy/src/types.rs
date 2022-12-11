use futures_util::stream::{SplitSink, SplitStream};
use ppaass_common::codec::PpaassMessageCodec;
use ppaass_common::PpaassMessage;
use tokio_util::codec::Framed;

pub(crate) type AgentMessageFramedRead<T, R> = SplitStream<Framed<T, PpaassMessageCodec<R>>>;
pub(crate) type AgentMessageFramedWrite<T, R> = SplitSink<Framed<T, PpaassMessageCodec<R>>, PpaassMessage>;
