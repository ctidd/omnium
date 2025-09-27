use axum::response::{IntoResponse, Response};
use axum::Json;
use hyper::{HeaderMap, StatusCode};
use log::error;
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
    pub fn of_json(body: T) -> JsonResponse<T>
    where
        T: Serialize,
    {
        JsonResponse {
            headers: HeaderMap::new(),
            code: StatusCode::OK,
            body,
        }
    }

    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_status(mut self, code: StatusCode) -> Self {
        self.code = code;
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

impl<T> From<JsonResponse<T>> for axum::response::Response
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

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.body = JsonStatus::of(self.code, Some(detail.into()));
        self
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
            Err(unhandled) => {
                error!("Internal error! {:?}", unhandled);

                JsonResponse::of_status(StatusCode::INTERNAL_SERVER_ERROR).into_response()
            }
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
