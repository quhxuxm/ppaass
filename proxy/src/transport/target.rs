use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use bytes::BytesMut;
use ppaass_protocol::{DomainResolveRequest, DomainResolveResponse};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpStream, UdpSocket},
    sync::{
        mpsc::{Receiver, Sender},
        OwnedSemaphorePermit,
    },
    task::JoinHandle,
};
use tracing::{debug, error};

use super::{AgentToTargetData, AgentToTargetDataType, TargetToAgentData, TargetToAgentDataType};

#[derive(Debug)]
pub(super) struct TargetEdge {
    transport_id: String,
    agent_to_target_data_receiver: Receiver<AgentToTargetData>,
    target_to_agent_data_sender: Sender<TargetToAgentData>,
    _connection_number_permit: OwnedSemaphorePermit,
}

impl TargetEdge {
    pub(super) fn new(
        transport_id: String, agent_to_target_data_receiver: Receiver<AgentToTargetData>, target_to_agent_data_sender: Sender<TargetToAgentData>,
        connection_number_permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            transport_id,
            agent_to_target_data_receiver,
            target_to_agent_data_sender,
            _connection_number_permit: connection_number_permit,
        }
    }

    pub(super) async fn exec(self) {
        let mut agent_to_target_data_receiver = self.agent_to_target_data_receiver;
        let target_to_agent_data_sender = self.target_to_agent_data_sender;
        let transport_id = self.transport_id.clone();
        let mut target_tcp_write = None::<OwnedWriteHalf>;
        let mut target_udp_socket = None::<Arc<UdpSocket>>;
        let mut target_to_agent_tcp_relay_guard = None::<JoinHandle<()>>;
        let mut target_to_agent_udp_relay_guard = None::<JoinHandle<()>>;
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
                    if let Some(mut write) = target_tcp_write {
                        if let Err(e) = write.shutdown().await {
                            error!("Fail to shudown target tcp stream because of error: {e:?}")
                        };
                        target_tcp_write = None;
                    }
                    if let Some(ref guard) = target_to_agent_tcp_relay_guard {
                        guard.abort();
                        target_to_agent_tcp_relay_guard = None;
                    }
                    let target_socket_addrs = match target_address.to_socket_addrs() {
                        Err(e) => {
                            error!("Transport [{transport_id}] fail connect to target becacuse of error when convert target address: {e:?}");
                            continue;
                        },
                        Ok(v) => v,
                    };
                    let target_socket_addrs = target_socket_addrs.collect::<Vec<SocketAddr>>();
                    let new_target_tcp_stream = match TcpStream::connect(target_socket_addrs.as_slice()).await {
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
                        Ok(v) => v,
                    };

                    let (mut target_tcp_read, new_target_tcp_write) = new_target_tcp_stream.into_split();
                    target_tcp_write = Some(new_target_tcp_write);
                    let target_to_agent_data_sender_clone = target_to_agent_data_sender.clone();
                    let source_address_clone = source_address.clone();
                    let target_address_clone = target_address.clone();
                    let user_token_clone = user_token.clone();
                    let transport_id_for_target_to_agent = transport_id.clone();
                    let new_target_to_agent_relay_guard = tokio::spawn(async move {
                        loop {
                            let mut target_tcp_buffer = BytesMut::with_capacity(64 * 1024);
                            match target_tcp_read.read_buf(&mut target_tcp_buffer).await {
                                Ok(0) => {
                                    drop(target_to_agent_data_sender_clone);
                                    return;
                                },
                                Ok(n) => {
                                    let data = target_tcp_buffer.split().freeze();
                                    if let Err(e) = target_to_agent_data_sender_clone
                                        .send(TargetToAgentData {
                                            data_type: TargetToAgentDataType::TcpReplaySuccess {
                                                source_address: source_address_clone.clone(),
                                                target_address: target_address_clone.clone(),
                                                user_token: user_token_clone.clone(),
                                                data: data.into(),
                                            },
                                        })
                                        .await
                                    {
                                        error!("Transport [{transport_id_for_target_to_agent}] fail to send tcp relay success message to agent becacuse of error: {e:?}");
                                        return;
                                    };
                                    debug!("Transport [{transport_id_for_target_to_agent}] success to read data from target, data size = {n}.");
                                    return;
                                },
                                Err(e) => {
                                    error!("Transport [{transport_id_for_target_to_agent}] fail read data from target becacuse of error: {e:?}");
                                    drop(target_to_agent_data_sender_clone);
                                    return;
                                },
                            }
                        }
                    });
                    target_to_agent_tcp_relay_guard = Some(new_target_to_agent_relay_guard);
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
                        drop(target_to_agent_data_sender);
                        if let Some(ref mut write) = target_tcp_write {
                            if let Err(e) = write.shutdown().await {
                                error!("Transport [{transport_id}] fail to shutdown write of the target becacuse of error: {e:?}");
                            };
                        }
                        if let Some(guard) = target_to_agent_tcp_relay_guard {
                            guard.abort();
                        }

                        return;
                    };
                },
                AgentToTargetDataType::TcpReplay {
                    data,
                    source_address,
                    target_address,
                    user_token,
                } => {
                    let current_target_tcp_write = match &mut target_tcp_write {
                        None => {
                            drop(target_to_agent_data_sender);
                            return;
                        },
                        Some(v) => v,
                    };
                    if let Err(e) = current_target_tcp_write.write(&data).await {
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
                        if let Some(ref mut write) = target_tcp_write {
                            if let Err(e) = write.shutdown().await {
                                error!("Transport [{transport_id}] fail to shutdown write of the target becacuse of error: {e:?}");
                            };
                        }
                        if let Some(guard) = target_to_agent_tcp_relay_guard {
                            guard.abort();
                        }
                        return;
                    }
                },
                AgentToTargetDataType::TcpDestory { .. } => {
                    let mut target_tcp_write = match target_tcp_write {
                        None => {
                            drop(target_to_agent_data_sender);
                            return;
                        },
                        Some(v) => v,
                    };
                    if let Err(e) = target_tcp_write.shutdown().await {
                        error!("Transport [{transport_id}] fail to shutdown target tcp stream becacuse of error: {e:?}");
                    };
                    drop(target_to_agent_data_sender);
                    return;
                },
                AgentToTargetDataType::ConnectionKeepAlive { user_token } => {
                    if let Err(e) = target_to_agent_data_sender
                        .send(TargetToAgentData {
                            data_type: TargetToAgentDataType::ConnectionKeepAliveSuccess { user_token },
                        })
                        .await
                    {
                        error!("Transport [{transport_id}] fail to send keep alive success message to agent becacuse of error: {e:?}");
                    };
                },
                AgentToTargetDataType::DomainNameResolve { data, user_token } => {
                    let DomainResolveRequest { id, name } = match serde_json::from_slice(&data) {
                        Err(e) => {
                            error!("Transport [{transport_id}] fail to do domain name resolve because of fail to parse request body: {e:?}");
                            if let Err(e) = target_to_agent_data_sender
                                .send(TargetToAgentData {
                                    data_type: TargetToAgentDataType::DomainNameResolveFail { user_token },
                                })
                                .await
                            {
                                error!("Transport [{transport_id}] fail to send domain name resolve fail message to agent becacuse of error: {e:?}");
                            };
                            continue;
                        },
                        Ok(v) => v,
                    };
                    let ip_addresses = match dns_lookup::lookup_host(name.as_str()) {
                        Err(e) => {
                            error!("Transport [{transport_id}] fail to do dns lookup host because of error: {e:?}");
                            if let Err(e) = target_to_agent_data_sender
                                .send(TargetToAgentData {
                                    data_type: TargetToAgentDataType::DomainNameResolveFail { user_token },
                                })
                                .await
                            {
                                error!("Transport [{transport_id}] fail to send domain name resolve fail message to agent becacuse of error: {e:?}");
                            };
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
                            error!("Transport [{transport_id}] fail to do domain name resolve because of fail to parse request body: {e:?}");
                            continue;
                        },
                        Ok(v) => v,
                    };
                    if let Err(e) = target_to_agent_data_sender
                        .send(TargetToAgentData {
                            data_type: TargetToAgentDataType::DomainNameResolveSuccess { user_token, data },
                        })
                        .await
                    {
                        error!("Transport [{transport_id}] fail to send domain name resolve success message to agent becacuse of error: {e:?}");
                    };
                },
                AgentToTargetDataType::UdpInitialize {
                    source_address,
                    target_address,
                    user_token,
                } => {
                    if let Some(ref udp_socket) = target_udp_socket {
                        drop(udp_socket);
                    }
                    if let Some(ref udp_relay_guard) = target_to_agent_udp_relay_guard {
                        udp_relay_guard.abort();
                    }
                    target_udp_socket = match UdpSocket::bind("0.0.0.0:0").await {
                        Ok(v) => Some(Arc::new(v)),
                        Err(e) => {
                            error!("Transport [{transport_id}] fail to initialize udp socket becacuse of error: {e:?}");
                            continue;
                        },
                    };

                    let target_udp_socket_for_relay = target_udp_socket.clone();

                    let source_address_clone = source_address.clone();
                    let target_address_clone = target_address.clone();
                    let user_token_clone = user_token.clone();
                    let target_to_agent_data_sender_clone = target_to_agent_data_sender.clone();
                    let transport_id_clone = transport_id.clone();
                    let new_target_to_agent_udp_relay_guard = tokio::spawn(async move {
                        let target_udp_socket_for_relay = target_udp_socket_for_relay.expect("Fail to get target udp socket for relay.");
                        loop {
                            let mut buf = [0u8; 1024 * 64];
                            if let Ok(size) = target_udp_socket_for_relay.recv(&mut buf).await {
                                if let Err(e) = target_to_agent_data_sender_clone
                                    .send(TargetToAgentData {
                                        data_type: TargetToAgentDataType::UdpReplaySuccess {
                                            source_address: source_address_clone.clone(),
                                            target_address: target_address_clone.clone(),
                                            user_token: user_token_clone.clone(),
                                            data: buf[..size].into(),
                                        },
                                    })
                                    .await
                                {
                                    error!("Transport [{transport_id_clone}] fail to send udp relay success message to agent becacuse of error: {e:?}");
                                };
                            } else {
                                error!("Fail to receive udp data from target because of error.");
                            };
                        }
                    });

                    target_to_agent_udp_relay_guard = Some(new_target_to_agent_udp_relay_guard);

                    if let Err(e) = target_to_agent_data_sender
                        .send(TargetToAgentData {
                            data_type: TargetToAgentDataType::UdpInitializeSuccess {
                                source_address,
                                target_address,
                                user_token,
                            },
                        })
                        .await
                    {
                        error!("Transport [{transport_id}] fail to send udp initialize success message to agent becacuse of error: {e:?}");
                    };
                },
                AgentToTargetDataType::UdpReplay {
                    source_address,
                    target_address,
                    user_token,
                    data,
                } => {
                    let current_target_udp_socket = match &mut target_udp_socket {
                        None => {
                            drop(target_to_agent_data_sender);
                            return;
                        },
                        Some(v) => v,
                    };
                    let target_socket_addrs = match target_address.to_socket_addrs() {
                        Err(e) => {
                            error!("Transport [{transport_id}] fail connect to target becacuse of error when convert target address: {e:?}");
                            continue;
                        },
                        Ok(v) => v,
                    };
                    let target_socket_addrs = target_socket_addrs.collect::<Vec<SocketAddr>>();
                    if let Err(e) = current_target_udp_socket.send_to(&data, target_socket_addrs.as_slice()).await {
                        error!("Transport [{transport_id}] fail to relay udp package to target becacuse of error: {e:?}");
                        if let Err(e) = target_to_agent_data_sender
                            .send(TargetToAgentData {
                                data_type: TargetToAgentDataType::UdpReplayFail {
                                    source_address,
                                    target_address,
                                    user_token,
                                },
                            })
                            .await
                        {
                            error!("Transport [{transport_id}] fail to relay udp data fail message to agent becacuse of error: {e:?}");
                        };
                        continue;
                    };
                },
                AgentToTargetDataType::UdpDestory { .. } => {
                    if let Some(ref udp_socket) = target_udp_socket {
                        drop(udp_socket);
                    }
                    if let Some(ref udp_relay_guard) = target_to_agent_udp_relay_guard {
                        udp_relay_guard.abort();
                    }
                },
            }
        }
    }
}
