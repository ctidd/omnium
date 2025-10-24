use std::{
    ops::Add,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use axum::extract::{MatchedPath, State};
use axum_extra::extract::CookieJar;

use axum::{extract::Request, http::StatusCode, middleware::Next};
use log::info;
use serde::{Deserialize, Serialize};

use crate::api::response::{JsonResponse, ResponseError};

pub const SESSION_CLAIMS_TYPE: &str = "session";

#[async_trait]
pub trait SessionManager<U> {
    async fn decode_claims(&self, token: Credential) -> anyhow::Result<SessionClaims>;

    async fn get_account(&self, account_id: String) -> anyhow::Result<Option<U>>;

    fn extract_credential(&self, request: &Request, cookies: &CookieJar) -> Option<Credential>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionClaims {
    pub sub: String,
    pub exp: usize,
    pub omn_cl_typ: String,
}

impl SessionClaims {
    pub fn expires_in(duration: Duration) -> anyhow::Result<usize> {
        Ok(usize::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .add(duration)
                .as_secs(),
        )?)
    }

    pub fn new(account_id: &str, expires_in: Duration) -> anyhow::Result<SessionClaims> {
        Ok(SessionClaims {
            sub: String::from(account_id),
            exp: SessionClaims::expires_in(expires_in)?,
            omn_cl_typ: SESSION_CLAIMS_TYPE.into(),
        })
    }
}

#[derive(Clone)]
pub struct Credential(pub String);

impl Credential {
    pub fn from_authorization_header(request: &Request) -> Option<Credential> {
        request
            .headers()
            .get("authorization")
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.strip_prefix("Bearer "))
            .map(|token| Credential(token.to_string()))
    }

    pub fn from_cookie(cookie_name: &str, cookies: &CookieJar) -> Option<Credential> {
        cookies
            .get(cookie_name)
            .and_then(|cookie| Some(cookie.value_trimmed()))
            .map(|header| Credential(header.into()))
    }
}

pub async fn authorize<U: Clone + Send + Sync + 'static, S: SessionManager<U>>(
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

pub async fn resolve<U: Clone + Send + Sync + 'static, S: SessionManager<U>>(
    State(session_manager): State<S>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> core::result::Result<axum::response::Response, ResponseError> {
    let path = request.extensions().get::<MatchedPath>();
    match path {
        Some(path) => info!("Authenticating path: {}", path.as_str()),
        None => info!("Authenticating path: {}", "No matched path"),
    }

    let credential = session_manager.extract_credential(&request, &cookies);

    if let Some(credential) = credential {
        if let Ok(decoded) = session_manager.decode_claims(credential).await {
            if decoded.omn_cl_typ != SESSION_CLAIMS_TYPE {
                info!("Account resolve failed! Illegal claims type.");
                return Ok(next.run(request).await);
            }

            let account_id = decoded.sub;

            let lookup = session_manager.get_account(account_id).await?;

            match lookup {
                Some(account) => {
                    request.extensions_mut().insert::<U>(account);
                    info!("Inserted account to request extensions...");
                }
                None => {
                    info!("Account resolve failed! Account lookup returned no result.");
                    return Ok(next.run(request).await);
                }
            }
        } else {
            info!("Account resolve failed! Unable to decode claims.");
            return Ok(next.run(request).await);
        }
    } else {
        info!("Account resolve skipped: No credential in request.");
        return Ok(next.run(request).await);
    }

    Ok(next.run(request).await)
}
