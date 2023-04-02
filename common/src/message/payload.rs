use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};

use crate::{serializer::vec_u8_to_base64, PpaassMessageAgentPayloadType, PpaassMessageProxyPayloadType};

pub mod domain_resolve;
pub mod heartbeat;
pub mod tcp_loop;
pub mod udp_loop;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessageAgentPayload {
    payload_type: PpaassMessageAgentPayloadType,
    #[serde(with = "vec_u8_to_base64")]
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
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessageAgentPayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("Fail to serialize PpaassMessageAgentPayload object to bytes")?;
        Ok(result)
    }
}

impl TryFrom<Vec<u8>> for PpaassMessageAgentPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).context("Fail to deserialize bytes to PpaassMessageAgentPayload object")?;
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessageProxyPayload {
    payload_type: PpaassMessageProxyPayloadType,
    #[serde(with = "vec_u8_to_base64")]
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
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessageProxyPayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("Fail to serialize PpaassMessageProxyPayload object to bytes")?;
        Ok(result)
    }
}

impl TryFrom<Vec<u8>> for PpaassMessageProxyPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).context("Fail to deserialize bytes to PpaassMessageProxyPayload object")?;
        Ok(result)
    }
}
