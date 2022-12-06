#![cfg(test)]
use anyhow::Result;
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryption, PpaassNetAddress};

#[test]
fn test() -> Result<()> {
    let src_address = PpaassNetAddress::IpV4 { ip: [1, 1, 1, 1], port: 80 };
    let dest_address = PpaassNetAddress::IpV4 { ip: [2, 2, 2, 2], port: 90 };
    let payload_encryption = PpaassMessagePayloadEncryption::Aes(vec![0, 0, 0]);
    let message = PpaassMessageGenerator::generate_tcp_loop_init_request("user1", src_address, dest_address, payload_encryption);
    Ok(())
}
