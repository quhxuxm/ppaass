use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};
use bytes::{Bytes, BytesMut};
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

////////////////////////////////
/// Udp data
///////////////////////////////

#[non_exhaustive]
#[derive(Serialize, Deserialize, Constructor)]
pub struct UdpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Bytes,
}

impl TryFrom<Bytes> for UdpData {
    type Error = CommonError;

    fn try_from(value: Bytes) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<&[u8]> for UdpData {
    type Error = CommonError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<Vec<u8>> for UdpData {
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<UdpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}

impl TryFrom<UdpData> for BytesMut {
    type Error = CommonError;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(BytesMut::from_iter)
            .map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}
