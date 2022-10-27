use snafu::{Backtrace, Snafu};

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub enum Error {
    #[snafu(display("Invalid socks5 init command: {message}"))]
    InvalidSocks5InitCommand { message: String, backtrace: Backtrace },
    #[snafu(display("Fail to parse socks5 address: {message}"))]
    Socks5AddressParse { message: String, backtrace: Backtrace },
}
