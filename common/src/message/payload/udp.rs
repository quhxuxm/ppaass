use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpInitRequest {}

#[derive(Serialize, Deserialize)]
pub enum UdpInitResponseType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpInitResponse {
    pub loop_key: String,
    pub response_type: UdpInitResponseType,
}

impl TryFrom<Vec<u8>> for UdpInitRequest {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate UdpInitRequest from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<UdpInitRequest> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpInitRequest) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| anyhow!("Fail generate bytes from UdpInitRequest because of error: {e:?}"))
    }
}

impl TryFrom<Vec<u8>> for UdpInitResponse {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate UdpInitResponse from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<UdpInitResponse> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpInitResponse) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| anyhow!("Fail generate bytes from UdpInitResponse because of error: {e:?}"))
    }
}

pub struct UdpDataParts {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub raw_data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UdpData {
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    raw_data: Vec<u8>,
}

impl UdpData {
    pub fn split(self) -> UdpDataParts {
        UdpDataParts {
            src_address: self.src_address,
            dst_address: self.dst_address,
            raw_data: self.raw_data,
        }
    }
}

impl From<UdpDataParts> for UdpData {
    fn from(value: UdpDataParts) -> Self {
        Self {
            src_address: value.src_address,
            dst_address: value.dst_address,
            raw_data: value.raw_data,
        }
    }
}
impl TryFrom<Vec<u8>> for UdpData {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate UdpData from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<UdpData> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| anyhow!("Fail generate bytes from UdpData because of error: {e:?}"))
    }
}
