use cipher::block_padding::UnpadError;
use snafu::{Backtrace, Snafu};
use std::fmt::Debug;

#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub(crate) enum InnerError {
    #[snafu(display("I/O error happen: {message}"))]
    Io {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("RSA public key error happen: {message}"))]
    RsaPublicKey {
        message: String,
        backtrace: Backtrace,
        source: rsa::pkcs8::spki::Error,
    },
    #[snafu(display("RSA private key error happen: {message}"))]
    RsaPrivateKey {
        message: String,
        backtrace: Backtrace,
        source: rsa::pkcs8::Error,
    },
    #[snafu(display("RSA crypto error happen: {message}"))]
    RsaCrypto {
        message: String,
        backtrace: Backtrace,
        source: rsa::errors::Error,
    },
    #[snafu(display("Crypto un-pad error happen: {message}"))]
    CryptoUnpad {
        message: String,
        backtrace: Backtrace,
        source: UnpadError,
    },
}
