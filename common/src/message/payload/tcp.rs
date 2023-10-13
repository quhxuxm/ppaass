use bytes::Bytes;
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
    ConnectToDstFail,
}

#[derive(Serialize, Deserialize, Constructor)]
pub struct ProxyTcpInit {
    pub id: String,
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub result_type: ProxyTcpInitResultType,
}

impl TryFrom<Bytes> for AgentTcpInit {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<AgentTcpInit> for Bytes {
    type Error = CommonError;

    fn try_from(value: AgentTcpInit) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::TcpInitRequest(e).into()))
    }
}

impl TryFrom<Bytes> for ProxyTcpInit {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpInitResponse(e).into()))
    }
}

impl TryFrom<ProxyTcpInit> for Bytes {
    type Error = CommonError;

    fn try_from(value: ProxyTcpInit) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::TcpInitResponse(e).into()))
    }
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Constructor)]
pub struct AgentTcpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Bytes,
}

impl TryFrom<Bytes> for AgentTcpData {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<AgentTcpData> for Bytes {
    type Error = CommonError;

    fn try_from(value: AgentTcpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::TcpData(e).into()))
    }
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Constructor)]
pub struct ProxyTcpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Bytes,
}

impl TryFrom<Bytes> for ProxyTcpData {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::TcpData(e).into()))
    }
}

impl TryFrom<ProxyTcpData> for Bytes {
    type Error = CommonError;

    fn try_from(value: ProxyTcpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::TcpData(e).into()))
    }
}
