use axum::{
    http::HeaderValue,
    response::{IntoResponse, Response},
    Json,
};
use hyper::{header::IntoHeaderName, HeaderMap, StatusCode};
use log::{error, info};
use serde::{Deserialize, Serialize};

pub struct ResponseError(pub anyhow::Error);

pub type JsonResult<T> = core::result::Result<JsonResponse<T>, ResponseError>;

#[derive(Debug)]
pub struct JsonResponse<T>
where
    T: Serialize,
{
    headers: HeaderMap,
    code: StatusCode,
    body: T,
}

impl<T> JsonResponse<T>
where
    T: Serialize,
{
    pub fn of(body: T) -> JsonResponse<T>
    where
        T: Serialize,
    {
        JsonResponse {
            headers: HeaderMap::new(),
            code: StatusCode::OK,
            body,
        }
    }

    pub fn of_json(body: T) -> JsonResponse<T>
    where
        T: Serialize,
    {
        JsonResponse::of(body)
    }

    pub fn with_status(mut self, code: StatusCode) -> Self {
        self.code = code;
        self
    }

    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    pub fn append_header<N>(mut self, key: N, value: HeaderValue) -> Self
    where
        N: IntoHeaderName,
    {
        self.headers.append(key, value);
        self
    }
}

impl<T> IntoResponse for JsonResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        let mut response = (self.code, Json(self.body)).into_response();

        for (k, v) in self.headers.iter() {
            response.headers_mut().append(k, v.clone());
        }

        response.into_response()
    }
}

impl<T> From<JsonResponse<T>> for Response
where
    T: Serialize,
{
    fn from(response: JsonResponse<T>) -> Self {
        response.into_response()
    }
}

impl<T> From<JsonResponse<T>> for JsonResult<T>
where
    T: Serialize,
{
    fn from(response: JsonResponse<T>) -> Self {
        Ok(response.into())
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonStatus {
    pub reason: Option<String>,
    pub detail: Option<String>,
}

impl JsonStatus {
    pub fn of(code: StatusCode, detail: Option<String>) -> JsonStatus {
        JsonStatus {
            reason: code.canonical_reason().map(String::from),
            detail,
        }
    }
}

impl JsonResponse<JsonStatus> {
    pub fn of_status(code: StatusCode) -> JsonResponse<JsonStatus> {
        JsonResponse {
            headers: HeaderMap::new(),
            code,
            body: JsonStatus::of(code, None),
        }
    }

    pub fn of_client_err(err: anyhow::Error, code: StatusCode) -> JsonResponse<JsonStatus> {
        info!("Client error: {} as {}", err, code);

        JsonResponse {
            headers: HeaderMap::new(),
            code,
            body: JsonStatus::of(code, None),
        }
    }

    pub fn of_internal_err(err: anyhow::Error) -> JsonResponse<JsonStatus> {
        error!("Internal error: {:?}", err);

        JsonResponse::of_status(StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.body = JsonStatus::of(self.code, Some(detail.into()));
        self
    }

    pub fn anyhow(self) -> anyhow::Error {
        anyhow::anyhow!(self)
    }
}

impl std::fmt::Display for JsonResponse<JsonStatus> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for JsonResponse<JsonStatus> {}

impl<E> From<E> for ResponseError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        return ResponseError(err.into());
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        match self.0.downcast::<JsonResponse<JsonStatus>>() {
            Ok(err) => return err.into_response(),
            Err(unhandled) => JsonResponse::of_internal_err(unhandled).into_response(),
        }
    }
}

#[macro_export]
macro_rules! respond {
    ($val:expr) => {
        return Ok($val.into());
    };
}

#[macro_export]
macro_rules! respond_err {
    ($($tt:tt)*) => {
        return Err($crate::api::response::ResponseError(anyhow::anyhow!($($tt)*)));
    }
}
