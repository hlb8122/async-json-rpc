use serde_json::Error as JsonError;

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

pub struct IncompleteRequest;

impl RequestBuilder {
    pub fn method(mut self, method: String) -> Self {
        self.method = Some(method);
        self
    }

    pub fn id(mut self, id: serde_json::Value) -> Self {
        self.id = Some(id);
        self
    }

    pub fn params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }

    pub fn jsonrpc(mut self, json_rpc: String) -> Self {
        self.json_rpc = Some(json_rpc);
        self
    }

    pub fn finish(self) -> Result<Request, IncompleteRequest> {
        if let (Some(id), Some(method), Some(params), Some(jsonrpc)) =
            (self.id, self.method, self.params, self.json_rpc)
        {
            Ok(Request {
                id,
                method,
                params,
                jsonrpc,
            })
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