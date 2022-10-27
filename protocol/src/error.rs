use snafu::{Backtrace, Snafu};
use std::fmt::Debug;
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub enum Error {
    #[snafu(display("I/O error happen: {message}"))]
    Io {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Json deserialize error happen: {message}"))]
    JsonDeserialize {
        message: String,
        backtrace: Backtrace,
        source: serde_json::error::Error,
    },
    #[snafu(display("Json serialize error happen: {message}"))]
    JsonSerialize {
        message: String,
        backtrace: Backtrace,
        source: serde_json::error::Error,
    },
    #[snafu(display("Other error happen: {message}"))]
    Other { message: String },
}
