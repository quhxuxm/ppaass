use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

use crate::{CommonError, DeserializeError, PpaassMessageAgentPayloadType, PpaassMessageProxyPayloadType, SerializeError};

pub mod dns;
pub mod tcp;
pub mod udp;

#[derive(Debug, Serialize, Deserialize, Constructor)]
pub struct PpaassMessageAgentPayload {
    pub payload_type: PpaassMessageAgentPayloadType,
    pub data: Vec<u8>,
}

impl TryFrom<PpaassMessageAgentPayload> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessageAgentPayload) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessageAgentPayload(e).into()))
    }
}

impl TryFrom<Vec<u8>> for PpaassMessageAgentPayload {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessageAgentPayload(e).into()))
    }
}

#[derive(Debug, Serialize, Deserialize, Constructor)]
pub struct PpaassMessageProxyPayload {
    pub payload_type: PpaassMessageProxyPayloadType,
    pub data: Vec<u8>,
}

impl TryFrom<PpaassMessageProxyPayload> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessageProxyPayload) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessageProxyPayload(e).into()))
    }
}

impl TryFrom<Vec<u8>> for PpaassMessageProxyPayload {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessageProxyPayload(e).into()))
    }
}
