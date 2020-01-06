pub use serde_json::Error as JsonError;

#[derive(Clone, Debug, PartialEq, Deserialize)]
/// A JSONRPC error object.
pub struct RpcError {
    /// The integer identifier of the error.
    pub code: i32,
    /// A string describing the error.
    pub message: String,
    /// Additional data specific to the error
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
/// Represents the JSONRPC request object.
pub struct Request {
    pub method: String,
    pub params: serde_json::Value,
    pub id: serde_json::Value,
    pub jsonrpc: String,
}

impl Request {
    pub fn build() -> RequestBuilder {
        RequestBuilder::default()
    }
}

#[derive(Default)]
pub struct RequestBuilder {
    id: Option<serde_json::Value>,
    method: Option<String>,
    params: Option<serde_json::Value>,
    json_rpc: Option<String>,
}

#[derive(Debug)]
pub struct IncompleteRequest;

impl RequestBuilder {
    pub fn method<S: Into<String>>(mut self, method: S) -> Self {
        self.method = Some(method.into());
        self
    }

    pub fn id<I: Into<serde_json::Value>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn params<V: Into<serde_json::Value>>(mut self, params: V) -> Self {
        self.params = Some(params.into());
        self
    }

    pub fn jsonrpc<S: Into<String>>(mut self, json_rpc: S) -> Self {
        self.json_rpc = Some(json_rpc.into());
        self
    }

    pub fn finish(self) -> Result<Request, IncompleteRequest> {
        let jsonrpc = if let Some(jsonrpc) = self.json_rpc {
            jsonrpc
        } else {
            "2.0".to_string()
        };
        if let (Some(id), Some(method)) = (self.id, self.method) {
            if let Some(params) = self.params {
                Ok(Request {
                    id,
                    method,
                    params,
                    jsonrpc,
                })
            } else {
                Ok(Request {
                    id,
                    method,
                    params: serde_json::Value::Null,
                    jsonrpc,
                })
            }
        } else {
            Err(IncompleteRequest)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
/// Represents the JSONRPC response object.
pub struct Response {
    pub result: Option<serde_json::Value>,
    pub error: Option<RpcError>,
    pub id: serde_json::Value,
    pub jsonrpc: Option<String>,
}

impl Response {
    /// Extract the result.
    pub fn result<T: serde::de::DeserializeOwned>(&self) -> Option<Result<T, JsonError>> {
        self.result.as_ref().map(T::deserialize)
    }

    /// Extract the result, consuming the response.
    pub fn into_result<T: serde::de::DeserializeOwned>(self) -> Option<Result<T, JsonError>> {
        self.result.map(serde_json::from_value)
    }

    /// Returns the [`RpcError`].
    pub fn error(self) -> Option<RpcError> {
        self.error
    }

    /// Returns `true` if the result field is [`Some`] value.
    pub fn is_result(&self) -> bool {
        self.result.is_some()
    }

    /// Returns `true` if the error field is [`Some`] value.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}
