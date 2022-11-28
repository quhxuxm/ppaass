use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpRelayPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for UdpRelayPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate UdpRelayPayload from input bytes")
    }
}

impl TryFrom<UdpRelayPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpRelayPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from UdpRelayPayload")
    }
}