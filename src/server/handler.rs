// src/server/handler.rs
use hyper::{Body, Request, Response};
use std::sync::Arc;
use tower::Service;

use crate::proxy::Proxy;

#[derive(Clone)]
pub struct RequestHandler {
    proxy: Arc<Proxy>,
}

impl RequestHandler {
    pub fn new(proxy: Arc<Proxy>) -> Self {
        Self { proxy }
    }
}

impl Service<Request<Body>> for RequestHandler {
    type Response = Response<Body>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = futures::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,  // Fixed: Added underscore
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let proxy = self.proxy.clone();
        Box::pin(async move {
            proxy.handle(req).await.map_err(|e| {
                tracing::error!(%e, "proxy error");
                // Fixed: Use a public error constructor
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })
        })
    }
}