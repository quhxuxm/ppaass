use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageAgentPayloadTypeValue {
    TcpInitialize = 101,
    TcpRelay = 102,
    TcpDestory = 103,
    UdpInitialize = 201,
    UdpRelay = 202,
    UdpDestory = 203,
    DomainNameResolve = 301,
    Heartbeat = 401,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageProxyPayloadTypeValue {
    TcpInitializeSuccess = 111,
    TcpInitializeFail = 121,
    TcpRelaySuccess = 112,
    TcpDestorySuccess = 113,
    UdpInitializeSuccess = 211,
    UdpRelaySuccess = 222,
    UdpDestorySuccess = 213,
    DomainNameResolveSuccess = 311,
    DomainNameResolveFail = 321,
    HeartbeatSuccess = 411,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessagePayloadType {
    AgentPayload(PpaassMessageAgentPayloadTypeValue),
    ProxyPayload(PpaassMessageProxyPayloadTypeValue),
}
