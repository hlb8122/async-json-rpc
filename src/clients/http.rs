use std::{
    error, fmt,
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
use futures_util::TryFutureExt;
use hyper::client::HttpConnector;
use hyper::{
    body::to_bytes,
    header::{AUTHORIZATION, CONTENT_TYPE},
    Body, Client as HyperClient, Error as HyperError, Request as HttpRequest,
    Response as HttpResponse,
};
use hyper_tls::HttpsConnector;
use tower_service::Service;
use tower_util::ServiceExt;

use super::{Error, RequestFactory};
use crate::objects::{Request, RequestBuilder, Response};

pub type HttpError<E> = Error<ConnectionError<E>>;

/// Error specific to HTTP connections.
#[derive(Debug)]
pub enum ConnectionError<E> {
    Poll(E),
    Service(E),
    Body(HyperError),
}

impl<E: fmt::Display> fmt::Display for ConnectionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poll(err) => write!(f, "polling error, {}", err),
            Self::Service(err) => write!(f, "service error, {}", err),
            Self::Body(err) => write!(f, "body error, {}", err),
        }
    }
}

impl<E: fmt::Display + fmt::Debug> error::Error for ConnectionError<E> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    url: String,
    user: Option<String>,
    password: Option<String>,
}

/// A handle to a remote HTTP JSON-RPC server.
#[derive(Clone, Debug)]
pub struct Client<S> {
    credentials: Arc<Credentials>,
    nonce: Arc<AtomicUsize>,
    inner_service: S,
}

impl<S> Client<S> {
    /// Creates a new HTTP client from a [`Service`].
    ///
    /// [`Service`]: tower::Service
    pub fn from_service(
        service: S,
        url: String,
        user: Option<String>,
        password: Option<String>,
    ) -> Self {
        let credentials = Arc::new(Credentials {
            url,
            user,
            password,
        });
        Client {
            credentials,
            inner_service: service,
            nonce: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Increment nonce and return the last value.
    pub fn next_nonce(&self) -> usize {
        self.nonce.load(Ordering::AcqRel)
    }
}

impl Client<HyperClient<HttpConnector>> {
    /// Creates a new HTTP client.
    pub fn new(url: String, user: Option<String>, password: Option<String>) -> Self {
        Self::from_service(HyperClient::new(), url, user, password)
    }
}

impl Client<HyperClient<HttpsConnector<HttpConnector>>> {
    /// Creates a new HTTPS client.
    pub fn new_tls(url: String, user: Option<String>, password: Option<String>) -> Self {
        let https = HttpsConnector::new();
        let service = HyperClient::builder().build::<_, Body>(https);
        Self::from_service(service, url, user, password)
    }
}

type FutResponse<R, E> = Pin<Box<dyn Future<Output = Result<R, E>> + 'static + Send>>;

impl<S> Service<Request> for Client<S>
where
    S: Service<HttpRequest<Body>, Response = HttpResponse<Body>>,
    S::Error: 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Error<ConnectionError<S::Error>>;
    type Future = FutResponse<Self::Response, Self::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner_service
            .poll_ready(cx)
            .map_err(ConnectionError::Poll)
            .map_err(Error::Connection)
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
            .inner_service
            .call(request)
            .map_err(ConnectionError::Service)
            .map_err(Error::Connection)
            .and_then(|response| async move {
                let body = to_bytes(response.into_body())
                    .await
                    .map_err(ConnectionError::Body)
                    .map_err(Error::Connection)?;
                Ok(serde_json::from_slice(&body).map_err(Error::Json)?)
            });

        Box::pin(fut)
    }
}

impl<S> Client<S>
where
    S: Service<HttpRequest<Body>, Response = HttpResponse<Body>> + Clone,
    S::Error: 'static,
    S::Future: Send + 'static,
{
    pub async fn send(
        &self,
        request: Request,
    ) -> Result<Response, Error<ConnectionError<S::Error>>> {
        self.clone().oneshot(request).await
    }
}

impl<C> RequestFactory for Client<C> {
    /// Build the request.
    fn build_request(&self) -> RequestBuilder {
        let id = serde_json::Value::Number(self.nonce.fetch_add(1, Ordering::AcqRel).into());
        Request::build().id(id)
    }
}
