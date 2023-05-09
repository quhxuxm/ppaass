use std::borrow::Cow;

use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

use crate::{CommonError, DeserializeError, PpaassMessageAgentPayloadType, PpaassMessageProxyPayloadType, SerializeError};

pub mod dns;
pub mod tcp;
pub mod udp;

#[derive(Debug, Serialize, Deserialize, Constructor)]
pub struct PpaassMessageAgentPayload<'a> {
    pub payload_type: PpaassMessageAgentPayloadType,
    pub data: Cow<'a, [u8]>,
}

impl TryFrom<PpaassMessageAgentPayload<'_>> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessageAgentPayload) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessageAgentPayload(e).into()))
    }
}

impl<'a> TryFrom<Cow<'a, [u8]>> for PpaassMessageAgentPayload<'a> {
    type Error = CommonError;

    fn try_from(value: Cow<'a, [u8]>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessageAgentPayload(e).into()))
    }
}

#[derive(Debug, Serialize, Deserialize, Constructor)]
pub struct PpaassMessageProxyPayload<'a> {
    pub payload_type: PpaassMessageProxyPayloadType,
    pub data: Cow<'a, [u8]>,
}

impl TryFrom<PpaassMessageProxyPayload<'_>> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessageProxyPayload) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessageProxyPayload(e).into()))
    }
}

impl TryFrom<Cow<'_, [u8]>> for PpaassMessageProxyPayload<'_> {
    type Error = CommonError;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessageProxyPayload(e).into()))
    }
}
