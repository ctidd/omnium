use axum::response::IntoResponse;
use axum::Json;
use hyper::{HeaderMap, StatusCode};
use log::error;
use serde::{Deserialize, Serialize};

pub type Result = core::result::Result<axum::response::Response, Response<StatusBody>>;

impl<T> From<Response<T>> for Result
where
    T: Serialize,
{
    fn from(response: Response<T>) -> Self {
        Ok(response.into_response())
    }
}

#[derive(Debug)]
pub struct Response<T>
where
    T: Serialize,
{
    headers: HeaderMap,
    code: StatusCode,
    body: T,
}

impl<T> Response<T>
where
    T: Serialize,
{
    pub fn json(body: T) -> Response<T>
    where
        T: Serialize,
    {
        Response {
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

impl<T> IntoResponse for Response<T>
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

impl Response<StatusBody> {
    pub fn status(code: StatusCode) -> Response<StatusBody> {
        Response {
            headers: HeaderMap::new(),
            code,
            body: StatusBody::of(code, None),
        }
    }

    pub fn with_detail(mut self, detail: String) -> Self {
        self.body = StatusBody::of(self.code, Some(detail));
        self
    }
}

impl<E> From<E> for Response<StatusBody>
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err: anyhow::Error = err.into();
        error!("Responding with internal server error! {:?}", err);

        Response::json(StatusBody::of(StatusCode::INTERNAL_SERVER_ERROR, None))
            .with_status(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
