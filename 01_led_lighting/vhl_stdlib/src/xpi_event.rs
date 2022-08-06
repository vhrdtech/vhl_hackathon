use crate::discrete::U;
use crate::q_numbers::UQ;
use crate::units::UnitStatic;

/// Unique node id in the context of the Link
/// May be absent if the link is point-to-point with only 2 nodes.
pub type NodeId = Option<u32>;

/// Sequence of numbers uniquely identifying an xPI resource
/// If there is a group in the uri with not numerical index - maybe map to numbers as well?
/// Variable length encoding can be used that will result in 4/8/16 or 32 bits
/// smallest size = 4 bits - 3 bits used - up to 7 resources, so that 49 resources can be addressed with just 1 byte
/// 8 bits - 6 bits used - up to 63 resources
/// 16 bits - 13 bits used - up to 4095 resources
/// 32 bits - 28 bits used - up to 268_435_455 resources
/// Most of the real use cases will fall into 4 or 8 bits, resulting in a very compact uri
pub type Uri<'i> = &'i [U<28>];

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

/// Mask that allows to select many resources at a particular level. Used in combination with [Uri] to
/// select the level to which UriMask applies.
/// /a
///     /1
///     /2
///     /3
/// /b
///     /x
///     /y
///     /z
///         /u
///         /v
/// For example at level /a LevelMask::ByBitfield(0b011) selects /a/2 and /a/3
/// If the same mask were applied at level /b then /b/y and /b/z would be selected.
pub enum LevelMask<'i> {
    /// Allows to choose any subgroup of up to 128 resources
    /// In Little Endian, so that adding resources to the end do not change previously used masks.
    ByBitfield(u128),
    /// Allows to choose one or more resource by their indices
    ByIndexes(&'i [u16]),
    /// Select all resources
    All
}

/// Allows to select any combination of resources in order to perform read/write or stream
/// operations on them all at once. Operations are performed sequentially in order of the resources
/// serial numbers, depth first. Responses to read requests or stream published values are arranged
/// in arbitrary order, that is deemed optimal at a time, all with proper uris attached, so it's possible
/// to distinguish them. So in response to one request, one or many responses may arrive.
/// Maximum packets sizes, publishing and observing rates, maximum jitter is taken into account when
/// grouping responses together.
///
/// Examples:
/// (/a, bitfield: 0b110), (/b, bitfield: 0b011) selects /a/2, /a/3, /b/x, /b/y
/// (/b, bitfield: 0b100) select /b/z/u and /b/z/v
/// (/b/z, indexes: 1) selects /b/z/v
pub type MultiUri<'i> = &'i [(Uri<'i>, LevelMask<'i>)];

/// Outgoing requests from the node into the Link.
/// When submitting request, additional values must be also provided:
/// Self node's id to distinguish nodes
/// Destination node id
/// RequestId to distinguish requests and map responses back to them
/// Priority to properly queue things
pub enum XpiRequest<'req> {
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
    //GetDescriptorBlock, -> move to a stream_out<chunk> resource, can also add separate const property with a link to the vhL source

    /// Call one or more methods.
    /// Results in [XpiReply::FnCallFailed] or [XpiReply::FnReturn] for each method.
    Call {
        uris: MultiUri<'req>,
        /// Arguments must be serialized with the chosen [Wire Format](https://github.com/vhrdtech/vhl/blob/master/book/src/wire_formats/wire_formats.md)
        /// Need to get buffer for serializing from user code, which decides how to handle memory
        args: &'req[ &'req [u8] ],
    },

    /// Read one or more resources.
    /// Reading several resources at once is more efficient as only one req-rep is needed in best case.
    /// Resources that support reads are: const, ro, ro + stream, rw, rw + stream
    Read(MultiUri<'req>),

    /// Write one or more resources.
    /// Resources that support writes are: wo, wo + stream, rw, rw + stream, stream_in<T> when open only.
    Write {
        uris: MultiUri<'req>,
        values: &'req[ &'req [u8] ],
    },

    /// Open one or more streams for read, writes, publishing or subscribing.
    /// stream_in<T> can be written into or published to.
    /// It is only a hint to codegen to create more useful abstractions, there is no functional
    /// difference between publishing or writing.
    ///
    /// stream_out<T> can be read or subscribed to.
    /// In contrast with writing vs publishing, reading is different from subscribing, as only
    /// one result is returned on read, but one or many after subscribing.
    ///
    /// Only opened streams can be written into, read from or subscribed to.
    /// Stream thus have a start and an end in contrast to properties with a +stream modifier.
    /// Stream are also inherently Borrowable (so writing stream_in<T> is equivalent to Cell<stream_in<T>>).
    /// When opening a closed stream, it is automatically borrowed. Opening an open stream returns an error.
    OpenStreams(MultiUri<'req>),

    /// Closes one or more streams.
    /// Can be used as an end mark for writing a file for example.
    CloseStreams(MultiUri<'req>),

    /// Subscribe to property changes or streams
    /// For each uri there must be a specified [Rate] provided.
    /// Resources must be be rw + stream, ro + stream or stream_out<T>.
    Subscribe {
        uris: MultiUri<'req>,
        rates: &'req [Rate],
    },

    /// Request a change in properties observing or stream publishing rates.
    ChangeRates {
        uris: MultiUri<'req>,
        rates: &'req [Rate],
    },

    /// Unsubscribe from one or many resources, unsubscribing from a stream do not close it,
    /// but releases a borrow, so that someone else can subscribe and continue receiving data.
    Unsubscribe(MultiUri<'req>),

    /// Borrow one or many resources for exclusive use. Only work ons streams and Cell<T> resources.
    /// Other nodes will receive an error if they attempt to access borrowed resources.
    Borrow(MultiUri<'req>),

    /// Release resources for others to use.
    Release(MultiUri<'req>),
}

/// Reply to a previously made request
/// Each reply must also be linked with:
/// request id that was sent initially
/// Source node id
pub enum XpiReply<'rep> {
    /// Result of an each call
    CallComplete(Result<&'rep [u8], FailReason>),

    /// Result of an each read.
    ReadComplete(Result<&'rep [u8], FailReason>),

    /// Result of an each read
    WriteComplete(Result<(), FailReason>),

    /// Result of an attempt to open a stream.
    /// If stream was closed before (and inherently not borrowed), Borrow(Ok(())) is received,
    /// followed by OpenStream(Ok(()))
    OpenStream(Result<(), FailReason>),
    /// Result of an attempt to close a stream.
    /// If stream was open before (and inherently borrowed by self node), Close(Ok(())) is received,
    /// followed by Release(Ok(())).
    CloseStream(Result<(), FailReason>),

    /// Result of an attempt to subscribe to a stream or observable property
    /// On success Some(current value) is returned for a property, first available chunk is returned
    /// for streams, if available during subscription time.
    Subscribe(Result<&'rep [u8], FailReason>),

    /// Result of a request to change observing / publishing rate.
    RateChange(Result<(), FailReason>),

    /// Result of an attempt to unsubscribe from a stream of from an observable property.
    /// Unsubscribing twice will result in an error.
    Unsubscribe(Result<(), FailReason>),

    /// Result of a resource borrow
    Borrow(Result<(), FailReason>),
    /// Result of a resource release
    Release(Result<(), FailReason>),
}

/// Bidirectional functionality of the Link. Node discovery and heartbeats.
/// Self node id
/// No request id is sent or received for XpiMulti
pub enum XpiMulti<'mul> {
    /// Broadcast request to all the nodes to announce themselves.
    /// Up to the user how to actually implement this (for example zeroconf or randomly
    /// delayed transmissions on CAN Bus if unique IDs wasn't assigned yet).
    DiscoverNodes,
    /// Sent by nodes in response to [XpiRequest::DiscoverNodes]. Received by everyone else.
    NodeInfo(NodeInfo<'mul>),
    /// Send by all nodes periodically, received by all nodes.
    Heartbeat(HeartbeatInfo),
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
    /// When trying to access a resource that was already borrowed by someone else
    ResourceIsAlreadyBorrowed,
    /// When trying to unsubscribe twice from a resource
    AlreadyUnsubscribed,
    /// When trying to open a stream twice
    StreamIsAlreadyOpen,
    /// When trying to close a stream twice
    StreamIsAlreadyClose,
    /// When trying to write into a const or ro property, write into stream_out or read from stream_in.
    OperationNotSupported,
}

/// Observing or publishing rate in [Hz].
pub struct Rate(UnitStatic<UQS<24, 8>, -1, 0, 0, 0, 0, 0, 0>);