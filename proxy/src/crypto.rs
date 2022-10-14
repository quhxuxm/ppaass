use std::{collections::HashMap, sync::Arc};

use ppaass_common::{PpaassError, RsaCrypto, RsaCryptoFetcher};
use tracing::error;

use crate::config::ProxyServerConfig;

pub(crate) struct ProxyServerRsaCryptoFetcher {
    cache: HashMap<String, RsaCrypto>,
}

impl ProxyServerRsaCryptoFetcher {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Result<Self, PpaassError> {
        let mut result = Self { cache: HashMap::new() };
        let rsa_dir_path = configuration.get_rsa_dir();
        let rsa_dir = std::fs::read_dir(rsa_dir_path)?;
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
            let public_key_path = std::path::Path::new(format!("{}{}/AgentPublicKey.pem", rsa_dir_path, user_token).as_str());
            let public_key_file = match std::fs::File::open(public_key_path) {
                Err(e) => {
                    error!("Fail to read public key file [{public_key_path:?}] because of error: {e:?}");
                    return;
                },
                Ok(v) => v,
            };
            let private_key_path = std::path::Path::new(std::path::Path::new(format!("{}{}/ProxyPrivateKey.pem", rsa_dir_path, user_token).as_str()));
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

impl RsaCryptoFetcher for ProxyServerRsaCryptoFetcher {
    fn fetch(&self, user_token: &str) -> Result<Option<&ppaass_common::RsaCrypto>, ppaass_common::PpaassError> {
        Ok(self.cache.get(user_token))
    }
}
