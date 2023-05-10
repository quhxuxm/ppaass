use crate::{CommonError, DeserializeError, SerializeError};

use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug, Constructor)]
#[non_exhaustive]
pub struct PpaassMessage {
    pub id: String,
    pub user_token: String,
    pub payload_encryption: PpaassMessagePayloadEncryption,
    pub payload: Vec<u8>,
}

impl TryFrom<&[u8]> for PpaassMessage {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::PpaassMessage(e).into()))
    }
}

impl TryFrom<PpaassMessage> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::PpaassMessage(e).into()))
    }
}
