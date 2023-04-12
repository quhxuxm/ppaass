use anyhow::Result;

use crate::{
    tcp::{TcpData, TcpDataParts, TcpInitRequest, TcpInitResponse, TcpInitResponseType},
    udp::{UdpData, UdpDataParts, UdpInitRequest, UdpInitResponse, UdpInitResponseType},
    PpaassMessage, PpaassMessageAgentPayload, PpaassMessageAgentPayloadType, PpaassMessagePayloadEncryption, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadType, PpaassNetAddress,
};

pub struct PpaassMessageGenerator;

impl PpaassMessageGenerator {
    pub fn generate_tcp_init_request(
        user_token: impl AsRef<str>, src_address: PpaassNetAddress, dst_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_init_request = TcpInitRequest { src_address, dst_address };
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::TcpInit, tcp_init_request.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_init_response(
        unique_key: String, user_token: impl AsRef<str>, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, response_type: TcpInitResponseType,
    ) -> Result<PpaassMessage> {
        let tcp_init_response = TcpInitResponse {
            unique_key,
            src_address,
            dst_address,
            response_type,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::TcpInit, tcp_init_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_init_request(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption) -> Result<PpaassMessage> {
        let udp_init_request = UdpInitRequest {};
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::UdpInit, udp_init_request.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_init_response(
        loop_key: impl AsRef<str>, user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption, response_type: UdpInitResponseType,
    ) -> Result<PpaassMessage> {
        let udp_init_response = UdpInitResponse {
            loop_key: loop_key.as_ref().to_owned(),
            response_type,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::UdpInit, udp_init_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_data(
        user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        raw_data_bytes: Vec<u8>,
    ) -> Result<PpaassMessage> {
        let tcp_data_parts = TcpDataParts {
            src_address,
            dst_address,
            raw_data: raw_data_bytes,
        };
        let tcp_data: TcpData = tcp_data_parts.into();
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, tcp_data.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_data(
        user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        raw_data_bytes: Vec<u8>,
    ) -> Result<PpaassMessage> {
        let udp_data_parts = UdpDataParts {
            src_address,
            dst_address,
            raw_data: raw_data_bytes,
        };
        let udp_data: UdpData = udp_data_parts.into();
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, udp_data.try_into()?);
        Ok(message)
    }
}
