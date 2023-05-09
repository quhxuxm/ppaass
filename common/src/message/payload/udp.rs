use std::borrow::Cow;

use crate::{CommonError, DeserializeError, PpaassNetAddress, SerializeError};
use derive_more::Constructor;
use serde_derive::{Deserialize, Serialize};

////////////////////////////////
/// Udp data
///////////////////////////////

#[derive(Serialize, Deserialize, Constructor)]
#[non_exhaustive]
pub struct UdpData {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub data: Vec<u8>,
}

impl<'a> TryFrom<Cow<'a, [u8]>> for UdpData {
    type Error = CommonError;

    fn try_from(value: Cow<'a, [u8]>) -> Result<Self, Self::Error> {
        bincode::deserialize(&value).map_err(|e| CommonError::Decoder(DeserializeError::UdpData(e).into()))
    }
}

impl TryFrom<UdpData> for Vec<u8> {
    type Error = CommonError;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        bincode::serialize(&value).map_err(|e| CommonError::Encoder(SerializeError::UdpData(e).into()))
    }
}
