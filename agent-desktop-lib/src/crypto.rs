use std::{collections::HashMap, sync::Arc};

use crate::error::ConfigurationItemMissedError;
use crate::error::IoError;
use crate::{config::AgentServerConfig, error::Error};
use ppaass_common::{RsaCrypto, RsaCryptoFetcher};
use snafu::{OptionExt, ResultExt};
use tracing::error;

#[derive(Debug)]
pub(crate) struct AgentServerRsaCryptoFetcher {
    cache: HashMap<String, RsaCrypto>,
}

impl AgentServerRsaCryptoFetcher {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>) -> Result<Self, Error> {
        let mut result = Self { cache: HashMap::new() };
        let rsa_dir_path = configuration.get_rsa_dir().context(ConfigurationItemMissedError { message: "rsa dir" })?;
        let rsa_dir = std::fs::read_dir(&rsa_dir_path).context(IoError {
            message: "Fail to read rsa directory.",
        })?;
        rsa_dir.for_each(|entry| {
            let entry = match entry {
                Err(e) => {
                    error!("Fail to read {} directory because of error: {:#?}", rsa_dir_path, e);
                    return;
                },
                Ok(v) => v,
            };
            let user_token = entry.file_name();
            let user_token = user_token.to_str();
            let user_token = match user_token {
                None => {
                    error!("Fail to read {}{:?} directory because of user token not exist", rsa_dir_path, entry.file_name());
                    return;
                },
                Some(v) => v,
            };
            let public_key_path = format!("{}{}/ProxyPublicKey.pem", rsa_dir_path, user_token);
            let public_key_path = std::path::Path::new(&public_key_path);
            let public_key_file = match std::fs::File::open(public_key_path) {
                Err(e) => {
                    error!("Fail to read public key file [{public_key_path:?}] because of error: {e:?}");
                    return;
                },
                Ok(v) => v,
            };
            let private_key_path = format!("{}{}/AgentPrivateKey.pem", rsa_dir_path, user_token);
            let private_key_path = std::path::Path::new(std::path::Path::new(&private_key_path));
            let private_key_file = match std::fs::File::open(private_key_path) {
                Err(e) => {
                    error!("Fail to read private key file [{private_key_path:?}] because of error: {e:?}");
                    return;
                },
                Ok(v) => v,
            };

            let rsa_crypto = match RsaCrypto::new(public_key_file, private_key_file) {
                Err(e) => {
                    error!("Fail to create rsa crypto for user: {} because of error: {:#?}", user_token, e);
                    return;
                },
                Ok(v) => v,
            };
            result.cache.insert(user_token.to_string(), rsa_crypto);
        });
        Ok(result)
    }
}

impl RsaCryptoFetcher for AgentServerRsaCryptoFetcher {
    fn fetch(&self, user_token: &str) -> Result<Option<&ppaass_common::RsaCrypto>, ppaass_common::error::Error> {
        Ok(self.cache.get(user_token))
    }
}
