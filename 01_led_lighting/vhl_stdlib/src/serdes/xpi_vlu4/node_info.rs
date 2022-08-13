#[derive(Copy, Clone, Debug)]
pub struct NodeInfo<'info> {
    /// User friendly name of the node, maybe changeable through it's xPI
    pub friendly_name: &'info str,
    /// Information about the underlying platform this node is running on
    pub running_on: PlatformInfo,
    /// UUID of the node, shouldn't change during operation, may change on reboot or can be fixed in firmware
    pub uuid: u128,
    /// Unique id of the project in [vhL Registry](https://www.notion.so/vhrdtech/vhL-Registry-5799542cf9dd41b0a92c702aa05f8c42).
    /// Node must implement and follow vhL sources of the exact version published
    pub vhl_registry_id: u32,
    /// Version of the project in the Registry.
    pub vhl_version: Version,
}

#[derive(Copy, Clone, Debug)]
pub enum PlatformInfo {
    Mcu {
        // series, core, hw_info (name, revision, variant), firmware_info (name, features, version, repo+sha, crc, size, signature)
    },
    Wasm {
        // running_on: PlatformInfo,
        // vm_info:
    },
    Mac,
    Linux,
    Windows,
    Ios,
    Android,
    Web,
    Other
}

/// Distributed periodically by all active nodes
/// Counter resetting means device has rebooted and all active subscriptions to it must be re-done.
/// Overflow over u32::MAX doesn't count.
///
/// More specific node status and information might be made available through it's specific xPI.
///
/// CAN Bus note: should be possible to encode more data into the same frame for more specific info.
/// So that resources are preserved. Expose it through node's own xPI.
#[derive(Copy, Clone, Debug)]
pub struct HeartbeatInfo {
    pub health: NodeHealthStatus,
    pub uptime_seconds: u32,
}

#[derive(Copy, Clone, Debug)]
pub enum NodeHealthStatus {
    /// Fully functioning node
    Norminal,
    /// Node can perform it's task, but is experiencing troubles
    Warning,
    /// Node cannot perform it's task
    Failure
}

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
