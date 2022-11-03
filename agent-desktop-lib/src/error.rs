use snafu::{Backtrace, GenerateImplicitData, Snafu};
use std::io::Error as StdIoError;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub enum Error {
    #[snafu(display("No base tcp stream error."))]
    CodecNoBaseTcpStream { backtrace: Backtrace },
    #[snafu(display("Socks5 codec error: {message}"))]
    Socks5Codec { message: String, backtrace: Backtrace },
    #[snafu(display("Http codec general error: {message}"))]
    HttpCodecGeneralFail {
        message: String,
        backtrace: Backtrace,
        source: bytecodec::Error,
    },
    #[snafu(display("Http codec fail to parse url: {url}"))]
    HttpCodecParseUrlFail {
        url: String,
        backtrace: Backtrace,
        source: url::ParseError,
    },
    #[snafu(display("Http codec fail to parse target host: {url}"))]
    HttpCodecParseTargetHostFail { url: String, backtrace: Backtrace },
    #[snafu(display("Http codec wrong method: {method}"))]
    HttpCodecWrongMethod { method: String, backtrace: Backtrace },
    #[snafu(display("I/O error: {message}"))]
    Io {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Invalid status: {message}"))]
    InvalidStatus { message: String, backtrace: Backtrace },
    #[snafu(display("Confiugration item missed: {message}"))]
    ConfigurationItemMissed { message: String, backtrace: Backtrace },
    #[snafu(display("Fail to accept agent tcp connection: {message}."))]
    AcceptClientTcpConnection { message: String, backtrace: Backtrace },
    #[snafu(display("Unsupported protocol: {message}."))]
    UsupportedProtocol { message: String, backtrace: Backtrace },
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
