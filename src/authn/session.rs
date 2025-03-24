use std::time::Duration;

use axum::extract::{MatchedPath, State};
use axum_extra::extract::CookieJar;

use axum::{extract::Request, http::StatusCode, middleware::Next};
use jsonwebtoken::{DecodingKey, EncodingKey};
use serde::{Deserialize, Serialize};

use crate::api::responses::{JsonResponse, JsonResult};
use crate::authn::claims::decode_claims;
use crate::authn::claims::{encode_claims, expires_in};
use crate::authn::secrets::OmniumSessionSecret;

pub const SESSION_CLAIMS_TYPE: &str = "session";

pub trait OmniumState<U> {
    fn session_secret(
        &self,
    ) -> impl std::future::Future<Output = anyhow::Result<&OmniumSessionSecret>> + Send;

    fn user_lookup(
        &self,
        user_id: String,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<U>>> + Send;
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

pub async fn authenticate<U: Clone + Send + Sync + 'static, S: OmniumState<U>>(
    State(state): State<S>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> JsonResult {
    let path = request.extensions().get::<MatchedPath>();
    match path {
        Some(path) => println!("Authorizing path: {}", path.as_str()),
        None => println!("Authorizing path: {}", "No matched path"),
    }

    // Extract credential from either session cookie or authorization header:
    let credential = cookies
        .get("__Host-session")
        .and_then(|cookie| Some(cookie.value_trimmed()))
        .or_else(|| {
            request
                .headers()
                .get("authorization")
                .and_then(|header| header.to_str().ok())
        });

    // Authenticate using the credential:
    if let Some(credential) = credential {
        if let Ok(decoded) = decode_claims::<SessionClaims>(
            &credential,
            &DecodingKey::from_secret(state.session_secret().await?.value.as_bytes()),
        ) {
            if decoded.claims.omn_cl_typ != SESSION_CLAIMS_TYPE {
                println!("Authentication rejected! Illegal claims type.");
                return JsonResponse::of_status(StatusCode::UNAUTHORIZED).into();
            }

            let user_id = decoded.claims.sub;

            let lookup = state.user_lookup(user_id).await?;

            match lookup {
                Some(user) => {
                    request.extensions_mut().insert::<U>(user);
                    println!("Inserted user to request extensions...");
                }
                None => {
                    println!("Authentication rejected! User lookup returned no result.");
                    return JsonResponse::of_status(StatusCode::UNAUTHORIZED).into();
                }
            }
        } else {
            println!("Authentication rejected! Unable to decode claims from credential.");
            return JsonResponse::of_status(StatusCode::UNAUTHORIZED).into();
        }
    } else {
        println!("Authentication rejected! No credential in request.");
        return JsonResponse::of_status(StatusCode::UNAUTHORIZED).into();
    }

    Ok(next.run(request).await)
}
