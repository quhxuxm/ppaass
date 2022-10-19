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
    ConnectionKeepAlive,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessageProxyPayloadTypeValue {
    TcpInitializeSuccess,
    TcpInitializeFail,
    TcpRelaySuccess,
    TcpRelayFail,
    TcpDestorySuccess,
    TcpDestoryFail,
    UdpInitializeSuccess,
    UdpRelaySuccess,
    UdpDestorySuccess,
    DomainNameResolveSuccess,
    DomainNameResolveFail,
    ConnectionKeepAliveSuccess,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PpaassMessagePayloadType {
    AgentPayload(PpaassMessageAgentPayloadTypeValue),
    ProxyPayload(PpaassMessageProxyPayloadTypeValue),
}
