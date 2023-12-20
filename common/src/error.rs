use std::io::Error as StdIoError;

use bincode::Error as BincodeError;
use cipher::block_padding::UnpadError;
use cipher::inout::PadError;
use rsa::errors::Error as ConcreteRsaError;
use rsa::pkcs8::spki::Error as RsaPkcs8SpkiError;
use rsa::pkcs8::Error as RsaPkcs8Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("Decoder fail because of error: {0:?}")]
    Decoder(#[from] DecoderError),
    #[error("Encoder fail because of error: {0:?}")]
    Encoder(#[from] EncoderError),
    #[error("IO error happen: {0:?}")]
    Io(#[from] StdIoError),
    #[error("Crypto error happen: {0:?}")]
    Crypto(#[from] CryptoError),
    #[error("Other error happen: {0:?}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Crypto fail because of aes error: {0:?}")]
    Aes(#[from] AesError),
    #[error("Crypto fail because of rsa error: {0:?}")]
    Rsa(#[from] RsaError),
}

#[derive(Debug, Error)]
pub enum AesError {
    #[error("AES unpad error happen: {0:?}")]
    UnPad(#[from] UnpadError),
    #[error("AES pad error happen: {0:?}")]
    Pad(#[from] PadError),
}

#[derive(Debug, Error)]
pub enum RsaError {
    #[error("Rsa crypto not found by user token: {0}")]
    NotFound(String),
    #[error("Rsa error happen: {0:?}")]
    ConcreteRsa(#[from] ConcreteRsaError),
    #[error("I/O error happen: {0:?}")]
    Io(#[from] StdIoError),
    #[error("Rsa pkcs8 spki error happen: {0:?}")]
    Pkcs8SpkiError(#[from] RsaPkcs8SpkiError),
    #[error("Rsa pkcs8 error happen: {0:?}")]
    Pkcs8Error(#[from] RsaPkcs8Error),
}

#[derive(Debug, Error)]
pub enum EncoderError {
    #[error("Decoding fail because of io error: {0:?}")]
    Io(#[from] StdIoError),
    #[error("Encoding fail because of serialize fail: {0:?}")]
    Serialize(#[from] SerializeError),
    #[error("Encoding fail because of crypto error: {0:?}")]
    Crypto(#[from] CryptoError),
}

#[derive(Debug, Error)]
pub enum SerializeError {
    #[error("Dns lookup request serialize fail because of error: {0:?}")]
    DnsLookupRequest(#[source] BincodeError),
    #[error("Dns lookup response serialize fail because of error: {0:?}")]
    DnsLookupResponse(#[source] BincodeError),
    #[error("Tcp init request serialize fail because of error: {0:?}")]
    TcpInitRequest(#[source] BincodeError),
    #[error("Tcp init response serialize fail because of error: {0:?}")]
    TcpInitResponse(#[source] BincodeError),
    #[error("Tcp data serialize fail because of error: {0:?}")]
    TcpData(#[source] BincodeError),
    #[error("Udp data serialize fail because of error: {0:?}")]
    UdpData(#[source] BincodeError),
    #[error("Ppaass message serialize fail because of error: {0:?}")]
    PpaassMessage(#[source] BincodeError),
    #[error("Ppaass message agent payload serialize fail because of error: {0:?}")]
    PpaassMessageAgentPayload(#[source] BincodeError),
    #[error("Ppaass message proxy payload serialize fail because of error: {0:?}")]
    PpaassMessageProxyPayload(#[source] BincodeError),
}

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Decoding fail because of io error: {0:?}")]
    Io(#[from] StdIoError),
    #[error("Decoding fail because of invalid message flag: {0:?}")]
    InvalidMessageFlag(String),
    #[error("Decoding fail because of deserialize fail: {0:?}")]
    Deserialize(#[from] DeserializeError),
    #[error("Decoding fail because of crypto error: {0:?}")]
    Crypto(#[from] CryptoError),
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("Dns lookup request deserialize fail because of error: {0:?}")]
    DnsLookupRequest(#[source] BincodeError),
    #[error("Dns lookup response deserialize fail because of error: {0:?}")]
    DnsLookupResponse(#[source] BincodeError),
    #[error("Tcp init request deserialize fail because of error: {0:?}")]
    TcpInitRequest(#[source] BincodeError),
    #[error("Tcp init response deserialize fail because of error: {0:?}")]
    TcpInitResponse(#[source] BincodeError),
    #[error("Tcp data deserialize fail because of error: {0:?}")]
    TcpData(#[source] BincodeError),
    #[error("Udp data deserialize fail because of error: {0:?}")]
    UdpData(#[source] BincodeError),
    #[error("Ppaass message deserialize fail because of error: {0:?}")]
    PpaassMessage(#[source] BincodeError),
    #[error("Ppaass message agent payload deserialize fail because of error: {0:?}")]
    PpaassMessageAgentPayload(#[source] BincodeError),
    #[error("Ppaass message proxy payload deserialize fail because of error: {0:?}")]
    PpaassMessageProxyPayload(#[source] BincodeError),
}
