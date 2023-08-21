use crate::{
    generate_uuid,
    tcp::{TcpData, TcpInitRequest, TcpInitResponse, TcpInitResponseType},
    udp::UdpData,
    CommonError, PpaassMessage, PpaassMessageAgentPayload, PpaassMessageAgentPayloadType, PpaassMessagePayloadEncryption, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadType, PpaassNetAddress,
};

pub struct PpaassMessageGenerator;

impl PpaassMessageGenerator {
    pub fn generate_tcp_init_request(
        user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_init_request = TcpInitRequest { src_address, dst_address };
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::TcpInit, tcp_init_request.try_into()?);
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_init_response(
        id: String, user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, response_type: TcpInitResponseType,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_init_response = TcpInitResponse {
            id,
            src_address,
            dst_address,
            response_type,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::TcpInit, tcp_init_response.try_into()?);
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_data(
        user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        data: Vec<u8>,
    ) -> Result<PpaassMessage, CommonError> {
        let tcp_data = TcpData::new(src_address, dst_address, data);
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload_encryption, tcp_data.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_data(
        user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        data: Vec<u8>,
    ) -> Result<PpaassMessage, CommonError> {
        let udp_data: UdpData = UdpData::new(src_address, dst_address, data);
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::UdpData, udp_data.try_into()?);
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }
}
