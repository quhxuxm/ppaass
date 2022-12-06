use anyhow::Result;
use ppaass_common::{generate_agent_key_pairs, generate_proxy_key_pairs};
fn main() -> Result<()> {
    generate_agent_key_pairs("./resources/rsa", "user1")?;
    generate_proxy_key_pairs("./resources/rsa", "user1")?;
    generate_agent_key_pairs("./resources/rsa", "user2")?;
    generate_proxy_key_pairs("./resources/rsa", "user2")?;
    Ok(())
}
