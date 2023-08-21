use crate::{CommonError, DeserializeError, SerializeError};

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
    TcpInitRequest,
    TcpData,
    UdpData,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PpaassMessageProxyPayloadType {
    TcpInitResponse,
    TcpData,
    UdpData,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PpaassMessagePayload {
    Agent {
        payload_type: PpaassMessageAgentPayloadType,
        data: Vec<u8>,
    },
    Proxy {
        payload_type: PpaassMessageProxyPayloadType,
        data: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize, Debug, Constructor)]
#[non_exhaustive]
pub struct PpaassMessage {
    pub id: String,
    pub user_token: String,
    pub encryption: PpaassMessagePayloadEncryption,
    pub payload: PpaassMessagePayload,
}

impl TryFrom<Vec<u8>> for PpaassMessage {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<&[u8]> for PpaassMessage {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassMessage> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}
