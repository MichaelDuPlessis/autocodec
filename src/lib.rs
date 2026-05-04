//! # autocodec
//!
//! Derive macro for automatic binary protocol serialization and deserialization.
//!
//! Annotate your structs and enums with `#[derive(Codec)]` to automatically generate
//! efficient binary encoding and decoding. All multi-byte integers use big-endian (network
//! byte order) by default, with per-field little-endian override via `#[codec(endian = "little")]`.
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
//! ```
//!
//! ## Enums
//!
//! Enums are prefixed with a `u8` discriminant (variant index, starting at 0).
//! Unit, tuple, and struct variants are all supported.
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! enum Message {
//!     Ping,                              // discriminant 0
//!     Data { id: u32, payload: Vec<u8> }, // discriminant 1
//!     Ack(u64),                          // discriminant 2
//! }
//!
//! let msg = Message::Data { id: 42, payload: vec![1, 2, 3] };
//! let mut buf = Vec::new();
//! msg.encode(&mut buf);
//!
//! let (decoded, _) = Message::decode(&buf).unwrap();
//! assert_eq!(decoded, msg);
//! ```
//!
//! ## Composability
//!
//! Any field whose type implements `Codec` works automatically, including nested structs:
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
//! ## Per-field Endianness
//!
//! ```
//! use autocodec::Codec;
//!
//! #[derive(Debug, PartialEq, Codec)]
//! struct Mixed {
//!     #[codec(endian = "big")]
//!     big_val: u32,
//!     #[codec(endian = "little")]
//!     little_val: u32,
//! }
//! ```
//!
//! ## Supported Types
//!
//! | Type | Wire format |
//! |------|-------------|
//! | `u8`, `i8` | 1 byte |
//! | `u16`, `i16` | 2 bytes, big-endian |
//! | `u32`, `i32` | 4 bytes, big-endian |
//! | `u64`, `i64` | 8 bytes, big-endian |
//! | `bool` | 1 byte (0 = false, nonzero = true) |
//! | `String` | u32 length prefix + UTF-8 bytes |
//! | `Vec<T>` | u32 length prefix + N encoded elements |
//! | `Option<T>` | u8 tag (0 = None, 1 = Some) + value |
//! | `[T; N]` | N encoded elements (no length prefix) |

pub use autocodec_derive::Codec;

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
    /// An enum's discriminant byte did not match any known variant.
    UnknownDiscriminant { value: u8 },
}

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
        }
    }
}

impl std::error::Error for CodecError {}

/// Trait for types that can be encoded to and decoded from a binary format.
///
/// Implementations are generated automatically via `#[derive(Codec)]`. Built-in
/// implementations are provided for primitive integers, `bool`, `String`, `Vec<T>`,
/// `Option<T>`, and fixed-size arrays `[T; N]`.
///
/// # Wire Format
///
/// - Structs: fields encoded sequentially in declaration order.
/// - Enums: a `u8` discriminant followed by the variant's fields.
/// - Multi-byte integers: big-endian by default.
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
    ///
    /// # Errors
    ///
    /// Returns [`CodecError`] if the input is malformed or too short.
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError>;

    /// Encode this value by appending bytes to `buf`.
    fn encode(&self, buf: &mut Vec<u8>);
}

#[inline]
fn check(input: &[u8], n: usize) -> Result<(), CodecError> {
    if input.len() < n {
        Err(CodecError::NotEnoughBytes { needed: n, available: input.len() })
    } else {
        Ok(())
    }
}

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
        }
    )*};
}

impl_int!(u8, u16, u32, u64, i8, i16, i32, i64);

/// Trait for types that support little-endian encoding.
///
/// Used internally by the derive macro when a field is annotated with
/// `#[codec(endian = "little")]`. You generally don't need to use this directly.
pub trait CodecLe: Sized {
    /// Decode from little-endian bytes.
    fn decode_le(input: &[u8]) -> Result<(Self, &[u8]), CodecError>;
    /// Encode as little-endian bytes.
    fn encode_le(&self, buf: &mut Vec<u8>);
}

macro_rules! impl_int_le {
    ($($t:ty),*) => {$(
        impl CodecLe for $t {
            #[inline]
            fn decode_le(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
                const N: usize = core::mem::size_of::<$t>();
                check(input, N)?;
                let val = <$t>::from_le_bytes(input[..N].try_into().unwrap());
                Ok((val, &input[N..]))
            }
            #[inline]
            fn encode_le(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }
        }
    )*};
}

impl_int_le!(u8, u16, u32, u64, i8, i16, i32, i64);

/// Decode a value using little-endian byte order.
///
/// Called by generated code for fields annotated with `#[codec(endian = "little")]`.
#[doc(hidden)]
#[inline]
pub fn decode_le<T: CodecLe>(input: &[u8]) -> Result<(T, &[u8]), CodecError> {
    T::decode_le(input)
}

/// Encode a value using little-endian byte order.
///
/// Called by generated code for fields annotated with `#[codec(endian = "little")]`.
#[doc(hidden)]
#[inline]
pub fn encode_le<T: CodecLe>(val: &T, buf: &mut Vec<u8>) {
    val.encode_le(buf);
}

impl Codec for bool {
    /// Decodes a single byte as a boolean (0 = false, nonzero = true).
    #[inline]
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (b, rest) = u8::decode(input)?;
        Ok((b != 0, rest))
    }

    /// Encodes as a single byte: `1` for true, `0` for false.
    #[inline]
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(if *self { 1 } else { 0 });
    }
}

impl<T: Codec> Codec for Vec<T> {
    /// Decodes a `u32` element count followed by that many elements.
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, mut rest) = u32::decode(input)?;
        let mut vec = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let (item, remaining) = T::decode(rest)?;
            vec.push(item);
            rest = remaining;
        }
        Ok((vec, rest))
    }

    /// Encodes a `u32` element count followed by each element.
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.len() as u32).encode(buf);
        for item in self {
            item.encode(buf);
        }
    }
}

impl Codec for String {
    /// Decodes a `u32` byte length followed by UTF-8 bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CodecError::InvalidUtf8`] if the bytes are not valid UTF-8.
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (len, rest) = u32::decode(input)?;
        let n = len as usize;
        check(rest, n)?;
        let s = std::str::from_utf8(&rest[..n])
            .map_err(|_| CodecError::InvalidUtf8)?
            .to_owned();
        Ok((s, &rest[n..]))
    }

    /// Encodes a `u32` byte length followed by the string's UTF-8 bytes.
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.len() as u32).encode(buf);
        buf.extend_from_slice(self.as_bytes());
    }
}

impl<T: Codec> Codec for Option<T> {
    /// Decodes a `u8` tag: 0 means `None`, any other value means `Some(T)` follows.
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let (tag, rest) = u8::decode(input)?;
        match tag {
            0 => Ok((None, rest)),
            _ => {
                let (val, rest) = T::decode(rest)?;
                Ok((Some(val), rest))
            }
        }
    }

    /// Encodes `0u8` for `None`, or `1u8` followed by the value for `Some`.
    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            None => 0u8.encode(buf),
            Some(val) => {
                1u8.encode(buf);
                val.encode(buf);
            }
        }
    }
}

impl<T: Codec, const N: usize> Codec for [T; N] {
    /// Decodes exactly `N` elements sequentially (no length prefix on the wire).
    fn decode(input: &[u8]) -> Result<(Self, &[u8]), CodecError> {
        let mut rest = input;
        let mut arr: [core::mem::MaybeUninit<T>; N] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        for item in arr.iter_mut() {
            let (val, remaining) = T::decode(rest)?;
            item.write(val);
            rest = remaining;
        }
        let result = unsafe { core::ptr::read(&arr as *const _ as *const [T; N]) };
        core::mem::forget(arr);
        Ok((result, rest))
    }

    /// Encodes exactly `N` elements sequentially (no length prefix).
    fn encode(&self, buf: &mut Vec<u8>) {
        for item in self {
            item.encode(buf);
        }
    }
}
