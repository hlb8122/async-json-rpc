use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use futures_core::{
    task::{Context, Poll},
    Future,
};
use futures_util::{stream::StreamExt, TryFutureExt};
pub use hyper::client::{connect::Connect, HttpConnector};
use hyper::{
    header::{AUTHORIZATION, CONTENT_TYPE},
    Body, Client as HyperClient, Error as HyperError,
};
pub use hyper_tls::HttpsConnector;
use tower_service::Service;

use super::{Error, RequestFactory};
use crate::objects::{Request, RequestBuilder, Response};

pub type HttpError = Error<HyperError>;

#[derive(Debug)]
pub struct Credentials {
    url: String,
    user: Option<String>,
    password: Option<String>,
}

/// A handle to a remote HTTP JSONRPC server.
#[derive(Clone, Debug)]
pub struct Client<C> {
    credentials: Arc<Credentials>,
    nonce: Arc<AtomicUsize>,
    inner_client: HyperClient<C>,
}

impl Client<HttpConnector> {
    /// Creates a new client.
    pub fn new(url: String, user: Option<String>, password: Option<String>) -> Self {
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(password.is_none() || user.is_some());
        let credentials = Arc::new(Credentials {
            url,
            user,
            password,
        });
        Client {
            credentials,
            inner_client: HyperClient::new(),
            nonce: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl<C> Client<C> {
    pub fn next_nonce(&self) -> usize {
        self.nonce.load(Ordering::AcqRel)
    }
}

impl Client<HttpsConnector<HttpConnector>> {
    /// Creates a new TLS client.
    pub fn new_tls(url: String, user: Option<String>, password: Option<String>) -> Self {
        // Check that if we have a password, we have a username; other way around is ok
        debug_assert!(password.is_none() || user.is_some());
        let https = HttpsConnector::new();
        let inner_client = HyperClient::builder().build::<_, Body>(https);
        let credentials = Arc::new(Credentials {
            url,
            user,
            password,
        });
        Client {
            credentials,
            inner_client,
            nonce: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl<C> Service<Request> for Client<C>
where
    C: Connect + Clone + Send + Sync + 'static,
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
        let mut builder = hyper::Request::post(&self.credentials.url);

        // Add authorization
        if let Some(ref user) = self.credentials.user {
            let pass_str = match &self.credentials.password {
                Some(some) => some,
                None => "",
            };
            builder = builder.header(
                AUTHORIZATION,
                format!(
                    "Basic {}",
                    base64::encode(&format!("{}:{}", user, pass_str))
                ),
            )
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

impl<C> Client<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    pub async fn send(&self, request: Request) -> Result<Response, Error<HyperError>> {
        self.clone().call(request).await
    }
}

impl<C> RequestFactory for Client<C> {
    fn build_request(&self) -> RequestBuilder {
        let id = serde_json::Value::Number(self.nonce.fetch_add(1, Ordering::AcqRel).into());
        Request::build().id(id)
    }
}
