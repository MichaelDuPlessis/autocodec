# autocodec

A Rust derive macro for automatic binary protocol serialization and deserialization. Zero runtime dependencies.

## Features

- `#[derive(Codec)]` on structs and enums — generates encode/decode automatically
- Big-endian (network byte order) by default, per-field or container-level little-endian override
- Configurable length prefixes (`u8`, `u16`, `u32`, `u64`)
- Custom enum discriminants via `#[repr(u8)]` + native `= N` syntax
- Bitfield packing with `#[codec(bits = N)]`
- Field validation, padding, magic constants, skip with custom defaults
- Custom codec delegation via `#[codec(with = "module")]`
- Zero-copy parsing with `Bytes<'a>`
- Allocation guards against malicious inputs (16 MiB limit)
- Computed `encoded_size()` without allocation
- Error context with field names for debugging
- Composable — any `Codec` type works as a field

## Quick Start

```rust
use autocodec::{Codec, CodecError};

#[derive(Debug, PartialEq, Codec)]
struct Header {
    version: u16,
    length: u32,
}

#[derive(Debug, PartialEq, Codec)]
enum Message {
    Ping,
    Data { id: u32, payload: Vec<u8> },
    Ack(u64),
}

// Encode
let header = Header { version: 1, length: 128 };
let mut buf = Vec::new();
header.encode(&mut buf);

// Decode
let (decoded, remaining) = Header::decode(&buf).unwrap();
assert_eq!(decoded, header);

// Decode exact (errors on trailing bytes)
let decoded = Header::decode_exact(&buf).unwrap();

// Pre-calculate size without encoding
assert_eq!(header.encoded_size(), 6);
```

## Supported Types

| Type | Wire format |
|------|-------------|
| `u8`–`u64`, `u128` | N bytes, big-endian |
| `i8`–`i64`, `i128` | N bytes, big-endian |
| `f32`, `f64` | IEEE 754, big-endian |
| `bool` | 1 byte |
| `String` | u32 length + UTF-8 |
| `Vec<T>` | u32 length + elements |
| `Option<T>` | u8 tag + value |
| `[T; N]` | N elements (no prefix) |
| `Box<T>` | transparent |
| `Box<[T]>` | u32 length + elements (like Vec) |
| `(A, B, ...)` | sequential (up to 8) |
| `HashMap<K, V>` | u32 length + pairs |
| `Bytes<'a>` | u32 length + bytes (zero-copy) |

## Attributes

### Container-level

```rust
#[derive(Codec)]
#[codec(endian = "little")]              // default endianness for all fields
#[codec(discriminant_type = "u16")]      // enum discriminant size
```

### Enum discriminants

Use Rust's native `#[repr]` and `= N` syntax:

```rust
#[derive(Codec)]
#[repr(u8)]
enum Command {
    Ping = 1,
    Pong = 2,
    Data = 10,
}
```

Or use `#[codec(discriminant = N)]` on individual variants.

### Field-level

```rust
#[codec(endian = "little")]              // per-field endianness
#[codec(len = "u8")]                     // length prefix type for Vec/String
#[codec(min_len = 1)]                    // minimum length
#[codec(max_len = 256)]                  // maximum length
#[codec(skip)]                           // not on wire, uses Default
#[codec(skip, default = "42")]           // not on wire, custom default
#[codec(padding = 3)]                    // N zero bytes after field
#[codec(magic = 0xDEADBEEF)]             // constant value, error on mismatch
#[codec(validate = "is_valid")]          // fn(&T) -> bool, error if false
#[codec(with = "my_module")]             // custom encode/decode functions
#[codec(bits = 4)]                       // bitfield: pack into N bits
```

## Bitfields

Consecutive fields annotated with `#[codec(bits = N)]` are packed into the minimum number of bytes (MSB-first):

```rust
#[derive(Codec)]
struct TcpFlags {
    #[codec(bits = 4)]
    version: u8,
    #[codec(bits = 4)]
    header_len: u8,
    #[codec(bits = 1)]
    syn: u8,
    #[codec(bits = 1)]
    ack: u8,
    #[codec(bits = 6)]
    reserved: u8,
    // Total: 16 bits = 2 bytes on the wire
}
```

## Zero-Copy Parsing

For read-only access to byte slices without allocation:

```rust
use autocodec::Bytes;

let data = [0, 0, 0, 3, 0xAA, 0xBB, 0xCC];
let (bytes, rest) = Bytes::decode(&data).unwrap();
assert_eq!(&*bytes, &[0xAA, 0xBB, 0xCC]);
// `bytes` borrows directly from `data` — no copy
```

## Error Handling

Errors include field context for debugging:

```rust
// "in field `version`: not enough bytes: needed 2, have 1"
```

Error types:
- `NotEnoughBytes` — input too short
- `InvalidUtf8` — string not valid UTF-8
- `UnknownDiscriminant` — enum variant not recognized
- `TooShort` / `TooLong` — length constraint violated
- `BadMagic` — magic constant mismatch
- `ValidationFailed` — custom validation returned false
- `TrailingBytes` — unexpected data after `decode_exact`
- `AllocationTooLarge` — length prefix exceeds 16 MiB safety limit
- `FieldError` — wraps any of the above with the field name

## About

This library was entirely AI-generated (by [Kiro](https://kiro.dev)) under my supervision and direction. I reviewed all the code, so any questionable decisions are mine — not the AI's.

## License

MIT OR Apache-2.0
