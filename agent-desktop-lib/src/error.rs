use snafu::{Backtrace, Snafu};

#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub(crate) enum InnerError {
    #[snafu(display("I/O error happen: {message}"))]
    InvalidSocks5Command {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
}
