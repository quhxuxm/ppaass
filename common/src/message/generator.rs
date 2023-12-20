use bytes::Bytes;
use uuid::Uuid;

use crate::{
    tcp::{AgentTcpData, AgentTcpInit, ProxyTcpInit, ProxyTcpInitResultType},
    udp::{AgentUdpData, ProxyUdpData},
    CommonError, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessageAgentProtocol, PpaassMessageAgentTcpPayloadType, PpaassMessageAgentUdpPayloadType,
    PpaassMessagePayloadEncryption, PpaassMessageProxyProtocol, PpaassMessageProxyTcpPayloadType, PpaassMessageProxyUdpPayloadType, PpaassNetAddress,
    PpaassProxyMessage, PpaassProxyMessagePayload,
};

pub struct PpaassMessageGenerator;

impl PpaassMessageGenerator {
    /// Generate the agent tcp init message
    pub fn generate_agent_tcp_init_message(
        user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassAgentMessage, CommonError> {
        let tcp_init = AgentTcpInit { src_address, dst_address };
        let payload = PpaassAgentMessagePayload {
            protocol: PpaassMessageAgentProtocol::Tcp(PpaassMessageAgentTcpPayloadType::Init),
            data: tcp_init.try_into()?,
        };
        let message = PpaassAgentMessage::new(Uuid::new_v4().to_string(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the proxy tcp init message
    pub fn generate_proxy_tcp_init_message(
        id: String, user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, encryption: PpaassMessagePayloadEncryption,
        result_type: ProxyTcpInitResultType,
    ) -> Result<PpaassProxyMessage, CommonError> {
        let tcp_init = ProxyTcpInit {
            id,
            src_address,
            dst_address,
            result_type,
        };
        let payload = PpaassProxyMessagePayload {
            protocol: PpaassMessageProxyProtocol::Tcp(PpaassMessageProxyTcpPayloadType::Init),
            data: tcp_init.try_into()?,
        };
        let message = PpaassProxyMessage::new(Uuid::new_v4().to_string(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the agent tcp data message
    pub fn generate_agent_tcp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Bytes,
    ) -> Result<PpaassAgentMessage, CommonError> {
        let tcp_data = AgentTcpData::new(src_address, dst_address, data);
        let payload = PpaassAgentMessagePayload {
            protocol: PpaassMessageAgentProtocol::Tcp(PpaassMessageAgentTcpPayloadType::Data),
            data: tcp_data.try_into()?,
        };
        let message = PpaassAgentMessage::new(Uuid::new_v4().to_string(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the proxy tcp data message
    pub fn generate_proxy_tcp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Bytes,
    ) -> Result<PpaassProxyMessage, CommonError> {
        let tcp_data = AgentTcpData::new(src_address, dst_address, data);
        let payload = PpaassProxyMessagePayload {
            protocol: PpaassMessageProxyProtocol::Tcp(PpaassMessageProxyTcpPayloadType::Data),
            data: tcp_data.try_into()?,
        };
        let message = PpaassProxyMessage::new(Uuid::new_v4().to_string(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the agent udp data message
    pub fn generate_agent_udp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Bytes,
        need_response: bool,
    ) -> Result<PpaassAgentMessage, CommonError> {
        let udp_data = AgentUdpData::new(src_address, dst_address, data, need_response);
        let payload = PpaassAgentMessagePayload {
            protocol: PpaassMessageAgentProtocol::Udp(PpaassMessageAgentUdpPayloadType::Data),
            data: udp_data.try_into()?,
        };
        let message = PpaassAgentMessage::new(Uuid::new_v4().to_string(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the proxy udp data message
    pub fn generate_proxy_udp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Bytes,
    ) -> Result<PpaassProxyMessage, CommonError> {
        let udp_data = ProxyUdpData::new(src_address, dst_address, data);
        let payload = PpaassProxyMessagePayload {
            protocol: PpaassMessageProxyProtocol::Udp(PpaassMessageProxyUdpPayloadType::Data),
            data: udp_data.try_into()?,
        };
        let message = PpaassProxyMessage::new(Uuid::new_v4().to_string(), user_token.to_string(), encryption, payload);
        Ok(message)
    }
}
