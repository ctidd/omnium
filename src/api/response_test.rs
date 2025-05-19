use std::fmt::Debug;

use anyhow::{anyhow, bail};
use axum::body::Body;
use axum::debug_handler;
use axum::http::{Method, Request};
use axum::{routing::MethodRouter, Router};
use http_body_util::BodyExt;
use hyper::StatusCode;
use serde::Deserialize;
use tower::util::ServiceExt;

use crate::api::response::{JsonResponse, JsonResult, JsonStatusBody, TypedJsonResult};

fn input() -> hyper::Request<axum::body::Body> {
    Request::builder()
        .uri("/test")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap()
}

async fn assert_response<T: for<'a> Deserialize<'a> + PartialEq + Debug>(
    response: axum::response::Response<Body>,
    expect_code: StatusCode,
    expect_body: T,
) {
    assert_eq!(response.status(), expect_code);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: T = serde_json::from_slice(&body).unwrap();

    assert_eq!(body, expect_body);
}

async fn assert_status_response(
    response: axum::response::Response<Body>,
    expect_code: StatusCode,
    expect_detail: Option<String>,
) {
    assert_eq!(response.status(), expect_code);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: JsonStatusBody = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        body,
        JsonStatusBody {
            reason: expect_code.canonical_reason().map(String::from),
            detail: expect_detail,
        },
    );
}

#[tokio::test]
async fn test_ok_status_to_response() {
    async fn handler() -> JsonResult {
        JsonResponse::of_status(StatusCode::OK).into()
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    assert_status_response(response, StatusCode::OK, None).await;
}

#[tokio::test]
async fn test_err_status_to_response() {
    async fn handler() -> JsonResult {
        JsonResponse::of_status(StatusCode::UNAUTHORIZED).into()
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_status_response(response, StatusCode::UNAUTHORIZED, None).await;
}

#[tokio::test]
async fn test_status_with_detail_to_response() {
    async fn handler() -> JsonResult {
        JsonResponse::of_status(StatusCode::UNAUTHORIZED)
            .with_detail("You shall not pass!")
            .into()
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_status_response(
        response,
        StatusCode::UNAUTHORIZED,
        Some("You shall not pass!".into()),
    )
    .await;
}

#[tokio::test]
async fn test_bail_to_response() {
    async fn handler() -> JsonResult {
        Err(anyhow!("An unhandled error was propagated!"))?;
        panic!("This line will never be reached.");
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_status_response(response, StatusCode::INTERNAL_SERVER_ERROR, None).await;
}

#[tokio::test]
async fn test_map_err_to_response() {
    async fn handler() -> JsonResult {
        Err(anyhow!("An error to be handled!")).map_err(|_| {
            JsonResponse::of_status(StatusCode::IM_A_TEAPOT).with_detail("Handled with detail.")
        })?;
        panic!("This line will never be reached.");
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_status_response(
        response,
        StatusCode::IM_A_TEAPOT,
        Some("Handled with detail.".into()),
    )
    .await;
}

#[tokio::test]
async fn test_json_to_response() {
    async fn handler() -> JsonResult {
        JsonResponse::of_json(JsonStatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        })
        .with_status(StatusCode::IM_A_TEAPOT)
        .into()
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_response(
        response,
        StatusCode::IM_A_TEAPOT,
        JsonStatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        },
    )
    .await;
}

#[tokio::test]
async fn test_json_to_response_default_ok() {
    async fn handler() -> JsonResult {
        JsonResponse::of_json(JsonStatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        })
        .into()
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_response(
        response,
        StatusCode::OK,
        JsonStatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        },
    )
    .await;
}

#[tokio::test]
async fn test_json_to_typed_response() {
    #[debug_handler()]
    async fn handler() -> TypedJsonResult<JsonStatusBody> {
        JsonResponse::of_json(JsonStatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        })
        .with_status(StatusCode::IM_A_TEAPOT)
        .into()
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_response(
        response,
        StatusCode::IM_A_TEAPOT,
        JsonStatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        },
    )
    .await;
}

#[tokio::test]
async fn test_status_to_typed_response() {
    async fn handler() -> TypedJsonResult<String> {
        Err(JsonResponse::of_status(StatusCode::UNAUTHORIZED).with_detail("You shall not pass!"))
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_status_response(
        response,
        StatusCode::UNAUTHORIZED,
        Some("You shall not pass!".into()),
    )
    .await;
}

#[tokio::test]
async fn test_map_err_to_typed_response() {
    async fn handler() -> TypedJsonResult<JsonStatusBody> {
        Err(anyhow!("An error to be handled!")).map_err(|_| {
            JsonResponse::of_status(StatusCode::IM_A_TEAPOT).with_detail("Handled with detail.")
        })?;
        panic!("This line will never be reached.");
    }

    let response = Router::new()
        .route("/test", MethodRouter::new().get(handler))
        .into_service()
        .oneshot(input())
        .await
        .unwrap();

    assert_status_response(
        response,
        StatusCode::IM_A_TEAPOT,
        Some("Handled with detail.".into()),
    )
    .await;
}
