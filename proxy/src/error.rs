#![allow(unused)]
use anyhow::Error as AnyhowError;
use ppaass_common::CommonError;
use std::io::Error as StdIoError;
use thiserror::Error;
#[derive(Debug, Error)]
pub(crate) enum ProxyServerError {
    #[error("General IO error happen: {0:?}")]
    GeneralIo(#[from] StdIoError),
    #[error("Ppaass common library error happen: {0:?}")]
    Common(#[from] CommonError),
    #[error("Timeout in {0} seconds")]
    Timeout(u64),
    #[error("Other error happen: {0}")]
    Other(String),
}
