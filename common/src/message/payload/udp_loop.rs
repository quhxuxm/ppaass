use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpLoopInitRequestPayload {}

#[derive(Serialize, Deserialize)]
pub enum UdpLoopInitResponseType {
    Success,
    Fail,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpLoopInitResponsePayload {
    pub loop_key: String,
    pub response_type: UdpLoopInitResponseType,
}

impl TryFrom<Vec<u8>> for UdpLoopInitRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("Fail generate UdpLoopInitRequestPayload from input bytes")
    }
}

impl TryFrom<UdpLoopInitRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpLoopInitRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("Fail generate bytes from UdpLoopInitRequestPayload")
    }
}

impl TryFrom<Vec<u8>> for UdpLoopInitResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("Fail generate UdpLoopInitResponsePayload from input bytes")
    }
}

impl TryFrom<UdpLoopInitResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpLoopInitResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("Fail generate bytes from UdpLoopInitResponsePayload")
    }
}
