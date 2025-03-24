use std::ops::Add;
use std::time::{Duration, SystemTime};

use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};

pub fn encode_claims<T: Serialize>(
    claims: &T,
    encoding_key: &EncodingKey,
) -> anyhow::Result<String> {
    let result = encode::<T>(&Header::new(Algorithm::HS512), &claims, &encoding_key)?;
    Ok(result)
}

pub fn decode_claims<T: for<'a> Deserialize<'a>>(
    token: &str,
    decoding_key: &DecodingKey,
) -> anyhow::Result<TokenData<T>> {
    let mut validation_config = Validation::new(Algorithm::HS512);
    validation_config.set_required_spec_claims(&["sub", "exp"]);
    let result = decode::<T>(&token, &decoding_key, &validation_config)?;
    Ok(result)
}

pub fn expires_in(duration: Duration) -> anyhow::Result<usize> {
    Ok(usize::try_from(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .add(duration)
            .as_secs(),
    )?)
}
