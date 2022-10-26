use crate::error::JsonSerializeError;
use crate::error::{Error, JsonDeserializeError};
use serde_derive::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;

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
#[serde(rename_all = "camelCase")]
pub struct PpaassMessagePayload {
    payload_type: PpaassMessagePayloadType,
    source_address: Option<PpaassProtocolAddress>,
    target_address: Option<PpaassProtocolAddress>,
    additional_info: HashMap<PayloadAdditionalInfoKey, PayloadAdditionalInfoValue>,
    #[serde(with = "vec_u8_to_base64")]
    data: Vec<u8>,
}

pub struct PpaassMessagePayloadParts {
    pub payload_type: PpaassMessagePayloadType,
    pub source_address: Option<PpaassProtocolAddress>,
    pub target_address: Option<PpaassProtocolAddress>,
    pub additional_info: HashMap<PayloadAdditionalInfoKey, PayloadAdditionalInfoValue>,
    pub data: Vec<u8>,
}

impl PpaassMessagePayload {
    pub fn new(
        source_address: Option<PpaassProtocolAddress>, target_address: Option<PpaassProtocolAddress>, payload_type: PpaassMessagePayloadType, data: Vec<u8>,
    ) -> Self {
        Self {
            payload_type,
            source_address,
            target_address,
            additional_info: HashMap::new(),
            data,
        }
    }

    pub fn get_additional_info(&self, key: &PayloadAdditionalInfoKey) -> Option<&PayloadAdditionalInfoValue> {
        self.additional_info.get(key)
    }

    pub fn add_additional_info(&mut self, key: PayloadAdditionalInfoKey, value: PayloadAdditionalInfoValue) -> Option<PayloadAdditionalInfoValue> {
        self.additional_info.insert(key, value)
    }

    pub fn get_source_address(&self) -> &Option<PpaassProtocolAddress> {
        &self.source_address
    }

    pub fn get_target_address(&self) -> &Option<PpaassProtocolAddress> {
        &self.target_address
    }

    pub fn get_payload_type(&self) -> &PpaassMessagePayloadType {
        &self.payload_type
    }

    pub fn split(self) -> PpaassMessagePayloadParts {
        PpaassMessagePayloadParts {
            data: self.data,
            payload_type: self.payload_type,
            source_address: self.source_address,
            target_address: self.target_address,
            additional_info: self.additional_info,
        }
    }
}

impl From<PpaassMessagePayloadParts> for PpaassMessagePayload {
    fn from(value: PpaassMessagePayloadParts) -> Self {
        Self {
            payload_type: value.payload_type,
            source_address: value.source_address,
            target_address: value.target_address,
            additional_info: value.additional_info,
            data: value.data,
        }
    }
}

impl TryFrom<PpaassMessagePayload> for Vec<u8> {
    type Error = Error;

    fn try_from(value: PpaassMessagePayload) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context(JsonSerializeError {
            message: "Fail to serialize PpaassMessagePayload object to bytes",
        })?;
        Ok(result)
    }
}

impl TryFrom<Vec<u8>> for PpaassMessagePayload {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(value.as_ref()).context(JsonDeserializeError {
            message: "Fail to deserialize bytes to PpaassMessagePayload object",
        })?;
        Ok(result)
    }
}
