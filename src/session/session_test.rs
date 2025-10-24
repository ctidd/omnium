use std::ops::Add;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::from_fn_with_state;
use axum::Extension;
use axum::{routing::get, Router};
use axum_extra::extract::CookieJar;
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::api::response::JsonStatus;
use crate::session::session::{authorize, resolve, Credential, SessionClaims, SessionManager};

#[derive(Clone)]
struct FakeAccount {
    name: String,
}

struct FakeAppState {}

fn fake_encode_claims(claims: &SessionClaims) -> anyhow::Result<String> {
    Ok(serde_json::to_string(claims)?)
}

fn fake_decode_claims(credential: &str) -> anyhow::Result<SessionClaims> {
    Ok(serde_json::from_str(credential)?)
}

#[async_trait]
impl SessionManager<FakeAccount> for Arc<FakeAppState> {
    async fn decode_claims(&self, credential: Credential) -> anyhow::Result<SessionClaims> {
        fake_decode_claims(&credential.0)
    }

    async fn get_account(&self, _account_id: String) -> anyhow::Result<Option<FakeAccount>> {
        Ok(Some(FakeAccount {
            name: "Test Account".into(),
        }))
    }

    fn extract_credential(
        &self,
        request: &axum::extract::Request,
        _cookies: &CookieJar,
    ) -> Option<Credential> {
        Credential::from_authorization_header(request)
    }
}

fn fake_app_state() -> Arc<FakeAppState> {
    Arc::new(FakeAppState {})
}

fn app(state: Arc<FakeAppState>) -> Router {
    Router::new()
        .route(
            "/api/account",
            get(|Extension(caller): Extension<FakeAccount>| async move {
                format!("Hello, {}!", caller.name)
            }),
        )
        .layer(from_fn_with_state(
            state.clone(),
            authorize::<FakeAccount, Arc<FakeAppState>>,
        ))
        .layer(from_fn_with_state(
            state.clone(),
            resolve::<FakeAccount, Arc<FakeAppState>>,
        ))
        .with_state(state)
}

#[tokio::test]
async fn test_session_header_is_accepted() {
    let state = fake_app_state();
    let claims = fake_encode_claims(
        &SessionClaims::new("test-account-id", Duration::from_secs(60)).unwrap(),
    );

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account")
                .method(Method::GET)
                .header("authorization", format!("Bearer {}", claims.unwrap()))
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "Hello, Test Account!"
    )
}

#[tokio::test]
async fn test_missing_header_bearer_prefix_is_rejected() {
    let state = fake_app_state();

    let claims = fake_encode_claims(
        &SessionClaims::new("test-account-id", Duration::from_secs(60)).unwrap(),
    );

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account")
                .method(Method::GET)
                .header("authorization", claims.unwrap())
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response_body = response.into_body().collect().await.unwrap().to_bytes();
    let response_body: JsonStatus = serde_json::from_slice(&response_body).unwrap();

    let expected_body = JsonStatus {
        reason: Some(String::from("Unauthorized")),
        detail: None,
    };

    assert_eq!(response_body, expected_body);
}

#[tokio::test]
async fn test_wrong_claims_type_is_rejected() {
    let state = fake_app_state();
    let claims = fake_encode_claims(&SessionClaims {
        sub: String::from("test-account-id"),
        exp: usize::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .add(Duration::from_secs(1000))
                .as_secs(),
        )
        .unwrap(),
        omn_cl_typ: "illegal".to_string(),
    });

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account")
                .method(Method::GET)
                .header("authorization", format!("Bearer {}", claims.unwrap()))
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response_body = response.into_body().collect().await.unwrap().to_bytes();
    let response_body: JsonStatus = serde_json::from_slice(&response_body).unwrap();

    let expected_body = JsonStatus {
        reason: Some(String::from("Unauthorized")),
        detail: None,
    };

    assert_eq!(response_body, expected_body);
}

#[tokio::test]
async fn test_missing_session_header_is_rejected() {
    let app = app(fake_app_state()).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/account")
                .method(Method::GET)
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response_body = response.into_body().collect().await.unwrap().to_bytes();
    let response_body: JsonStatus = serde_json::from_slice(&response_body).unwrap();

    let expected_body = JsonStatus {
        reason: Some(String::from("Unauthorized")),
        detail: None,
    };

    assert_eq!(response_body, expected_body);
}
