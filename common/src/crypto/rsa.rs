use std::{fmt::Debug, path::Path};
use std::{fs, io::Read};

use crate::error::IoError;
use crate::error::RsaPrivateKeyError;
use crate::error::RsaPublicKeyError;
use crate::error::{Error, RsaCryptoError};

use pretty_hex::pretty_hex;
use rand::rngs::OsRng;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::{PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey};
use snafu::ResultExt;
const DEFAULT_AGENT_PRIVATE_KEY_PATH: &str = "AgentPrivateKey.pem";
const DEFAULT_AGENT_PUBLIC_KEY_PATH: &str = "AgentPublicKey.pem";
const DEFAULT_PROXY_PRIVATE_KEY_PATH: &str = "ProxyPrivateKey.pem";
const DEFAULT_PROXY_PUBLIC_KEY_PATH: &str = "ProxyPublicKey.pem";

/// The rsa crypto fetcher,
/// each player have a rsa crypto
/// which can be fund from the storage
/// with user token
pub trait RsaCryptoFetcher {
    /// Fetch the rsa crypto by user token
    fn fetch(&self, user_token: &str) -> Result<Option<&RsaCrypto>, Error>;
}

/// The util to do RSA encryption and decryption.
#[derive(Debug)]
pub struct RsaCrypto {
    /// The private used to do decryption
    private_key: RsaPrivateKey,
    /// The public used to do encryption
    public_key: RsaPublicKey,
}

impl RsaCrypto {
    pub fn new<A, B>(mut public_key_read: A, mut private_key_read: B) -> Result<Self, Error>
    where
        A: Read + Debug,
        B: Read + Debug,
    {
        let mut public_key_string = String::new();
        public_key_read.read_to_string(&mut public_key_string).context(IoError {
            message: format!("{public_key_read:?}"),
        })?;
        let public_key = RsaPublicKey::from_public_key_pem(&public_key_string).context(RsaPublicKeyError { message: public_key_string })?;
        let mut private_key_string = String::new();
        private_key_read.read_to_string(&mut private_key_string).context(IoError {
            message: format!("{private_key_read:?}"),
        })?;
        let private_key = RsaPrivateKey::from_pkcs8_pem(&private_key_string).context(RsaPrivateKeyError { message: private_key_string })?;
        Ok(Self { public_key, private_key })
    }

    pub fn encrypt(&self, target: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(self
            .public_key
            .encrypt(&mut OsRng, PaddingScheme::PKCS1v15Encrypt, target.as_ref())
            .context(RsaCryptoError { message: pretty_hex(&target) })?)
    }

    pub fn decrypt(&self, target: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(self
            .private_key
            .decrypt(PaddingScheme::PKCS1v15Encrypt, target.as_ref())
            .context(RsaCryptoError { message: pretty_hex(&target) })?)
    }
}

pub fn generate_agent_key_pairs(base_dir: &str, user_token: &str) -> Result<(), Error> {
    let private_key_path = format!("{}/{}/{}", base_dir, user_token, DEFAULT_AGENT_PRIVATE_KEY_PATH);
    let private_key_path = Path::new(private_key_path.as_str());
    let public_key_path = format!("{}/{}/{}", base_dir, user_token, DEFAULT_AGENT_PUBLIC_KEY_PATH);
    let public_key_path = Path::new(public_key_path.as_str());
    generate_rsa_key_pairs(private_key_path, public_key_path)
}

pub fn generate_proxy_key_pairs(base_dir: &str, user_token: &str) -> Result<(), Error> {
    let private_key_path = format!("{}/{}/{}", base_dir, user_token, DEFAULT_PROXY_PRIVATE_KEY_PATH);
    let private_key_path = Path::new(private_key_path.as_str());
    let public_key_path = format!("{}/{}/{}", base_dir, user_token, DEFAULT_PROXY_PUBLIC_KEY_PATH);
    let public_key_path = Path::new(public_key_path.as_str());
    generate_rsa_key_pairs(private_key_path, public_key_path)
}

fn generate_rsa_key_pairs(private_key_path: &Path, public_key_path: &Path) -> Result<(), Error> {
    let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("Fail to generate private key");
    let public_key = RsaPublicKey::from(&private_key);
    let private_key_pem = private_key.to_pkcs8_pem(LineEnding::CRLF).expect("Fail to generate pem for private key.");
    let public_key_pem = public_key.to_public_key_pem(LineEnding::CRLF).expect("Fail to generate pem for public key.");
    match private_key_path.parent() {
        None => {
            println!("Write private key: {:?}", private_key_path.to_str());
            fs::write(private_key_path, private_key_pem.as_bytes()).context(IoError {
                message: "Fail to write rsa private key content.",
            })?;
        },
        Some(parent) => {
            if !parent.exists() {
                println!("Create parent directory :{:?}", parent.to_str());
                fs::create_dir_all(parent).context(IoError {
                    message: "Fail to create parent directory to save private key.",
                })?;
            }
            println!("Write private key: {:?}", private_key_path.to_str());
            fs::write(private_key_path, private_key_pem.as_bytes()).context(IoError {
                message: "Fail to write rsa private key content.",
            })?;
        },
    };
    match public_key_path.parent() {
        None => {
            println!("Write public key: {:?}", public_key_path.to_str());
            fs::write(public_key_path, public_key_pem.as_bytes()).context(IoError {
                message: "Fail to write rsa public key content.",
            })?;
        },
        Some(parent) => {
            if !parent.exists() {
                println!("Create parent directory :{:?}", parent.to_str());
                fs::create_dir_all(parent).context(IoError {
                    message: "Fail to create parent directory to save public key.",
                })?;
            }
            println!("Write public key: {:?}", public_key_path.to_str());
            fs::write(public_key_path, public_key_pem.as_bytes()).context(IoError {
                message: "Fail to write rsa public key content.",
            })?;
        },
    };
    Ok(())
}
