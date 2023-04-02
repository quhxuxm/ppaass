use std::sync::Arc;

use futures::SinkExt;
use ppaass_common::{generate_uuid, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress, RsaCryptoFetcher};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::error;

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};

use super::{AgentConnectionRead, AgentConnectionWrite};
use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct UdpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    agent_connection_read: AgentConnectionRead<T, R>,
    agent_connection_write: AgentConnectionWrite<T, R>,
    key: String,
    user_token: String,
    agent_connection_id: String,
    configuration: Arc<ProxyServerConfig>,
}

impl UdpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    fn generate_key(agent_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]")
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }

    pub(crate) async fn exec(self) -> Result<()> {}
}

pub(crate) struct UdpLoopBuilder<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    agent_connection_id: Option<String>,
    agent_connection_read: Option<AgentConnectionRead<T, R>>,
    agent_connection_write: Option<AgentConnectionWrite<T, R>>,
    user_token: Option<String>,
    agent_address: Option<PpaassNetAddress>,
}

impl<T, R> UdpLoopBuilder<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            agent_connection_id: None,
            agent_connection_read: None,
            agent_connection_write: None,
            user_token: None,
            agent_address: None,
        }
    }
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> UdpLoopBuilder<T, R> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> UdpLoopBuilder<T, R> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> UdpLoopBuilder<T, R> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn agent_connection_read(mut self, agent_connection_read: AgentConnectionRead<T, R>) -> UdpLoopBuilder<T, R> {
        self.agent_connection_read = Some(agent_connection_read);
        self
    }

    pub(crate) fn agent_connection_write(mut self, agent_connection_write: AgentConnectionWrite<T, R>) -> UdpLoopBuilder<T, R> {
        self.agent_connection_write = Some(agent_connection_write);
        self
    }

    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<UdpLoop<T, R>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for tcp loop builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for tcp loop builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp loop builder")?;
        let key = UdpLoop::<T, R>::generate_key(&agent_address);
        let mut agent_connection_write = self
            .agent_connection_write
            .context("Agent message framed write not assigned for tcp loop builder")?;
        let agent_connection_read = self
            .agent_connection_read
            .context("Agent message framed read not assigned for tcp loop builder")?;

        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let udp_initialize_success_message = PpaassMessageGenerator::generate_udp_loop_init_success_response(&key, &user_token, payload_encryption_token)?;
        if let Err(e) = agent_connection_write.send(udp_initialize_success_message).await {
            error!("Agent connection [{agent_connection_id}] fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow::anyhow!(e));
        };
        Ok(UdpLoop {
            key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            agent_connection_id,
            configuration,
        })
    }
}
