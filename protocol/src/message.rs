use ppaass_common::PpaassError;
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

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassMessage {
    id: String,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    payload_bytes: Vec<u8>,
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
