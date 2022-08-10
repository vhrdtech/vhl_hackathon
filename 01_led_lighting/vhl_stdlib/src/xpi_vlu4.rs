//! Compact impementation of xPI requests and replies based on variable length encoding.
//! Nibble (4 bits) level access to buffers are used to save substantial amount of space for
//! lower bandwidth channels (for example CAN Bus). With the tricks employed, it is for exampple
//! possible to encode up to 4 function calls into 6 bytes, leaving one byte free while the last
//! byte is used by UAVCAN trasport layer.
//!
//! One request or reply takes 3+ nibbles depending on the Uri length and resource tree
//! organization.
//!
//!

//! Resource index / serial
//! LSB bit of each nibble == 1 means there is another nibble carrying 3 more bits.
//! Little endian.
//! Minimum size is 4b => 0..=7
//! 8b => 0..=63
//! 12b => 0..=511
//! 16b => 0..=4095
// pub type UriPart = VarInt<vlu4>;

//! Variable length encoding is used consisting of nibbles. Uri = PartCount followed by Parts.
//! Smallest size = 4 bits => empty Uri.
//! 8 bits => up to 8 resources from root == / one of 8
//! 12 bits => Uri(/ one of 8 / one of 8) or Uri(/one of 64)
//! 16 bits => Uri(/ one of 8 / one of 64) or Uri(/one of 64 / one of 8) or Uri(/ one of 8 / one of 8 / one of 8)
//! And so one with 4 bits steps.
//! 32 bits => 28 bits used for Uri = 7 nibbles each carrying 3 bits => up to 2_097_152 resources addressable.
//! Most of the realistic use cases will fall into 12 or 16 bits, resulting in a very compact uri
// pub type Uri<'i> = &'i [UriPart];

//! It is possible to perform operations on a set of resources at once for reducing requests and
//! responses amount.
//!
//! If operation is only targeted at one resource, there are more efficient ways to select it than
//! using [MultiUri].
//! It is possible to select one resource in several different ways for efficiency reasons.
//! If there are several choices on how to construct the same uri, select the smallest one in size.
//! If both choices are the same size, choose [Uri].
//!
//! [MultiUri] is the only way to select several resources at once within one request.
// pub enum XpiResourceSet<'i> {
    // One of the alternative addressing modes.
    // Selects / one of 16.
    // Size required is 4 bits. Same Uri would be 12 bits.
    // Alpha(U4),

    // One of the alternative addressing modes.
    // Selects / one of 16 / one of 16.
    // Size required is 8 bits. Same Uri would be 20 bits.
    // Beta(U4, U4),

    // One of the alternative addressing modes.
    // Selects / one of 16 / one of 16 / one of 16.
    // Size required is 12 bits. Same Uri would be 28 bits.
    // Gamma(U4, U4, U4),

    // One of the alternative addressing modes.
    // Selects / one of 64 / one of 8 / one of 8.
    // Size required is 12 bits. Same Uri would be 20 bits.
    // Delta(U6, U3, U3),

    // One of the alternative addressing modes.
    // Selects / one of 64 / one of 64 / one of 16.
    // Size required is 16 bits. Same Uri would be 28 bits.
    // Epsilon(U6, U6, U4),

    // Select any one resource at any depth.
    // May use more space than alpha-epsilon modes.
    // Size required is variable, most use cases should be in the range of 16-20 bits.
    // Minimum size is 4 bits for 0 sized Uri (root / resource) - also the way to select
    // root resource (probably never needed).
    // Uri(Uri<'i>),

    // Selects any set of resources at any depths at once.
    // Use more space than Uri and alpha-epsilon modes but selects a whole set at once.
    // Minimum size is 12 bits for one 0 sized Uri and [UriMask::All] - selecting all resources
    // at root level ( / * ).
    // MultiUri(MultiUri<'i>),
// }