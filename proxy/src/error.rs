#![allow(unused)]
use anyhow::Error as AnyhowError;
use ppaass_common::CommonError;
use std::io::Error as StdIoError;
use thiserror::Error;
#[derive(Debug, Error)]
pub(crate) enum ProxyServerError {
    #[error("Standard I/O error happen: {0:?}")]
    StdIo(#[from] StdIoError),
    #[error("Ppaass common library error happen: {0:?}")]
    Common(#[from] CommonError),
    #[error("Other error happen: {0}")]
    Other(String),
}
