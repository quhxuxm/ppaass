use std::io::Error as StdIoError;
use thiserror::Error;
#[derive(Debug, Error)]
pub(crate) enum ProxyError {
    #[error("Network error happen: {0:?}")]
    Network(#[from] NetworkError),
    #[error("State error happen: {0:?}")]
    State(#[from] StateError),
    #[error(transparent)]
    Common(#[from] CommonError),
}

#[derive(Debug, Error)]
pub(crate) enum NetworkError {
    DestinationConnectFail {
        #[backtrace]
        source: StdIoError,
    },
    DestinationRelayFail {
        #[backtrace]
        source: StdIoError,
    },
    AgentRelayFail {
        #[backtrace]
        source: StdIoError,
    },
}

#[derive(Debug, Error)]
pub(crate) enum StateError {
    NotTcpInit,
    NotTcpData,
    NotUdpData,
    NotDnsRequest,
}
