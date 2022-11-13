use std::{collections::HashMap, sync::Arc};

use crate::config::ProxyServerConfig;
use anyhow::{Context, Result};
use ppaass_common::{RsaCrypto, RsaCryptoFetcher};

use tracing::error;

#[derive(Debug)]
pub(crate) struct ProxyServerRsaCryptoFetcher {
    cache: HashMap<String, RsaCrypto>,
}

impl ProxyServerRsaCryptoFetcher {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Result<Self> {
        let mut result = Self { cache: HashMap::new() };
        let rsa_dir_path = configuration.get_rsa_dir();
        let rsa_dir = std::fs::read_dir(&rsa_dir_path).context(format!("fail to read rsa directory: {rsa_dir_path}"))?;
        rsa_dir.for_each(|entry| {
            let Ok(entry) = entry else{
                error!("fail to read {rsa_dir_path} directory");
                return;
            };
            let user_token = entry.file_name();
            let user_token = user_token.to_str();
            let Some(user_token) =user_token else{
                error!("fail to read {}{:?} directory because of user token not exist", rsa_dir_path, entry.file_name());
                return;
            };
            let public_key_path = format!("{}{}/AgentPublicKey.pem", rsa_dir_path, user_token);
            let public_key_path = std::path::Path::new(&public_key_path);
            let Ok(public_key_file) =  std::fs::File::open(public_key_path) else {
                  error!("Fail to read public key file: {public_key_path:?}.");
                    return;
            };
            let private_key_path = format!("{}{}/ProxyPrivateKey.pem", rsa_dir_path, user_token);
            let private_key_path = std::path::Path::new(std::path::Path::new(&private_key_path));
            let Ok(private_key_file) =  std::fs::File::open(private_key_path) else{
               error!("Fail to read private key file :{private_key_path:?}.");
                    return;
            };

            let Ok(rsa_crypto) =  RsaCrypto::new(public_key_file, private_key_file) else{
               error!("Fail to create rsa crypto for user: {user_token}.");
                    return;
            };
            result.cache.insert(user_token.to_string(), rsa_crypto);
        });
        Ok(result)
    }
}

impl RsaCryptoFetcher for ProxyServerRsaCryptoFetcher {
    fn fetch(&self, user_token: impl AsRef<str>) -> Result<Option<&ppaass_common::RsaCrypto>> {
        Ok(self.cache.get(user_token.as_ref()))
    }
}
