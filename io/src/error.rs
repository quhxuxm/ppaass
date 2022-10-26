use snafu::{Backtrace, GenerateImplicitData, Snafu};
use std::fmt::Debug;
#[derive(Debug, Snafu)]
pub struct Error(InnerError);

impl From<std::io::Error> for Error {
    fn from(io_error: std::io::Error) -> Self {
        Error(InnerError::Io {
            message: format!("Io error happen: {io_error:?}"),
            backtrace: Backtrace::generate_with_source(&io_error),
            source: io_error,
        })
    }
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)), context(suffix(Error)))]
pub(crate) enum InnerError {
    #[snafu(display("Io error happen: {message}"))]
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
}
