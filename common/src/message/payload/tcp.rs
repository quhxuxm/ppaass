use serde_derive::{Deserialize, Serialize};

use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};

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

impl TryFrom<Vec<u8>> for TcpInitRequest {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<TcpInitRequest> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: TcpInitRequest) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<Vec<u8>> for TcpInitResponse {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitResponse(e).into()))
    }
}

impl TryFrom<TcpInitResponse> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: TcpInitResponse) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpInitResponse(e).into()))
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
    #[serde(with = "serde_bytes")]
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
impl TryFrom<Vec<u8>> for TcpData {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<TcpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: TcpData) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpData(e).into()))
    }
}
