use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PpaassMessageAgentPayloadType {
    TcpInit,
    UdpData,
    DnsLookupRequest,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PpaassMessageProxyPayloadType {
    TcpInit,
    UdpData,
    DnsLookupResponse,
}
