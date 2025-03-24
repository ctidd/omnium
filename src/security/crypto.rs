use std::io::Read;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{bail, Error};
use data_encoding::BASE64;
use ring::rand::{self, SecureRandom};

pub fn encrypt_string_aes256_gcm(plain_text: &str, secret: &str) -> anyhow::Result<String> {
    // Fill a 256-bit key:
    let mut key = [0u8; 32];
    secret.as_bytes().read_exact(&mut key)?; // A service secret is longer than needed for Aes256Gcm

    let rng = rand::SystemRandom::new();

    // Generate 96-bit nonce for Aes256Gcm:
    let mut nonce = [0u8; 12];
    rng.fill(&mut nonce)?;
    let nonce = Nonce::from_slice(&nonce);

    // Encrypt using 256-bit key and 96-bit nonce:
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| Error::msg("Failed to create cipher!"))?;
    let cipher_text = cipher.encrypt(nonce, plain_text.as_bytes());

    if cipher_text.is_err() {
        bail!("Failed to encrypt!");
    }

    // Combine the nonce and cipher text and encode in base64:
    let mut result = Vec::new();
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&cipher_text.unwrap());
    Ok(BASE64.encode(&result))
}

pub fn decrypt_string_aes256_gcm(encrypted_text: &str, secret: &str) -> anyhow::Result<String> {
    let mut key = [0u8; 32];
    secret.as_bytes().read_exact(&mut key)?; // A service secret is longer than needed for Aes256Gcm

    let data = BASE64.decode(encrypted_text.as_bytes())?;

    let (nonce, cipher_text) = data.split_at(12);

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| Error::msg("Failed to create cipher!"))?;
    let result = cipher.decrypt(Nonce::from_slice(nonce), cipher_text);

    if result.is_err() {
        bail!("Failed to decrypt!");
    }

    Ok(String::from_utf8(result.unwrap())?)
}
