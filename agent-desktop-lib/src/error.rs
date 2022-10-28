use snafu::{Backtrace, GenerateImplicitData, Snafu};
use std::io::Error as StdIoError;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub enum Error {
    #[snafu(display("Socks5 codec error: {message}"))]
    Socks5Codec { message: String, backtrace: Backtrace },
    #[snafu(display("Http codec error: {message}"))]
    HttpCodec {
        message: String,
        backtrace: Backtrace,
        source: bytecodec::Error,
    },
    #[snafu(display("I/O error: {message}"))]
    Io {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
}

impl From<StdIoError> for Error {
    fn from(io_error: StdIoError) -> Self {
        Error::Io {
            message: format!("{io_error}"),
            backtrace: Backtrace::generate_with_source(&io_error),
            source: io_error,
        }
    }
}
