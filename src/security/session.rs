use std::time::Duration;

use axum::extract::{MatchedPath, State};
use axum_extra::extract::CookieJar;

use axum::{extract::Request, http::StatusCode, middleware::Next};
use jsonwebtoken::{DecodingKey, EncodingKey};
use log::info;
use serde::{Deserialize, Serialize};

use crate::api::response::{JsonResponse, ResponseError};
use crate::security::claims::decode_claims;
use crate::security::claims::{encode_claims, expires_in};
use crate::security::secrets::ServiceSecret;

pub const SESSION_CLAIMS_TYPE: &str = "session";

pub trait SessionManager<U> {
    fn get_service_secret(
        &self,
    ) -> impl std::future::Future<Output = anyhow::Result<&ServiceSecret>> + Send;

    fn get_account(
        &self,
        account_id: String,
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
    account_id: &str,
    encoding_key: &EncodingKey,
    duration: Duration,
) -> anyhow::Result<String> {
    encode_claims(
        &SessionClaims {
            sub: String::from(account_id),
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
            .and_then(|header| header.strip_prefix("Bearer "))
            .map(|token| Credential(token.to_string()))
    }

    pub fn from_cookie(cookies: &CookieJar) -> Option<Credential> {
        cookies
            .get("__Host-omn-sess")
            .and_then(|cookie| Some(cookie.value_trimmed()))
            .map(|header| Credential(header.into()))
    }
}

pub async fn authenticate<U: Clone + Send + Sync + 'static, S: SessionManager<U>>(
    request: Request,
    next: Next,
) -> core::result::Result<axum::response::Response, ResponseError> {
    if request.extensions().get::<U>().is_some() {
        Ok(next.run(request).await)
    } else {
        info!("Unauthorized! Authentication was required.");
        Err(JsonResponse::of_status(StatusCode::UNAUTHORIZED).into())
    }
}

pub async fn decorate<U: Clone + Send + Sync + 'static, S: SessionManager<U>>(
    State(session_manager): State<S>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> core::result::Result<axum::response::Response, ResponseError> {
    let path = request.extensions().get::<MatchedPath>();
    match path {
        Some(path) => info!("Authorizing path: {}", path.as_str()),
        None => info!("Authorizing path: {}", "No matched path"),
    }

    // Extract credential from either session cookie or authorization header:
    let credential = session_manager
        .extract_credential(&request, &cookies)
        .map(|credential| credential.0);

    // Authenticate using the credential:
    if let Some(credential) = credential {
        if let Ok(decoded) = decode_claims::<SessionClaims>(
            &credential,
            &DecodingKey::from_secret(session_manager.get_service_secret().await?.value.as_bytes()),
        ) {
            if decoded.claims.omn_cl_typ != SESSION_CLAIMS_TYPE {
                info!("Authentication failed! Illegal claims type.");
                return Ok(next.run(request).await);
            }

            let account_id = decoded.claims.sub;

            let lookup = session_manager.get_account(account_id).await?;

            match lookup {
                Some(account) => {
                    request.extensions_mut().insert::<U>(account);
                    info!("Inserted account to request extensions...");
                }
                None => {
                    info!("Authentication failed! Account lookup returned no result.");
                    return Ok(next.run(request).await);
                }
            }
        } else {
            info!("Authentication failed! Unable to decode claims from credential.");
            return Ok(next.run(request).await);
        }
    } else {
        info!("Authentication failed! No credential in request.");
        return Ok(next.run(request).await);
    }

    Ok(next.run(request).await)
}
