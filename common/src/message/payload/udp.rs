use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};
use bytes::Bytes;
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

impl TryFrom<UdpData> for Bytes {
    type Error = CommonError;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value)
            .map(Bytes::from)
            .map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}
