use axum::response::IntoResponse;
use axum::Json;
use hyper::{HeaderMap, StatusCode};
use log::error;
use serde::{Deserialize, Serialize};

pub type JsonResult = core::result::Result<axum::response::Response, JsonResponse<JsonStatusBody>>;

impl<T> From<JsonResponse<T>> for JsonResult
where
    T: Serialize,
{
    fn from(response: JsonResponse<T>) -> Self {
        Ok(response.into_response())
    }
}

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
    fn into_response(self) -> axum::response::Response {
        let mut response = (self.code, Json(self.body)).into_response();

        for (k, v) in self.headers.iter() {
            response.headers_mut().append(k, v.clone());
        }

        response
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct JsonStatusBody {
    pub reason: Option<String>,
    pub detail: Option<String>,
}

impl JsonStatusBody {
    pub fn of(code: StatusCode, detail: Option<String>) -> JsonStatusBody {
        JsonStatusBody {
            reason: code.canonical_reason().map(String::from),
            detail,
        }
    }
}

impl JsonResponse<JsonStatusBody> {
    pub fn of_status(code: StatusCode) -> JsonResponse<JsonStatusBody> {
        JsonResponse {
            headers: HeaderMap::new(),
            code,
            body: JsonStatusBody::of(code, None),
        }
    }

    pub fn with_detail(mut self, detail: String) -> Self {
        self.body = JsonStatusBody::of(self.code, Some(detail));
        self
    }
}

impl<E> From<E> for JsonResponse<JsonStatusBody>
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err: anyhow::Error = err.into();
        error!("Internal error! {:?}", err);

        JsonResponse::of_json(JsonStatusBody::of(StatusCode::INTERNAL_SERVER_ERROR, None))
            .with_status(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
