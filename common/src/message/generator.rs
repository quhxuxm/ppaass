use crate::{
    generate_uuid,
    tcp::{TcpData, TcpInitRequest, TcpInitResponse, TcpInitResponseType},
    udp::UdpData,
    CommonError, PpaassMessage, PpaassMessageAgentPayloadType, PpaassMessagePayload, PpaassMessagePayloadEncryption, PpaassMessageProxyPayloadType,
    PpaassNetAddress,
};

pub struct PpaassMessageGenerator;

impl PpaassMessageGenerator {
    pub fn generate_agent_tcp_init_request(
        user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_init_request = TcpInitRequest { src_address, dst_address };
        let payload = PpaassMessagePayload::Agent {
            encryption,
            payload_type: PpaassMessageAgentPayloadType::TcpInitRequest,
            data: tcp_init_request.try_into()?,
        };
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload);
        Ok(message)
    }

    pub fn generate_proxy_tcp_init_response(
        id: String, user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, encryption: PpaassMessagePayloadEncryption,
        response_type: TcpInitResponseType,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_init_response = TcpInitResponse {
            id,
            src_address,
            dst_address,
            response_type,
        };
        let payload = PpaassMessagePayload::Proxy {
            encryption,
            payload_type: PpaassMessageProxyPayloadType::TcpInitResponse,
            data: tcp_init_response.try_into()?,
        };
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload);
        Ok(message)
    }

    pub fn generate_agent_tcp_data(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_data = TcpData::new(src_address, dst_address, data);
        let payload = PpaassMessagePayload::Agent {
            encryption,
            payload_type: PpaassMessageAgentPayloadType::TcpData,
            data: tcp_data.try_into()?,
        };
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload);
        Ok(message)
    }

    pub fn generate_proxy_tcp_data(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_data = TcpData::new(src_address, dst_address, data);
        let payload = PpaassMessagePayload::Proxy {
            encryption,
            payload_type: PpaassMessageProxyPayloadType::TcpData,
            data: tcp_data.try_into()?,
        };
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload);
        Ok(message)
    }

    pub fn generate_agent_udp_data(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassMessage, CommonError> {
        let udp_data: UdpData = UdpData::new(src_address, dst_address, data);
        let payload = PpaassMessagePayload::Agent {
            encryption,
            payload_type: PpaassMessageAgentPayloadType::UdpData,
            data: udp_data.try_into()?,
        };
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload);
        Ok(message)
    }

    pub fn generate_proxy_udp_data(
        user_token: impl ToString, encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, data: Vec<u8>,
    ) -> Result<PpaassMessage, CommonError> {
        let udp_data: UdpData = UdpData::new(src_address, dst_address, data);
        let payload = PpaassMessagePayload::Proxy {
            encryption,
            payload_type: PpaassMessageProxyPayloadType::UdpData,
            data: udp_data.try_into()?,
        };
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload);
        Ok(message)
    }
}
