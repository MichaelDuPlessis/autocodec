# autocodec

A Rust derive macro for automatic binary protocol serialization and deserialization. Zero runtime dependencies.

## Features

- `#[derive(Codec)]` on structs and enums
- Big-endian by default (network byte order)
- Per-field and container-level endianness control
- Configurable length prefixes (`u8`, `u16`, `u32`, `u64`)
- Custom enum discriminant values and types
- Field validation, padding, magic constants, skip
- Custom codec delegation via `#[codec(with = "module")]`
- Allocation guards against malicious inputs
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
| `(A, B, ...)` | sequential (up to 8) |
| `HashMap<K, V>` | u32 length + pairs |

## Attributes

### Container-level

```rust
#[derive(Codec)]
#[codec(endian = "little")]              // default endianness for all fields
#[codec(discriminant_type = "u16")]      // enum discriminant size (u8/u16/u32)
```

### Variant-level

```rust
#[codec(discriminant = 100)]             // custom discriminant value
```

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
```

## Error Types

- `NotEnoughBytes` — input too short
- `InvalidUtf8` — string not valid UTF-8
- `UnknownDiscriminant` — enum variant not recognized
- `TooShort` / `TooLong` — length constraint violated
- `BadMagic` — magic constant mismatch
- `ValidationFailed` — custom validation returned false
- `TrailingBytes` — unexpected data after `decode_exact`
- `AllocationTooLarge` — length prefix exceeds safety limit (16 MiB)

## License

MIT OR Apache-2.0
