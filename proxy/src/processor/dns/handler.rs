use crate::common::ProxyServerPayloadEncryptionSelector;
use anyhow::{anyhow, Context, Result};
use futures::SinkExt;
use ppaass_common::{
    dns::{DnsLookupRequest, DnsLookupRequestParts},
    generate_uuid, PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress,
    RsaCryptoFetcher,
};
use std::{collections::HashMap, fmt::Debug};
use std::{fmt::Display, net::IpAddr};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{error, info};

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct DnsLookupHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: String,
    user_token: String,
}

impl<T, R, I> DnsLookupHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn generate_handler_key(user_token: &str, agent_address: &PpaassNetAddress, ppaass_connection_id: &str) -> String {
        format!("[{ppaass_connection_id}]#[{user_token}]@DNSLOOKUP::[{agent_address}]")
    }
    pub(crate) fn new(
        connection_id: String, agent_address: PpaassNetAddress, user_token: String, agent_connection_write: PpaassConnectionWrite<T, R, I>,
    ) -> Self {
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &connection_id);
        Self {
            agent_connection_write,
            handler_key,
            user_token,
        }
    }

    pub(crate) async fn exec(self, dns_lookup_request: DnsLookupRequest) -> Result<()> {
        let mut agent_connection_write = self.agent_connection_write;

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
                    if let Err(e) = agent_connection_write.close().await {
                        error!("Dns lookup handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(
                        "Dns lookup handler {handler_key} fail to generate response for dns lookup [{request_id}] because of error: {e:?}"
                    ));
                },
            };

        if let Err(e) = agent_connection_write.send(dns_lookup_response_message).await {
            error!("Dns lookup handler {handler_key} fail to send response for dns lookup [{request_id}] to agent because of error: {e:?}");
            if let Err(e) = agent_connection_write.close().await {
                error!("Dns lookup handler {handler_key} fail to close tcp connection because of error: {e:?}");
            }
            return Err(anyhow!(
                "Dns lookup handler {handler_key} fail to send response for dns lookup [{request_id}] to agent because of error: {e:?}"
            ));
        };
        if let Err(e) = agent_connection_write.close().await {
            error!("Dns lookup handler {handler_key} fail to close tcp connection because of error: {e:?}");
        }

        Ok(())
    }
}
