use std::error::Error;

use ppaass_protocol::{
    PayloadAdditionalInfoKey, PayloadAdditionalInfoValue, PpaassMessageAgentPayloadTypeValue, PpaassMessagePayload, PpaassMessagePayloadEncryption,
    PpaassMessagePayloadType, PpaassProtocolAddress,
};

#[test]
fn test_serialize_message() -> Result<(), Box<dyn Error>> {
    let message = ppaass_protocol::PpaassMessage::new("user1".to_string(), PpaassMessagePayloadEncryption::Aes(vec![1, 2, 3]), vec![1, 2, 3]);
    println!("{}", serde_json::to_string_pretty(&message)?);
    Ok(())
}

#[test]
fn test_serialize_message_payload() -> Result<(), Box<dyn Error>> {
    let source_address = PpaassProtocolAddress::IpV4 { ip: [1, 2, 3, 4], port: 80 };
    let target_address = PpaassProtocolAddress::IpV4 {
        ip: [10, 20, 30, 40],
        port: 800,
    };
    let mut message_payload = PpaassMessagePayload::new(
        Some(source_address),
        Some(target_address),
        PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize),
        vec![1; 100],
    );
    message_payload.add_additional_info(
        PayloadAdditionalInfoKey::ReferenceMessageId,
        PayloadAdditionalInfoValue::ReferenceMessageIdValue("reference_message_id_01".to_string()),
    );
    message_payload.add_additional_info(
        PayloadAdditionalInfoKey::ReferenceMessageId,
        PayloadAdditionalInfoValue::ReferenceMessageIdValue("reference_message_id_02".to_string()),
    );
    println!("{}", serde_json::to_string_pretty(&message_payload)?);
    Ok(())
}
