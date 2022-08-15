use thiserror::Error;
use crate::serdes::nibble_buf;

#[derive(Copy, Clone, Debug)]
pub enum FailReason {
    /// No response was received in time
    Timeout,
    /// Node reboot was detected before it was able to answer
    DeviceRebooted,
    /// Request or response wasn't fitted into memory because more important data was needing space at a time.
    PriorityLoss,
    /// Request rejected by rate shaper, even if space was available, not to exceed underlying channel bandwidth.
    /// Rejecting function calls and other non-streaming operations must be avoided.
    /// First lossy requests / subscriptions should be shaped. Then lossless (while still giving a fair
    /// chance to lossy ones) and in the latest are all other requests and responses.
    ShaperReject,
    /// When trying to access a resource that was already borrowed by someone else
    ResourceIsAlreadyBorrowed,
    /// When trying to unsubscribe twice from a resource
    AlreadyUnsubscribed,
    /// When trying to open a stream twice
    StreamIsAlreadyOpen,
    /// When trying to close a stream twice
    StreamIsAlreadyClosed,
    /// When trying to write into a const or ro property, write into stream_out or read from stream_in.
    OperationNotSupported,
}

#[derive(Error, Debug)]
pub enum XpiVlu4Error {
    #[error("Nibble buf reader error")]
    NibbleBuf(#[from] nibble_buf::Error),
    #[error("Unreachable reached")]
    InternalError,
    #[error("Reserved uri mask type")]
    UriMaskReserved,
    #[error("Unsupported uri mask type")]
    UriMaskUnsupportedType,

}