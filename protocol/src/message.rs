use anyhow::Context;
use ppaass_common::generate_uuid;
use serde_derive::{Deserialize, Serialize};

mod address;
mod encryption;
mod payload;
mod types;
pub use address::*;
pub use encryption::*;
pub use payload::*;
pub use types::*;

use crate::serializer::vec_u8_to_base64;
use crate::tcp_session_init::{TcpSessionInitRequestPayload, TcpSessionInitResponsePayload};
use crate::tcp_session_relay::TcpSessionRelayPayload;
use anyhow::Result;
use heartbeat::HeartbeatRequestPayload;

use self::{
    domain_resolve::{DomainResolveRequestPayload, DomainResolveResponsePayload},
    heartbeat::HeartbeatResponsePayload,
    tcp_session_destroy::TcpSessionDestroyRequestPayload,
    tcp_session_relay::TcpSessionRelayStatus,
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PpaassMessage {
    id: String,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    #[serde(with = "vec_u8_to_base64")]
    payload_bytes: Vec<u8>,
}

pub struct PpaassMessageParts {
    pub id: String,
    pub user_token: String,
    pub payload_encryption: PpaassMessagePayloadEncryption,
    pub payload_bytes: Vec<u8>,
}

impl PpaassMessage {
    pub fn new(user_token: &str, payload_encryption: PpaassMessagePayloadEncryption, payload_bytes: Vec<u8>) -> Self {
        Self {
            id: generate_uuid(),
            user_token: user_token.to_owned(),
            payload_encryption,
            payload_bytes,
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_user_token(&self) -> &str {
        &self.user_token
    }

    pub fn get_payload_encryption(&self) -> &PpaassMessagePayloadEncryption {
        &self.payload_encryption
    }

    pub fn split(self) -> PpaassMessageParts {
        PpaassMessageParts {
            id: self.id,
            user_token: self.user_token,
            payload_encryption: self.payload_encryption,
            payload_bytes: self.payload_bytes,
        }
    }
}

impl From<PpaassMessageParts> for PpaassMessage {
    fn from(value: PpaassMessageParts) -> Self {
        Self {
            id: value.id,
            user_token: value.user_token,
            payload_encryption: value.payload_encryption,
            payload_bytes: value.payload_bytes,
        }
    }
}

impl TryFrom<Vec<u8>> for PpaassMessage {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let result = serde_json::from_slice(&value).context("fail to deserialize bytes to PpaassMessage object")?;
        Ok(result)
    }
}

impl TryFrom<PpaassMessage> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: PpaassMessage) -> Result<Self, Self::Error> {
        let result = serde_json::to_vec(&value).context("fail to serialize PpaassMessage object to bytes")?;
        Ok(result)
    }
}

pub struct PpaassMessageUtil;

impl PpaassMessageUtil {
    pub fn create_agent_heartbeat_request(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption) -> Result<PpaassMessage> {
        let timestamp = chrono::Utc::now().timestamp();
        let heartbeat_request = HeartbeatRequestPayload { timestamp };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::IdleHeartbeat),
            heartbeat_request.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_proxy_idle_heartbeat_response(user_token: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption) -> Result<PpaassMessage> {
        let timestamp = chrono::Utc::now().timestamp();
        let heartbeat_response = HeartbeatResponsePayload { timestamp };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::IdleHeartbeatSuccess),
            heartbeat_response.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_agent_domain_resolve_request(
        user_token: impl AsRef<str>, request_id: impl AsRef<str>, domain_name: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let domain_resolve_request = DomainResolveRequestPayload {
            request_id: request_id.as_ref().to_string(),
            domain_name: domain_name.as_ref().to_string(),
        };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::DomainNameResolve),
            domain_resolve_request.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_proxy_domain_resolve_success_response(
        user_token: impl AsRef<str>, request_id: impl AsRef<str>, domain_name: impl AsRef<str>, resolved_ip_addresses: Vec<[u8; 4]>,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let domain_resolve_response = DomainResolveResponsePayload {
            request_id: request_id.as_ref().to_string(),
            domain_name: domain_name.as_ref().to_string(),
            resolved_ip_addresses: Some(resolved_ip_addresses),
        };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::DomainNameResolveSuccess),
            domain_resolve_response.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_proxy_domain_resolve_fail_response(
        user_token: impl AsRef<str>, request_id: impl AsRef<str>, domain_name: impl AsRef<str>, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let domain_resolve_response = DomainResolveResponsePayload {
            request_id: request_id.as_ref().to_string(),
            domain_name: domain_name.as_ref().to_string(),
            resolved_ip_addresses: None,
        };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::DomainNameResolveFail),
            domain_resolve_response.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_agent_tcp_session_initialize_request(
        user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_initialize_request = TcpSessionInitRequestPayload { src_address, dest_address };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionInitialize),
            tcp_initialize_request.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_proxy_tcp_session_initialize_success_response(
        user_token: impl AsRef<str>, session_key: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_initialize_response = TcpSessionInitResponsePayload {
            session_key: Some(session_key.as_ref().to_owned()),
            src_address,
            dest_address,
        };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeSuccess),
            tcp_initialize_response.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_proxy_tcp_session_initialize_fail_response(
        user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_initialize_response = TcpSessionInitResponsePayload {
            session_key: None,
            src_address,
            dest_address,
        };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeFail),
            tcp_initialize_response.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_tcp_session_relay_data(
        user_token: impl AsRef<str>, session_key: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, data: Vec<u8>, agent: bool,
    ) -> Result<PpaassMessage> {
        let tcp_relay = TcpSessionRelayPayload {
            session_key: session_key.as_ref().to_owned(),
            src_address,
            dest_address,
            data,
            status: TcpSessionRelayStatus::Data,
        };
        let payload_type = if agent {
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionRelay)
        } else {
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionRelay)
        };
        let message_payload = PpaassMessagePayload::new(payload_type, tcp_relay.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_tcp_session_relay_complete(
        user_token: impl AsRef<str>, session_key: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, agent: bool,
    ) -> Result<PpaassMessage> {
        let tcp_relay = TcpSessionRelayPayload {
            session_key: session_key.as_ref().to_owned(),
            src_address,
            dest_address,
            data: Vec::new(),
            status: TcpSessionRelayStatus::Complete,
        };
        let payload_type = if agent {
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionRelay)
        } else {
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionRelay)
        };
        let message_payload = PpaassMessagePayload::new(payload_type, tcp_relay.try_into()?);
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }

    pub fn create_agent_tcp_session_destroy_request(
        session_key: impl AsRef<str>, user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<PpaassMessage> {
        let tcp_destroy_request = TcpSessionDestroyRequestPayload {
            session_key: session_key.as_ref().to_string(),
            src_address,
            dest_address,
        };
        let message_payload = PpaassMessagePayload::new(
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionDestroy),
            tcp_destroy_request.try_into()?,
        );
        let message = PpaassMessage::new(user_token.as_ref(), payload_encryption, message_payload.try_into()?);
        Ok(message)
    }
}
