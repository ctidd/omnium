use std::fmt::Debug;

use anyhow::bail;
use axum::body::Body;
use axum::http::{Method, Request};
use axum::{routing::MethodRouter, Router};
use http_body_util::BodyExt;
use hyper::StatusCode;
use serde::Deserialize;
use tower::util::ServiceExt;

use crate::api::response::{JsonResponse, JsonResult, JsonStatus};
use crate::respond_err;

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
    let body: JsonStatus = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        body,
        JsonStatus {
            reason: expect_code.canonical_reason().map(String::from),
            detail: expect_detail,
        },
    );
}

#[tokio::test]
async fn test_ok_status_to_response() {
    async fn handler() -> JsonResult<JsonStatus> {
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
    async fn handler() -> JsonResult<JsonStatus> {
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
    async fn handler() -> JsonResult<JsonStatus> {
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
async fn test_bail_propagate_status_to_response() {
    async fn dependency() -> anyhow::Result<()> {
        bail!(JsonResponse::of_status(StatusCode::IM_A_TEAPOT)
            .with_detail("Handled with detail.".to_string()));
    }

    async fn handler() -> JsonResult<()> {
        dependency().await?;
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
async fn test_bail_propagate_err_to_response() {
    async fn dependency() -> anyhow::Result<()> {
        bail!("An unhandled error was propagated!");
    }

    async fn handler() -> JsonResult<()> {
        dependency().await?;
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
async fn test_bail_direct_status_to_response() {
    async fn handler() -> JsonResult<()> {
        // Can't use bail! here because it creates an anyhow::Error, not a ResponseError,
        // and unfortunately, we can't implement IntoResponse for anyhow::Error, and need
        // ResponseError as a wrapper type.
        respond_err!(JsonResponse::of_status(StatusCode::IM_A_TEAPOT)
            .with_detail("Handled with detail.".to_string()));
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
async fn test_bail_direct_err_to_response() {
    async fn handler() -> JsonResult<()> {
        respond_err!("An unhandled error was propagated!");
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
async fn test_json_to_response() {
    async fn handler() -> JsonResult<JsonStatus> {
        JsonResponse::of_json(JsonStatus {
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
        JsonStatus {
            reason: Some("test".into()),
            detail: Some("content".into()),
        },
    )
    .await;
}

#[tokio::test]
async fn test_json_to_response_default_ok() {
    async fn handler() -> JsonResult<JsonStatus> {
        JsonResponse::of_json(JsonStatus {
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
        JsonStatus {
            reason: Some("test".into()),
            detail: Some("content".into()),
        },
    )
    .await;
}
