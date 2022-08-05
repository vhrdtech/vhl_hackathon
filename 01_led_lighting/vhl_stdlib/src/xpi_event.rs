/// Unique node id in the context of the Link
/// May be absent if the link is point-to-point with only 2 nodes.
pub type NodeId = Option<u32>;
/// Sequence of numbers uniquely identifying an xPI resource
/// If there is a group in the uri with not numerical index - maybe map to numbers as well?
pub type Uri<'i> = &'i [u16];
/// * 1 and higher — losses unacceptable to an extent, re-transmissions must be done, according to priority level.
/// * 0 — losses are acceptable, no re-transmissions, e.g. heartbeat (maybe it actually should be high priority).
/// * -1 and lower — losses are acceptable, but priority is given to lower numbers,
///     e.g. -1 can be assigned to a temperature stream and -2 to actuator position stream.
pub type Priority = i8;
/// Each outgoing request must be marked with an increasing number in order to distinguish
///     requests of the same kind and map responses
/// Might be narrowed down to less bits. Detect an overflow when old request(s) was still unanswered.
/// Should pause in that case or cancel all old requests.
pub type RequestId = u32;

/// Outgoing requests from the node into the Link.
/// Self node's id and RequestId should also be added to distinguish requests and map responses back to them.
pub enum XpiRequest<'req> {
    /// Broadcast request to all the nodes to announce themselves.
    /// Up to the user how to actually implement this (for example zeroconf or randomly
    /// delayed transmissions on CAN Bus if unique IDs wasn't assigned yet).
    DiscoverNodes,
    /// Request binary descriptor block from a node.
    /// Descriptor block is a compiled binary version of a vhL source.
    /// It carries all the important information that is needed to interact with the node.
    /// Including:
    /// * All the data types, also those coming from dependencies
    /// * Unique IDs of all the dependencies and of itself (everything must be published to the
    ///     repository before binary block can be compiled or dirty flag can be set for dev)
    /// * All the xPI blocks with strings (names, descriptions), examples and valid values.
    ///
    /// [Format description (notion)](https://www.notion.so/vhrdtech/Descriptor-block-d0fb717035574255a9baebdb18b8a4f2)
    GetDescriptorBlock {
        from: NodeId,
        uri: Uri<'req>,
    },
    /// Initiate a method call to a node's resource. Indicate selected priority to the Link.
    /// Results in [XpiReply::FnCallFailed] or [XpiReply::FnReturn].
    FnCall {
        from: NodeId,
        uri: Uri<'req>,
        priority: Priority,
        /// Arguments must be serialized with the chosen [Wire Format](https://github.com/vhrdtech/vhl/blob/master/book/src/wire_formats/wire_formats.md)
        /// Need to get buffer for serializing from user code, which decides how to handle memory
        args: &'req [u8],
    },
}

pub enum XpiReply<'rep> {
    /// Sent by nodes in response to [XpiRequest::DiscoverNodes]
    NodeDiscovery(NodeId, NodeInfo<'rep>),
    /// Sent by nodes periodically to understand their status and keep subscriptions/streams going
    Heartbeat(NodeId, HeartbeatInfo),
    /// Failed function call
    FnCallFailed {
        req_id: RequestId,
        reason: FailReason,
    },
    /// Successful return of a function call
    FnReturn {
        /// Same id that was sent during FnCall request.
        req_id: RequestId,
        /// Need to deserialize return values with the same Wire Format used on request.
        value: &'rep [u8]
    }
}

pub struct NodeInfo<'info> {
    /// User friendly name of the node, maybe changeable through it's xPI
    friendly_name: &'info str,
    /// Information about the underlying platform this node is running on
    running_on: PlatformInfo,
    /// UUID of the node, shouldn't change during operation, may change on reboot or can be fixed in firmware
    uuid: u128,
    /// Unique id of the project in [vhL Registry](https://www.notion.so/vhrdtech/vhL-Registry-5799542cf9dd41b0a92c702aa05f8c42).
    /// Node must implement and follow vhL sources of the exact version published
    vhl_registry_id: u32,
    /// Version of the project in the Registry.
    vhl_version: SemVer,
}

pub enum PlatformInfo {
    Mcu {
        // series, core, hw_info (name, revision, variant), firmware_info (name, features, version, repo+sha, crc, size, signature)
    },
    Wasm {
        running_on: PlatformInfo,
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
pub struct HeartbeatInfo {
    health: NodeHealthStatus,
    uptime_seconds: u32,
}

pub enum NodeHealthStatus {
    /// Fully functioning node
    Norminal,
    /// Node can perform it's task, but is experiencing troubles
    Warning,
    /// Node cannot perform it's task
    Failure
}

pub enum FailReason {
    /// No response was received in time
    Timeout,
    /// Node reboot was detected before it was able to answer
    DeviceRebooted,
    /// Request or response wasn't fitted into memory because more important data was needing space at a time.
    PriorityLoss,

}