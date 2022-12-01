use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageAgentPayloadType {
    TcpLoopInit,
    UdpLoopInit,

    DomainNameResolve,
    IdleHeartbeat,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageProxyPayloadType {
    TcpLoopInitSuccess,
    TcpLoopInitFail,
    UdpLoopInitSuccess,
    UdpLoopInitFail,
    DomainNameResolveSuccess,
    DomainNameResolveFail,
    IdleHeartbeat,
}
