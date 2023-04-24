use std::borrow::Cow;

use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpInitRequest {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
}

#[derive(Serialize, Deserialize)]
pub enum TcpInitResponseType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpInitResponse {
    pub unique_key: String,
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub response_type: TcpInitResponseType,
}

impl TryFrom<Cow<'_, [u8]>> for TcpInitRequest {
    type Error = anyhow::Error;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate TcpInitRequest from input bytes because of error: {e:}"))
    }
}

impl TryFrom<TcpInitRequest> for Cow<'_, [u8]> {
    type Error = anyhow::Error;

    fn try_from(value: TcpInitRequest) -> Result<Self, Self::Error> {
        Ok(serde_json::to_vec(&value)
            .map_err(|e| anyhow!("Fail generate bytes from TcpInitRequest object because of error: {e:}"))?
            .into())
    }
}

impl TryFrom<Cow<'_, [u8]>> for TcpInitResponse {
    type Error = anyhow::Error;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate TcpInitResponse from input bytes because of error: {e:}"))
    }
}

impl TryFrom<TcpInitResponse> for Cow<'_, [u8]> {
    type Error = anyhow::Error;

    fn try_from(value: TcpInitResponse) -> Result<Self, Self::Error> {
        Ok(serde_json::to_vec(&value)
            .map_err(|e| anyhow!("Fail generate bytes from TcpInitResponse object because of error: {e:}"))?
            .into())
    }
}

pub struct TcpDataParts {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub raw_data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TcpData {
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    raw_data: Vec<u8>,
}

impl TcpData {
    pub fn split(self) -> TcpDataParts {
        TcpDataParts {
            src_address: self.src_address,
            dst_address: self.dst_address,
            raw_data: self.raw_data,
        }
    }
}

impl From<TcpDataParts> for TcpData {
    fn from(value: TcpDataParts) -> Self {
        Self {
            src_address: value.src_address,
            dst_address: value.dst_address,
            raw_data: value.raw_data,
        }
    }
}
impl TryFrom<Cow<'_, [u8]>> for TcpData {
    type Error = anyhow::Error;

    fn try_from(value: Cow<'_, [u8]>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate TcpData from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<TcpData> for Cow<'_, [u8]> {
    type Error = anyhow::Error;

    fn try_from(value: TcpData) -> Result<Self, Self::Error> {
        Ok(serde_json::to_vec(&value)
            .map_err(|e| anyhow!("Fail generate bytes from TcpData because of error: {e:?}"))?
            .into())
    }
}
