use std::collections::HashMap;

use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};

use crate::PpaassNetAddress;

////////////////////////////////
/// Udp data
///////////////////////////////

pub struct UdpDataParts {
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
    pub raw_data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct UdpData {
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    raw_data: Vec<u8>,
}

impl UdpData {
    pub fn split(self) -> UdpDataParts {
        UdpDataParts {
            src_address: self.src_address,
            dst_address: self.dst_address,
            raw_data: self.raw_data,
        }
    }
}

impl From<UdpDataParts> for UdpData {
    fn from(value: UdpDataParts) -> Self {
        Self {
            src_address: value.src_address,
            dst_address: value.dst_address,
            raw_data: value.raw_data,
        }
    }
}
impl TryFrom<Vec<u8>> for UdpData {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate UdpData from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<UdpData> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: UdpData) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| anyhow!("Fail generate bytes from UdpData because of error: {e:?}"))
    }
}

////////////////////////////////
/// Dns lookup request
///////////////////////////////

pub struct DnsLookupRequestParts {
    pub request_id: u16,
    pub domain_names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DnsLookupRequest {
    request_id: u16,
    domain_names: Vec<String>,
}

impl DnsLookupRequest {
    pub fn split(self) -> DnsLookupRequestParts {
        DnsLookupRequestParts {
            domain_names: self.domain_names,
            request_id: self.request_id,
        }
    }
}

impl From<DnsLookupRequestParts> for DnsLookupRequest {
    fn from(value: DnsLookupRequestParts) -> Self {
        Self {
            domain_names: value.domain_names,
            request_id: value.request_id,
        }
    }
}
impl TryFrom<Vec<u8>> for DnsLookupRequest {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate UdpData from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<DnsLookupRequest> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: DnsLookupRequest) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| anyhow!("Fail generate bytes from UdpData because of error: {e:?}"))
    }
}

////////////////////////////////
/// Dns lookup response
///////////////////////////////

pub struct DnsLookupResponseParts {
    pub addresses: HashMap<String, Option<Vec<[u8; 4]>>>,
    pub request_id: u16,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DnsLookupResponse {
    addresses: HashMap<String, Option<Vec<[u8; 4]>>>,
    request_id: u16,
}

impl DnsLookupResponse {
    pub fn split(self) -> DnsLookupResponseParts {
        DnsLookupResponseParts {
            request_id: self.request_id,
            addresses: self.addresses,
        }
    }
}

impl From<DnsLookupResponseParts> for DnsLookupResponse {
    fn from(value: DnsLookupResponseParts) -> Self {
        Self {
            request_id: value.request_id,
            addresses: value.addresses,
        }
    }
}
impl TryFrom<Vec<u8>> for DnsLookupResponse {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        serde_json::from_slice(&value).map_err(|e| anyhow!("Fail generate UdpData from input bytes because of error: {e:?}"))
    }
}

impl TryFrom<DnsLookupResponse> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: DnsLookupResponse) -> Result<Self, Self::Error> {
        serde_json::to_vec(&value).map_err(|e| anyhow!("Fail generate bytes from UdpData because of error: {e:?}"))
    }
}
