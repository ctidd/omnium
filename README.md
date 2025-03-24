# omnium

A set of extensions for building web applications on axum.

## api

The `api` module provides response conventions for axum handlers, implementing `IntoResponse` for typical use cases.

A handler returns `JsonResult`, which handles responses as `Ok(...)` arm and propagated errors as `Err(...)` arm:

```rs
async fn handler() -> JsonResult {
    // ...
}
```

A handler can bail with `Into<anyhow::Error>`, which will be handled as an `INTERNAL_SERVER_ERROR` response:

```rs
async fn handler() -> JsonResult {
    let success = try_do_or_err().await?;
    // ...
}
```

Functionality is provided through the `JsonResponse` struct. The handler result type `JsonResult` implements `From<JsonResponse>`, so `JsonResponse` can be returned from a handler with a call to `.into()`.

A handler can return a JSON response for any serializable body:

```rs
async fn handler() -> JsonResult {
    JsonResponse.of(StatusCode::OK).body(body).into()
}
```

A handler can return a simple status response, deriving a response body as appropriate for the status:

```rs
async fn handler() -> JsonResult {
    JsonResponse.of_status(StatusCode::OK).into()
}
```

An additional detail message can be added to the status response body:

```rs
async fn handler() -> JsonResult {
    JsonResponse.of_status_detail(StatusCode::OK, "Additional detail".into()).into()
}
```

## authn

The `authn` module provides JWT authentication middleware.

Create a session secret:

```rs
let session_secret = create_session_secret();
```

Configure authentication middleware:

```rs
#[derive(Clone)]
struct AppUser {}

struct AppOmniumState {
    pub session_secret: OmniumSessionSecret,
}

impl OmniumState<AppUser> for Arc<AppOmniumState> {
    async fn session_secret(&self) -> anyhow::Result<&OmniumSessionSecret> {
        // Return secret from application secret manager:
        Ok(&self.session_secret)
    }


    async fn user_lookup(&self, _user_id: String) -> anyhow::Result<Option<AppUser>> {
        // Return user from application: database:
        Ok(Some(AppUser {}))
    }
}
```

Attach authentication middleware:

```rs
fn app(state: Arc<AppOmniumState>) -> Router {
    Router::new()
        .route("/api/user", get(|| async { "Hello, user!" }))
        .layer(from_fn_with_state(
            state.clone(),
            authenticate::<AppUser, Arc<AppOmniumState>>,
        ))
        .with_state(state)
}
```

Requests from an unauthenticated user will reject with a 401 response.

For an authenticated user, the user object from `user_lookup` can be retrieved from request state to avoid redundant lookup in handlers.

```rs
pub async fn handler(
    Extension(caller): Extension<AppUser>,
) {
    println!("Caller is: {}", caller);
}
```
