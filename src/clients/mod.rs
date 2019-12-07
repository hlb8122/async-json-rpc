pub mod http;

pub trait RequestFactory {
    fn build_request(&self) -> crate::objects::RequestBuilder;
}

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
