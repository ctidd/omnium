use std::fmt::Debug;

use anyhow::anyhow;
use axum::body::Body;
use axum::http::{Method, Request};
use axum::response::Response;
use axum::{routing::MethodRouter, Router};
use http_body_util::BodyExt;
use hyper::StatusCode;
use serde::Serialize;
use tower::util::ServiceExt;

use crate::api::{JsonResponse, JsonResult, StatusBody};

fn input() -> hyper::Request<axum::body::Body> {
    Request::builder()
        .uri("/test")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap()
}

async fn assert_response<T>(response: Response<Body>, expect_code: StatusCode, expect_body: T)
where
    T: Serialize + PartialEq + Debug,
{
    assert_eq!(response.status(), expect_code);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body, serde_json::to_vec(&expect_body).unwrap());
}

async fn assert_status_response(
    response: Response<Body>,
    expect_code: StatusCode,
    expect_detail: Option<String>,
) {
    assert_eq!(response.status(), expect_code);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: StatusBody = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        body,
        StatusBody {
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
        JsonResponse::of_status_detail(StatusCode::UNAUTHORIZED, "You shall not pass!".into())
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
async fn test_json_to_response() {
    async fn handler() -> JsonResult {
        JsonResponse::of(StatusCode::IM_A_TEAPOT)
            .body(StatusBody {
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
        StatusCode::IM_A_TEAPOT,
        StatusBody {
            reason: Some("test".into()),
            detail: Some("content".into()),
        },
    )
    .await;
}
