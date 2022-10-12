use std::collections::HashMap;

use ppaass_common::PpaassError;
use serde_derive::{Deserialize, Serialize};
use tracing::error;

use crate::{serializer::vec_u8_to_base64, PpaassMessagePayloadType, PpaassProtocolAddress};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PayloadAdditionalInfoKey {
    ReferenceMessageId,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PayloadAdditionalInfoValue {
    ReferenceMessageIdValue(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PpaassMessagePayload {
    payload_type: PpaassMessagePayloadType,
    source_address: Option<PpaassProtocolAddress>,
    target_address: Option<PpaassProtocolAddress>,
    additional_info: HashMap<PayloadAdditionalInfoKey, PayloadAdditionalInfoValue>,
    #[serde(with = "vec_u8_to_base64")]
    data: Vec<u8>,
}

impl TryFrom<PpaassMessagePayload> for Vec<u8> {
    type Error = PpaassError;

    fn try_from(value: PpaassMessagePayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).map_err(|e| {
            error!("Fail to convert message payload object to bytes because of error: {e:#?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}

impl TryFrom<Vec<u8>> for PpaassMessagePayload {
    type Error = PpaassError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).map_err(|e| {
            error!("Fail to convert bytes to message payload object because of error: {e:?}");
            PpaassError::CodecError
        })?;
        Ok(result)
    }
}
