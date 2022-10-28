use snafu::{Backtrace, GenerateImplicitData, Snafu};
use std::fmt::Debug;
use std::io::Error as StdIoError;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub enum Error {
    #[snafu(display("I/O error: {message}"))]
    Io {
        message: String,
        backtrace: Backtrace,
        source: std::io::Error,
    },
    #[snafu(display("Rsa crypto not exist for user: {user_token}"))]
    RsaCryptoNotExist { user_token: String, backtrace: Backtrace },
    #[snafu(display("Fail to fetch rsa crypto for user: {user_token}"))]
    FetchRsaCryptoFail {
        user_token: String,
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Fail to encrypt aes token"))]
    EncryptAesTokenFail {
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Fail to encrypt bloofish token"))]
    EncryptBloofishTokenFail {
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Fail to decrypt aes token"))]
    DecryptAesTokenFail {
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Fail to decrypt aes data"))]
    DecryptAesDataFail {
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Fail to decrypt bloofish token"))]
    DecryptBloofishTokenFail {
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Fail to decrypt bloofish data"))]
    DecryptBloofishDataFail {
        backtrace: Backtrace,
        source: ppaass_common::error::Error,
    },
    #[snafu(display("Codec error happen: {message}"))]
    Codec {
        message: String,
        backtrace: Backtrace,
        source: ppaass_protocol::error::Error,
    },
    #[snafu(whatever, display("{message}"))]
    Other {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
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
