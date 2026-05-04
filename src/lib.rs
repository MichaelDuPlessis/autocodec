//! # autocodec
//!
//! Derive macro for automatic binary protocol serialization and deserialization.
//! Zero runtime dependencies.
//!
//! Annotate your structs and enums with `#[derive(Codec)]` to automatically generate
//! efficient binary encoding and decoding. All multi-byte integers use big-endian (network
//! byte order) by default.
//!
//! ## Quick Start
//!
//! ```
//! use autocodec::{Codec, CodecError};
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Header {
//!     version: u16,
//!     length: u32,
//! }
//!
//! let header = Header { version: 1, length: 128 };
//!
//! // Encode
//! let mut buf = Vec::new();
//! header.encode(&mut buf);
//! assert_eq!(buf.len(), 6); // 2 + 4 bytes
//!
//! // Decode
//! let (decoded, remaining) = Header::decode(&buf).unwrap();
//! assert_eq!(decoded, header);
//! assert!(remaining.is_empty());
//!
//! // Decode exact (errors if trailing bytes)
//! let decoded = Header::decode_exact(&buf).unwrap();
//!
//! // Pre-calculate encoded size without allocating
//! assert_eq!(header.encoded_size(), 6);
//! ```
//!
//! ## Enums
//!
//! Enums use a discriminant byte. Supports `#[repr(u8)]` with native `= N` syntax:
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! #[repr(u8)]
//! enum Command {
//!     Ping = 1,
//!     Pong = 2,
//!     Data = 10,
//! }
//!
//! let mut buf = Vec::new();
//! Command::Data.encode(&mut buf);
//! assert_eq!(buf, [10]);
//! let (cmd, _) = Command::decode(&[2]).unwrap();
//! assert_eq!(cmd, Command::Pong);
//! ```
//!
//! Enums with fields work too:
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! enum Message {
//!     Ping,
//!     Data { id: u32, payload: Vec<u8> },
//!     Ack(u64),
//! }
//!
//! let msg = Message::Data { id: 42, payload: vec![1, 2, 3] };
//! let mut buf = Vec::new();
//! msg.encode(&mut buf);
//! let (decoded, _) = Message::decode(&buf).unwrap();
//! assert_eq!(decoded, msg);
//! ```
//!
//! ## Composability
//!
//! Any field whose type implements `Codec` works automatically:
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Header { version: u16 }
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Packet {
//!     header: Header,
//!     payload: Vec<u8>,
//! }
//!
//! let pkt = Packet { header: Header { version: 1 }, payload: vec![0xFF; 10] };
//! let mut buf = Vec::new();
//! pkt.encode(&mut buf);
//! let (decoded, _) = Packet::decode(&buf).unwrap();
//! assert_eq!(decoded, pkt);
//! ```
//!
//! ## Endianness
//!
//! Per-field or container-level:
//!
//! ```
//! use autocodec::Codec;
//!
//! // Container-level: all fields default to little-endian
//! #[derive(Debug, PartialEq, Codec)]
//! #[codec(endian = "little")]
//! struct LittlePacket {
//!     a: u16,
//!     b: u32,
//! }
//!
//! // Per-field override
//! #[derive(Debug, PartialEq, Codec)]
//! struct MixedPacket {
//!     #[codec(endian = "little")]
//!     little: u32,
//!     big: u32, // default big-endian
//! }
//! ```
//!
//! ## Length Prefixes
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Compact {
//!     #[codec(len = "u8")]
//!     items: Vec<u8>,       // 1-byte length prefix (max 255)
//!     #[codec(len = "u16")]
//!     name: String,         // 2-byte length prefix
//!     data: Vec<u8>,        // default u32 length prefix
//! }
//! ```
//!
//! ## Length Validation
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Bounded {
//!     #[codec(min_len = 1)]
//!     items: Vec<u8>,       // must have at least 1 element
//!     #[codec(max_len = 100)]
//!     name: String,         // must be at most 100 bytes
//! }
//! ```
//!
//! ## Bitfields
//!
//! Consecutive `bits` fields are packed into minimum bytes (MSB-first):
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Flags {
//!     #[codec(bits = 4)]
//!     version: u8,
//!     #[codec(bits = 4)]
//!     header_len: u8,
//!     #[codec(bits = 1)]
//!     syn: u8,
//!     #[codec(bits = 1)]
//!     ack: u8,
//!     #[codec(bits = 6)]
//!     reserved: u8,
//!     // 16 bits total = 2 bytes on the wire
//! }
//!
//! let flags = Flags { version: 4, header_len: 5, syn: 1, ack: 0, reserved: 0 };
//! let mut buf = Vec::new();
//! flags.encode(&mut buf);
//! assert_eq!(buf.len(), 2);
//! ```
//!
//! ## Skip, Default, Padding
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Protocol {
//!     id: u32,
//!     #[codec(skip)]
//!     cached: u32,              // not on wire, Default on decode
//!     #[codec(skip, default = "42")]
//!     answer: u32,              // not on wire, custom default
//!     #[codec(padding = 3)]
//!     flags: u8,                // 3 zero bytes inserted after this field
//! }
//! ```
//!
//! ## Magic Constants
//!
//! ```
//! use autocodec::{Codec, CodecError};
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Frame {
//!     #[codec(magic = 0xCAFEBABE)]
//!     _magic: u32,              // validated on decode, error if mismatch
//!     version: u16,
//! }
//!
//! // Decoding with wrong magic fails:
//! let bad = [0, 0, 0, 0, 0, 1];
//! assert!(Frame::decode(&bad).is_err());
//! ```
//!
//! ## Validation
//!
//! ```
//! use autocodec::{Codec, CodecError};
//!
//! fn is_nonzero(val: &u16) -> bool { *val != 0 }
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Config {
//!     #[codec(validate = "is_nonzero")]
//!     port: u16,
//! }
//!
//! let bad = [0, 0]; // port = 0
//! assert!(Config::decode(&bad).is_err());
//! ```
//!
//! ## Custom Codec
//!
//! Delegate encode/decode to a module with `decode` and `encode` functions:
//!
//! ```
//! use autocodec::{Codec, CodecError};
//!
//! mod le_u16 {
//!     use autocodec::CodecError;
//!     pub fn decode(input: &[u8]) -> Result<(u16, &[u8]), CodecError> {
//!         if input.len() < 2 {
//!             return Err(CodecError::NotEnoughBytes { needed: 2, available: input.len() });
//!         }
//!         Ok((u16::from_le_bytes([input[0], input[1]]), &input[2..]))
//!     }
//!     pub fn encode(val: &u16, buf: &mut Vec<u8>) {
//!         buf.extend_from_slice(&val.to_le_bytes());
//!     }
//! }
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Custom {
//!     #[codec(with = "le_u16")]
//!     value: u16,
//! }
//! ```
//!
//! ## Zero-Copy Parsing
//!
//! ```
//! use autocodec::Bytes;
//!
//! let data = [0, 0, 0, 3, 0xAA, 0xBB, 0xCC, 0xFF];
//! let (bytes, rest) = Bytes::decode(&data).unwrap();
//! assert_eq!(&*bytes, &[0xAA, 0xBB, 0xCC]);
//! assert_eq!(rest, &[0xFF]);
//! // `bytes` borrows directly from `data` — no allocation
//! ```
//!
//! ## Error Context
//!
//! Decode errors include the field name:
//!
//! ```
//! use autocodec::{Codec, CodecError};
//!
//! #[derive(Debug, Codec)]
//! struct Msg { version: u16, length: u32 }
//!
//! let err = Msg::decode(&[0, 1]).unwrap_err(); // version ok, length fails
//! assert_eq!(err.to_string(), "in field `length`: not enough bytes: needed 4, have 0");
//! ```
//!
//! ## Supported Types
//!
//! | Type | Wire format |
//! |------|-------------|
//! | `u8`–`u128`, `i8`–`i128` | N bytes, big-endian |
//! | `f32`, `f64` | IEEE 754, big-endian |
//! | `bool` | 1 byte (0 = false, nonzero = true) |
//! | `String` | u32 length prefix + UTF-8 bytes |
//! | `Vec<T>` | u32 length prefix + N encoded elements |
//! | `Option<T>` | u8 tag (0 = None, 1 = Some) + value |
//! | `[T; N]` | N encoded elements (no length prefix) |
//! | `Box<T>` | transparent (same as T) |
//! | `(A, B, ...)` | sequential fields (up to 8 elements) |
//! | `HashMap<K, V>` | u32 length prefix + key-value pairs |
//! | `Bytes<'a>` | u32 length prefix + bytes (zero-copy) |

pub use autocodec_derive::Codec;

use std::collections::HashMap;
use std::hash::Hash;

/// Errors that can occur during decoding.
///
/// # Examples
///
/// ```
/// use autocodec::{Codec, CodecError};
///
/// // Not enough bytes to decode a u32
/// let result = u32::decode(&[0x01, 0x02]);
/// assert_eq!(result, Err(CodecError::NotEnoughBytes { needed: 4, available: 2 }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The input buffer did not contain enough bytes for the type being decoded.
    NotEnoughBytes { needed: usize, available: usize },
    /// A `String` field contained bytes that are not valid UTF-8.
    InvalidUtf8,
    /// An enum's discriminant did not match any known variant.
    UnknownDiscriminant { value: u8 },
    /// A field's length was below the required minimum.
    TooShort { min: usize, actual: usize },
    /// A field's length exceeded the allowed maximum.
    TooLong { max: usize, actual: usize },
    /// A magic/constant value did not match the expected value.
    BadMagic,
    /// A custom validation function failed.
    ValidationFailed,
    /// Input had leftover bytes after decoding (used by `decode_exact`).
    TrailingBytes { count: usize },
    /// A length prefix would require an unreasonably large allocation.
    AllocationTooLarge { requested: usize },
    /// An error occurred while decoding a specific field.
    FieldError { field: &'static str, source: Box<CodecError> },
}

/// Maximum number of elements allowed in a single Vec/String decode.
/// Prevents OOM from malicious length prefixes. Default: 16 MiB worth of elements.
pub const MAX_DECODE_LEN: usize = 16 * 1024 * 1024;

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotEnoughBytes { needed, available } => {
                write!(f, "not enough bytes: needed {needed}, have {available}")
            }
            Self::InvalidUtf8 => write!(f, "invalid UTF-8"),
            Self::UnknownDiscriminant { value } => {
                write!(f, "unknown discriminant: {value}")
            }
            Self::TooShort { min, actual } => {
                write!(f, "length too short: minimum {min}, got {actual}")
            }
            Self::TooLong { max, actual } => {
                write!(f, "length too long: maximum {max}, got {actual}")
            }
            Self::BadMagic => write!(f, "magic value mismatch"),
            Self::ValidationFailed => write!(f, "validation failed"),
            Self::TrailingBytes { count } => {
                write!(f, "trailing bytes: {count} unexpected bytes after decode")
            }
            Self::AllocationTooLarge { requested } => {
                write!(f, "allocation too large: {requested} elements requested")
            }
            Self::FieldError { field, source } => {
                write!(f, "in field `{field}`: {source}")
            }
        }
    }
}

impl std::error::Error for CodecError {}

/// Trait for types that can be encoded to and decoded from a binary format.
///
/// Implementations are generated automatically via `#[derive(Codec)]`. Built-in
/// implementations are provided for primitive integers, floats, `bool`, `String`,
/// `Vec<T>`, `Option<T>`, `Box<T>`, tuples, `HashMap<K,V>`, and fixed-size arrays.
///
/// # Wire Format
///
/// - Structs: fields encoded sequentially in declaration order.
/// - Enums: a discriminant byte followed by the variant's fields.
/// - Multi-byte integers/floats: big-endian by default.
///
/// # Examples
///
/// ```
/// use autocodec::{Codec, CodecError};
///
/// // Primitives
/// let mut buf = Vec::new();
/// 42u16.encode(&mut buf);
/// assert_eq!(buf, [0x00, 0x2A]);
///
/// let (val, rest) = u16::decode(&buf).unwrap();
/// assert_eq!(val, 42);
/// assert!(rest.is_empty());
/// ```
pub trait Codec: Sized {
    /// Decode a value from the front of `input`, returning the value and remaining bytes.
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError>;

    /// Encode this value by appending bytes to `buf`.
    fn encode(&self, buf: &mut Vec<u8>);

    /// Calculate the encoded size in bytes without actually encoding.
    /// Override for efficiency; the default allocates a temporary buffer.
    fn encoded_size(&self) -> usize {
        let mut buf = Vec::new();
        self.encode(&mut buf);
        buf.len()
    }

    /// Decode a value, returning an error if there are leftover bytes.
    fn decode_exact(input: &[u8]) -> Result<Self, CodecError> {
        let (val, rest) = Self::decode(input)?;
        if !rest.is_empty() {
            return Err(CodecError::TrailingBytes { count: rest.len() });
        }
        Ok(val)
    }
}

#[doc(hidden)]
#[inline]
pub fn check_len(input: &[u8], n: usize) -> Result<(), CodecError> {
    if input.len() < n {
        Err(CodecError::NotEnoughBytes { needed: n, available: input.len() })
    } else {
        Ok(())
    }
}

#[inline]
fn check(input: &[u8], n: usize) -> Result<(), CodecError> {
    check_len(input, n)
}

// --- Integer impls ---

macro_rules! impl_int {
    ($($t:ty),*) => {$(
        impl Codec for $t {
            #[inline]
            fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
                const N: usize = core::mem::size_of::<$t>();
                check(input, N)?;
                let val = <$t>::from_be_bytes(input[..N].try_into().unwrap());
                Ok((val, &input[N..]))
            }
            #[inline]
            fn encode(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_be_bytes());
            }
            #[inline]
            fn encoded_size(&self) -> usize { core::mem::size_of::<$t>() }
        }
    )*};
}

impl_int!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

// --- Float impls ---

impl Codec for f32 {
    #[inline]
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        check(input, 4)?;
        let val = f32::from_be_bytes(input[..4].try_into().unwrap());
        Ok((val, &input[4..]))
    }
    #[inline]
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
    #[inline]
    fn encoded_size(&self) -> usize { 4 }
}

impl Codec for f64 {
    #[inline]
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        check(input, 8)?;
        let val = f64::from_be_bytes(input[..8].try_into().unwrap());
        Ok((val, &input[8..]))
    }
    #[inline]
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
    #[inline]
    fn encoded_size(&self) -> usize { 8 }
}

// --- Endian traits ---

/// Trait for explicit big-endian encoding.
#[doc(hidden)]
pub trait CodecBe: Sized {
    fn decode_be(input: &[u8]) -> Result<(Self, &[u8]), CodecError>;
    fn encode_be(&self, buf: &mut Vec<u8>);
}

/// Trait for little-endian encoding.
#[doc(hidden)]
pub trait CodecLe: Sized {
    fn decode_le(input: &[u8]) -> Result<(Self, &[u8]), CodecError>;
    fn encode_le(&self, buf: &mut Vec<u8>);
}

macro_rules! impl_endian {
    ($($t:ty),*) => {$(
        impl CodecBe for $t {
            #[inline]
            fn decode_be(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
                const N: usize = core::mem::size_of::<$t>();
                check(input, N)?;
                Ok((<$t>::from_be_bytes(input[..N].try_into().unwrap()), &input[N..]))
            }
            #[inline]
            fn encode_be(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_be_bytes());
            }
        }
        impl CodecLe for $t {
            #[inline]
            fn decode_le(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
                const N: usize = core::mem::size_of::<$t>();
                check(input, N)?;
                Ok((<$t>::from_le_bytes(input[..N].try_into().unwrap()), &input[N..]))
            }
            #[inline]
            fn encode_le(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}

impl_endian!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);

#[doc(hidden)]
#[inline]
pub fn decode_be<T: CodecBe>(input: &[u8]) -> Result<(T, &[u8]), CodecError> { T::decode_be(input) }

#[doc(hidden)]
#[inline]
pub fn encode_be<T: CodecBe>(val: &T, buf: &mut Vec<u8>) { val.encode_be(buf); }

#[doc(hidden)]
#[inline]
pub fn decode_le<T: CodecLe>(input: &[u8]) -> Result<(T, &[u8]), CodecError> { T::decode_le(input) }

#[doc(hidden)]
#[inline]
pub fn encode_le<T: CodecLe>(val: &T, buf: &mut Vec<u8>) { val.encode_le(buf); }

// --- Length prefix ---

#[doc(hidden)]
pub trait LenPrefix: Codec {
    fn to_usize(self) -> usize;
    fn from_usize(n: usize) -> Self;
}

impl LenPrefix for u8 {
    #[inline] fn to_usize(self) -> usize { self as usize }
    #[inline] fn from_usize(n: usize) -> Self { n as u8 }
}

impl LenPrefix for u16 {
    #[inline] fn to_usize(self) -> usize { self as usize }
    #[inline] fn from_usize(n: usize) -> Self { n as u16 }
}

impl LenPrefix for u32 {
    #[inline] fn to_usize(self) -> usize { self as usize }
    #[inline] fn from_usize(n: usize) -> Self { n as u32 }
}

impl LenPrefix for u64 {
    #[inline] fn to_usize(self) -> usize { self as usize }
    #[inline] fn from_usize(n: usize) -> Self { n as u64 }
}

#[doc(hidden)]
pub trait CodecWithLen: Sized {
    fn decode_with_len<L: LenPrefix>(input: &[u8]) -> Result<(Self, &[u8]), CodecError>;
    fn encode_with_len<L: LenPrefix>(&self, buf: &mut Vec<u8>);
}

impl<T: Codec> CodecWithLen for Vec<T> {
    fn decode_with_len<L: LenPrefix>(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, mut rest) = L::decode(input)?;
        let count = len.to_usize();
        if count > MAX_DECODE_LEN {
            return Err(CodecError::AllocationTooLarge { requested: count });
        }
        let mut vec = Vec::with_capacity(count.min(rest.len()));
        for _ in 0..count {
            let (item, remaining) = T::decode(rest)?;
            vec.push(item);
            rest = remaining;
        }
        Ok((vec, rest))
    }
    fn encode_with_len<L: LenPrefix>(&self, buf: &mut Vec<u8>) {
        L::from_usize(self.len()).encode(buf);
        for item in self { item.encode(buf); }
    }
}

impl CodecWithLen for String {
    fn decode_with_len<L: LenPrefix>(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, rest) = L::decode(input)?;
        let n = len.to_usize();
        if n > MAX_DECODE_LEN {
            return Err(CodecError::AllocationTooLarge { requested: n });
        }
        check(rest, n)?;
        let s = String::from_utf8(rest[..n].to_vec()).map_err(|_| CodecError::InvalidUtf8)?;
        Ok((s, &rest[n..]))
    }
    fn encode_with_len<L: LenPrefix>(&self, buf: &mut Vec<u8>) {
        L::from_usize(self.len()).encode(buf);
        buf.extend_from_slice(self.as_bytes());
    }
}

#[doc(hidden)]
#[inline]
pub fn decode_with_len<L: LenPrefix, T: CodecWithLen>(input: &[u8]) -> Result<(T, &[u8]), CodecError> {
    T::decode_with_len::<L>(input)
}

#[doc(hidden)]
#[inline]
pub fn encode_with_len<L: LenPrefix, T: CodecWithLen>(val: &T, buf: &mut Vec<u8>) {
    val.encode_with_len::<L>(buf);
}

// --- Length validation ---

#[doc(hidden)]
pub trait HasLen {
    fn codec_len(&self) -> usize;
}

impl<T> HasLen for Vec<T> {
    #[inline] fn codec_len(&self) -> usize { self.len() }
}

impl HasLen for String {
    #[inline] fn codec_len(&self) -> usize { self.len() }
}

#[doc(hidden)]
#[inline]
pub fn check_min_len<T: HasLen>(val: &T, min: usize) -> Result<(), CodecError> {
    let actual = val.codec_len();
    if actual < min { Err(CodecError::TooShort { min, actual }) } else { Ok(()) }
}

#[doc(hidden)]
#[inline]
pub fn check_max_len<T: HasLen>(val: &T, max: usize) -> Result<(), CodecError> {
    let actual = val.codec_len();
    if actual > max { Err(CodecError::TooLong { max, actual }) } else { Ok(()) }
}

/// Wrap an error with field context.
#[doc(hidden)]
#[inline]
pub fn field_err(field: &'static str, e: CodecError) -> CodecError {
    CodecError::FieldError { field, source: Box::new(e) }
}

// --- Skip helper ---

#[doc(hidden)]
#[inline]
pub fn skip_decode<T: Default>() -> T { T::default() }

// --- Padding helpers ---

#[doc(hidden)]
#[inline]
pub fn decode_padding(input: &[u8], n: usize) -> Result<&[u8], CodecError> {
    check(input, n)?;
    Ok(&input[n..])
}

#[doc(hidden)]
#[inline]
pub fn encode_padding(buf: &mut Vec<u8>, n: usize) {
    buf.extend(std::iter::repeat_n(0u8, n));
}

// --- Magic helpers ---

#[doc(hidden)]
pub fn decode_magic_u8(input: &[u8], expected: u8) -> Result<&[u8], CodecError> {
    let (val, rest) = u8::decode(input)?;
    if val != expected { return Err(CodecError::BadMagic); }
    Ok(rest)
}

#[doc(hidden)]
pub fn decode_magic_u16(input: &[u8], expected: u16) -> Result<&[u8], CodecError> {
    let (val, rest) = u16::decode(input)?;
    if val != expected { return Err(CodecError::BadMagic); }
    Ok(rest)
}

#[doc(hidden)]
pub fn decode_magic_u32(input: &[u8], expected: u32) -> Result<&[u8], CodecError> {
    let (val, rest) = u32::decode(input)?;
    if val != expected { return Err(CodecError::BadMagic); }
    Ok(rest)
}

#[doc(hidden)]
pub fn decode_magic_u64(input: &[u8], expected: u64) -> Result<&[u8], CodecError> {
    let (val, rest) = u64::decode(input)?;
    if val != expected { return Err(CodecError::BadMagic); }
    Ok(rest)
}

// --- Bitfield helpers ---

/// Extract `bits` bits starting at `bit_offset` from a byte slice (MSB-first).
#[doc(hidden)]
#[inline]
pub fn extract_bits(bytes: &[u8], bit_offset: usize, bits: usize) -> u64 {
    // Read enough bytes into a u64, then shift/mask
    let byte_start = bit_offset / 8;
    let byte_end = (bit_offset + bits).div_ceil(8);
    let n = byte_end - byte_start;

    let mut raw: u64 = 0;
    for i in 0..n {
        raw = (raw << 8) | bytes[byte_start + i] as u64;
    }

    // The bits we want are at position (n*8 - (bit_offset%8) - bits) from the right
    let shift = n * 8 - (bit_offset % 8) - bits;
    let mask = (1u64 << bits) - 1;
    (raw >> shift) & mask
}

/// Set `bits` bits starting at `bit_offset` in a byte slice (MSB-first).
#[doc(hidden)]
#[inline]
pub fn set_bits(bytes: &mut [u8], bit_offset: usize, bits: usize, val: u64) {
    let byte_start = bit_offset / 8;
    let byte_end = (bit_offset + bits).div_ceil(8);
    let n = byte_end - byte_start;

    // Read current bytes into a u64
    let mut raw: u64 = 0;
    for i in 0..n {
        raw = (raw << 8) | bytes[byte_start + i] as u64;
    }

    // Set the bits
    let shift = n * 8 - (bit_offset % 8) - bits;
    let mask = (1u64 << bits) - 1;
    raw &= !(mask << shift);
    raw |= (val & mask) << shift;

    // Write back
    for i in (0..n).rev() {
        bytes[byte_start + i] = (raw & 0xFF) as u8;
        raw >>= 8;
    }
}

// --- Standard type impls ---

impl Codec for bool {
    #[inline]
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (b, rest) = u8::decode(input)?;
        Ok((b != 0, rest))
    }
    #[inline]
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(if *self { 1 } else { 0 });
    }
    #[inline]
    fn encoded_size(&self) -> usize { 1 }
}

impl<T: Codec> Codec for Vec<T> {
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, mut rest) = u32::decode(input)?;
        let count = len as usize;
        if count > MAX_DECODE_LEN {
            return Err(CodecError::AllocationTooLarge { requested: count });
        }
        let mut vec = Vec::with_capacity(count.min(rest.len()));
        for _ in 0..count {
            let (item, remaining) = T::decode(rest)?;
            vec.push(item);
            rest = remaining;
        }
        Ok((vec, rest))
    }
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.len() as u32).encode(buf);
        for item in self { item.encode(buf); }
    }
    fn encoded_size(&self) -> usize {
        4 + self.iter().map(|i| i.encoded_size()).sum::<usize>()
    }
}

impl Codec for String {
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, rest) = u32::decode(input)?;
        let n = len as usize;
        if n > MAX_DECODE_LEN {
            return Err(CodecError::AllocationTooLarge { requested: n });
        }
        check(rest, n)?;
        let s = String::from_utf8(rest[..n].to_vec()).map_err(|_| CodecError::InvalidUtf8)?;
        Ok((s, &rest[n..]))
    }
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.len() as u32).encode(buf);
        buf.extend_from_slice(self.as_bytes());
    }
    fn encoded_size(&self) -> usize { 4 + self.len() }
}

impl<T: Codec> Codec for Option<T> {
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (tag, rest) = u8::decode(input)?;
        match tag {
            0 => Ok((None, rest)),
            _ => { let (val, rest) = T::decode(rest)?; Ok((Some(val), rest)) }
        }
    }
    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            None => 0u8.encode(buf),
            Some(val) => { 1u8.encode(buf); val.encode(buf); }
        }
    }
    fn encoded_size(&self) -> usize {
        1 + match self { None => 0, Some(val) => val.encoded_size() }
    }
}

impl<T: Codec> Codec for Box<T> {
    #[inline]
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (val, rest) = T::decode(input)?;
        Ok((Box::new(val), rest))
    }
    #[inline]
    fn encode(&self, buf: &mut Vec<u8>) {
        (**self).encode(buf);
    }
    #[inline]
    fn encoded_size(&self) -> usize { (**self).encoded_size() }
}

impl<K: Codec + Eq + Hash, V: Codec> Codec for HashMap<K, V> {
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, mut rest) = u32::decode(input)?;
        let mut map = HashMap::with_capacity(len as usize);
        for _ in 0..len {
            let (k, remaining) = K::decode(rest)?;
            let (v, remaining) = V::decode(remaining)?;
            map.insert(k, v);
            rest = remaining;
        }
        Ok((map, rest))
    }
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.len() as u32).encode(buf);
        for (k, v) in self { k.encode(buf); v.encode(buf); }
    }
    fn encoded_size(&self) -> usize {
        4 + self.iter().map(|(k, v)| k.encoded_size() + v.encoded_size()).sum::<usize>()
    }
}

impl<T: Codec, const N: usize> Codec for [T; N] {
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let mut rest = input;
        let mut vec = Vec::with_capacity(N);
        for _ in 0..N {
            let (val, remaining) = T::decode(rest)?;
            vec.push(val);
            rest = remaining;
        }
        let arr: [T; N] = match vec.try_into() {
            Ok(a) => a,
            Err(_) => unreachable!(),
        };
        Ok((arr, rest))
    }
    fn encode(&self, buf: &mut Vec<u8>) {
        for item in self { item.encode(buf); }
    }
    fn encoded_size(&self) -> usize {
        self.iter().map(|i| i.encoded_size()).sum()
    }
}

// --- Tuple impls ---

macro_rules! impl_tuple {
    ($($idx:tt $T:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($T: Codec),+> Codec for ($($T,)+) {
            fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
                $(let ($T, input) = $T::decode(input)?;)+
                Ok((($($T,)+), input))
            }
            fn encode(&self, buf: &mut Vec<u8>) {
                $(self.$idx.encode(buf);)+
            }
            fn encoded_size(&self) -> usize {
                0 $(+ self.$idx.encoded_size())+
            }
        }
    };
}

impl_tuple!(0 A);
impl_tuple!(0 A, 1 B);
impl_tuple!(0 A, 1 B, 2 C);
impl_tuple!(0 A, 1 B, 2 C, 3 D);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);

// --- Zero-copy bytes ---

/// A zero-copy byte slice reference for efficient binary protocol parsing.
///
/// Unlike `Vec<u8>`, `Bytes` borrows directly from the input buffer without copying.
/// Wire format: u32 length prefix followed by raw bytes (same as `Vec<u8>`).
///
/// # Example
///
/// ```
/// use autocodec::{Bytes, CodecError};
///
/// let data = [0, 0, 0, 3, 0xAA, 0xBB, 0xCC, 0xFF];
/// let (bytes, rest) = Bytes::decode(&data).unwrap();
/// assert_eq!(bytes.as_ref(), &[0xAA, 0xBB, 0xCC]);
/// assert_eq!(rest, &[0xFF]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bytes<'a>(pub &'a [u8]);

impl<'a> Bytes<'a> {
    /// Decode a length-prefixed byte slice from input without copying.
    pub fn decode(input: &'a [u8]) -> Result<(Self, &'a [u8]), CodecError> {
        let (len, rest) = u32::decode(input)?;
        let n = len as usize;
        if n > MAX_DECODE_LEN {
            return Err(CodecError::AllocationTooLarge { requested: n });
        }
        check(rest, n)?;
        Ok((Bytes(&rest[..n]), &rest[n..]))
    }

    /// Decode with a custom length prefix type.
    pub fn decode_with_len<L: LenPrefix>(input: &'a [u8]) -> Result<(Self, &'a [u8]), CodecError> {
        let (len, rest) = L::decode(input)?;
        let n = len.to_usize();
        if n > MAX_DECODE_LEN {
            return Err(CodecError::AllocationTooLarge { requested: n });
        }
        check(rest, n)?;
        Ok((Bytes(&rest[..n]), &rest[n..]))
    }

    /// Encode this byte slice with a u32 length prefix.
    pub fn encode(&self, buf: &mut Vec<u8>) {
        (self.0.len() as u32).encode(buf);
        buf.extend_from_slice(self.0);
    }

    /// Encoded size in bytes.
    pub fn encoded_size(&self) -> usize { 4 + self.0.len() }
}

impl<'a> AsRef<[u8]> for Bytes<'a> {
    fn as_ref(&self) -> &[u8] { self.0 }
}

impl<'a> core::ops::Deref for Bytes<'a> {
    type Target = [u8];
    fn deref(&self) -> &[u8] { self.0 }
}
