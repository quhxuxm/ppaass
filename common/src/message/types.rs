use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PpaassMessageAgentPayloadType {
    TcpLoopInit,
    // TcpLoopDestroy,
    UdpLoopInit,
    // UdpLoopDestroy,
    DomainNameResolve,
    IdleHeartbeat,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum PpaassMessageProxyPayloadType {
    TcpLoopInit,
    // TcpLoopDestroy,
    UdpLoopInit,
    // UdpLoopDestroy,
    DomainNameResolve,
    IdleHeartbeat,
}
