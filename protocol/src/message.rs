use ppaass_common::{generate_uuid, PpaassError};
use serde_derive::{Deserialize, Serialize};
use tracing::error;

mod address;
mod encryption;
mod payload;
mod types;

pub use address::*;
pub use encryption::*;
pub use payload::*;
pub use types::*;

use crate::serializer::vec_u8_to_base64;

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
    pub fn new(user_token: String, payload_encryption: PpaassMessagePayloadEncryption, payload_bytes: Vec<u8>) -> Self {
        Self {
            id: generate_uuid(),
            user_token,
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
    type Error = PpaassError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(&value).map_err(|e| {
            error!("Fail to convert bytes to message object because of error: {e:?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}

impl TryFrom<PpaassMessage> for Vec<u8> {
    type Error = PpaassError;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).map_err(|e| {
            error!("Fail to convert message object to bytes because of error: {e:#?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}
