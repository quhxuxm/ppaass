use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageAgentPayloadTypeValue {
    TcpInitialize,
    TcpRelay,
    TcpDestory,
    UdpInitialize,
    UdpRelay,
    UdpDestory,
    DomainNameResolve,
    IdleConnectionKeepAlive,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageProxyPayloadTypeValue {
    TcpInitializeSuccess,
    TcpInitializeFail,
    TcpRelaySuccess,
    TcpDestorySuccess,
    UdpInitializeSuccess,
    UdpRelaySuccess,
    UdpDestorySuccess,
    DomainNameResolveSuccess,
    DomainNameResolveFail,
    HeartbeatSuccess,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessagePayloadType {
    AgentPayload(PpaassMessageAgentPayloadTypeValue),
    ProxyPayload(PpaassMessageProxyPayloadTypeValue),
}
