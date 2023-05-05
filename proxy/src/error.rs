use anyhow::Error as AnyhowError;
use ppaass_common::CommonError;
use std::io::Error as StdIoError;
use thiserror::Error;
#[derive(Debug, Error)]
pub(crate) enum ProxyError {
    #[error("Network error happen: {0:?}")]
    Network(#[from] NetworkError),
    #[error("Connection state error happen: {0:?}")]
    ConnectionState(#[from] ConnectionStateError),
    #[error(transparent)]
    Common(#[from] CommonError),
    #[error(transparent)]
    Io(#[from] StdIoError),
    #[error(transparent)]
    Other(#[from] AnyhowError),
}

#[derive(Debug, Error)]
pub(crate) enum NetworkError {
    #[error("Connect destination fail because of error: {0:?}")]
    DestinationConnectFail(#[source] StdIoError),
    #[error("Read data from destination fail because of error: {0:?}")]
    DestinationReadFail(#[source] StdIoError),
    #[error("Write data to destination fail because of error: {0:?}")]
    DestinationWriteFail(#[source] StdIoError),
    #[error("Accept agent fail because of error: {0:?}")]
    AgentAcceptFail(#[source] StdIoError),
    #[error("Read data from agent fail because of error: {0:?}")]
    AgentReadFail(#[source] StdIoError),
    #[error("Write data to agent fail because of error: {0:?}")]
    AgentWriteFail(#[source] StdIoError),
    #[error("Bind port fail because of error: {0:?}")]
    PortBindingFail(#[source] StdIoError),
}

#[derive(Debug, Error)]
pub(crate) enum ConnectionStateError {
    #[error("Connection should receive tcp init")]
    NotTcpInit,
    #[error("Connection should receive tcp data")]
    NotTcpData,
    #[error("Connection should receive udp data")]
    NotUdpData,
    #[error("Connection should receive dns lookup request")]
    NotDnsLookupRequest,
}
