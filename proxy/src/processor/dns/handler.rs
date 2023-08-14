use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    crypto::ProxyServerRsaCryptoFetcher,
    error::{NetworkError, ProxyError},
};

use derive_more::{Constructor, Display};
use dns_lookup::lookup_host;
use futures::SinkExt;
use ppaass_common::{dns::DnsLookupRequest, generate_uuid, PpaassConnection, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};
use std::{collections::HashMap, fmt::Debug};
use std::{fmt::Display, net::IpAddr};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{error, info};

#[derive(Debug, Clone, Constructor, Display)]
#[display(fmt = "[{}]#[{}]@DNS::[{}]", connection_id, user_token, agent_address)]
pub(crate) struct DnsLookupHandlerKey {
    connection_id: String,
    user_token: String,
    agent_address: PpaassNetAddress,
}

#[derive(Constructor)]
#[non_exhaustive]
pub(crate) struct DnsLookupHandler<'r, T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
    'r: 'static,
{
    handler_key: DnsLookupHandlerKey,
    agent_connection: PpaassConnection<'r, T, ProxyServerRsaCryptoFetcher, I>,
}

impl<'r, T, I> DnsLookupHandler<'r, T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
    'r: 'static,
{
    pub(crate) async fn exec(self, dns_lookup_request: DnsLookupRequest) -> Result<(), ProxyError> {
        let mut agent_connection = self.agent_connection;
        let handler_key = self.handler_key;
        let DnsLookupRequest { domain_names, request_id, .. } = dns_lookup_request;
        info!("Dns lookup handler {handler_key} receive agent request for domains [{domain_names:?}] with request id [{request_id}]");
        let addresses = domain_names
            .iter()
            .map(|domain_name| match lookup_host(domain_name) {
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

        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));

        let dns_lookup_response_message =
            PpaassMessageGenerator::generate_dns_lookup_response(handler_key.user_token.clone(), payload_encryption, request_id, addresses)?;

        if let Err(e) = agent_connection.send(dns_lookup_response_message).await {
            error!("Dns lookup handler {handler_key} fail to send response for dns lookup [{request_id}] to agent because of error: {e:?}");
            return Err(ProxyError::Network(NetworkError::AgentWrite(e)));
        };
        Ok(())
    }
}
