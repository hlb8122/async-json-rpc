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

/// The error type for RPCs.
#[derive(Debug)]
pub enum Error<E> {
    /// The batch response contained a duplicate ID.
    BatchDuplicateResponseId(serde_json::Value),
    /// A connection error occured.
    Connection(E),
    /// Batches can't be empty.
    EmptyBatch,
    /// An error occured during respnse JSON deserialization.
    Json(serde_json::Error),
    /// The response did not have the expected nonce.
    NonceMismatch,
    /// The response had a jsonrpc field other than "2.0".
    VersionMismatch,
    /// The batch response contained an ID that didn't correspond to any request ID.
    WrongBatchResponseId(serde_json::Value),
    /// Too many responses returned in batch.
    WrongBatchResponseSize,
}

/// A handle to a remote HTTP JSONRPC server.
pub struct HttpClient<C> {
    url: String,
    user: Option<String>,
    password: Option<String>,
    nonce: AtomicUsize,
    inner_client: C,
}

impl HttpClient<HyperClient<HttpConnector>> {
    /// Creates a new client.
    pub fn new(url: String, user: Option<String>, password: Option<String>) -> Self {
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(password.is_none() || user.is_some());
        HttpClient {
            url,
            user,
            password,
            inner_client: HyperClient::new(),
            nonce: AtomicUsize::new(0),
        }
    }

    pub fn next_nonce(&self) -> usize {
        self.nonce.load(Ordering::AcqRel)
    }
}

impl HttpClient<HyperClient<HttpsConnector<HttpConnector>>> {
    /// Creates a new TLS client.
    pub fn new_tls(url: String, user: Option<String>, password: Option<String>) -> Self {
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(password.is_none() || user.is_some());
        let https = HttpsConnector::new();
        let inner_client = HyperClient::builder().build::<_, Body>(https);
        HttpClient {
            url,
            user,
            password,
            inner_client,
            nonce: AtomicUsize::new(0),
        }
    }
}

impl<I> Service<Request> for HttpClient<I>
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

pub trait RequestFactory {
    fn build_request(&self) -> RequestBuilder;
}

impl<C> RequestFactory for HttpClient<C> {
    fn build_request(&self) -> RequestBuilder {
        let id = serde_json::Value::Number(self.nonce.fetch_add(1, Ordering::AcqRel).into());
        Request::build().id(id)
    }
}
