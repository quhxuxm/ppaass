use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpRelayPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for TcpRelayPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate TcpRelayPayload from input bytes")
    }
}

impl TryFrom<TcpRelayPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpRelayPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from TcpRelayPayload")
    }
}
