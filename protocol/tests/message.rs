use std::error::Error;

use ppaass_protocol::PpaassMessagePayloadEncryption;

#[test]
fn test_serialize_message() -> Result<(), Box<dyn Error>> {
    let message = ppaass_protocol::PpaassMessage::new("user1".to_string(), PpaassMessagePayloadEncryption::Aes(vec![1, 2, 3]), vec![1, 2, 3]);
    println!("{}", serde_json::to_string(&message)?);
    Ok(())
}
