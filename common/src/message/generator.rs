use std::{borrow::Cow, collections::HashMap};

use crate::{
    dns::{DnsLookupRequest, DnsLookupResponse},
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
    ) -> Result<PpaassMessage<'static>, CommonError> {
        let tcp_init_request = TcpInitRequest { src_address, dst_address };
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::TcpInit, Cow::Owned(tcp_init_request.try_into()?));
        let message = PpaassMessage::new(
            generate_uuid(),
            user_token.to_string(),
            payload_encryption,
            Cow::Owned(message_payload.try_into()?),
        );
        Ok(message)
    }

    pub fn generate_tcp_init_response(
        id: String, user_token: impl ToString, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, response_type: TcpInitResponseType,
    ) -> Result<PpaassMessage<'static>, CommonError> {
        let tcp_init_response = TcpInitResponse {
            id,
            src_address,
            dst_address,
            response_type,
        };
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::TcpInit, Cow::Owned(tcp_init_response.try_into()?));
        let message = PpaassMessage::new(
            generate_uuid(),
            user_token.to_string(),
            payload_encryption,
            Cow::Owned(message_payload.try_into()?),
        );
        Ok(message)
    }

    pub fn generate_tcp_data(
        user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        data: Vec<u8>,
    ) -> Result<PpaassMessage<'static>, CommonError> {
        let tcp_data = TcpData::new(src_address, dst_address, data);
        let message = PpaassMessage::new(generate_uuid(), user_token.to_string(), payload_encryption, Cow::Owned(tcp_data.try_into()?));
        Ok(message)
    }

    pub fn generate_udp_data(
        user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        data: Vec<u8>,
    ) -> Result<PpaassMessage<'static>, CommonError> {
        let udp_data: UdpData = UdpData::new(src_address, dst_address, data);
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::UdpData, Cow::Owned(udp_data.try_into()?));
        let message = PpaassMessage::new(
            generate_uuid(),
            user_token.to_string(),
            payload_encryption,
            Cow::Owned(message_payload.try_into()?),
        );
        Ok(message)
    }

    pub fn generate_dns_lookup_request(
        user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, request_id: u16, domain_names: Vec<String>,
    ) -> Result<PpaassMessage<'static>, CommonError> {
        let dns_lookup_request = DnsLookupRequest::new(request_id, domain_names);
        let message_payload = PpaassMessageAgentPayload::new(PpaassMessageAgentPayloadType::DnsLookupRequest, Cow::Owned(dns_lookup_request.try_into()?));
        let message = PpaassMessage::new(
            generate_uuid(),
            user_token.to_string(),
            payload_encryption,
            Cow::Owned(message_payload.try_into()?),
        );
        Ok(message)
    }

    pub fn generate_dns_lookup_response(
        user_token: impl ToString, payload_encryption: PpaassMessagePayloadEncryption, request_id: u16, addresses: HashMap<String, Option<Vec<[u8; 4]>>>,
    ) -> Result<PpaassMessage<'static>, CommonError> {
        let dns_lookup_response = DnsLookupResponse::new(request_id, addresses);
        let message_payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::DnsLookupResponse, Cow::Owned(dns_lookup_response.try_into()?));
        let message = PpaassMessage::new(
            generate_uuid(),
            user_token.to_string(),
            payload_encryption,
            Cow::Owned(message_payload.try_into()?),
        );
        Ok(message)
    }
}
