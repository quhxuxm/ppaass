use crate::{generate_uuid, CommonError, DeserializeError, SerializeError};

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
#[non_exhaustive]
pub struct PpaassMessage {
    id: String,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    #[serde(with = "vec_u8_to_base64")]
    payload: Vec<u8>,
}

pub struct PpaassMessageParts {
    pub id: String,
    pub user_token: String,
    pub payload_encryption: PpaassMessagePayloadEncryption,
    pub payload: Vec<u8>,
}

impl PpaassMessage {
    pub fn new(user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, payload: Vec<u8>) -> Self {
        Self {
            id: generate_uuid(),
            user_token: user_token.to_string(),
            payload_encryption,
            payload,
        }
    }

    pub fn split(self) -> PpaassMessageParts {
        PpaassMessageParts {
            id: self.id,
            user_token: self.user_token,
            payload_encryption: self.payload_encryption,
            payload: self.payload,
        }
    }
}

impl From<PpaassMessageParts> for PpaassMessage {
    fn from(value: PpaassMessageParts) -> Self {
        Self {
            id: value.id,
            user_token: value.user_token,
            payload_encryption: value.payload_encryption,
            payload: value.payload,
        }
    }
}

impl TryFrom<Vec<u8>> for PpaassMessage {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassMessage> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}
