# omnium

A set of extensions for building web applications on axum.

**Unstable:** This crate is not ready for use. The author is building out these extensions to iterate on a proof of concept, and the surface may change frequently.

## api

The `api::responses` module provides a set of response conventions for axum handlers, implementing axum's `IntoResponse` trait for typical use cases.

A handler returns `JsonResult` or `TypedJsonResult<T>`, where `JsonResult` can be used for type-erased responses and `TypedJsonResult<T>` can be used for consistently-typed responses.

The `Ok(...)` arm on these types can be used regardless of status code, to return custom responses:

```rs
// ...
use omnium::api::{JsonResult, JsonResponse};

async fn handler() -> JsonResult {
    let result = try_do_or_err().await;
    match result {
        Ok => JsonResponse::of_status(StatusCode::ACCEPTED).into()
        Err => JsonResponse::of_status(StatusCode::CONFLICT).into()
    }
}
```

Response conventions are provided through the `JsonResponse<T>` struct, which implements `Into<JsonResult>` and `Into<TypedJsonResult<T>>`, as well as axum's `IntoResponse`.

A handler can return a JSON response for any serializable body, with a default `OK` status:

```rs
async fn handler() -> JsonResult {
    JsonResponse::of_json(body).into()
}
```

Another status code can be set on the response:

```rs
async fn handler() -> JsonResult {
    JsonResponse::of_json(body).with_status(StatusCode::IM_A_TEAPOT).into()
}
```

A handler can return a simple `JsonStatusBody` status response, implicitly deriving the response body as appropriate for the status:

```rs
async fn handler() -> JsonResult {
    JsonResponse::of_status(StatusCode::OK).into()
}
```

An additional detail message can be added to the `JsonStatusBody`:

```rs
async fn handler() -> JsonResult {
    JsonResponse::of_status(StatusCode::OK).with_detail("Additional detail").into()
}
```

The default `JsonResult` erases the response payload type. If you are returning a consistent type on response, you can return a `TypedJsonResult<T>`:

```rs
async fn handler() -> TypedJsonResult<JsonStatusBody> {
    JsonResponse::of_status(StatusCode::OK).with_detail("Additional detail").into()
}
```

The `Err` arm is available on both `JsonResult` and `TypedJsonResult<T>` to handle status responses. For example, when using `TypedJsonResult<T>` for the happy path, you may choose to return status responses to communicate other results to the caller, whether for an error, informational result, or other case:

```rs
async fn handler() -> TypedJsonResult<JsonStatusBody> {
    Err(JsonResponse::of_status(StatusCode::OK).with_detail("Additional detail"))
}
```

Finally, the `Err` arm is used to automatically handle internal server errors. A handler can return `Err(Into<anyhow::Error>)`, which will be rendered as an `INTERNAL_SERVER_ERROR` response.

```rs
async fn handler() -> JsonResult {
    let success = try_do_or_err().await?;
    // ...
}
```

With this convention, unhandled errors have built-in `IntoResponse` rendering and other errors must be rendered explicitly by a handler. When the handler returns an internal error in this way, the error details are not visible to the caller.

Exposing visible errors explicitly at all call sites can be verbose. For scenarios where a variety of visible errors may bubble up from within implementation, you can propagate and expose such an error as a `JsonResponse`:

```rs
async fn get_account_by_id(account_id: &AccountId) -> Result<Option<AccountItem>> {
    if let Err(error) = account_id.validate() {
        bail!(JsonResponse::of_status(StatusCode::BAD_REQUEST).with_detail(error.to_string()));
    }
    // ...
}

async fn get_account(account_id: &AccountId) -> JsonResult {
    let account = get_account_by_id(account_id)?;
    // ...
}
```

## security

The `security` module provides JWT-based authentication middleware, with utilities for a cookie-based credential exchange or the `authorization` header for browser-based or programmatic authentication.

Create a service secret:

```rs
let service_secret = create_service_secret();
```

Configure authentication middleware:

```rs
#[derive(Clone)]
struct AppAccount {}

struct AppState {
    pub service_secret: ServiceSecret,
}

impl SessionManager<AppAccount> for Arc<AppState> {
    async fn get_service_secret(&self) -> anyhow::Result<&ServiceSecret> {
        // Return secret from application secret manager:
        Ok(&self.service_secret)
    }


    async fn get_account(&self, _account_id: String) -> anyhow::Result<Option<AppAccount>> {
        // Return account from application database:
        Ok(Some(AppAccount {}))
    }

    fn extract_credential(&self, request: &Request, _cookies: &CookieJar) -> Option<Credential> {
        // Extract credential from request:
        Credential::from_authorization_header(&request)
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
            authenticate::<AppAccount, Arc<AppState>>,
        ))
         .layer(from_fn_with_state(
            state.clone(),
            decorate::<AppAccount, Arc<AppState>>,
        ))
        .with_state(state)
}
```

Note that the authentication middleware is attached in two parts: 1) decorating the authenticated account on the request, and 2) enforcing authentication for the request.

Create the account session:

```rs
create_session(
    "some-account-id",
    &EncodingKey::from_secret(state.service_secret.value.as_bytes()),
    Duration::from_secs(60),
);
```

With `Credential::from_authorization_header`, a client may pass the session as the `authorization` header. With `Credential::from_cookie`, a client may pass the session as the `__Host-omn-sess` cookie. For a simple, user-facing web application, you can set the `__Host-omn-sess` cookie when the account signs in, in order to authenticate requests to the service running on the same origin. If the application shares a session across multiple services on different origins, it can expose the session for use by the client in the `authorization` header for programmatic, cross-origin requests. You can plug in your own handling for extracting credentials from requests with a custom `extract_credential` handler.

Requests from an unauthenticated caller will reject with a 401 response.

For an authenticated caller, the account object from `account_lookup` can be retrieved from request state to avoid redundant lookup in handlers:

```rs
pub async fn handler(
    Extension(caller): Extension<AppAccount>,
) {
    println!("Caller is: {}", caller);
}
```

Because a variety of data may need to be passed and verified between an application and its callers, `encode_claims` and `decode_claims` may also be used for other purposes other than authentication. Note that claims are signed but not encrypted.

In addition to claims-based utilities, this crate wraps `aes_gcm` to provide utilties `encrypt_string_aes256_gcm` and `decrypt_string_aes256_gcm`. A secret created by `create_service_secret` can also be used with these encryption utilities.
