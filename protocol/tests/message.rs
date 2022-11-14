use std::error::Error;

use ppaass_protocol::{
    tcp_initialize::TcpInitializeRequestPayload, PpaassMessageAgentPayloadTypeValue, PpaassMessagePayload, PpaassMessagePayloadEncryption,
    PpaassMessagePayloadParts, PpaassMessagePayloadType, PpaassNetAddress,
};

#[test]
fn test_serialize_message() -> Result<(), Box<dyn Error>> {
    let message = ppaass_protocol::PpaassMessage::new("user1", PpaassMessagePayloadEncryption::Aes(vec![1, 2, 3]), vec![1, 2, 3]);
    println!("{}", serde_json::to_string_pretty(&message)?);
    Ok(())
}

#[test]
fn test_serialize_message_payload() -> Result<(), Box<dyn Error>> {
    let src_address = PpaassNetAddress::IpV4 { ip: [1, 2, 3, 4], port: 80 };
    let dest_address = PpaassNetAddress::IpV4 {
        ip: [10, 20, 30, 40],
        port: 800,
    };
    let tcp_initialize_payload = TcpInitializeRequestPayload { src_address, dest_address };

    let message_payload = PpaassMessagePayload::new(
        PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionInitialize),
        tcp_initialize_payload.try_into()?,
    );

    println!("{}", serde_json::to_string_pretty(&message_payload)?);
    let PpaassMessagePayloadParts { data, .. } = message_payload.split();
    println!("{:?}", data);
    Ok(())
}
