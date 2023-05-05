use anyhow::Error as AnyhowError;
use serde_json::Error as SerdeJsonError;
use std::io::Error as StdIoError;
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
    #[from]
    source: AnyhowError,
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct BlowfishError {
    #[from]
    source: AnyhowError,
}

#[derive(Debug, Error)]
pub enum RsaError {
    #[error("Rsa crypto not found by user token: {0}")]
    NotFound(String),
    #[error(transparent)]
    Other(#[from] AnyhowError),
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
    DnsLookupRequest(#[source] SerdeJsonError),
    #[error("Dns lookup response serialize fail because of error: {0:?}")]
    DnsLookupResponse(#[source] SerdeJsonError),
    #[error("Tcp init request serialize fail because of error: {0:?}")]
    TcpInitRequest(#[source] SerdeJsonError),
    #[error("Tcp init response serialize fail because of error: {0:?}")]
    TcpInitResponse(#[source] SerdeJsonError),
    #[error("Tcp data serialize fail because of error: {0:?}")]
    TcpData(#[source] SerdeJsonError),
    #[error("Udp data serialize fail because of error: {0:?}")]
    UdpData(#[source] SerdeJsonError),
    #[error("Ppaass message serialize fail because of error: {0:?}")]
    PpaassMessage(#[source] SerdeJsonError),
    #[error("Ppaass message agent payload serialize fail because of error: {0:?}")]
    PpaassMessageAgentPayload(#[source] SerdeJsonError),
    #[error("Ppaass message proxy payload serialize fail because of error: {0:?}")]
    PpaassMessageProxyPayload(#[source] SerdeJsonError),
}

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("Decoding fail because of io error: {0:?}")]
    Io(#[from] StdIoError),
    #[error("Decoding fail because of invalid message flag: {0:?}")]
    InvalidMessageFlag(#[source] AnyhowError),
    #[error("Decoding fail because of deserialize fail: {0:?}")]
    Deserialize(#[from] DeserializeError),
    #[error("Decoding fail because of crypto error: {0:?}")]
    Crypto(#[from] CryptoError),
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("Dns lookup request deserialize fail because of error: {0:?}")]
    DnsLookupRequest(#[source] SerdeJsonError),
    #[error("Dns lookup response deserialize fail because of error: {0:?}")]
    DnsLookupResponse(#[source] SerdeJsonError),
    #[error("Tcp init request deserialize fail because of error: {0:?}")]
    TcpInitRequest(#[source] SerdeJsonError),
    #[error("Tcp init response deserialize fail because of error: {0:?}")]
    TcpInitResponse(#[source] SerdeJsonError),
    #[error("Tcp data deserialize fail because of error: {0:?}")]
    TcpData(#[source] SerdeJsonError),
    #[error("Udp data deserialize fail because of error: {0:?}")]
    UdpData(#[source] SerdeJsonError),
    #[error("Ppaass message deserialize fail because of error: {0:?}")]
    PpaassMessage(#[source] SerdeJsonError),
    #[error("Ppaass message agent payload deserialize fail because of error: {0:?}")]
    PpaassMessageAgentPayload(#[source] SerdeJsonError),
    #[error("Ppaass message proxy payload deserialize fail because of error: {0:?}")]
    PpaassMessageProxyPayload(#[source] SerdeJsonError),
}
