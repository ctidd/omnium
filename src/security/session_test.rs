use std::ops::{Add, Sub};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::from_fn_with_state;
use axum::Extension;
use axum::{routing::get, Router};
use http_body_util::BodyExt;
use jsonwebtoken::EncodingKey;
use tower::ServiceExt;

use crate::api::responses::StatusBody;
use crate::security::claims::encode_claims;
use crate::security::secrets::{create_service_secret, OmniumServiceSecret};
use crate::security::session::{
    authenticate, create_session, OmniumState, SessionClaims, SESSION_CLAIMS_TYPE,
};

#[derive(Clone)]
struct FakeUser {
    name: String,
}

struct FakeOmniumState {
    pub service_secret: OmniumServiceSecret,
}

impl OmniumState<FakeUser> for Arc<FakeOmniumState> {
    async fn service_secret(&self) -> anyhow::Result<&OmniumServiceSecret> {
        Ok(&self.service_secret)
    }

    async fn user_lookup(&self, _user_id: String) -> anyhow::Result<Option<FakeUser>> {
        Ok(Some(FakeUser {
            name: "Test User".into(),
        }))
    }
}

fn fake_app_state() -> Arc<FakeOmniumState> {
    Arc::new(FakeOmniumState {
        service_secret: create_service_secret().unwrap(),
    })
}

fn app(state: Arc<FakeOmniumState>) -> Router {
    Router::new()
        .route(
            "/api/user",
            get(|Extension(caller): Extension<FakeUser>| async move {
                format!("Hello, {}!", caller.name)
            }),
        )
        .layer(from_fn_with_state(
            state.clone(),
            authenticate::<FakeUser, Arc<FakeOmniumState>>,
        ))
        .with_state(state)
}

#[tokio::test]
async fn test_session_header_is_accepted() {
    let state = fake_app_state();

    let claims = create_session(
        "test-user-id",
        &EncodingKey::from_secret(state.service_secret.value.as_bytes()),
        Duration::from_secs(60),
    );

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/user")
                .method(Method::GET)
                .header("authorization", claims.unwrap())
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "Hello, Test User!"
    )
}

#[tokio::test]
async fn test_barely_expired_session_header_is_still_accepted() {
    let state = fake_app_state();

    let claims = encode_claims(
        &SessionClaims {
            sub: String::from("test-user-id"),
            exp: usize::try_from(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .sub(Duration::from_secs(45))
                    .as_secs(),
            )
            .unwrap(),
            omn_cl_typ: SESSION_CLAIMS_TYPE.into(),
        },
        &EncodingKey::from_secret(state.service_secret.value.as_bytes()),
    );

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/user")
                .method(Method::GET)
                .header("authorization", claims.unwrap())
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_expired_session_header_is_rejected() {
    let state = fake_app_state();

    let claims = encode_claims(
        &SessionClaims {
            sub: String::from("test-user-id"),
            exp: usize::try_from(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .sub(Duration::from_secs(120))
                    .as_secs(),
            )
            .unwrap(),
            omn_cl_typ: SESSION_CLAIMS_TYPE.into(),
        },
        &EncodingKey::from_secret(state.service_secret.value.as_bytes()),
    );

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/user")
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
    let response_body: StatusBody = serde_json::from_slice(&response_body).unwrap();

    let expected_body = StatusBody {
        reason: Some(String::from("Unauthorized")),
        detail: None,
    };

    assert_eq!(response_body, expected_body);
}

#[tokio::test]
async fn test_wrong_claims_type_is_rejected() {
    let state = fake_app_state();

    let claims = encode_claims(
        &SessionClaims {
            sub: String::from("test-user-id"),
            exp: usize::try_from(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .add(Duration::from_secs(1000))
                    .as_secs(),
            )
            .unwrap(),
            omn_cl_typ: "illegal".to_string(),
        },
        &EncodingKey::from_secret(state.service_secret.value.as_bytes()),
    );

    let app = app(state).into_service();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/user")
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
    let response_body: StatusBody = serde_json::from_slice(&response_body).unwrap();

    let expected_body = StatusBody {
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
                .uri("/api/user")
                .method(Method::GET)
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    println!("{:#?}", response);

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response_body = response.into_body().collect().await.unwrap().to_bytes();
    let response_body: StatusBody = serde_json::from_slice(&response_body).unwrap();

    let expected_body = StatusBody {
        reason: Some(String::from("Unauthorized")),
        detail: None,
    };

    assert_eq!(response_body, expected_body);
}
