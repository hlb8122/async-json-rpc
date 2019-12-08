use std::{
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
};

use futures_core::{
    task::{Context, Poll},
    Future,
};
use futures_util::{stream::StreamExt, TryFutureExt};
use hyper::{
    client::HttpConnector,
    header::{AUTHORIZATION, CONTENT_TYPE},
    Body, Client as HyperClient, Error as HyperError, Request as HttpRequest,
    Response as HttpResponse,
};
use hyper_tls::HttpsConnector;
use tower_service::Service;

use crate::objects::{Request, RequestBuilder, Response};

use super::{Error, RequestFactory};

pub type HttpTransport = HyperClient<HttpConnector>;
pub type HttpsTransport = HyperClient<HttpsConnector<HttpConnector>>;
pub type HttpError = Error<HyperError>;

/// A handle to a remote HTTP JSONRPC server.
pub struct Client<C> {
    url: String,
    user: Option<String>,
    password: Option<String>,
    nonce: AtomicUsize,
    inner_client: C,
}

impl Client<HttpTransport> {
    /// Creates a new client.
    pub fn new(url: String, user: Option<String>, password: Option<String>) -> Self {
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(password.is_none() || user.is_some());
        Client {
            url,
            user,
            password,
            inner_client: HyperClient::new(),
            nonce: AtomicUsize::new(0),
        }
    }
}

impl<C> Client<C> {
    pub fn next_nonce(&self) -> usize {
        self.nonce.load(Ordering::AcqRel)
    }
}

impl Client<HttpsTransport> {
    /// Creates a new TLS client.
    pub fn new_tls(url: String, user: Option<String>, password: Option<String>) -> Self {
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(password.is_none() || user.is_some());
        let https = HttpsConnector::new();
        let inner_client = HyperClient::builder().build::<_, Body>(https);
        Client {
            url,
            user,
            password,
            inner_client,
            nonce: AtomicUsize::new(0),
        }
    }
}

impl<I> Service<Request> for Client<I>
where
    I: Service<HttpRequest<Body>, Response = HttpResponse<Body>, Error = HyperError>,
    I::Future: 'static,
{
    type Response = Response;
    type Error = Error<HyperError>;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Self::Error>>>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let json_raw = serde_json::to_vec(&request).unwrap(); // This is safe
        let body = Body::from(json_raw);
        let mut builder = hyper::Request::post(&self.url);

        // Add authorization
        if let Some(ref user) = self.user {
            let pass_str = match &self.password {
                Some(some) => some,
                None => "",
            };
            builder = builder.header(AUTHORIZATION, format!("Basic {}:{}", user, pass_str))
        };

        // Add headers and body
        let request = builder
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .unwrap(); // This is safe

        // Send request
        let fut = self
            .inner_client
            .call(request)
            .map_err(Error::Connection)
            .and_then(|mut response| {
                async move {
                    let mut body = Vec::new();
                    while let Some(chunk) = response.body_mut().next().await {
                        body.extend_from_slice(&chunk.map_err(Error::Connection)?);
                    }
                    Ok(serde_json::from_slice(&body).map_err(Error::Json)?)
                }
            });

        Box::pin(fut)
    }
}

impl<C> RequestFactory for Client<C> {
    fn build_request(&self) -> RequestBuilder {
        let id = serde_json::Value::Number(self.nonce.fetch_add(1, Ordering::AcqRel).into());
        Request::build().id(id)
    }
}