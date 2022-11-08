use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpInitializeRequestPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for TcpInitializeRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate TcpInitializeRequestPayload from input bytes")
    }
}

impl TryFrom<TcpInitializeRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpInitializeRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from TcpInitializeRequestPayload")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpInitializeResponsePayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for TcpInitializeResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate TcpInitializeResponsePayload from input bytes")
    }
}

impl TryFrom<TcpInitializeResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpInitializeResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from TcpInitializeResponsePayload")
    }
}
