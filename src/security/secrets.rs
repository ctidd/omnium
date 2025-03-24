use data_encoding::BASE64;
use ring::rand::{self, SecureRandom};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct OmniumServiceSecret {
    pub value: String,
}

pub fn create_service_secret() -> anyhow::Result<OmniumServiceSecret> {
    let rng = rand::SystemRandom::new();
    let mut new_secret_value = [0u8; 64]; // HS512 secret length
    rng.fill(&mut new_secret_value)?;

    Ok(OmniumServiceSecret {
        value: BASE64.encode(new_secret_value.as_slice()),
    })
}
