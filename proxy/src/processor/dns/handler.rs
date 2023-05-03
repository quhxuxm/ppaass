use crate::common::ProxyServerPayloadEncryptionSelector;
use anyhow::{anyhow, Context, Result};
use futures::SinkExt;
use ppaass_common::{
    generate_uuid,
    udp::{DnsLookupRequest, DnsLookupRequestParts},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress, RsaCryptoFetcher,
};
use std::{collections::HashMap, fmt::Debug};
use std::{fmt::Display, net::IpAddr};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{error, info};

pub(crate) struct DnsLookupHandlerBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    ppaass_connection_id: Option<String>,
    ppaass_connection_read: Option<PpaassConnectionRead<T, R, I>>,
    ppaass_connection_write: Option<PpaassConnectionWrite<T, R, I>>,
    user_token: Option<String>,
    agent_address: Option<PpaassNetAddress>,
}

impl<T, R, I> DnsLookupHandlerBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            ppaass_connection_id: None,
            ppaass_connection_read: None,
            ppaass_connection_write: None,
            user_token: None,
            agent_address: None,
        }
    }
    pub(crate) fn ppaass_connection_id(mut self, ppaass_connection_id: impl AsRef<str>) -> DnsLookupHandlerBuilder<T, R, I> {
        self.ppaass_connection_id = Some(ppaass_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> DnsLookupHandlerBuilder<T, R, I> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> DnsLookupHandlerBuilder<T, R, I> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn ppaass_connection_read(mut self, ppaass_connection_read: PpaassConnectionRead<T, R, I>) -> DnsLookupHandlerBuilder<T, R, I> {
        self.ppaass_connection_read = Some(ppaass_connection_read);
        self
    }

    pub(crate) fn ppaass_connection_write(mut self, ppaass_connection_write: PpaassConnectionWrite<T, R, I>) -> DnsLookupHandlerBuilder<T, R, I> {
        self.ppaass_connection_write = Some(ppaass_connection_write);
        self
    }

    fn generate_handler_key(user_token: &str, agent_address: &PpaassNetAddress, ppaass_connection_id: &str) -> String {
        format!("[{ppaass_connection_id}]#[{user_token}]@DNSLOOKUP::[{agent_address}]")
    }
    pub(crate) async fn build(self) -> Result<DnsLookupHandler<T, R, I>> {
        let ppaass_connection_id = self
            .ppaass_connection_id
            .context("Agent connection id not assigned for dns lookup handler builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for dns lookup handler builder")?;
        let user_token = self.user_token.context("User token not assigned for dns lookup handler builder")?;
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &ppaass_connection_id);
        let ppaass_connection_write = self
            .ppaass_connection_write
            .context("Agent message framed write not assigned for dns lookup handler builder")?;

        Ok(DnsLookupHandler {
            handler_key,
            ppaass_connection_write,
            user_token,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct DnsLookupHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    ppaass_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: String,
    user_token: String,
}

impl<T, R, I> DnsLookupHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) async fn exec(self, dns_lookup_request: DnsLookupRequest) -> Result<()> {
        let mut ppaass_connection_write = self.ppaass_connection_write;

        let user_token = self.user_token;
        let handler_key = self.handler_key;

        let DnsLookupRequestParts { domain_names, request_id } = dns_lookup_request.split();

        info!("Dns lookup handler {handler_key} receive agent request for domains [{domain_names:?}] with request id [{request_id}]");

        let addresses = domain_names
            .iter()
            .map(|domain_name| match dns_lookup::lookup_host(domain_name) {
                Err(e) => {
                    error!("Dns lookup handler fail to lookup domain name: [{domain_name}] because of error: {e:?}");
                    (domain_name.to_owned(), None)
                },
                Ok(addresses_of_current_domain) => {
                    info!("Dns lookup handler success lookup domain name [{domain_name}] (before ipv4 filter): {addresses_of_current_domain:?}");
                    let addresses_of_current_domain = addresses_of_current_domain
                        .iter()
                        .filter_map(|addr| match addr {
                            IpAddr::V4(ip_addr) => Some(ip_addr.octets()),
                            IpAddr::V6(_) => None,
                        })
                        .collect::<Vec<[u8; 4]>>();
                    info!("Dns lookup handler success lookup domain name [{domain_name}] (ipv4 only): {addresses_of_current_domain:?}");
                    if addresses_of_current_domain.is_empty() {
                        (domain_name.to_owned(), None)
                    } else {
                        (domain_name.to_owned(), Some(addresses_of_current_domain))
                    }
                },
            })
            .collect::<HashMap<String, Option<Vec<[u8; 4]>>>>();

        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));

        let dns_lookup_response_message =
            match PpaassMessageGenerator::generate_dns_lookup_response(user_token.clone(), payload_encryption, request_id, addresses) {
                Ok(data_message) => data_message,
                Err(e) => {
                    error!("Dns lookup handler {handler_key} fail to generate response for dns lookup [{request_id}] because of error: {e:?}");
                    if let Err(e) = ppaass_connection_write.close().await {
                        error!("Dns lookup handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(
                        "Dns lookup handler {handler_key} fail to generate response for dns lookup [{request_id}] because of error: {e:?}"
                    ));
                },
            };

        if let Err(e) = ppaass_connection_write.send(dns_lookup_response_message).await {
            error!("Dns lookup handler {handler_key} fail to send response for dns lookup [{request_id}] to agent because of error: {e:?}");
            if let Err(e) = ppaass_connection_write.close().await {
                error!("Dns lookup handler {handler_key} fail to close tcp connection because of error: {e:?}");
            }
            return Err(anyhow!(
                "Dns lookup handler {handler_key} fail to send response for dns lookup [{request_id}] to agent because of error: {e:?}"
            ));
        };
        if let Err(e) = ppaass_connection_write.close().await {
            error!("Dns lookup handler {handler_key} fail to close tcp connection because of error: {e:?}");
        }

        Ok(())
    }
}
