use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};

#[derive(Serialize, Deserialize, Constructor)]
pub struct AgentTcpInit {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
}

#[derive(Serialize, Deserialize)]
pub enum ProxyTcpInitResultType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize, Constructor)]
pub struct ProxyTcpInit {
    pub id: String,
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub result_type: ProxyTcpInitResultType,
}

impl TryFrom<Vec<u8>> for AgentTcpInit {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<&[u8]> for AgentTcpInit {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<AgentTcpInit> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: AgentTcpInit) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<Vec<u8>> for ProxyTcpInit {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitResponse(e).into()))
    }
}

impl TryFrom<&[u8]> for ProxyTcpInit {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitResponse(e).into()))
    }
}

impl TryFrom<ProxyTcpInit> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: ProxyTcpInit) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpInitResponse(e).into()))
    }
}

#[derive(Serialize, Deserialize, Constructor)]
#[non_exhaustive]
pub struct AgentTcpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Vec<u8>,
}

impl TryFrom<&[u8]> for AgentTcpData {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<Vec<u8>> for AgentTcpData {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<AgentTcpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: AgentTcpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpData(e).into()))
    }
}

#[derive(Serialize, Deserialize, Constructor)]
#[non_exhaustive]
pub struct ProxyTcpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Vec<u8>,
}

impl TryFrom<&[u8]> for ProxyTcpData {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<Vec<u8>> for ProxyTcpData {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<ProxyTcpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: ProxyTcpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::TcpData(e).into()))
    }
}
