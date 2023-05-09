use serde_derive::{Deserialize, Serialize};

use crate::{CommonError, DeserializeError, PpaassMessageAgentPayloadType, PpaassMessageProxyPayloadType, SerializeError};

pub mod dns;
pub mod tcp;
pub mod udp;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessageAgentPayload {
    payload_type: PpaassMessageAgentPayloadType,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

pub struct PpaassMessageAgentPayloadParts {
    pub payload_type: PpaassMessageAgentPayloadType,
    pub data: Vec<u8>,
}

impl PpaassMessageAgentPayload {
    pub fn new(payload_type: PpaassMessageAgentPayloadType, data: Vec<u8>) -> Self {
        Self { payload_type, data }
    }

    pub fn split(self) -> PpaassMessageAgentPayloadParts {
        PpaassMessageAgentPayloadParts {
            data: self.data,
            payload_type: self.payload_type,
        }
    }
}

impl From<PpaassMessageAgentPayloadParts> for PpaassMessageAgentPayload {
    fn from(value: PpaassMessageAgentPayloadParts) -> Self {
        Self {
            payload_type: value.payload_type,
            data: value.data,
        }
    }
}

impl TryFrom<PpaassMessageAgentPayload> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessageAgentPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessageAgentPayload(e).into()))
    }
}

impl TryFrom<Vec<u8>> for PpaassMessageAgentPayload {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(value.as_ref()).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessageAgentPayload(e).into()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessageProxyPayload {
    payload_type: PpaassMessageProxyPayloadType,
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

pub struct PpaassMessageProxyPayloadParts {
    pub payload_type: PpaassMessageProxyPayloadType,
    pub data: Vec<u8>,
}

impl PpaassMessageProxyPayload {
    pub fn new(payload_type: PpaassMessageProxyPayloadType, data: Vec<u8>) -> Self {
        Self { payload_type, data }
    }

    pub fn split(self) -> PpaassMessageProxyPayloadParts {
        PpaassMessageProxyPayloadParts {
            data: self.data,
            payload_type: self.payload_type,
        }
    }
}

impl From<PpaassMessageProxyPayloadParts> for PpaassMessageProxyPayload {
    fn from(value: PpaassMessageProxyPayloadParts) -> Self {
        Self {
            payload_type: value.payload_type,
            data: value.data,
        }
    }
}

impl TryFrom<PpaassMessageProxyPayload> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessageProxyPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessageProxyPayload(e).into()))
    }
}

impl TryFrom<Vec<u8>> for PpaassMessageProxyPayload {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(value.as_ref()).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessageProxyPayload(e).into()))
    }
}
