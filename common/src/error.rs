/// The general error happen in ppaass project.

type StdIoError = std::io::Error;
type StdIoErrorKind = std::io::ErrorKind;

#[derive(thiserror::Error, Debug)]
pub enum PpaassError {
    #[error("Codec error happen.")]
    CodecError,
    #[error("Error happen, original io error: {:?}", source)]
    IoError {
        #[from]
        source: StdIoError,
    },
    #[error("RSA public key error happen, original io error: {:?}", source)]
    RsaPublicKeyError {
        #[from]
        source: rsa::pkcs8::spki::Error,
    },
    #[error("RSA private key error happen, original io error: {:?}", source)]
    RsaPrivateKeyError {
        #[from]
        source: rsa::pkcs8::Error,
    },
}

impl From<PpaassError> for StdIoError {
    fn from(value: PpaassError) -> Self {
        match value {
            PpaassError::CodecError => StdIoError::new(StdIoErrorKind::InvalidData, value),
            PpaassError::IoError { source } => source,
            PpaassError::RsaPublicKeyError { source } => StdIoError::new(StdIoErrorKind::InvalidData, source),
            PpaassError::RsaPrivateKeyError { source } => StdIoError::new(StdIoErrorKind::InvalidData, source),
        }
    }
}
