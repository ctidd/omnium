#[cfg(test)]
mod mod_test;

use anyhow::Error;
use axum::response::IntoResponse;
use axum::Json;
use hyper::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

pub type JsonResult = Result<axum::response::Response, JsonResponse<StatusBody>>;

impl<T> From<JsonResponse<T>> for JsonResult
where
    T: Serialize,
{
    fn from(json: JsonResponse<T>) -> Self {
        Ok(json.into_response())
    }
}

#[derive(Debug)]
pub struct JsonResponse<T>
where
    T: Serialize,
{
    headers: HeaderMap,
    code: StatusCode,
    body: Option<T>,
    internal_server_error: Option<Error>,
}

impl<T> JsonResponse<T>
where
    T: Serialize,
{
    pub fn of(code: StatusCode) -> JsonResponse<T>
    where
        T: Serialize,
    {
        JsonResponse {
            headers: HeaderMap::new(),
            code,
            body: None,
            internal_server_error: None,
        }
    }

    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    pub fn body(mut self, body: T) -> Self {
        self.body = Some(body);
        self
    }

    pub fn internal_server_error(mut self, error: Error) -> Self {
        self.internal_server_error = Some(error);
        self
    }
}

impl<T> IntoResponse for JsonResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        if self.code == StatusCode::INTERNAL_SERVER_ERROR {
            println!("Internal server error! {:?}", self.internal_server_error);
        }

        let mut response = (self.code, Json(self.body)).into_response();

        for (k, v) in self.headers.iter() {
            response.headers_mut().append(k, v.clone());
        }

        response
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusBody {
    pub reason: Option<String>,
    pub detail: Option<String>,
}

impl StatusBody {
    pub fn of(code: StatusCode, detail: Option<String>) -> StatusBody {
        StatusBody {
            reason: code.canonical_reason().map(String::from),
            detail,
        }
    }
}

impl JsonResponse<StatusBody> {
    pub fn of_status(code: StatusCode) -> JsonResponse<StatusBody> {
        JsonResponse {
            headers: HeaderMap::new(),
            code,
            body: Some(StatusBody::of(code, None)),
            internal_server_error: None,
        }
    }

    pub fn of_status_detail(code: StatusCode, detail: String) -> JsonResponse<StatusBody> {
        JsonResponse {
            headers: HeaderMap::new(),
            code,
            body: Some(StatusBody::of(code, Some(detail))),
            internal_server_error: None,
        }
    }
}

impl<E> From<E> for JsonResponse<StatusBody>
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        JsonResponse::of(StatusCode::INTERNAL_SERVER_ERROR)
            .internal_server_error(err.into())
            .body(StatusBody::of(StatusCode::INTERNAL_SERVER_ERROR, None))
    }
}
