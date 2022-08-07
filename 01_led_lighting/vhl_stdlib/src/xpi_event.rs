use crate::discrete::U;
use crate::q_numbers::UQ;
use crate::units::UnitStatic;
use crate::varint::{VarInt, vlu4};

/// Unique node id in the context of the Link
/// May be absent if the link is point-to-point with only 2 nodes.
pub type NodeId = Option<u32>;

/// Resource index / serial
/// LSB bit of each nibble == 1 means there is another nibble carrying 3 more bits.
/// Little endian.
/// Minimum size is 4b => 0..=7
/// 8b => 0..=63
/// 12b => 0..=511
/// 16b => 0..=4095
pub type UriPart = VarInt<vlu4>;

/// Sequence of numbers uniquely identifying an xPI resource
/// If there is a group in the uri with not numerical index - it must be mapped into numbers.
///
/// Variable length encoding is used consisting of nibbles. Uri = PartCount followed by Parts.
/// Smallest size = 4 bits => empty Uri.
/// 8 bits => up to 8 resources from root == / one of 8
/// 12 bits => Uri(/ one of 8 / one of 8) or Uri(/one of 64)
/// 16 bits => Uri(/ one of 8 / one of 64) or Uri(/one of 64 / one of 8) or Uri(/ one of 8 / one of 8 / one of 8)
/// And so one with 4 bits steps.
/// 32 bits => 28 bits used for Uri = 7 nibbles each carrying 3 bits => up to 2_097_152 resources addressable.
/// Most of the realistic use cases will fall into 12 or 16 bits, resulting in a very compact uri
pub type Uri<'i> = &'i [UriPart];

/// Priority selection: lossy or lossless (to an extent).
/// Truly lossless mode is not achievable, for example if connection is physically lost mid-transfer,
/// or memory is exceeded.
///
/// Higher priority in either mode means higher chance of successfully transferring a message.
/// If channels is wide enough, all messages will go through unaffected.
///
/// Some form of fair queueing must be implemented not to starve lossy channels by lossless ones.
/// Or several underlying channels may be used to separate the two. Up to the Link to decide on
/// implementation.
///
/// Some form of rate shaping should be implemented to be able to work with different channel speeds.
/// Rates can be changed in real time, limiting property observing or streams bandwidth.
/// TCP algorithms for congestion control may be applied here?
/// Alternatively discrete event simulation may be attempted to prove lossless properties.
/// Knowing streaming rates and precise size of various messages can help with that.
///
/// If loss occurs in lossy mode, it is silently ignored.
/// If loss occurs in lossless mode, it is flagged as an error.
///
/// Priority may be mapped into fewer levels by the underlying Link? (needed for constrained channels)
pub enum Priority {
    Lossy(U<7>), // numbers must be u<7, +1> (range 1..=128) or natural to avoid confusions
    Lossless(U<7>),
}

/// Each outgoing request must be marked with an increasing number in order to distinguish
/// requests of the same kind and map responses.
/// Might be narrowed down to less bits. Detect an overflow when old request(s) was still unanswered.
/// Should pause in that case or cancel all old requests. Overflow is ignored for subscriptions.
pub type RequestId = u16;

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
pub enum UriMask<'i> {
    /// Allows to choose any subgroup of up to 128 resources
    /// Resource serial are mapped as Little Endian, so that adding resources to the end do not change previously used masks.
    ByBitfield8(u8),
    ByBitfield16(u16),
    ByBitfield32(u32),
    ByBitfield64(u64),
    ByBitfield128(u128),
    /// Allows to choose one or more resource by their indices
    ByIndices(&'i [UriPart]),
    /// Select all resources
    All
}

/// Allows to select any combination of resources in order to perform read/write or stream
/// operations on them all at once. Operations are performed sequentially in order of the resources
/// serial numbers, depth first. Responses to read requests or stream published values are arranged
/// in arbitrary order, that is deemed optimal at a time, all with proper uris attached, so it's possible
/// to distinguish them. In response to one request, one or many responses may arrive.
/// Maximum packets sizes, publishing and observing rates, maximum jitter is taken into account when
/// grouping responses together.
///
/// Examples:
/// (/a, bitfield: 0b110), (/b, bitfield: 0b011) selects /a/2, /a/3, /b/x, /b/y
/// (/b, bitfield: 0b100) select /b/z/u and /b/z/v
/// (/b/z, indexes: 1) selects /b/z/v
pub type MultiUri<'i> = &'i [(Uri<'i>, UriMask<'i>)];

/// Global type id from the Registry
pub struct GlobalTypeId {
    pub id: U<38>,

}

/// Unique identifier compatibility checker of a type inside the Registry.
pub struct GlobalTypeIdBound {
    /// Globally unique identifier of any type or trait. Created when publishing to Registry from:
    /// username + project name + file name + module name + identifier
    pub unique_id: GlobalTypeId,
    /// Which version to choose from
    pub semver_req: SemVerBound,
}

/// Requests are sent to the Link by the initiator of an exchange, which can be any node on the Link.
/// One or several Responses are sent back for each kind of request.
///
/// In case of subscribing to property updates or streams, responses will continue to arrive
/// until unsubscribed, stream exhausted or closed or one of the nodes rebooting.
///
/// After subscribers node reboot, one or more responses may arrive, until publishing nodes notices
/// subscribers reboot, unless subscribed again.
pub struct XpiRequest<'req> {
    /// Destination node or nodes
    pub node_set: NodeSet<'req>,
    /// Set of resources that are considered in this request
    pub resource_set: XpiResourceSet<'req>,
    /// What kind of operation is request on a set of resources
    pub kind: XpiRequestKind<'req>,
    /// Modulo number to map responses with requests.
    /// When wrapping to 0, if there are any outgoing unanswered requests that are not subscriptions.
    pub request_id: RequestId,
    /// Priority selection: lossy or lossless (to an extent).
    pub priority: Priority,
}

pub enum NodeSet<'i> {
    /// Request is targeted at only one specific node.
    /// Any resources can be used from the node's vhL description.
    Unicast(NodeId),
    /// Request is targeted at many nodes at once. Only nodes implementing a set of common traits can
    /// be addressed that way.
    ///
    /// Trait in this context is an xPI block defined and published to the Registry with a particular version.
    /// Might be thought of as an abstract class as well.
    ///
    /// Examples of xpi traits:
    /// * log - to e.g. subscribe to all node's logs at once
    /// * bootloader - to e.g. request all firmware versions
    /// * power_mgmt - to e.g. put all nodes to sleep
    /// Other more specific traits that only some nodes would implement:
    /// * led_feedback - to e.g. enable or disable led on devices
    /// * canbus_counters - to monitor CANBus status across the whole network
    Multicast {
        /// List of traits a node have to implement.
        /// Uri structure is arranged differently for this kind of requests.
        /// For example if 3 traits were provided, then there are /0, /1, /2 resources,
        /// each corresponding to the trait specified, in order.
        /// So e.g. it is possible to call 3 different functions from 3 different traits in one request.
        traits: &'i [GlobalTypeIdBound],
    },
    // Broadcast,
}

/// It is possible to perform operations on a set of resources at once for reducing requests and
/// responses amount.
///
/// If operation is only targeted at one resource, there are more efficient ways to select it than
/// using [MultiUri].
/// It is possible to select one resource in several different ways for efficiency reasons.
/// If there are several choices on how to construct the same uri, select the smallest one in size.
/// If both choices are the same size, choose [Uri].
///
/// [MultiUri] is the only way to select several resources at once within one request.
pub enum XpiResourceSet<'i> {
    /// One of the alternative addressing modes.
    /// Selects / one of 16.
    /// Size required is 4 bits. Same Uri would be 12 bits.
    Alpha(U<4>),

    /// One of the alternative addressing modes.
    /// Selects / one of 16 / one of 16.
    /// Size required is 8 bits. Same Uri would be 20 bits.
    Beta(U<4>, U<4>),

    /// One of the alternative addressing modes.
    /// Selects / one of 16 / one of 16 / one of 16.
    /// Size required is 12 bits. Same Uri would be 28 bits.
    Gamma(U<4>, U<4>, U<4>),

    /// One of the alternative addressing modes.
    /// Selects / one of 64 / one of 8 / one of 8.
    /// Size required is 12 bits. Same Uri would be 20 bits.
    Delta(U<6>, U<3>, U<3>),

    /// One of the alternative addressing modes.
    /// Selects / one of 64 / one of 64 / one of 16.
    /// Size required is 16 bits. Same Uri would be 28 bits.
    Epsilon(U<6>, U<6>, U<4>),

    /// Select any one resource at any depth.
    /// May use more space than alpha-epsilon modes.
    /// Size required is variable, most use cases should be in the range of 16-20 bits.
    /// Minimum size is 4 bits for 0 sized Uri (root / resource) - also the way to select
    /// root resource (probably never needed).
    Uri(Uri<'i>),

    /// Selects any set of resources at any depths at once.
    /// Use more space than Uri and alpha-epsilon modes but selects a whole set at once.
    /// Minimum size is 12 bits for one 0 sized Uri and [UriMask::All] - selecting all resources
    /// at root level ( / * ).
    MultiUri(MultiUri<'i>),
}

/// Select what to do with one ore more selected resources.
pub enum XpiRequestKind<'req> {
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
        /// Arguments must be serialized with the chosen [Wire Format](https://github.com/vhrdtech/vhl/blob/master/book/src/wire_formats/wire_formats.md)
        /// Need to get buffer for serializing from user code, which decides how to handle memory
        args: &'req[ &'req [u8] ],
    },

    /// Perform f(g(h(... (args) ...))) call on the destination node, saving
    /// round trip request and replies.
    /// Arguments must be compatible across all the members of a chain.
    /// One response is sent back for the outer most function.
    /// May not be supported by all nodes.
    /// Do not cover all the weird use cases, so maybe better be replaced with full-blown expression
    /// executor only were applicable and really needed?
    ChainCall {
        args: &'req [u8],
    },

    /// Read one or more resources.
    /// Reading several resources at once is more efficient as only one req-rep is needed in best case.
    /// Resources that support reads are: const, ro, ro + stream, rw, rw + stream
    Read,

    /// Write one or more resources.
    /// Resources that support writes are: wo, wo + stream, rw, rw + stream, stream_in<T> when open only.
    Write {
        /// Must be exactly the size of non-zero resources selected for writing in order of
        /// increasing serial numbers, depth first.
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
    OpenStreams,

    /// Closes one or more streams.
    /// Can be used as an end mark for writing a file for example.
    CloseStreams,

    /// Subscribe to property changes or streams.
    /// Resources must be be rw + stream, ro + stream or stream_out<T>.
    ///
    /// To change rates, subscribe again to the same or different set of resources.
    ///
    /// Publishers must avoid emitting changes with higher than requested rates.
    Subscribe {
        /// For each uri there must be a specified [Rate] provided.
        rates: &'req [Rate],
    },

    // /// Request a change in properties observing or stream publishing rates.
    // ChangeRates {
    //     /// For each uri there must be a specified [Rate] provided.
    //     rates: &'req [Rate],
    // },

    /// Unsubscribe from one or many resources, unsubscribing from a stream do not close it,
    /// but releases a borrow, so that someone else can subscribe and continue receiving data.
    Unsubscribe,

    /// Borrow one or many resources for exclusive use. Only work ons streams and Cell<T> resources.
    /// Other nodes will receive an error if they attempt to access borrowed resources.
    ///
    /// Nodes may implement more logic to allow or block borrowing of a resource.
    /// For example expecting a correct configuration or a key first.
    /// /main {
    ///     /key<wo String> {}
    ///     /dangerous_things<Cell<_>> {
    ///         /wipe_data<fn()> {}
    ///     }
    /// }
    /// In this example one would first have to write a correct key and then try to borrow
    /// /dangerous_things. If the key is incorrect, borrow can be rejected. Stronger security
    /// algorithms can probably be also implemented to granularly restrict access.
    /// Link between the nodes can also be encrypted, with a common key or a set of keys between all nodes.
    /// Encryption is out of scope of this document though.
    ///
    /// Might be a good idea to introduce some limitation on how many borrows can be made from one node.
    /// Depends on the kind of resource. Do not protect against malicious attempts, as node ids can be
    /// faked, but can prevent bugs.
    Borrow,

    /// Release resources for others to use.
    Release,

    /// Get information about resources.
    /// Type information for all resources.
    /// In addition:
    /// * Cell<T>: whether resource is borrowed or not.
    /// * stream_in<T> or stream_out<T>: whether stream is opened or
    /// not (when implicit Cell is already borrowed) + subscribers info + rates.
    /// * +stream: subscribers info + rates
    /// * fn: nothing at the moment
    /// * const: nothing at the moment
    /// * array of resources: size of the array
    GetInfo,
}

/// Replies are sent to the Link in response to requests.
/// One request can result in one or more replies.
/// For subscriptions and streams many replies will be sent asynchronously.
pub struct XpiReply<'rep> {
    /// Source node id that yielded reply
    pub source_node: NodeId,
    /// Kind of reply
    pub kind: XpiRequestKind<'rep>,
    /// Set of resources that are considered in this reply
    pub resource_set: XpiResourceSet<'rep>,
    /// Original request id used to map responses to requests.
    /// None for StreamsUpdates kind.
    pub request_id: Option<RequestId>,
}
/// Reply to a previously made request
/// Each reply must also be linked with:
/// request id that was sent initially
/// Source node id
pub enum XpiReplyKind<'rep> {
    /// Result of an each call
    CallComplete(Result<&'rep [u8], FailReason>),

    /// Result of an each read.
    ReadComplete(Result<&'rep [&'rep [u8]], FailReason>),

    /// Result of an each read
    WriteComplete(Result<(), FailReason>),

    /// Result of an attempt to open a stream.
    /// If stream was closed before (and inherently not borrowed), Borrow(Ok(())) is received,
    /// followed by OpenStream(Ok(()))
    OpenStream(Result<(), FailReason>),

    /// Changed property or new element of a stream.
    /// request_id for this case is None, as counter may wrap many times while subscriptions are active.
    /// Mapping is straight forward without a request_id, since uri for each resource is known.
    /// Distinguishing between different updates is not needed as in case of 2 function calls vs 1 for example.
    ///
    /// Updates may be silently lost if lossy mode is selected, more likely so with lower priority.
    ///
    /// Updates are very unlikely to be lost in lossless mode, unless underlying channel is destroyed
    /// or memory is exceeded, in which case only an error can be reported to flag the issue.
    /// If lossless channel is affected, CloseStream is yielded with a failure reason indicated in it.
    StreamUpdate(&'rep [u8]),

    /// Result of an attempt to close a stream or unrecoverable loss in lossless mode (priority > 0).
    /// If stream was open before (and inherently borrowed by self node), Close(Ok(())) is received,
    /// followed by Release(Ok(())).
    CloseStream(Result<(), FailReason>),

    /// Result of an attempt to subscribe to a stream or observable property
    /// On success Some(current value) is returned for a property, first available item is returned
    /// for streams, if available during subscription time.
    Subscribe(Result<Option<&'rep [u8]>, FailReason>),

    /// Result of a request to change observing / publishing rate.
    RateChange(Result<(), FailReason>),

    /// Result of an attempt to unsubscribe from a stream of from an observable property.
    /// Unsubscribing twice will result in an error.
    Unsubscribe(Result<(), FailReason>),

    /// Result of a resource borrow
    Borrow(Result<(), FailReason>),
    /// Result of a resource release
    Release(Result<(), FailReason>),

    /// Result of a GetInfo request
    Info(Result<ResourceInfo<'rep>, FailReason>),
}

pub enum ResourceInfo<'i> {
    FreeResource,
    BorrowedResource {
        borrowed_by: NodeId
    },
    ClosedStream,
    OpenStream {
        /// As all streams are implicitly wrapped in a Cell<_> in order to use it, node have to
        /// make a borrow first.
        borrowed_by: NodeId,
        /// TODO: Not sure whether multiple stream subscribers is needed, and how to get around Cell in that case
        subscribers: &'i [NodeId],
        rates: RatesInfo,
    },
    RwStreamProperty {
        subscribers: &'i [NodeId],
        /// Incoming data rates
        rates_in: RatesInfo,
        /// Outgoing data rates
        rates_out: RatesInfo,
    },
    WoStreamProperty {
        subscribers: &'i [NodeId],
        /// Incoming data rates
        rates_in: RatesInfo,
    },
    RoStreamProperty {
        subscribers: &'i [NodeId],
        /// Outgoing data rates
        rates_out: RatesInfo,
    },
    Array {
        size: VarInt<vlu4>,
    }
}

pub struct RatesInfo {
    /// Current instant rate of this stream, may differ from requested by congestion control
    current_rate: Rate,
    /// Rate that was requested when subscribing
    requested_rate: Rate,
    /// Maximum allowed rate of this stream
    maximum_rate: Rate,
}

/// Bidirectional functionality of the Link. Node discovery and heartbeats.
/// Self node id
/// No request id is sent or received for XpiMulti
pub enum XpiBroadcast<'mul> {
    /// Broadcast request to all the nodes to announce themselves.
    /// Up to the user how to actually implement this (for example zeroconf or randomly
    /// delayed transmissions on CAN Bus if unique IDs wasn't assigned yet).
    DiscoverNodes,
    /// Sent by nodes in response to [XpiRequest::DiscoverNodes]. Received by everyone else.
    NodeInfo(NodeInfo<'mul>),
    /// Sent by all nodes periodically, received by all nodes.
    /// Must be sent with maximum lossy priority.
    /// If emergency stop messages exist in a system, heartbeats should be sent with the next lower priority.
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

/// Observing or publishing rate in [Hz].
pub struct Rate(UnitStatic<UQS<24, 8>, -1, 0, 0, 0, 0, 0, 0>);