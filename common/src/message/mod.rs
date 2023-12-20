mod address;
mod encryption;
mod generator;
mod payload;

use crate::{make_as_bytes, CommonError};
pub use address::*;
use anyhow::Result;
use bytes::Bytes;
use derive_more::Constructor;
pub use encryption::*;
pub use generator::*;
pub use payload::*;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageAgentTcpPayloadType {
    Init,
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageAgentUdpPayloadType {
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageAgentProtocol {
    Tcp(PpaassMessageAgentTcpPayloadType),
    Udp(PpaassMessageAgentUdpPayloadType),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassAgentMessagePayload {
    pub protocol: PpaassMessageAgentProtocol,
    pub data: Bytes,
}

make_as_bytes! {
    #[non_exhaustive]
    #[derive(Serialize, Deserialize, Debug, Constructor)]
    pub struct PpaassAgentMessage {
        pub id: String,
        pub user_token: String,
        pub encryption: PpaassMessagePayloadEncryption,
        pub payload: PpaassAgentMessagePayload,
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageProxyTcpPayloadType {
    Init,
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageProxyUdpPayloadType {
    Data,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum PpaassMessageProxyProtocol {
    Tcp(PpaassMessageProxyTcpPayloadType),
    Udp(PpaassMessageProxyUdpPayloadType),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PpaassProxyMessagePayload {
    pub protocol: PpaassMessageProxyProtocol,
    pub data: Bytes,
}

make_as_bytes! {
    #[non_exhaustive]
    #[derive(Serialize, Deserialize, Debug, Constructor)]
    pub struct PpaassProxyMessage {
        pub id: String,
        pub user_token: String,
        pub encryption: PpaassMessagePayloadEncryption,
        pub payload: PpaassProxyMessagePayload,
    }
}
