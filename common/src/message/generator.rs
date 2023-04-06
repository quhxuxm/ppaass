use anyhow::Result;

use crate::{
    domain_resolve::{DomainResolveRequestPayload, DomainResolveResponsePayload, DomainResolveResponseType},
    heartbeat::{HeartbeatRequestPayload, HeartbeatResponsePayload},
    tcp_loop::{TcpLoopInitRequestPayload, TcpLoopInitResponsePayload, TcpLoopInitResponseType},
    udp_loop::{UdpLoopData, UdpLoopDataParts, UdpLoopInitRequestPayload, UdpLoopInitResponsePayload, UdpLoopInitResponseType},
    PpaassMessage, PpaassMessageAgentPayload, PpaassMessageAgentPayloadType, PpaassMessagePayloadEncryption, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadType, PpaassNetAddress,
};

pub struct PpaassMessageGenerator;

impl PpaassMessageGenerator {
    pub fn generate_heartbeat_request(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption) -> Result<PpaassMessage> {
        let timestamp = chrono::Utc::now().timestamp();
        let heartbeat_request = HeartbeatRequestPayload { timestamp };
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::IdleHeartbeat, heartbeat_request.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_heartbeat_response(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption) -> Result<PpaassMessage> {
        let timestamp = chrono::Utc::now().timestamp();
        let heartbeat_response = HeartbeatResponsePayload { timestamp };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::IdleHeartbeat, heartbeat_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_domain_resolve_request(
        user_token: impl AsRef<str>, request_id: impl AsRef<str>, domain_name: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let domain_resolve_request = DomainResolveRequestPayload {
            request_id: request_id.as_ref().to_string(),
            domain_name: domain_name.as_ref().to_string(),
            src_address,
            dest_address,
        };
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::DomainNameResolve, domain_resolve_request.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_domain_resolve_success_response(
        user_token: impl AsRef<str>, request_id: impl AsRef<str>, domain_name: impl AsRef<str>, resolved_ip_addresses: Vec<[u8; 4]>,
        src_address: PpaassNetAddress, dest_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let domain_resolve_response = DomainResolveResponsePayload {
            request_id: request_id.as_ref().to_string(),
            domain_name: domain_name.as_ref().to_string(),
            resolved_ip_addresses: Some(resolved_ip_addresses),
            response_type: DomainResolveResponseType::Success,
            src_address,
            dest_address,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::DomainNameResolve, domain_resolve_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_domain_resolve_fail_response(
        user_token: impl AsRef<str>, request_id: impl AsRef<str>, domain_name: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let domain_resolve_response = DomainResolveResponsePayload {
            request_id: request_id.as_ref().to_string(),
            domain_name: domain_name.as_ref().to_string(),
            resolved_ip_addresses: None,
            response_type: DomainResolveResponseType::Fail,
            src_address,
            dest_address,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::DomainNameResolve, domain_resolve_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_loop_init_request(
        user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_loop_init_request = TcpLoopInitRequestPayload { src_address, dest_address };
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::TcpLoopInit, tcp_loop_init_request.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_loop_init_success_response(
        loop_key: impl AsRef<str>, user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_loop_init_response = TcpLoopInitResponsePayload {
            loop_key: loop_key.as_ref().to_owned(),
            src_address,
            dest_address,
            response_type: TcpLoopInitResponseType::Success,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::TcpLoopInit, tcp_loop_init_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_loop_init_success_response(
        loop_key: impl AsRef<str>, user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let udp_loop_init_response = UdpLoopInitResponsePayload {
            loop_key: loop_key.as_ref().to_owned(),
            response_type: UdpLoopInitResponseType::Success,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::UdpLoopInit, udp_loop_init_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_loop_init_fail_response(
        loop_key: impl AsRef<str>, user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_loop_init_response = TcpLoopInitResponsePayload {
            loop_key: loop_key.as_ref().to_owned(),
            src_address,
            dest_address,
            response_type: TcpLoopInitResponseType::Fail,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::TcpLoopInit, tcp_loop_init_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_loop_init_fail_response(
        loop_key: impl AsRef<str>, user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let udp_loop_init_response = UdpLoopInitResponsePayload {
            loop_key: loop_key.as_ref().to_owned(),
            response_type: UdpLoopInitResponseType::Fail,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::UdpLoopInit, udp_loop_init_response.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_tcp_raw_data(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption, data: Vec<u8>) -> Result<PpaassMessage> {
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, data);
        Ok(message)
    }

    pub fn generate_udp_loop_init_request(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption) -> Result<PpaassMessage> {
        let udp_loop_init_request = UdpLoopInitRequestPayload {};
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::UdpLoopInit, udp_loop_init_request.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn generate_udp_loop_data(
        user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        raw_data_bytes: Vec<u8>,
    ) -> Result<PpaassMessage> {
        let udp_loop_data_parts = UdpLoopDataParts {
            src_address,
            dst_address,
            raw_data_bytes,
        };
        let udp_loop_data: UdpLoopData = udp_loop_data_parts.into();
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, udp_loop_data.try_into()?);
        Ok(message)
    }
}
