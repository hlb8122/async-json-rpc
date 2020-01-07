pub mod http;

use std::fmt;

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

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match self {
            Error::BatchDuplicateResponseId(err) => {
                return write!(f, "duplicate batch response id, {}", err)
            }
            Error::Connection(err) => return err.fmt(f),
            Error::EmptyBatch => "empty batch",
            Error::Json(err) => return err.fmt(f),
            Error::NonceMismatch => "nonce mismatch",
            Error::VersionMismatch => "version mismatch",
            Error::WrongBatchResponseId(err) => {
                return write!(f, "wrong batch response id, {}", err)
            }
            Error::WrongBatchResponseSize => "wrong batch response size",
        };
        write!(f, "{}", printable)
    }
}
