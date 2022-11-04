use async_trait::async_trait;

use crate::error::Error;

pub(crate) mod dispatcher;

mod http;
mod socks;

#[async_trait]
pub(crate) trait ClientFlow {
    async fn exec(&mut self) -> Result<(), Error>;
}
