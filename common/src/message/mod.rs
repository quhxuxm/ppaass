use crate::{CommonError, DeserializeError, SerializeError};

use bytes::BytesMut;
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

use anyhow::Result;

mod address;
mod encryption;
mod generator;
mod payload;

pub use address::*;
pub use encryption::*;
pub use generator::*;
pub use payload::*;

#[derive(Serialize, Deserialize, Debug)]
pub enum PpaassMessageAgentPayloadType {
    TcpInit,
    TcpData,
    UdpData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassAgentMessagePayload {
    pub payload_type: PpaassMessageAgentPayloadType,
    pub data: BytesMut,
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Constructor)]
pub struct PpaassAgentMessage {
    pub id: String,
    pub user_token: String,
    pub encryption: PpaassMessagePayloadEncryption,
    pub payload: PpaassAgentMessagePayload,
}

impl TryFrom<Vec<u8>> for PpaassAgentMessage {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<&[u8]> for PpaassAgentMessage {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassAgentMessage> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassAgentMessage) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PpaassMessageProxyPayloadType {
    TcpInit,
    TcpData,
    UdpData,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassProxyMessagePayload {
    pub payload_type: PpaassMessageProxyPayloadType,
    pub data: BytesMut,
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Constructor)]
pub struct PpaassProxyMessage {
    pub id: String,
    pub user_token: String,
    pub encryption: PpaassMessagePayloadEncryption,
    pub payload: PpaassProxyMessagePayload,
}

impl TryFrom<BytesMut> for PpaassProxyMessage {
    type Error = CommonError;

    fn try_from(value: BytesMut) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<&[u8]> for PpaassProxyMessage {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassProxyMessage> for BytesMut {
    type Error = CommonError;

    fn try_from(value: PpaassProxyMessage) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(BytesMut::from_iter)
            .map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}
