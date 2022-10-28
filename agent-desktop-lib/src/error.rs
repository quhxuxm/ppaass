use snafu::{Backtrace, GenerateImplicitData, Snafu};
use std::io::Error as StdIoError;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub enum Error {
    #[snafu(display("Invalid socks5 init command: {message}"))]
    InvalidSocks5InitCommand { message: String, backtrace: Backtrace },
    #[snafu(display("Fail to parse socks5 address: {message}"))]
    Socks5AddressParse { message: String, backtrace: Backtrace },
    #[snafu(display("Fail to parse socks5 address to socket address: {message}"))]
    Socks5AddressParseToSocketAddr {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Fail to parse socks5 udp data packet."))]
    InvalidSocks5UdpDataPacket { backtrace: Backtrace },
    #[snafu(display("Socks 5 codec error: {message}"))]
    Socks5Codec { message: String, backtrace: Backtrace },
    #[snafu(display("Http codec error: {message}"))]
    HttpCodec {
        message: String,
        backtrace: Backtrace,
        source: bytecodec::Error,
    },
    #[snafu(display("Codec I/O error: {message}"))]
    CodecIo {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
}

impl From<StdIoError> for Error {
    fn from(io_error: StdIoError) -> Self {
        Error::CodecIo {
            message: format!("Io error happen: {io_error:?}"),
            backtrace: Backtrace::generate_with_source(&io_error),
            source: io_error,
        }
    }
}
