use anyhow::Error as AnyhowError;
use ppaass_common::CommonError;
use std::io::Error as StdIoError;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum AgentError {
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
pub(crate) enum NetworkError {}

#[derive(Debug, Error)]
pub(crate) enum ConnectionStateError {}
