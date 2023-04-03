use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PpaassMessageAgentPayloadType {
    TcpLoopInit,
    UdpLoopInit,
    DomainNameResolve,
    IdleHeartbeat,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PpaassMessageProxyPayloadType {
    TcpLoopInit,
    UdpLoopInit,
    DomainNameResolve,
    IdleHeartbeat,
}
