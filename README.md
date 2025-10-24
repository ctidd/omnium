# omnium

A set of extensions for building web applications on axum.

**Unstable:** This crate is not ready for use. The author is building out these extensions to iterate on a proof of concept, and the surface may change frequently.

## Responses

The `api::responses` module provides a set of response conventions for axum handlers, implementing axum's `IntoResponse` trait for typical use cases.

A handler returns `JsonResult<T>`:

```rs
// ...
use omnium::api::{JsonResult, JsonResponse, JsonStatus};

async fn handler() -> JsonResult<JsonStatus> {
    let result = try_do_or_err().await;
    match result {
        Ok => JsonResponse::of_status(StatusCode::ACCEPTED).into()
        Err => JsonResponse::of_status(StatusCode::CONFLICT).into()
    }
}
```

You can build responses using `JsonResponse<T>`, which implements `Into<JsonResult>`, as well as axum's `IntoResponse`.

For clarity, you can use the provided `respond!` macro instead of calling `.into()`.

A handler can return a JSON response for any serializable body, with a default `OK` status:

```rs
async fn handler() -> JsonResult<SomeBodyType> {
    respond!(JsonResponse::of(body));
}
```

Another status code can be set on the response:

```rs
async fn handler() -> JsonResult<JsonStatus> {
    respond!(JsonResponse::of(body).with_status(StatusCode::IM_A_TEAPOT));
}
```

A handler can return a simple `JsonStatus` status response, implicitly deriving the response body as appropriate for the status:

```rs
async fn handler() -> JsonResult<JsonStatus> {
    respond!(JsonResponse::of_status(StatusCode::OK));
}
```

An additional detail message can be added to the `JsonStatus`:

```rs
async fn handler() -> JsonResult<JsonStatus> {
    respond!(JsonResponse::of_status(StatusCode::OK).with_detail("Additional detail"));
}
```

## Errors

A status response can be returned on the `Err` arm of `JsonResult`, regardless of the happy path response type.

```rs
async fn handler() -> JsonResult<()> {
    Err(JsonResponse::of_status(StatusCode::OK).with_detail("Additional detail"))
}
```

An arbitrary error propagated to the top of a handler is treated as an internal server error, and is rendered with an opaque `INTERNAL_SERVER_ERROR` response that masks any error details from the caller.


```rs
async fn handler() -> JsonResult<()> {
    let success = try_do_or_err().await?;
    // ...
}
```

Run your application with the `RUST_BACKTRACE=1` environment variable set for such an internal error to display a backtrace when logged.

Depending on whether you are in a function returning `anyhow::Result` or `JsonResult`, you can return errors using `anyhow::bail!` or `respond_err!`, respectively. In both cases, you can propagate an arbitrary error to be handled as an internal server error, or you can provide a `JsonResult` status response.

```rs
async fn handler() -> anyhow::Result<()> {
    anyhow::bail!("Internal error!");
    // ...
}
```

```rs
async fn handler() -> JsonResult<()> {
    respond_err!("Internal error!");
    // ...
}
```

## Authentication

The `session` module provides JWT-based authentication middleware, with utilities for a cookie-based credential exchange or the `authorization` header for browser-based or programmatic authentication.

Configure authentication middleware:

```rs
struct UserAccount {}

struct AppState {}

#[async_trait]
impl SessionManager<UserAccount> for Arc<AppState> {
    fn decode_claims(&self, credential: Credential) -> anyhow::Result<SessionClaims> {
        // Decode claims from credential...
    }


    async fn get_account(&self, account_id: String) -> anyhow::Result<Option<UserAccount>> {
        // Return account from application database...
    }

    fn extract_credential(&self, request: &Request, _cookies: &CookieJar) -> Option<Credential> {
        // Extract credential from request...
    }
}
```

Attach authentication middleware:

```rs
fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/account", get(|| async { "Hello, caller!" }))
        .layer(from_fn_with_state(
            state.clone(),
            authorize::<UserAccount, Arc<AppState>>,
        ))
         .layer(from_fn_with_state(
            state.clone(),
            resolve::<UserAccount, Arc<AppState>>,
        ))
        .with_state(state)
}
```

Note that the authentication middleware is attached in two parts: 1) resolving the authenticated account on the request, and 2) enforcing authentication for the request. You can layer these middleware components to support account-aware routes for your entire application, while a subset of the application requires authorizing.

Create the account session:

```rs
create_session(
    "some-account-id",
    &EncodingKey::from_secret(...),
    Duration::from_secs(60),
);
```

With `Credential::from_authorization_header`, a client may pass the session as the `authorization` header. With `Credential::from_cookie`, a client may pass the session as a cookie. For a simple, user-facing web application, you can set a `__Host-` cookie when the account signs in, in order to authenticate requests to the service running on the same origin. If the application shares a session across multiple services on different origins, it might expose the session for use by the client in the `authorization` header for programmatic or cross-origin requests. You can plug in your own handling for extracting credentials from requests with a custom `extract_credential` handler.

Requests from an unauthenticated caller will reject with a 401 response.

For an authenticated caller, the account object from `get_account` can be used throughout the request lifecycle to avoid redundant lookup in handlers:

```rs
pub async fn handler(
    Extension(caller): Extension<AppAccount>,
) {
    println!("Caller is: {}", caller);
}
```
