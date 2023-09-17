use crate::{CommonError, DeserializeError, SerializeError};

use bytes::Bytes;
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageAgentTcpPayloadType {
    Init,
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageAgentUdpPayloadType {
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageAgentProtocol {
    Tcp(PpaassMessageAgentTcpPayloadType),
    Udp(PpaassMessageAgentUdpPayloadType),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassAgentMessagePayload {
    pub protocol: PpaassMessageAgentProtocol,
    pub data: Bytes,
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Constructor)]
pub struct PpaassAgentMessage {
    pub id: String,
    pub user_token: String,
    pub encryption: PpaassMessagePayloadEncryption,
    pub payload: PpaassAgentMessagePayload,
}

impl TryFrom<Bytes> for PpaassAgentMessage {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassAgentMessage> for Bytes {
    type Error = CommonError;

    fn try_from(value: PpaassAgentMessage) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageProxyTcpPayloadType {
    Init,
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageProxyUdpPayloadType {
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageProxyProtocol {
    Tcp(PpaassMessageProxyTcpPayloadType),
    Udp(PpaassMessageProxyUdpPayloadType),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassProxyMessagePayload {
    pub protocol: PpaassMessageProxyProtocol,
    pub data: Bytes,
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Constructor)]
pub struct PpaassProxyMessage {
    pub id: String,
    pub user_token: String,
    pub encryption: PpaassMessagePayloadEncryption,
    pub payload: PpaassProxyMessagePayload,
}

impl TryFrom<Bytes> for PpaassProxyMessage {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassProxyMessage> for Bytes {
    type Error = CommonError;

    fn try_from(value: PpaassProxyMessage) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}
