use std::borrow::Cow;

use crate::generate_uuid;
use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};

use crate::serializer::caw_u8_slince_to_base64;

use anyhow::Result;

mod address;
mod encryption;
mod generator;
mod payload;
mod types;

pub use address::*;
pub use encryption::*;
pub use generator::*;
pub use payload::*;
pub use types::*;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PpaassMessage<'a, 'b> {
    id: String,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption<'b>,
    #[serde(with = "caw_u8_slince_to_base64")]
    payload: Cow<'a, [u8]>,
}

pub struct PpaassMessageParts<'a, 'b> {
    pub id: String,
    pub user_token: String,
    pub payload_encryption: PpaassMessagePayloadEncryption<'b>,
    pub payload: Cow<'a, [u8]>,
}

impl PpaassMessage<'_, '_> {
    pub fn new(user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption<'_>, payload: Cow<'_, [u8]>) -> Self {
        Self {
            id: generate_uuid(),
            user_token: user_token.to_string(),
            payload_encryption,
            payload,
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_user_token(&self) -> &str {
        &self.user_token
    }

    pub fn get_payload_encryption(&self) -> &PpaassMessagePayloadEncryption {
        &self.payload_encryption
    }

    pub fn split<'a, 'b>(self) -> PpaassMessageParts<'a, 'b> {
        PpaassMessageParts {
            id: self.id,
            user_token: self.user_token,
            payload_encryption: self.payload_encryption,
            payload: self.payload,
        }
    }
}

impl From<PpaassMessageParts<'_, '_>> for PpaassMessage<'_, '_> {
    fn from(value: PpaassMessageParts<'_, '_>) -> Self {
        Self {
            id: value.id,
            user_token: value.user_token,
            payload_encryption: value.payload_encryption,
            payload: value.payload,
        }
    }
}

impl TryFrom<Cow<'_, [u8]>> for PpaassMessage<'_, '_> {
    type Error = anyhow::Error;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(&value).map_err(|e| anyhow!("Fail to deserialize bytes to PpaassMessage object because of error: {e:?}"))?;
        Ok(result)
    }
}

impl TryFrom<PpaassMessage<'_, '_>> for Cow<'_, [u8]> {
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).map_err(|e| anyhow!("Fail to serialize PpaassMessage object to bytes because of error: {e:?}"))?;
        Ok(result.into())
    }
}
