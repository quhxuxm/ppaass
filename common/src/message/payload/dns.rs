use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{CommonError, DeserializeError, SerializeError};

////////////////////////////////
/// Dns lookup request
///////////////////////////////
pub struct DnsLookupRequestParts {
    pub request_id: u16,
    pub domain_names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DnsLookupRequest {
    request_id: u16,
    domain_names: Vec<String>,
}

impl DnsLookupRequest {
    pub fn split(self) -> DnsLookupRequestParts {
        DnsLookupRequestParts {
            domain_names: self.domain_names,
            request_id: self.request_id,
        }
    }
}

impl From<DnsLookupRequestParts> for DnsLookupRequest {
    fn from(value: DnsLookupRequestParts) -> Self {
        Self {
            domain_names: value.domain_names,
            request_id: value.request_id,
        }
    }
}
impl TryFrom<Vec<u8>> for DnsLookupRequest {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::DnsLookupRequest(e).into()))
    }
}

impl TryFrom<DnsLookupRequest> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: DnsLookupRequest) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::DnsLookupRequest(e).into()))
    }
}

////////////////////////////////
/// Dns lookup response
///////////////////////////////

pub struct DnsLookupResponseParts {
    pub addresses: HashMap<String, Option<Vec<[u8; 4]>>>,
    pub request_id: u16,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DnsLookupResponse {
    addresses: HashMap<String, Option<Vec<[u8; 4]>>>,
    request_id: u16,
}

impl DnsLookupResponse {
    pub fn split(self) -> DnsLookupResponseParts {
        DnsLookupResponseParts {
            request_id: self.request_id,
            addresses: self.addresses,
        }
    }
}

impl From<DnsLookupResponseParts> for DnsLookupResponse {
    fn from(value: DnsLookupResponseParts) -> Self {
        Self {
            request_id: value.request_id,
            addresses: value.addresses,
        }
    }
}
impl TryFrom<Vec<u8>> for DnsLookupResponse {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::DnsLookupResponse(e).into()))
    }
}

impl TryFrom<DnsLookupResponse> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: DnsLookupResponse) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::DnsLookupResponse(e).into()))
    }
}
