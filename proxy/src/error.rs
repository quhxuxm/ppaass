use snafu::{Backtrace, Snafu};

#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub(crate) enum InnerError {
    #[snafu(display("I/O error happen: {message}"))]
    Io {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Fail to parse configuration file: {file_name}"))]
    ConfigurtionFileParseFail {
        file_name: String,
        backtrace: Backtrace,
        source: toml::de::Error,
    },
    #[snafu(whatever, display("{message}"))]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
    }
}
