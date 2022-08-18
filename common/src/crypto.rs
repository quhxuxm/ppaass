use std::fs;
use std::path::Path;

use aes::Aes256;
use blowfish::Blowfish;
use bytes::Bytes;
use cipher::{block_padding::Pkcs7, BlockEncryptMut};
use cipher::{BlockDecryptMut, KeyInit};

use rand::rngs::OsRng;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::{PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey};
use tracing::error;

use crate::PpaassError;

const AGENT_PRIVATE_KEY_PATH: &str = "AgentPrivateKey.pem";
const AGENT_PUBLIC_KEY_PATH: &str = "AgentPublicKey.pem";
const PROXY_PRIVATE_KEY_PATH: &str = "ProxyPrivateKey.pem";
const PROXY_PUBLIC_KEY_PATH: &str = "ProxyPublicKey.pem";

type AesEncryptor = ecb::Encryptor<Aes256>;
type AesDecryptor = ecb::Decryptor<Aes256>;
type BlowfishEncryptor = ecb::Encryptor<Blowfish>;
type BlowfishDecryptor = ecb::Decryptor<Blowfish>;

type PaddingMode = Pkcs7;

/// The rsa crypto fetcher,
/// each player have a rsa crypto
/// which can be fund from the storage
/// with user token
pub trait RsaCryptoFetcher {
    /// Fetch the rsa crypto by user token
    fn fetch<Q>(&self, user_token: Q) -> Result<Option<&RsaCrypto>, PpaassError>
    where
        Q: AsRef<str>;
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
    pub fn new<PU, PR>(public_key: PU, private_key: PR) -> Result<Self, PpaassError>
    where
        PU: AsRef<str>,
        PR: AsRef<str>,
    {
        let public_key = match RsaPublicKey::from_public_key_pem(public_key.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                error!("Fail to parse rsa key because of error: {:#?}", e);
                return Err(PpaassError::CodecError);
            },
        };
        let private_key = match RsaPrivateKey::from_pkcs8_pem(private_key.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                error!("Fail to parse rsa key because of error: {:#?}", e);
                return Err(PpaassError::CodecError);
            },
        };
        Ok(Self { public_key, private_key })
    }

    pub fn encrypt(&self, target: &Bytes) -> Result<Bytes, PpaassError> {
        self.public_key
            .encrypt(&mut OsRng, PaddingScheme::PKCS1v15Encrypt, target)
            .map_err(|e| {
                error!("Fail to encrypt data with rsa because of error: {:#?}", e);
                PpaassError::CodecError
            })
            .map(|v| v.into())
    }

    pub fn decrypt(&self, target: &Bytes) -> Result<Bytes, PpaassError> {
        self.private_key
            .decrypt(PaddingScheme::PKCS1v15Encrypt, target)
            .map_err(|e| {
                error!("Fail to decrypt data with rsa because of error: {:#?}", e);
                PpaassError::CodecError
            })
            .map(|v| v.into())
    }
}

pub fn encrypt_with_blowfish(encryption_token: &[u8], target: &[u8]) -> Vec<u8> {
    let encryptor = BlowfishEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target)
}

pub fn decrypt_with_blowfish<'a>(encryption_token: &[u8], target: &[u8]) -> Result<Vec<u8>, PpaassError> {
    let decryptor = BlowfishDecryptor::new(encryption_token.into());
    let result = match decryptor.decrypt_padded_vec_mut::<PaddingMode>(target) {
        Ok(result) => result,
        Err(_) => return Err(PpaassError::CodecError),
    };
    Ok(result)
}

pub fn encrypt_with_aes(encryption_token: &[u8], target: &[u8]) -> Vec<u8> {
    let encryptor = AesEncryptor::new(encryption_token.into());
    encryptor.encrypt_padded_vec_mut::<PaddingMode>(target)
}

pub fn decrypt_with_aes<'a>(encryption_token: &[u8], target: &[u8]) -> Result<Vec<u8>, PpaassError> {
    let decryptor = AesDecryptor::new(encryption_token.into());
    let result = match decryptor.decrypt_padded_vec_mut::<PaddingMode>(target) {
        Ok(result) => result,
        Err(_) => return Err(PpaassError::CodecError),
    };
    Ok(result)
}

pub fn generate_agent_key_pairs(base_dir: &str, user_token: &str) -> Result<(), PpaassError> {
    let private_key_path = format!("{}/{}/{}", base_dir, user_token, AGENT_PRIVATE_KEY_PATH);
    let private_key_path = Path::new(private_key_path.as_str());
    let public_key_path = format!("{}/{}/{}", base_dir, user_token, AGENT_PUBLIC_KEY_PATH);
    let public_key_path = Path::new(public_key_path.as_str());
    generate_rsa_key_pairs(private_key_path, public_key_path)
}

pub fn generate_proxy_key_pairs(base_dir: &str, user_token: &str) -> Result<(), PpaassError> {
    let private_key_path = format!("{}/{}/{}", base_dir, user_token, PROXY_PRIVATE_KEY_PATH);
    let private_key_path = Path::new(private_key_path.as_str());
    let public_key_path = format!("{}/{}/{}", base_dir, user_token, PROXY_PUBLIC_KEY_PATH);
    let public_key_path = Path::new(public_key_path.as_str());
    generate_rsa_key_pairs(private_key_path, public_key_path)
}

fn generate_rsa_key_pairs(private_key_path: &Path, public_key_path: &Path) -> Result<(), PpaassError> {
    let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("Fail to generate private key");
    let public_key = RsaPublicKey::from(&private_key);
    let private_key_pem = private_key.to_pkcs8_pem(LineEnding::CRLF).expect("Fail to generate pem for private key.");
    let public_key_pem = public_key.to_public_key_pem(LineEnding::CRLF).expect("Fail to generate pem for public key.");
    match private_key_path.parent() {
        None => {
            println!("Write private key: {:?}", private_key_path.to_str());
            fs::write(private_key_path, private_key_pem.as_bytes())?;
        },
        Some(parent) => {
            if !parent.exists() {
                println!("Create parent directory :{:?}", parent.to_str());
                fs::create_dir_all(parent)?;
            }
            println!("Write private key: {:?}", private_key_path.to_str());
            fs::write(private_key_path, private_key_pem.as_bytes())?;
        },
    };
    match public_key_path.parent() {
        None => {
            println!("Write public key: {:?}", public_key_path.to_str());
            fs::write(public_key_path, public_key_pem.as_bytes())?;
        },
        Some(parent) => {
            if !parent.exists() {
                println!("Create parent directory :{:?}", parent.to_str());
                fs::create_dir_all(parent)?;
            }
            println!("Write public key: {:?}", public_key_path.to_str());
            fs::write(public_key_path, public_key_pem.as_bytes())?;
        },
    };
    Ok(())
}
