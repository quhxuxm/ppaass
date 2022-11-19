use anyhow::Context;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatRequestPayload {
    pub timestamp: i64,
}

impl TryFrom<Vec<u8>> for HeartbeatRequestPayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate HeartbeatRequestPayload from input bytes")
    }
}

impl TryFrom<HeartbeatRequestPayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: HeartbeatRequestPayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from HeartbeatRequestPayload")
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatResponsePayload {
    pub timestamp: i64,
}

impl TryFrom<Vec<u8>> for HeartbeatResponsePayload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("fail generate HeartbeatResponsePayload from input bytes")
    }
}

impl TryFrom<HeartbeatResponsePayload> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: HeartbeatResponsePayload) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("fail generate bytes from HeartbeatResponsePayload")
    }
}
