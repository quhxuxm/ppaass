use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageAgentPayloadTypeValue {
    TcpSessionInitialize,
    TcpSessionRelay,
    TcpSessionDestroy,
    UdpSessionInitialize,
    UdpSessionRelay,
    UdpSessionDestroy,
    DomainNameResolve,
    IdleHeartbeat,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageProxyPayloadTypeValue {
    TcpSessionInitializeSuccess,
    TcpSessionInitializeFail,
    TcpSessionRelay,
    UdpSessionInitializeSuccess,
    UdpSessionInitializeFail,
    UdpSessionRelay,
    DomainNameResolveSuccess,
    DomainNameResolveFail,
    IdleHeartbeatSuccess,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessagePayloadType {
    AgentPayload(PpaassMessageAgentPayloadTypeValue),
    ProxyPayload(PpaassMessageProxyPayloadTypeValue),
}