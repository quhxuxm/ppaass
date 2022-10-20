use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use ppaass_protocol::{DomainResolveRequest, DomainResolveResponse};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};
use tracing::{debug, error};

use super::{AgentToTargetData, AgentToTargetDataType, TargetToAgentData, TargetToAgentDataType};

#[derive(Debug)]
pub(super) struct TargetEdge {
    transport_id: String,
    agent_to_target_data_receiver: Receiver<AgentToTargetData>,
    target_to_agent_data_sender: Sender<TargetToAgentData>,
}

impl TargetEdge {
    pub(super) fn new(
        transport_id: String, agent_to_target_data_receiver: Receiver<AgentToTargetData>, target_to_agent_data_sender: Sender<TargetToAgentData>,
    ) -> Self {
        Self {
            transport_id,
            agent_to_target_data_receiver,
            target_to_agent_data_sender,
        }
    }

    pub(super) async fn exec(self) {
        let mut agent_to_target_data_receiver = self.agent_to_target_data_receiver;
        let target_to_agent_data_sender = self.target_to_agent_data_sender;
        let transport_id = self.transport_id;
        tokio::spawn(async move {
            let mut target_tcp_stream = None::<TcpStream>;
            loop {
                let AgentToTargetData { data_type: request_type } = match agent_to_target_data_receiver.recv().await {
                    None => {
                        return;
                    },
                    Some(v) => v,
                };
                match request_type {
                    AgentToTargetDataType::TcpInitialize {
                        target_address,
                        source_address,
                        user_token,
                    } => {
                        let target_socket_addrs = match target_address.to_socket_addrs() {
                            Err(e) => {
                                continue;
                            },
                            Ok(v) => v,
                        };
                        let target_socket_addrs = target_socket_addrs.collect::<Vec<SocketAddr>>();
                        target_tcp_stream = match TcpStream::connect(target_socket_addrs.as_slice()).await {
                            Err(e) => {
                                error!("Transport [{transport_id}] fail connect to target becacuse of error: {e:?}");
                                if let Err(e) = target_to_agent_data_sender
                                    .send(TargetToAgentData {
                                        data_type: TargetToAgentDataType::TcpInitializeFail {
                                            source_address,
                                            target_address,
                                            user_token,
                                        },
                                    })
                                    .await
                                {
                                    error!("Transport [{transport_id}] fail to send target connect fail message to agent becacuse of error: {e:?}");
                                };
                                drop(target_to_agent_data_sender);
                                return;
                            },
                            Ok(v) => {
                                if let Err(e) = target_to_agent_data_sender
                                    .send(TargetToAgentData {
                                        data_type: TargetToAgentDataType::TcpInitializeSuccess {
                                            source_address,
                                            target_address,
                                            user_token,
                                        },
                                    })
                                    .await
                                {
                                    error!("Transport [{transport_id}] fail to send target connect success message to agent becacuse of error: {e:?}");
                                };
                                Some(v)
                            },
                        };
                    },
                    AgentToTargetDataType::TcpReplay {
                        data,
                        source_address,
                        target_address,
                        user_token,
                    } => {
                        let target_tcp_stream = match &mut target_tcp_stream {
                            None => {
                                continue;
                            },
                            Some(v) => v,
                        };
                        if let Err(e) = target_tcp_stream.write(&data).await {
                            error!("Transport [{transport_id}] fail to write data to target becacuse of error: {e:?}");
                            if let Err(e) = target_to_agent_data_sender
                                .send(TargetToAgentData {
                                    data_type: TargetToAgentDataType::TcpReplayFail {
                                        source_address,
                                        target_address,
                                        user_token,
                                    },
                                })
                                .await
                            {
                                error!("Transport [{transport_id}] fail to send relay fail message to agent becacuse of error: {e:?}");
                            };
                            drop(target_to_agent_data_sender);
                            return;
                        }
                    },
                    AgentToTargetDataType::TcpDestory {
                        source_address,
                        target_address,
                        user_token,
                    } => {
                        let mut target_tcp_stream = match target_tcp_stream {
                            None => {
                                continue;
                            },
                            Some(v) => v,
                        };
                        if let Err(e) = target_tcp_stream.shutdown().await {
                            error!("Transport [{transport_id}] fail to shutdown target tcp stream becacuse of error: {e:?}");
                        };
                        drop(target_to_agent_data_sender);
                        return;
                    },
                    AgentToTargetDataType::ConnectionKeepAlive { user_token } => {
                        target_to_agent_data_sender
                            .send(TargetToAgentData {
                                data_type: TargetToAgentDataType::ConnectionKeepAliveSuccess { user_token },
                            })
                            .await;
                    },
                    AgentToTargetDataType::DomainNameResolve { data, user_token } => {
                        let DomainResolveRequest { id, name } = match serde_json::from_slice(&data) {
                            Err(e) => {
                                target_to_agent_data_sender
                                    .send(TargetToAgentData {
                                        data_type: TargetToAgentDataType::ConnectionKeepAliveSuccess { user_token },
                                    })
                                    .await;
                                continue;
                            },
                            Ok(v) => v,
                        };
                        let ip_addresses = match dns_lookup::lookup_host(name.as_str()) {
                            Err(e) => {
                                target_to_agent_data_sender
                                    .send(TargetToAgentData {
                                        data_type: TargetToAgentDataType::DomainNameResolveFail { user_token },
                                    })
                                    .await;
                                continue;
                            },
                            Ok(v) => v,
                        };
                        let mut addresses = Vec::new();
                        ip_addresses.iter().for_each(|addr| {
                            if let IpAddr::V4(v4_addr) = addr {
                                let ip_bytes = v4_addr.octets();
                                addresses.push(ip_bytes);
                                return;
                            }
                        });
                        let domain_resolve_response = DomainResolveResponse {
                            id,
                            ip_addresses: addresses,
                            name,
                        };
                        let data = match serde_json::to_vec(&domain_resolve_response) {
                            Err(e) => {
                                return;
                            },
                            Ok(v) => v,
                        };
                        target_to_agent_data_sender
                            .send(TargetToAgentData {
                                data_type: TargetToAgentDataType::DomainNameResolveSuccess { user_token, data },
                            })
                            .await;
                    },
                }
            }
        });
    }
}
