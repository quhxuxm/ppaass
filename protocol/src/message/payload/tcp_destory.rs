use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpDestoryRequestPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for TcpDestoryRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate TcpDestoryRequestPayload from input bytes")
    }
}

impl TryFrom<TcpDestoryRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpDestoryRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from TcpDestoryRequestPayload")
    }
}
