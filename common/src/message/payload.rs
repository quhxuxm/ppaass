use std::borrow::Cow;

use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};

use crate::{serializer::caw_u8_slince_to_base64, PpaassMessageAgentPayloadType, PpaassMessageProxyPayloadType};

pub mod tcp;
pub mod udp;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessageAgentPayload<'a> {
    payload_type: PpaassMessageAgentPayloadType,
    #[serde(with = "caw_u8_slince_to_base64")]
    data: Cow<'a, [u8]>,
}

pub struct PpaassMessageAgentPayloadParts<'a> {
    pub payload_type: PpaassMessageAgentPayloadType,
    pub data: Cow<'a, [u8]>,
}

impl PpaassMessageAgentPayload<'_> {
    pub fn new(payload_type: PpaassMessageAgentPayloadType, data: Cow<'_, [u8]>) -> Self {
        Self { payload_type, data }
    }

    pub fn split<'b>(self) -> PpaassMessageAgentPayloadParts<'b> {
        PpaassMessageAgentPayloadParts {
            data: self.data,
            payload_type: self.payload_type,
        }
    }
}

impl From<PpaassMessageAgentPayloadParts<'_>> for PpaassMessageAgentPayload<'_> {
    fn from(value: PpaassMessageAgentPayloadParts) -> Self {
        Self {
            payload_type: value.payload_type,
            data: value.data,
        }
    }
}

impl TryFrom<PpaassMessageAgentPayload<'_>> for Cow<'_, [u8]> {
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessageAgentPayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("Fail to serialize PpaassMessageAgentPayload object to bytes")?;
        Ok(result.into())
    }
}

impl TryFrom<Cow<'_, [u8]>> for PpaassMessageAgentPayload<'_> {
    type Error = anyhow::Error;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).context("Fail to deserialize bytes to PpaassMessageAgentPayload object")?;
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessageProxyPayload<'a> {
    payload_type: PpaassMessageProxyPayloadType,
    #[serde(with = "caw_u8_slince_to_base64")]
    data: Cow<'a, [u8]>,
}

pub struct PpaassMessageProxyPayloadParts<'a> {
    pub payload_type: PpaassMessageProxyPayloadType,
    pub data: Cow<'a, [u8]>,
}

impl<'a> PpaassMessageProxyPayload<'a> {
    pub fn new(payload_type: PpaassMessageProxyPayloadType, data: Cow<'_, [u8]>) -> Self {
        Self { payload_type, data }
    }

    pub fn split<'b>(self) -> PpaassMessageProxyPayloadParts<'b> {
        PpaassMessageProxyPayloadParts {
            data: self.data,
            payload_type: self.payload_type,
        }
    }
}

impl From<PpaassMessageProxyPayloadParts<'_>> for PpaassMessageProxyPayload<'_> {
    fn from(value: PpaassMessageProxyPayloadParts) -> Self {
        Self {
            payload_type: value.payload_type,
            data: value.data,
        }
    }
}

impl TryFrom<PpaassMessageProxyPayload<'_>> for Cow<'_, [u8]> {
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessageProxyPayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("Fail to serialize PpaassMessageProxyPayload object to bytes")?;
        Ok(result.into())
    }
}

impl TryFrom<Cow<'_, [u8]>> for PpaassMessageProxyPayload<'_> {
    type Error = anyhow::Error;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).context("Fail to deserialize bytes to PpaassMessageProxyPayload object")?;
        Ok(result)
    }
}
