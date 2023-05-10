use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};

#[derive(Serialize, Deserialize, Constructor)]

pub struct TcpInitRequest {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
}

#[derive(Serialize, Deserialize)]
pub enum TcpInitResponseType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize, Constructor)]

pub struct TcpInitResponse {
    pub id: String,
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub response_type: TcpInitResponseType,
}

impl TryFrom<&[u8]> for TcpInitRequest {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<TcpInitRequest> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: TcpInitRequest) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<&[u8]> for TcpInitResponse {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitResponse(e).into()))
    }
}

impl TryFrom<TcpInitResponse> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: TcpInitResponse) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpInitResponse(e).into()))
    }
}

#[derive(Serialize, Deserialize, Constructor)]
#[non_exhaustive]
pub struct TcpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Vec<u8>,
}

impl TryFrom<&[u8]> for TcpData {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<TcpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: TcpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpData(e).into()))
    }
}
