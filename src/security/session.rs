use std::time::Duration;

use axum::extract::{MatchedPath, State};
use axum_extra::extract::CookieJar;

use axum::{extract::Request, http::StatusCode, middleware::Next};
use jsonwebtoken::{DecodingKey, EncodingKey};
use log::info;
use serde::{Deserialize, Serialize};

use crate::api::responses::{Response, Result};
use crate::security::claims::decode_claims;
use crate::security::claims::{encode_claims, expires_in};
use crate::security::secrets::ServiceSecret;

pub const SESSION_CLAIMS_TYPE: &str = "session";

pub trait SessionState<U> {
    fn service_secret(
        &self,
    ) -> impl std::future::Future<Output = anyhow::Result<&ServiceSecret>> + Send;

    fn user_lookup(
        &self,
        user_id: String,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<U>>> + Send;

    fn extract_credential(&self, request: &Request, cookies: &CookieJar) -> Option<Credential>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionClaims {
    pub sub: String,
    pub exp: usize,
    pub omn_cl_typ: String,
}

pub fn create_session(
    user_id: &str,
    encoding_key: &EncodingKey,
    duration: Duration,
) -> anyhow::Result<String> {
    encode_claims(
        &SessionClaims {
            sub: String::from(user_id),
            exp: expires_in(duration)?,
            omn_cl_typ: SESSION_CLAIMS_TYPE.into(),
        },
        encoding_key,
    )
}

#[derive(Clone)]
pub struct Credential(String);

impl Credential {
    pub fn from_authorization_header(request: &Request) -> Option<Credential> {
        request
            .headers()
            .get("authorization")
            .and_then(|header| header.to_str().ok())
            .map(|header| Credential(header.into()))
    }

    pub fn from_cookie(cookies: &CookieJar) -> Option<Credential> {
        cookies
            .get("__Host-omn-sess")
            .and_then(|cookie| Some(cookie.value_trimmed()))
            .map(|header| Credential(header.into()))
    }
}

pub async fn authenticate<U: Clone + Send + Sync + 'static, S: SessionState<U>>(
    State(state): State<S>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result {
    let path = request.extensions().get::<MatchedPath>();
    match path {
        Some(path) => info!("Authorizing path: {}", path.as_str()),
        None => info!("Authorizing path: {}", "No matched path"),
    }

    // Extract credential from either session cookie or authorization header:
    let credential = state
        .extract_credential(&request, &cookies)
        .map(|credential| credential.0);

    // Authenticate using the credential:
    if let Some(credential) = credential {
        if let Ok(decoded) = decode_claims::<SessionClaims>(
            &credential,
            &DecodingKey::from_secret(state.service_secret().await?.value.as_bytes()),
        ) {
            if decoded.claims.omn_cl_typ != SESSION_CLAIMS_TYPE {
                info!("Authentication rejected! Illegal claims type.");
                return Response::status(StatusCode::UNAUTHORIZED).into();
            }

            let user_id = decoded.claims.sub;

            let lookup = state.user_lookup(user_id).await?;

            match lookup {
                Some(user) => {
                    request.extensions_mut().insert::<U>(user);
                    info!("Inserted user to request extensions...");
                }
                None => {
                    info!("Authentication rejected! User lookup returned no result.");
                    return Response::status(StatusCode::UNAUTHORIZED).into();
                }
            }
        } else {
            info!("Authentication rejected! Unable to decode claims from credential.");
            return Response::status(StatusCode::UNAUTHORIZED).into();
        }
    } else {
        info!("Authentication rejected! No credential in request.");
        return Response::status(StatusCode::UNAUTHORIZED).into();
    }

    Ok(next.run(request).await)
}
