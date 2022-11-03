use std::sync::Arc;

use async_trait::async_trait;
use ppaass_common::RsaCryptoFetcher;
use ppaass_protocol::PpaassProtocolAddress;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error};

pub(crate) mod dispatcher;

mod http;
mod socks;

#[async_trait]
pub(crate) trait ClientFlow {
    async fn exec(&mut self) -> Result<(), Error>;
}
