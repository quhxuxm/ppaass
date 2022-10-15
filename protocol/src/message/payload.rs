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
#[serde(rename_all = "camelCase")]
pub struct PpaassMessagePayload {
    connection_id: Option<String>,
    payload_type: PpaassMessagePayloadType,
    source_address: Option<PpaassProtocolAddress>,
    target_address: Option<PpaassProtocolAddress>,
    additional_info: HashMap<PayloadAdditionalInfoKey, PayloadAdditionalInfoValue>,
    #[serde(with = "vec_u8_to_base64")]
    data: Vec<u8>,
}

pub struct PpaassMessagePayloadParts {
    pub connection_id: Option<String>,
    pub payload_type: PpaassMessagePayloadType,
    pub source_address: Option<PpaassProtocolAddress>,
    pub target_address: Option<PpaassProtocolAddress>,
    pub additional_info: HashMap<PayloadAdditionalInfoKey, PayloadAdditionalInfoValue>,
    pub data: Vec<u8>,
}

impl PpaassMessagePayload {
    pub fn new(
        connection_id: Option<String>, source_address: Option<PpaassProtocolAddress>, target_address: Option<PpaassProtocolAddress>,
        payload_type: PpaassMessagePayloadType, data: Vec<u8>,
    ) -> Self {
        Self {
            connection_id,
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

    pub fn get_client_connection_id(&self) -> &Option<String> {
        &self.connection_id
    }

    pub fn split(self) -> PpaassMessagePayloadParts {
        PpaassMessagePayloadParts {
            connection_id: self.connection_id,
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
            connection_id: value.connection_id,
            payload_type: value.payload_type,
            source_address: value.source_address,
            target_address: value.target_address,
            additional_info: value.additional_info,
            data: value.data,
        }
    }
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
