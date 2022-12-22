use crate::PpaassNetAddress;
use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::serializer::option_vec_array_u8_l4_to_base64;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainResolveRequestPayload {
    pub domain_name: String,
    pub request_id: String,
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for DomainResolveRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate DomainResolveRequestPayload from input bytes")
    }
}

impl TryFrom<DomainResolveRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: DomainResolveRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from DomainResolveRequestPayload")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DomainResolveResponseType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainResolveResponsePayload {
    pub request_id: String,
    pub domain_name: String,
    #[serde(with = "option_vec_array_u8_l4_to_base64")]
    pub resolved_ip_addresses: Option<Vec<[u8; 4]>>,
    pub response_type: DomainResolveResponseType,
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for DomainResolveResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate DomainResolveResponsePayload from input bytes")
    }
}

impl TryFrom<DomainResolveResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: DomainResolveResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from DomainResolveResponsePayload")
    }
}
