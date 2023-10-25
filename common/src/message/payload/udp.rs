use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};
use bytes::Bytes;
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

////////////////////////////////
/// Udp data
///////////////////////////////

#[non_exhaustive]
#[derive(Serialize, Deserialize, Constructor)]
pub struct AgentUdpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Bytes,
    pub need_response: bool,
}

impl TryFrom<Bytes> for AgentUdpData {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<AgentUdpData> for Bytes {
    type Error = CommonError;

    fn try_from(value: AgentUdpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Constructor)]
pub struct ProxyUdpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Bytes,
}

impl TryFrom<Bytes> for ProxyUdpData {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<ProxyUdpData> for Bytes {
    type Error = CommonError;

    fn try_from(value: ProxyUdpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}
