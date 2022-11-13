use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};

use crate::{serializer::vec_u8_to_base64, PpaassMessagePayloadType, PpaassNetAddress};

pub mod domain_resolve;
pub mod heartbeat;
pub mod tcp_destroy;
pub mod tcp_initialize;
pub mod tcp_relay;
pub mod udp_destory;
pub mod udp_initialize;
pub mod udp_relay;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessagePayload {
    payload_type: PpaassMessagePayloadType,
    #[serde(with = "vec_u8_to_base64")]
    data: Vec<u8>,
}

pub struct PpaassMessagePayloadParts {
    pub payload_type: PpaassMessagePayloadType,
    pub data: Vec<u8>,
}

impl PpaassMessagePayload {
    pub fn new(payload_type: PpaassMessagePayloadType, data: Vec<u8>) -> Self {
        Self { payload_type, data }
    }

    pub fn split(self) -> PpaassMessagePayloadParts {
        PpaassMessagePayloadParts {
            data: self.data,
            payload_type: self.payload_type,
        }
    }
}

impl From<PpaassMessagePayloadParts> for PpaassMessagePayload {
    fn from(value: PpaassMessagePayloadParts) -> Self {
        Self {
            payload_type: value.payload_type,
            data: value.data,
        }
    }
}

impl TryFrom<PpaassMessagePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessagePayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("fail to serialize PpaassMessagePayload object to bytes")?;
        Ok(result)
    }
}

impl TryFrom<Vec<u8>> for PpaassMessagePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).context("fail to deserialize bytes to PpaassMessagePayload object")?;
        Ok(result)
    }
}
