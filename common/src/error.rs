/// The general error happen in ppaass project.
#[derive(thiserror::Error, Debug)]
pub enum PpaassError {
    #[error("Codec error happen.")]
    CodecError,
    #[error("Error happen, original io error: {:?}", source)]
    IoError {
        #[from]
        source: std::io::Error,
    },
}

impl From<PpaassError> for std::io::Error {
    fn from(_: PpaassError) -> Self {
        todo!()
    }
}
