use crate::generate_uuid;
use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::serializer::vec_u8_to_base64;

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
pub struct PpaassMessage {
    id: String,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    #[serde(with = "vec_u8_to_base64")]
    payload_bytes: Vec<u8>,
}

pub struct PpaassMessageParts {
    pub id: String,
    pub user_token: String,
    pub payload_encryption: PpaassMessagePayloadEncryption,
    pub payload_bytes: Vec<u8>,
}

impl PpaassMessage {
    pub fn new(user_token: &str, payload_encryption: PpaassMessagePayloadEncryption, payload_bytes: Vec<u8>) -> Self {
        Self {
            id: generate_uuid(),
            user_token: user_token.to_owned(),
            payload_encryption,
            payload_bytes,
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

    pub fn split(self) -> PpaassMessageParts {
        PpaassMessageParts {
            id: self.id,
            user_token: self.user_token,
            payload_encryption: self.payload_encryption,
            payload_bytes: self.payload_bytes,
        }
    }
}

impl From<PpaassMessageParts> for PpaassMessage {
    fn from(value: PpaassMessageParts) -> Self {
        Self {
            id: value.id,
            user_token: value.user_token,
            payload_encryption: value.payload_encryption,
            payload_bytes: value.payload_bytes,
        }
    }
}

impl TryFrom<Vec<u8>> for PpaassMessage {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(&value).context("fail to deserialize bytes to PpaassMessage object")?;
        Ok(result)
    }
}

impl TryFrom<PpaassMessage> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("fail to serialize PpaassMessage object to bytes")?;
        Ok(result)
    }
}
