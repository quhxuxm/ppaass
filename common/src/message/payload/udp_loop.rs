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

pub struct UdpLoopDataParts {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub raw_data_bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpLoopData {
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    raw_data_bytes: Vec<u8>,
}

impl UdpLoopData {
    pub fn split(self) -> UdpLoopDataParts {
        UdpLoopDataParts {
            src_address: self.src_address,
            dst_address: self.dst_address,
            raw_data_bytes: self.raw_data_bytes,
        }
    }
}

impl From<UdpLoopDataParts> for UdpLoopData {
    fn from(value: UdpLoopDataParts) -> Self {
        Self {
            src_address: value.src_address,
            dst_address: value.dst_address,
            raw_data_bytes: value.raw_data_bytes,
        }
    }
}
impl TryFrom<Vec<u8>> for UdpLoopData {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).context("Fail generate UdpLoopData from input bytes")
    }
}

impl TryFrom<UdpLoopData> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpLoopData) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).context("Fail generate bytes from UdpLoopData")
    }
}
