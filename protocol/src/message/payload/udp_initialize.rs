use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpInitializeRequestPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for UdpInitializeRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate UdpInitializeRequestPayload from input bytes")
    }
}

impl TryFrom<UdpInitializeRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpInitializeRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from UdpInitializeRequestPayload")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpInitializeResponsePayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for UdpInitializeResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate UdpInitializeResponsePayload from input bytes")
    }
}

impl TryFrom<UdpInitializeResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpInitializeResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from UdpInitializeResponsePayload")
    }
}
