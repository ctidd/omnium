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

Functionality is provided through the `JsonResponse` struct. The handler result type `JsonResult` implements `From<JsonResponse>`, so `JsonResponse` can be returned directly from a handler.

A handler can return a JSON response for any serializable body:

```rs
async fn handler() -> JsonResult {
    JsonResponse.of(StatusCode::OK).body(body)
}
```

A handler can return a simple status response, deriving a response body as appropriate for the status:

```rs
async fn handler() -> JsonResult {
    JsonResponse.of_status(StatusCode::OK)
}
```

An additional detail message can be added to the status response body:

```rs
async fn handler() -> JsonResult {
    JsonResponse.of_status_detail(StatusCode::OK, "Additional detail".into())
}
```
