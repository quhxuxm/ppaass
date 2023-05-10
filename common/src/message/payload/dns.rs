use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{CommonError, DeserializeError, SerializeError};

////////////////////////////////
/// Dns lookup request
///////////////////////////////

#[derive(Serialize, Deserialize, Debug, Constructor)]
#[non_exhaustive]
pub struct DnsLookupRequest {
    pub request_id: u16,
    pub domain_names: Vec<String>,
}

impl TryFrom<&[u8]> for DnsLookupRequest {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::DnsLookupRequest(e).into()))
    }
}

impl TryFrom<DnsLookupRequest> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: DnsLookupRequest) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::DnsLookupRequest(e).into()))
    }
}

////////////////////////////////
/// Dns lookup response
///////////////////////////////

#[derive(Serialize, Deserialize, Debug, Constructor)]
#[non_exhaustive]
pub struct DnsLookupResponse {
    pub request_id: u16,
    pub addresses: HashMap<String, Option<Vec<[u8; 4]>>>,
}

impl TryFrom<&[u8]> for DnsLookupResponse {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::DnsLookupResponse(e).into()))
    }
}

impl TryFrom<DnsLookupResponse> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: DnsLookupResponse) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::DnsLookupResponse(e).into()))
    }
}
