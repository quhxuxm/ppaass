#![cfg(test)]
use anyhow::Result;
use ppaass_common::{
    tcp_loop::TcpLoopInitRequestPayload, PpaassMessageGenerator, PpaassMessagePayloadEncryption, PpaassMessageProxyPayload, PpaassMessageProxyPayloadType,
    PpaassNetAddress,
};

#[test]
fn test_ppaass_net_address() -> Result<()> {
    let ipv4_address = PpaassNetAddress::IpV4 { ip: [1, 1, 1, 1], port: 80 };
    let ipv6_address = PpaassNetAddress::IpV6 {
        ip: [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
        port: 81,
    };
    let domain_address = PpaassNetAddress::Domain {
        host: "www.google.com".to_string(),
        port: 82,
    };
    let result_string = serde_json::to_string_pretty(&ipv4_address)?;
    println!("{result_string}");
    let result_string = serde_json::to_string_pretty(&ipv6_address)?;
    println!("{result_string}");
    let result_string = serde_json::to_string_pretty(&domain_address)?;
    println!("{result_string}");
    Ok(())
}

#[test]
fn test_ppaass_message_payload() -> Result<()> {
    let payload = PpaassMessageProxyPayload::new(PpaassMessageProxyPayloadType::DomainNameResolve, vec![1, 2, 3, 4, 5]);

    let result_string = serde_json::to_string_pretty(&payload)?;
    println!("{result_string}");
    Ok(())
}

#[test]
fn test_ppaass_message() -> Result<()> {
    let src_address = PpaassNetAddress::IpV4 { ip: [1, 1, 1, 1], port: 80 };
    let dest_address = PpaassNetAddress::IpV4 { ip: [2, 2, 2, 2], port: 90 };

    let payload_encryption = PpaassMessagePayloadEncryption::Aes(vec![0, 0, 0]);
    let message = PpaassMessageGenerator::generate_tcp_loop_init_request("user1", src_address, dest_address, payload_encryption)?;
    let result_string = serde_json::to_string_pretty(&message)?;
    println!("{result_string}");
    Ok(())
}

#[test]
fn test_payload_encryption() -> Result<()> {
    let payload_encryption = PpaassMessagePayloadEncryption::Aes(vec![0, 0, 0]);
    let result_string = serde_json::to_string_pretty(&payload_encryption)?;
    println!("{result_string}");
    Ok(())
}

#[test]
fn test_tcp_loop_init_request() -> Result<()> {
    let src_address = PpaassNetAddress::IpV4 { ip: [1, 1, 1, 1], port: 80 };
    let dest_address = PpaassNetAddress::Domain {
        host: "www.google.com".to_string(),
        port: 90,
    };

    let tcp_loop_init_request = TcpLoopInitRequestPayload { src_address, dest_address };

    let result_string = serde_json::to_string_pretty(&tcp_loop_init_request)?;
    println!("{result_string}");
    Ok(())
}
