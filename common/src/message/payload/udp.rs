use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};
use serde_derive::{Deserialize, Serialize};

////////////////////////////////
/// Udp data
///////////////////////////////

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
    #[serde(with = "serde_bytes")]
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
    type Error = CommonError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<UdpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}
