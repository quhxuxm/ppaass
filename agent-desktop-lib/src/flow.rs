use async_trait::async_trait;



pub(crate) mod dispatcher;
use anyhow::Result;
mod http;
mod socks;

#[async_trait]
pub(crate) trait ClientFlow {
    async fn exec(&mut self) -> Result<()>;
}
