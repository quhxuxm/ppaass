use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpLoopInitRequestPayload {
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TcpLoopInitResponseType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpLoopInitResponsePayload {
    pub session_key: String,
    pub src_address: PpaassNetAddress,
    pub dest_address: PpaassNetAddress,
    pub response_type: TcpLoopInitResponseType,
}

impl TryFrom<Vec<u8>> for TcpLoopInitRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("Fail generate TcpLoopInitRequestPayload from input bytes")
    }
}

impl TryFrom<TcpLoopInitRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpLoopInitRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("Fail generate bytes from TcpLoopInitRequestPayload")
    }
}

impl TryFrom<Vec<u8>> for TcpLoopInitResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("Fail generate TcpLoopInitResponsePayload from input bytes")
    }
}

impl TryFrom<TcpLoopInitResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: TcpLoopInitResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("Fail generate bytes from TcpLoopInitResponsePayload")
    }
}
