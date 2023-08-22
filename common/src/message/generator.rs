use crate::{
    generate_uuid,
    tcp::{AgentTcpData, AgentTcpInit, ProxyTcpInit, ProxyTcpInitResultType},
    udp::UdpData,
    CommonError, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessageAgentPayloadType, PpaassMessagePayloadEncryption, PpaassMessageProxyPayloadType,
    PpaassNetAddress, PpaassProxyMessage, PpaassProxyMessagePayload,
};

pub struct PpaassMessageGenerator;

impl PpaassMessageGenerator {
    /// Generate the agent tcp init message
    pub fn generate_agent_tcp_init_message(
        user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassAgentMessage, CommonError> {
        let tcp_init = AgentTcpInit { src_address, dst_address };
        let payload = PpaassAgentMessagePayload {
            payload_type: PpaassMessageAgentPayloadType::TcpInit,
            data: tcp_init.try_into()?,
        };
        let message = PpaassAgentMessage::new(generate_uuid(), user_token.to_string(), encryption, payload);
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
            payload_type: PpaassMessageProxyPayloadType::TcpInit,
            data: tcp_init.try_into()?,
        };
        let message = PpaassProxyMessage::new(generate_uuid(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the agent tcp data message
    pub fn generate_agent_tcp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassAgentMessage, CommonError> {
        let tcp_data = AgentTcpData::new(src_address, dst_address, data);
        let payload = PpaassAgentMessagePayload {
            payload_type: PpaassMessageAgentPayloadType::TcpData,
            data: tcp_data.try_into()?,
        };
        let message = PpaassAgentMessage::new(generate_uuid(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the proxy tcp data message
    pub fn generate_proxy_tcp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassProxyMessage, CommonError> {
        let tcp_data = AgentTcpData::new(src_address, dst_address, data);
        let payload = PpaassProxyMessagePayload {
            payload_type: PpaassMessageProxyPayloadType::TcpData,
            data: tcp_data.try_into()?,
        };
        let message = PpaassProxyMessage::new(generate_uuid(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the agent udp data message
    pub fn generate_agent_udp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassAgentMessage, CommonError> {
        let udp_data: UdpData = UdpData::new(src_address, dst_address, data);
        let payload = PpaassAgentMessagePayload {
            payload_type: PpaassMessageAgentPayloadType::UdpData,
            data: udp_data.try_into()?,
        };
        let message = PpaassAgentMessage::new(generate_uuid(), user_token.to_string(), encryption, payload);
        Ok(message)
    }

    /// Generate the proxy udp data message
    pub fn generate_proxy_udp_data_message(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassProxyMessage, CommonError> {
        let udp_data: UdpData = UdpData::new(src_address, dst_address, data);
        let payload = PpaassProxyMessagePayload {
            payload_type: PpaassMessageProxyPayloadType::UdpData,
            data: udp_data.try_into()?,
        };
        let message = PpaassProxyMessage::new(generate_uuid(), user_token.to_string(), encryption, payload);
        Ok(message)
    }
}
