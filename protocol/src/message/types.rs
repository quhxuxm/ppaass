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
    Heartbeat,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageProxyPayloadTypeValue {
    TcpInitializeSuccess,
    TcpInitializeFail,
    TcpRelay,
    UdpInitializeSuccess,
    UdpInitializeFail,
    UdpRelay,
    DomainNameResolveSuccess,
    DomainNameResolveFail,
    HeartbeatSuccess,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessagePayloadType {
    AgentPayload(PpaassMessageAgentPayloadTypeValue),
    ProxyPayload(PpaassMessageProxyPayloadTypeValue),
}
