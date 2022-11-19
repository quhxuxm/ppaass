use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpSessionInitRequestPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

impl TryFrom<Vec<u8>> for TcpSessionInitRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate TcpInitializeRequestPayload from input bytes")
    }
}

impl TryFrom<TcpSessionInitRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpSessionInitRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from TcpInitializeRequestPayload")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpSessionInitResponsePayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
    pub session_key: Option<String>,
}

impl TryFrom<Vec<u8>> for TcpSessionInitResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate TcpInitializeResponsePayload from input bytes")
    }
}

impl TryFrom<TcpSessionInitResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpSessionInitResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from TcpInitializeResponsePayload")
    }
}
