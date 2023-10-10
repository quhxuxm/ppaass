#![allow(unused)]
use anyhow::Error as AnyhowError;
use ppaass_common::CommonError;
use std::io::Error as StdIoError;
use thiserror::Error;
#[derive(Debug, Error)]
pub(crate) enum ProxyError {
    #[error("Connect destination fail because of error: {0:?}")]
    DestinationConnect(#[source] StdIoError),
    #[error("Configure destination connection fail because of error: {0:?}")]
    DestinationConnectionConfig(#[source] StdIoError),
    #[error("Read data from destination fail because of error: {0:?}")]
    DestinationRead(#[source] StdIoError),
    #[error("Write data to destination fail because of error: {0:?}")]
    DestinationWrite(#[source] StdIoError),
    #[error("Accept agent fail because of error: {0:?}")]
    AgentAccept(#[source] StdIoError),
    #[error("Bind port fail because of error: {0:?}")]
    PortBinding(#[source] StdIoError),
    #[error("General error happen: {0:?}")]
    GeneralIo(#[from] StdIoError),
    #[error("Read data from agent fail because of error: {0:?}")]
    AgentRead(#[source] CommonError),
    #[error("Write data to agent fail because of error: {0:?}")]
    AgentWrite(#[source] CommonError),
    #[error("Close agent connection fail because of error: {0:?}")]
    AgentClose(#[source] CommonError),
    #[error("Common error happen because of error: {0:?}")]
    Common(#[source] CommonError),
    #[error("Timeout in {0} seconds")]
    Timeout(u64),
    #[error("Other error happen: {0}")]
    Other(String),
}
