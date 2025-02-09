// auth_layer.rs
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use tower::{Layer, Service};

use crate::auth::verify_clerk_token; // your Clerk logic

#[derive(Clone)]
pub struct AuthLayer;

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

impl<S> Service<Request<Body>> for AuthMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let mut svc = self.inner.clone();

        Box::pin(async move {
            // 1) Extract the token from query or header. For example, query:
            let maybe_token: Option<String> = req.uri().query().and_then(|q| {
                for pair in q.split('&') {
                    let mut kv = pair.split('=');
                    let key = kv.next()?;
                    let val = kv.next()?;
                    if key == "token" {
                        return Some(val.to_string());
                    }
                }
                None
            });

            let Some(token) = maybe_token else {
                let body = Json(json!({"error": "no token"})).into_response();
                let resp = (StatusCode::UNAUTHORIZED, body).into_response();
                return Ok(resp);
            };

            // 2) Verify with Clerk
            match verify_clerk_token(&token).await {
                Ok(claims) => {
                    // 3) Put user info in request extensions so the handler can see it
                    req.extensions_mut().insert(claims.sub);

                    // 4) Hand off to the next service (your actual route/handler)
                    let response = svc.call(req).await?;
                    Ok(response)
                }
                Err(_e) => {
                    let body = Json(json!({"error": "invalid token"})).into_response();
                    let resp = (StatusCode::UNAUTHORIZED, body).into_response();
                    Ok(resp)
                }
            }
        })
    }
}
