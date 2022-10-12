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
}

impl From<PpaassError> for StdIoError {
    fn from(value: PpaassError) -> Self {
        match value {
            PpaassError::CodecError => StdIoError::new(StdIoErrorKind::InvalidData, value),
            PpaassError::IoError { source } => source,
        }
    }
}
