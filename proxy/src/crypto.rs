use std::{
    collections::HashMap,
    fs::{read_dir, File},
    path::Path,
    sync::Arc,
};

use crate::config::ProxyServerConfig;

use anyhow::anyhow;
use ppaass_common::{RsaCrypto, RsaCryptoFetcher, RsaError};

use tracing::error;

#[derive(Debug)]
pub(crate) struct ProxyServerRsaCryptoFetcher {
    cache: HashMap<String, RsaCrypto>,
}

impl ProxyServerRsaCryptoFetcher {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Result<Self, RsaError> {
        let mut result = Self { cache: HashMap::new() };
        let rsa_dir_path = configuration.get_rsa_dir();
        let rsa_dir = read_dir(&rsa_dir_path).map_err(|e| RsaError::Other(anyhow!(e)))?;
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
            let public_key_path = format!("{rsa_dir_path}{user_token}/AgentPublicKey.pem");
            let public_key_path = Path::new(&public_key_path);
            let Ok(public_key_file) =  File::open(public_key_path) else {
                error!("Fail to read public key file: {public_key_path:?}.");
                return;
            };
            let private_key_path = format!("{rsa_dir_path}{user_token}/ProxyPrivateKey.pem");
            let private_key_path = Path::new(Path::new(&private_key_path));
            let Ok(private_key_file) =  File::open(private_key_path) else {
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
    fn fetch(&self, user_token: impl AsRef<str>) -> Result<Option<&RsaCrypto>, RsaError> {
        Ok(self.cache.get(user_token.as_ref()))
    }
}
