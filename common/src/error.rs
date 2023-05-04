use std::backtrace::{self, Backtrace};

use anyhow::Error as AnyhowError;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum CommonError {
    #[error("Crypto fail because of error: {0:?}")]
    CryptoFail(#[from] CryptoError),
    #[error("Encoding fail because of error: {0:?}")]
    EncodeFail(#[from] EncodeError),
    #[error("Decoding fail because of error: {0:?}")]
    DecodeFail(#[from] DecodeError),
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Crypto fail because of aes error.")]
    Aes(#[from] AesError),
    #[error("Crypto fail because of blowfish error.")]
    Blowfish(#[from] BlowfishError),
    #[error("Crypto fail because of rsa error.")]
    Rsa(#[from] RsaError),
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct AesError {
    source: AnyhowError,
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct BlowfishError {
    source: AnyhowError,
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct RsaError {
    source: AnyhowError,
}

#[derive(Debug, Error)]
pub enum EncodeError {}

#[derive(Debug, Error)]
pub enum DecodeError {}
