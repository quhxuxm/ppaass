use anyhow::anyhow;
use anyhow::Result;

use bytes::Bytes;
use common::{
    generate_uuid, AgentMessagePayloadTypeValue, MessageFramedRead, MessageFramedReader, MessageFramedWrite, MessageFramedWriter, MessagePayload, NetAddress,
    PayloadEncryptionTypeSelectRequest, PayloadEncryptionTypeSelectResult, PayloadEncryptionTypeSelector, PayloadType, ProxyMessagePayloadTypeValue,
    ReadMessageFramedError, ReadMessageFramedRequest, ReadMessageFramedResult, ReadMessageFramedResultContent, RsaCryptoFetcher, WriteMessageFramedError,
    WriteMessageFramedRequest, WriteMessageFramedResult,
};

use std::{
    fmt::Debug,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
};

use tokio::net::{TcpStream, UdpSocket};

use crate::{
    config::{ProxyConfig, DEFAULT_UDP_RELAY_TIMEOUT_SECONDS},
    service::{tcp::connect::TcpConnectFlowError, udp::associate::UdpAssociateFlowError},
};
use pretty_hex::pretty_hex;
use tracing::{debug, error, info};

use super::{
    tcp::connect::{TcpConnectFlow, TcpConnectFlowRequest, TcpConnectFlowResult},
    udp::associate::{UdpAssociateFlow, UdpAssociateFlowRequest, UdpAssociateFlowResult},
};

const DEFAULT_AGENT_CONNECTION_READ_TIMEOUT: u64 = 1200;
const SIZE_64KB: usize = 65535;
#[derive(Debug)]
pub(crate) struct InitFlowRequest<'a, T, TcpStream>
where
    T: RsaCryptoFetcher,
{
    pub connection_id: &'a str,
    pub message_framed_read: MessageFramedRead<T, TcpStream>,
    pub message_framed_write: MessageFramedWrite<T, TcpStream>,
    pub agent_address: SocketAddr,
}

#[allow(unused)]
pub(crate) enum InitFlowResult<T>
where
    T: RsaCryptoFetcher,
{
    Heartbeat {
        message_framed_read: MessageFramedRead<T, TcpStream>,
        message_framed_write: MessageFramedWrite<T, TcpStream>,
    },
    Tcp {
        target_stream: TcpStream,
        message_framed_read: MessageFramedRead<T, TcpStream>,
        message_framed_write: MessageFramedWrite<T, TcpStream>,
        message_id: String,
        source_address: NetAddress,
        target_address: NetAddress,
        user_token: String,
    },
    UdpSocks {
        message_framed_read: MessageFramedRead<T, TcpStream>,
        message_framed_write: MessageFramedWrite<T, TcpStream>,
        message_id: String,
        source_address: Option<NetAddress>,
        user_token: String,
    },
    UdpAndroid {
        message_framed_read: MessageFramedRead<T, TcpStream>,
        message_framed_write: MessageFramedWrite<T, TcpStream>,
        message_id: String,
        source_address: Option<NetAddress>,
        user_token: String,
    },
}

#[derive(Clone, Default)]
pub(crate) struct InitializeFlow;

impl InitializeFlow {
    pub async fn exec<'a, T>(
        InitFlowRequest {
            connection_id,
            message_framed_read,
            mut message_framed_write,
            agent_address,
        }: InitFlowRequest<'a, T, TcpStream>,
        configuration: &ProxyConfig,
    ) -> Result<InitFlowResult<T>>
    where
        T: RsaCryptoFetcher + Send + Sync + Debug + 'static,
    {
        let read_timeout = configuration.agent_connection_read_timeout().unwrap_or(DEFAULT_AGENT_CONNECTION_READ_TIMEOUT);
        match MessageFramedReader::read(ReadMessageFramedRequest {
            connection_id: connection_id.clone(),
            message_framed_read,
            timeout: Some(read_timeout),
        })
        .await
        {
            Ok(ReadMessageFramedResult {
                message_framed_read,
                content:
                    Some(ReadMessageFramedResultContent {
                        user_token,
                        message_id,
                        message_payload:
                            Some(MessagePayload {
                                source_address,
                                target_address,
                                payload_type: PayloadType::AgentPayload(AgentMessagePayloadTypeValue::Heartbeat),
                                ..
                            }),
                        ..
                    }),
                ..
            }) => {
                let PayloadEncryptionTypeSelectResult { payload_encryption_type, .. } =
                    PayloadEncryptionTypeSelector::select(PayloadEncryptionTypeSelectRequest {
                        encryption_token: generate_uuid().into(),
                        user_token: user_token.as_str(),
                    })
                    .await?;
                let heartbeat_success = MessagePayload {
                    source_address: None,
                    target_address: None,
                    payload_type: PayloadType::ProxyPayload(ProxyMessagePayloadTypeValue::HeartbeatSuccess),
                    data: None,
                };
                let message_framed_write = match MessageFramedWriter::write(WriteMessageFramedRequest {
                    message_framed_write,
                    message_payloads: Some(vec![heartbeat_success]),
                    payload_encryption_type,
                    user_token: user_token.as_str(),
                    ref_id: Some(message_id.as_str()),
                    connection_id: Some(connection_id),
                })
                .await
                {
                    Err(WriteMessageFramedError { source, .. }) => {
                        error!("Connection [{}] fail to write heartbeat success to agent because of error, source address: {:?}, target address: {:?}, client address: {:?}", connection_id, source_address, target_address, agent_address);
                        return Err(anyhow!(source));
                    },
                    Ok(WriteMessageFramedResult { message_framed_write }) => message_framed_write,
                };
                return Ok(InitFlowResult::Heartbeat {
                    message_framed_write,
                    message_framed_read,
                });
            },
            Ok(ReadMessageFramedResult {
                message_framed_read,
                content:
                    Some(ReadMessageFramedResultContent {
                        message_id,
                        user_token,
                        message_payload:
                            Some(MessagePayload {
                                payload_type: PayloadType::AgentPayload(AgentMessagePayloadTypeValue::TcpConnect),
                                target_address: Some(target_address),
                                source_address: Some(source_address),
                                ..
                            }),
                        ..
                    }),
                ..
            }) => {
                debug!(
                    "Connection [{}] begin tcp connect, source address: {:?}, target address: {:?}, client address: {:?}",
                    connection_id, source_address, target_address, agent_address
                );
                match TcpConnectFlow::exec(
                    TcpConnectFlowRequest {
                        connection_id,
                        message_id: message_id.as_str(),
                        message_framed_read,
                        message_framed_write,
                        agent_address,
                        source_address,
                        target_address,
                        user_token: user_token.as_str(),
                    },
                    configuration,
                )
                .await
                {
                    Err(TcpConnectFlowError { connection_id, source, .. }) => {
                        error!("Connection [{connection_id}] handle agent connection fail to do tcp connect because of error: {source:#?}.");
                        Err(anyhow!(
                            "Connection [{connection_id}] handle agent connection fail to do tcp connect because of error: {source:#?}."
                        ))
                    },
                    Ok(TcpConnectFlowResult {
                        target_stream,
                        message_framed_read,
                        message_framed_write,
                        source_address,
                        target_address,
                        user_token,
                        message_id,
                        ..
                    }) => {
                        debug!(
                            "Connection [{}] complete tcp connect, source address: {:?}, target address: {:?}, client address: {:?}",
                            connection_id, source_address, target_address, agent_address
                        );
                        Ok(InitFlowResult::Tcp {
                            message_framed_write,
                            message_framed_read,
                            target_stream,
                            message_id,
                            source_address,
                            target_address,
                            user_token,
                        })
                    },
                }
            },
            Ok(ReadMessageFramedResult {
                message_framed_read,
                content:
                    Some(ReadMessageFramedResultContent {
                        message_id,
                        user_token,
                        message_payload:
                            Some(MessagePayload {
                                payload_type: PayloadType::AgentPayload(AgentMessagePayloadTypeValue::UdpDataAndroid),
                                target_address: Some(target_address),
                                source_address: Some(source_address),
                                data: Some(data),
                            }),
                        ..
                    }),
                ..
            }) => {
                let udp_socket = match UdpSocket::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0))).await {
                    Err(e) => {
                        error!("Udp data relay [android] fail to bind udp socket, connection id: [{connection_id}], error : {e:#?}");
                        return Err(anyhow!(
                            "Udp data relay [android] fail to bind udp socket, connection id: [{connection_id}], error : {:#?}",
                            e
                        ));
                    },
                    Ok(v) => v,
                };
                let udp_target_addresses = match target_address.clone().to_socket_addrs() {
                    Err(e) => {
                        error!("Udp relay [android] fail to convert target address, connection id: [{connection_id}], error : {e:#?}");
                        return Err(anyhow!(
                            "Udp relay [android] fail to convert target address, connection id: [{connection_id}], error : {:#?}",
                            e
                        ));
                    },
                    Ok(v) => v,
                };
                if let Err(e) = udp_socket.connect(udp_target_addresses.collect::<Vec<_>>().as_slice()).await {
                    error!("Udp relay fail [android] connect to target, target: [{target_address:?}], connection id: [{connection_id}], error: {e:#?}");
                    return Err(anyhow!(
                        "Udp relay fail [android] connect to target, target: [{target_address:?}], connection id: [{connection_id}], error: {:#?}",
                        e
                    ));
                };
                info!(
                    "Udp relay [android] begin to send udp data from agent to target: [{target_address:?}], connection id: [{connection_id}], data:\n{}\n",
                    pretty_hex::pretty_hex(&data)
                );
                if let Err(e) = udp_socket.send(&data).await {
                    error!(
                        "Udp relay [android] fail to send udp packet to target, connection id:[{connection_id}], target: [{target_address:?}], error: {e:#?}"
                    );
                    return Err(anyhow!(
                        "Udp relay [android] fail to send udp packet to target, connection id:[{connection_id}], target: [{target_address:?}], error: {:#?}",
                        e
                    ));
                };
                let mut receive_buffer = [0u8; SIZE_64KB];

                let udp_relay_timeout = configuration.udp_relay_timeout().unwrap_or(DEFAULT_UDP_RELAY_TIMEOUT_SECONDS);
                let received_data_size = match tokio::time::timeout(std::time::Duration::from_secs(udp_relay_timeout), udp_socket.recv(&mut receive_buffer))
                    .await
                {
                    Err(_elapsed) => {
                        error!("Timeout({udp_relay_timeout} seconds) [android] to receive udp packet from target, connection id: [{connection_id}], target: [{target_address:?}]");
                        return Err(anyhow!("Timeout({udp_relay_timeout} seconds) to receive udp packet from target, connection id: [{connection_id}], target: [{target_address:?}]"));
                    },
                    Ok(Err(e)) => {
                        error!(
                            "Udp relay [android] fail to receive udp packet from target, connection id: [{connection_id}], target: [{target_address:?}], error:{e:#?}"
                        );
                        return Err(anyhow!(
                            "Udp relay [android] fail to receive udp packet from target, connection id: [{connection_id}], target: [{target_address:?}], error:{e:#?}"
                        ));
                    },
                    Ok(Ok(v)) => v,
                };

                let received_data = &receive_buffer[0..received_data_size];
                info!(
                    "Udp relay [android] receive data from target, connection id:[{connection_id}], target:[{target_address:?}], data:\n{}\n",
                    pretty_hex(&received_data)
                );
                let PayloadEncryptionTypeSelectResult {
                    user_token,
                    payload_encryption_type,
                    ..
                } = match PayloadEncryptionTypeSelector::select(PayloadEncryptionTypeSelectRequest {
                    encryption_token: generate_uuid().into(),
                    user_token: user_token.as_str(),
                })
                .await
                {
                    Err(e) => {
                        error!(
                            "Udp relay [android] fail to select payload encryption, connection id: [{connection_id}], target address: [{target_address:?}], error:{e:#?}"
                        );
                        return Err(anyhow!("Udp relay [android] fail to select payload encryption, connection id: [{connection_id}], target address: [{target_address:?}], error:{e:#?}"));
                    },
                    Ok(v) => v,
                };

                message_framed_write = match MessageFramedWriter::write(WriteMessageFramedRequest {
                    connection_id: Some(connection_id),
                    message_framed_write,
                    message_payloads: Some(vec![MessagePayload {
                        data: Some(Bytes::copy_from_slice(received_data)),
                        payload_type: PayloadType::ProxyPayload(ProxyMessagePayloadTypeValue::UdpDataAndroid),
                        source_address: Some(source_address.clone()),
                        target_address: Some(target_address.clone()),
                    }]),
                    payload_encryption_type: payload_encryption_type.clone(),
                    ref_id: Some(message_id.as_str()),
                    user_token: user_token.as_str(),
                })
                .await
                {
                    Err(WriteMessageFramedError { message_framed_write, source }) => {
                        error!(
                            "Udp relay fail to write data to target, connection id: [{connection_id}], target address: [{target_address:?}], error:{source:#?}"
                        );
                        message_framed_write
                    },
                    Ok(WriteMessageFramedResult { message_framed_write }) => message_framed_write,
                };
                Ok(InitFlowResult::UdpAndroid {
                    message_framed_write,
                    message_framed_read,
                    message_id,
                    source_address: Some(source_address),
                    user_token,
                })
            },
            Ok(ReadMessageFramedResult {
                message_framed_read,
                content:
                    Some(ReadMessageFramedResultContent {
                        message_id,
                        user_token,
                        message_payload:
                            Some(MessagePayload {
                                payload_type: PayloadType::AgentPayload(AgentMessagePayloadTypeValue::UdpAssociateSocks),
                                target_address: None,
                                source_address,
                                ..
                            }),
                        ..
                    }),
                ..
            }) => {
                debug!("Connection [{}] begin udp associate, client address: {:?}", connection_id, source_address);
                match UdpAssociateFlow::exec(
                    UdpAssociateFlowRequest {
                        message_framed_read,
                        message_framed_write,
                        agent_address,
                        connection_id,
                        message_id: message_id.as_str(),
                        source_address,
                        user_token: user_token.as_str(),
                    },
                    configuration,
                )
                .await
                {
                    Err(UdpAssociateFlowError { connection_id, source, .. }) => {
                        error!("Connection [{connection_id}] handle agent connection fail to do udp associate because of error: {source:#?}.");
                        Err(anyhow!(
                            "Connection [{connection_id}] handle agent connection fail to do udp associate because of error: {source:#?}."
                        ))
                    },
                    Ok(UdpAssociateFlowResult {
                        connection_id,
                        message_id,
                        user_token,
                        message_framed_read,
                        message_framed_write,
                        source_address,
                    }) => {
                        debug!("Connection [{}] complete udp associate, client address: {:?}", connection_id, source_address);
                        Ok(InitFlowResult::UdpSocks {
                            message_framed_write,
                            message_framed_read,
                            message_id,
                            source_address,
                            user_token,
                        })
                    },
                }
            },
            Ok(read_message_framed_result) => {
                error!("Connection [{connection_id}] handle agent connection fail because of invalid message content:\n{read_message_framed_result:#?}\n.");
                Err(anyhow!(
                    "Connection [{connection_id}] handle agent connection fail because of invalid message content."
                ))
            },
            Err(ReadMessageFramedError { source, .. }) => {
                error!("Connection [{connection_id}] handle agent connection fail because of error: {source}.");
                Err(anyhow!("Connection [{connection_id}] handle agent connection fail because of error: {source}."))
            },
        }
    }
}
