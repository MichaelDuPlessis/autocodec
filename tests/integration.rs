use autocodec::{Codec, CodecError};
use std::collections::HashMap;

// --- Basic structs ---

#[derive(Debug, PartialEq, Codec)]
struct Header {
    version: u16,
    length: u32,
    flags: u8,
}

#[derive(Debug, PartialEq, Codec)]
struct Wrapper(u32);

#[derive(Debug, PartialEq, Codec)]
struct Unit;

// --- Enums ---

#[derive(Debug, PartialEq, Codec)]
enum Message {
    Ping,
    Data { id: u32, payload: Vec<u8> },
    Ack(u64),
}

// --- Nested ---

#[derive(Debug, PartialEq, Codec)]
struct Nested {
    header: Header,
    name: String,
}

// --- Option ---

#[derive(Debug, PartialEq, Codec)]
struct WithOptional {
    id: u32,
    label: Option<String>,
}

// --- Fixed array ---

#[derive(Debug, PartialEq, Codec)]
struct WithArray {
    data: [u8; 4],
    values: [u16; 3],
}

// --- Endianness ---

#[derive(Debug, PartialEq, Codec)]
struct MixedEndian {
    #[codec(endian = "big")]
    big: u32,
    #[codec(endian = "little")]
    little: u32,
}

// --- Custom length prefix ---

#[derive(Debug, PartialEq, Codec)]
struct ShortVec {
    #[codec(len = "u8")]
    items: Vec<u16>,
}

#[derive(Debug, PartialEq, Codec)]
struct ShortString {
    #[codec(len = "u16")]
    name: String,
}

// --- min_len / max_len ---

#[derive(Debug, PartialEq, Codec)]
struct NonEmpty {
    #[codec(min_len = 1)]
    items: Vec<u8>,
}

#[derive(Debug, PartialEq, Codec)]
struct Bounded {
    #[codec(max_len = 3)]
    items: Vec<u8>,
}

// --- skip ---

#[derive(Debug, PartialEq, Codec)]
struct WithSkip {
    id: u32,
    #[codec(skip)]
    cached: u32,
    name: String,
}

// --- padding ---

#[derive(Debug, PartialEq, Codec)]
struct WithPadding {
    id: u8,
    #[codec(padding = 3)]
    flags: u8,
    data: u32,
}

// --- magic ---

#[derive(Debug, PartialEq, Codec)]
struct WithMagic {
    #[codec(magic = 0xDEADBEEF)]
    _magic: u32,
    version: u16,
}

// --- validate ---

fn is_even(val: &u32) -> bool {
    val.is_multiple_of(2)
}

#[derive(Debug, PartialEq, Codec)]
struct Validated {
    #[codec(validate = "is_even")]
    value: u32,
}

// --- container endian ---

#[derive(Debug, PartialEq, Codec)]
#[codec(endian = "little")]
struct AllLittle {
    a: u16,
    b: u32,
}

// --- custom discriminant ---

#[derive(Debug, PartialEq, Codec)]
enum CustomDisc {
    #[codec(discriminant = 10)]
    Foo,
    #[codec(discriminant = 20)]
    Bar(u32),
}

// --- discriminant_type ---

#[derive(Debug, PartialEq, Codec)]
#[codec(discriminant_type = "u16")]
enum WideEnum {
    A,
    B(u8),
}

// --- f32/f64 ---

#[derive(Debug, PartialEq, Codec)]
struct Floats {
    x: f32,
    y: f64,
}

// --- Box ---

#[derive(Debug, PartialEq, Codec)]
struct Boxed {
    inner: Box<u32>,
}

// --- Tuples ---

#[derive(Debug, PartialEq, Codec)]
struct WithTuple {
    pair: (u8, u16),
}

// ============ TESTS ============

#[test]
fn roundtrip_struct() {
    let h = Header { version: 1, length: 42, flags: 0xFF };
    let mut buf = Vec::new();
    h.encode(&mut buf);
    assert_eq!(buf.len(), 7);
    let (decoded, rest) = Header::decode(&buf).unwrap();
    assert_eq!(decoded, h);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_tuple_struct() {
    let w = Wrapper(12345);
    let mut buf = Vec::new();
    w.encode(&mut buf);
    let (decoded, rest) = Wrapper::decode(&buf).unwrap();
    assert_eq!(decoded, w);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_unit_struct() {
    let mut buf = Vec::new();
    Unit.encode(&mut buf);
    assert!(buf.is_empty());
    let (decoded, rest) = Unit::decode(&buf).unwrap();
    assert_eq!(decoded, Unit);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_enum_unit() {
    let msg = Message::Ping;
    let mut buf = Vec::new();
    msg.encode(&mut buf);
    assert_eq!(buf, vec![0]);
    let (decoded, _) = Message::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn roundtrip_enum_named() {
    let msg = Message::Data { id: 99, payload: vec![1, 2, 3] };
    let mut buf = Vec::new();
    msg.encode(&mut buf);
    let (decoded, _) = Message::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn roundtrip_enum_tuple() {
    let msg = Message::Ack(0xDEADBEEF);
    let mut buf = Vec::new();
    msg.encode(&mut buf);
    let (decoded, _) = Message::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
}

#[test]
fn roundtrip_nested() {
    let n = Nested {
        header: Header { version: 2, length: 100, flags: 0x01 },
        name: "hello".to_string(),
    };
    let mut buf = Vec::new();
    n.encode(&mut buf);
    let (decoded, _) = Nested::decode(&buf).unwrap();
    assert_eq!(decoded, n);
}

#[test]
fn roundtrip_option() {
    let some = WithOptional { id: 7, label: Some("test".into()) };
    let none = WithOptional { id: 7, label: None };
    for val in [some, none] {
        let mut buf = Vec::new();
        val.encode(&mut buf);
        let (decoded, _) = WithOptional::decode(&buf).unwrap();
        assert_eq!(decoded, val);
    }
}

#[test]
fn roundtrip_fixed_array() {
    let val = WithArray { data: [0xAA, 0xBB, 0xCC, 0xDD], values: [1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 10);
    let (decoded, _) = WithArray::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn mixed_endian_encoding() {
    let val = MixedEndian { big: 0x01020304, little: 0x01020304 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(&buf[0..4], &[0x01, 0x02, 0x03, 0x04]);
    assert_eq!(&buf[4..8], &[0x04, 0x03, 0x02, 0x01]);
    let (decoded, _) = MixedEndian::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn custom_len_u8() {
    let val = ShortVec { items: vec![1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 7); // 1 + 3*2
    assert_eq!(buf[0], 3);
    let (decoded, _) = ShortVec::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn custom_len_u16_string() {
    let val = ShortString { name: "hello".into() };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 7); // 2 + 5
    let (decoded, _) = ShortString::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn min_len_accepts_valid() {
    let val = NonEmpty { items: vec![1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, _) = NonEmpty::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn min_len_rejects_empty() {
    let val = NonEmpty { items: vec![] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let err = NonEmpty::decode(&buf).unwrap_err();
    assert_eq!(err, CodecError::FieldError {
        field: "items",
        source: Box::new(CodecError::TooShort { min: 1, actual: 0 }),
    });
}

#[test]
fn max_len_accepts_valid() {
    let val = Bounded { items: vec![1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, _) = Bounded::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn max_len_rejects_too_long() {
    let val = Bounded { items: vec![1, 2, 3, 4] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let err = Bounded::decode(&buf).unwrap_err();
    assert_eq!(err, CodecError::FieldError {
        field: "items",
        source: Box::new(CodecError::TooLong { max: 3, actual: 4 }),
    });
}

#[test]
fn skip_field() {
    let val = WithSkip { id: 42, cached: 999, name: "hi".into() };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // Should not contain the cached field (4 bytes for id + 4+2 for string = 10)
    assert_eq!(buf.len(), 10);
    let (decoded, _) = WithSkip::decode(&buf).unwrap();
    assert_eq!(decoded.id, 42);
    assert_eq!(decoded.cached, 0); // Default
    assert_eq!(decoded.name, "hi");
}

#[test]
fn padding_field() {
    let val = WithPadding { id: 1, flags: 2, data: 3 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // 1 (id) + 1 (flags) + 3 (padding) + 4 (data) = 9
    assert_eq!(buf.len(), 9);
    assert_eq!(&buf[2..5], &[0, 0, 0]); // padding bytes
    let (decoded, _) = WithPadding::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn magic_field() {
    let val = WithMagic { _magic: 0, version: 1 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // 4 (magic) + 2 (version) = 6
    assert_eq!(buf.len(), 6);
    assert_eq!(&buf[0..4], &0xDEADBEEFu32.to_be_bytes());
    let (decoded, _) = WithMagic::decode(&buf).unwrap();
    assert_eq!(decoded.version, 1);
}

#[test]
fn magic_field_rejects_wrong_value() {
    let buf = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
    let err = WithMagic::decode(&buf).unwrap_err();
    assert_eq!(err, CodecError::FieldError {
        field: "_magic",
        source: Box::new(CodecError::BadMagic),
    });
}

#[test]
fn validate_accepts_valid() {
    let val = Validated { value: 4 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, _) = Validated::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn validate_rejects_invalid() {
    let val = Validated { value: 3 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let err = Validated::decode(&buf).unwrap_err();
    assert_eq!(err, CodecError::FieldError {
        field: "value",
        source: Box::new(CodecError::ValidationFailed),
    });
}

#[test]
fn container_endian() {
    let val = AllLittle { a: 0x0102, b: 0x01020304 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(&buf[0..2], &[0x02, 0x01]); // little-endian u16
    assert_eq!(&buf[2..6], &[0x04, 0x03, 0x02, 0x01]); // little-endian u32
    let (decoded, _) = AllLittle::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn custom_discriminant() {
    let foo = CustomDisc::Foo;
    let bar = CustomDisc::Bar(42);
    let mut buf = Vec::new();
    foo.encode(&mut buf);
    assert_eq!(buf[0], 10);
    let (decoded, _) = CustomDisc::decode(&buf).unwrap();
    assert_eq!(decoded, foo);

    buf.clear();
    bar.encode(&mut buf);
    assert_eq!(buf[0], 20);
    let (decoded, _) = CustomDisc::decode(&buf).unwrap();
    assert_eq!(decoded, bar);
}

#[test]
fn wide_discriminant() {
    let a = WideEnum::A;
    let b = WideEnum::B(7);
    let mut buf = Vec::new();
    a.encode(&mut buf);
    assert_eq!(&buf[0..2], &[0, 0]); // u16 discriminant = 0
    let (decoded, _) = WideEnum::decode(&buf).unwrap();
    assert_eq!(decoded, a);

    buf.clear();
    b.encode(&mut buf);
    assert_eq!(&buf[0..2], &[0, 1]); // u16 discriminant = 1
    let (decoded, _) = WideEnum::decode(&buf).unwrap();
    assert_eq!(decoded, b);
}

#[test]
fn roundtrip_floats() {
    let val = Floats { x: 1.5, y: 2.5 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 12);
    let (decoded, _) = Floats::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn roundtrip_box() {
    let val = Boxed { inner: Box::new(42) };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, _) = Boxed::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn roundtrip_tuple() {
    let val = WithTuple { pair: (1, 2) };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 3); // 1 + 2
    let (decoded, _) = WithTuple::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn roundtrip_hashmap() {
    let mut map = HashMap::new();
    map.insert(1u16, "one".to_string());
    map.insert(2u16, "two".to_string());
    let mut buf = Vec::new();
    map.encode(&mut buf);
    let (decoded, _) = <HashMap<u16, String>>::decode(&buf).unwrap();
    assert_eq!(decoded, map);
}

#[test]
fn error_not_enough_bytes() {
    let err = Header::decode(&[0x00]).unwrap_err();
    assert_eq!(err, CodecError::FieldError {
        field: "version",
        source: Box::new(CodecError::NotEnoughBytes { needed: 2, available: 1 }),
    });
}

#[test]
fn error_unknown_discriminant() {
    assert_eq!(Message::decode(&[255]), Err(CodecError::UnknownDiscriminant { value: 255 }));
}

#[test]
fn error_invalid_utf8() {
    let data = [0, 0, 0, 2, 0xFF, 0xFE];
    assert_eq!(String::decode(&data), Err(CodecError::InvalidUtf8));
}

#[test]
fn remaining_bytes_preserved() {
    let h = Header { version: 1, length: 0, flags: 0 };
    let mut buf = Vec::new();
    h.encode(&mut buf);
    buf.extend_from_slice(&[0xAA, 0xBB]);
    let (_, rest) = Header::decode(&buf).unwrap();
    assert_eq!(rest, &[0xAA, 0xBB]);
}

// --- u128/i128 ---

#[derive(Debug, PartialEq, Codec)]
struct BigInt {
    val: u128,
    signed: i128,
}

#[test]
fn roundtrip_u128() {
    let val = BigInt { val: u128::MAX, signed: -1 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 32);
    let (decoded, _) = BigInt::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

// --- decode_exact ---

#[test]
fn decode_exact_success() {
    let h = Header { version: 1, length: 2, flags: 3 };
    let mut buf = Vec::new();
    h.encode(&mut buf);
    let decoded = Header::decode_exact(&buf).unwrap();
    assert_eq!(decoded, h);
}

#[test]
fn decode_exact_trailing_bytes() {
    let h = Header { version: 1, length: 2, flags: 3 };
    let mut buf = Vec::new();
    h.encode(&mut buf);
    buf.push(0xFF);
    let result = Header::decode_exact(&buf);
    assert_eq!(result, Err(CodecError::TrailingBytes { count: 1 }));
}

// --- encoded_size ---

#[test]
fn encoded_size_works() {
    let h = Header { version: 1, length: 2, flags: 3 };
    assert_eq!(h.encoded_size(), 7);
}

// --- allocation guard ---

#[test]
fn allocation_guard_rejects_huge_vec() {
    // Craft a buffer with a u32 length prefix of 0x01000001 (16M+1)
    let mut buf = Vec::new();
    (autocodec::MAX_DECODE_LEN as u32 + 1).encode(&mut buf);
    let result = <Vec<u8>>::decode(&buf);
    assert!(matches!(result, Err(CodecError::AllocationTooLarge { .. })));
}

// --- with module ---

mod custom_codec {
    use autocodec::CodecError;

    pub fn decode(input: &[u8]) -> Result<(u16, &[u8]), CodecError> {
        // decode as little-endian u16
        if input.len() < 2 {
            return Err(CodecError::NotEnoughBytes { needed: 2, available: input.len() });
        }
        let val = u16::from_le_bytes([input[0], input[1]]);
        Ok((val, &input[2..]))
    }

    pub fn encode(val: &u16, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&val.to_le_bytes());
    }
}

#[derive(Debug, PartialEq, Codec)]
struct WithCustomCodec {
    #[codec(with = "custom_codec")]
    value: u16,
}

#[test]
fn with_module_roundtrip() {
    let val = WithCustomCodec { value: 0x0102 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(&buf, &[0x02, 0x01]); // little-endian via custom module
    let (decoded, _) = WithCustomCodec::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

// --- default expr ---

#[derive(Debug, PartialEq, Codec)]
struct WithDefault {
    id: u32,
    #[codec(skip, default = "42")]
    answer: u32,
}

#[test]
fn skip_with_custom_default() {
    let val = WithDefault { id: 1, answer: 99 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 4); // only id encoded
    let (decoded, _) = WithDefault::decode(&buf).unwrap();
    assert_eq!(decoded.id, 1);
    assert_eq!(decoded.answer, 42); // custom default, not 0
}

// --- Bitfields ---

#[derive(Debug, PartialEq, Codec)]
struct Flags {
    #[codec(bits = 1)]
    syn: u8,
    #[codec(bits = 1)]
    ack: u8,
    #[codec(bits = 1)]
    fin: u8,
    #[codec(bits = 5)]
    reserved: u8,
    // total: 8 bits = 1 byte
}

#[derive(Debug, PartialEq, Codec)]
struct TcpFlags {
    #[codec(bits = 4)]
    version: u8,
    #[codec(bits = 4)]
    ihl: u8,
    #[codec(bits = 8)]
    dscp: u8,
    // total: 16 bits = 2 bytes
}

#[test]
fn bitfield_roundtrip() {
    let val = Flags { syn: 1, ack: 0, fin: 1, reserved: 0b10101 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 1);
    // syn=1, ack=0, fin=1, reserved=10101 -> 1 0 1 10101 = 0b10110101 = 0xB5
    assert_eq!(buf[0], 0b10110101);
    let (decoded, rest) = Flags::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn bitfield_two_bytes() {
    let val = TcpFlags { version: 4, ihl: 5, dscp: 0xFF };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 2);
    // version=0100, ihl=0101 -> byte 0 = 0b01000101 = 0x45
    // dscp=11111111 -> byte 1 = 0xFF
    assert_eq!(buf[0], 0x45);
    assert_eq!(buf[1], 0xFF);
    let (decoded, _) = TcpFlags::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

// --- Error context ---

#[test]
fn error_context_shows_field_name() {
    let err = Header::decode(&[0x00, 0x01]).unwrap_err(); // version ok (2 bytes), length fails
    match err {
        CodecError::FieldError { field, .. } => assert_eq!(field, "length"),
        other => panic!("expected FieldError, got {other:?}"),
    }
}

// --- repr(u8) with native discriminants ---

#[derive(Debug, PartialEq, Codec)]
#[repr(u8)]
enum ReprEnum {
    A = 1,
    B = 5,
    C = 10,
}

#[test]
fn repr_u8_enum() {
    let mut buf = Vec::new();
    ReprEnum::A.encode(&mut buf);
    assert_eq!(buf, [1]);
    buf.clear();
    ReprEnum::B.encode(&mut buf);
    assert_eq!(buf, [5]);
    buf.clear();
    ReprEnum::C.encode(&mut buf);
    assert_eq!(buf, [10]);

    let (decoded, _) = ReprEnum::decode(&[5]).unwrap();
    assert_eq!(decoded, ReprEnum::B);

    assert!(ReprEnum::decode(&[0]).is_err());
}

#[derive(Debug, PartialEq, Codec)]
#[repr(u16)]
enum ReprU16Enum {
    X = 256,
    Y = 512,
}

#[test]
fn repr_u16_enum() {
    let mut buf = Vec::new();
    ReprU16Enum::X.encode(&mut buf);
    assert_eq!(buf, [1, 0]); // 256 big-endian
    let (decoded, _) = ReprU16Enum::decode(&buf).unwrap();
    assert_eq!(decoded, ReprU16Enum::X);
}

// --- Zero-copy Bytes ---

#[test]
fn bytes_zero_copy() {
    use autocodec::Bytes;
    let data = [0, 0, 0, 5, 1, 2, 3, 4, 5, 0xFF];
    let (bytes, rest) = Bytes::decode(&data).unwrap();
    assert_eq!(bytes.as_ref(), &[1, 2, 3, 4, 5]);
    assert_eq!(rest, &[0xFF]);
    // Verify it's truly zero-copy: the slice points into the original data
    assert_eq!(bytes.0.as_ptr(), data[4..].as_ptr());
}

#[test]
fn bytes_encode() {
    use autocodec::Bytes;
    let b = Bytes(&[0xAA, 0xBB]);
    let mut buf = Vec::new();
    b.encode(&mut buf);
    assert_eq!(buf, [0, 0, 0, 2, 0xAA, 0xBB]);
}

#[test]
fn bytes_encoded_size() {
    use autocodec::Bytes;
    let b = Bytes(&[1, 2, 3]);
    assert_eq!(b.encoded_size(), 7); // 4 + 3
}

// --- Error context in enums ---

#[derive(Debug, PartialEq, Codec)]
enum ContextEnum {
    Foo { x: u32, y: String },
}

#[test]
fn error_context_enum_variant_field() {
    // Discriminant 0, x = valid u32, y = string with bad length
    let mut buf = vec![0u8]; // disc
    buf.extend_from_slice(&42u32.to_be_bytes()); // x
    buf.extend_from_slice(&100u32.to_be_bytes()); // y length = 100 but not enough bytes

    let err = ContextEnum::decode(&buf).unwrap_err();
    match err {
        CodecError::FieldError { field, .. } => assert_eq!(field, "Foo::y"),
        other => panic!("expected FieldError, got {other:?}"),
    }
}

// --- Error context in tuple structs ---

#[test]
fn error_context_tuple_struct() {
    let err = Wrapper::decode(&[0, 0]).unwrap_err(); // needs 4 bytes, only 2
    match err {
        CodecError::FieldError { field, .. } => assert_eq!(field, "0"),
        other => panic!("expected FieldError, got {other:?}"),
    }
}

// --- encoded_size for enums ---

#[test]
fn encoded_size_enum() {
    let msg = Message::Data { id: 1, payload: vec![0; 10] };
    // disc(1) + id(4) + vec_len(4) + 10 bytes = 19
    assert_eq!(msg.encoded_size(), 19);
    assert_eq!(Message::Ping.encoded_size(), 1);
}

// --- Bitfield edge cases ---

#[derive(Debug, PartialEq, Codec)]
struct BitfieldMixed {
    #[codec(bits = 3)]
    a: u8,
    #[codec(bits = 5)]
    b: u8,
    // 8 bits = 1 byte, then a normal field
    normal: u16,
}

#[test]
fn bitfield_followed_by_normal() {
    let val = BitfieldMixed { a: 0b101, b: 0b11010, normal: 0x1234 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // a=101, b=11010 -> 10111010 = 0xBA, then u16 0x1234
    assert_eq!(buf.len(), 3);
    assert_eq!(buf[0], 0b10111010);
    assert_eq!(&buf[1..3], &[0x12, 0x34]);
    let (decoded, _) = BitfieldMixed::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[derive(Debug, PartialEq, Codec)]
struct BitfieldWide {
    #[codec(bits = 12)]
    val12: u16,
    #[codec(bits = 4)]
    val4: u8,
    // 16 bits = 2 bytes
}

#[test]
fn bitfield_wide_values() {
    let val = BitfieldWide { val12: 0xABC, val4: 0xD };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf.len(), 2);
    // 0xABC = 1010 1011 1100, 0xD = 1101 -> 1010 1011 1100 1101 = 0xABCD
    assert_eq!(buf, [0xAB, 0xCD]);
    let (decoded, _) = BitfieldWide::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

// --- Empty struct with padding ---

#[derive(Debug, PartialEq, Codec)]
struct PaddedEmpty {
    #[codec(skip, padding = 4)]
    _reserved: u32,
}

#[test]
fn empty_struct_with_padding() {
    let val = PaddedEmpty { _reserved: 0 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    assert_eq!(buf, [0, 0, 0, 0]);
    let (decoded, rest) = PaddedEmpty::decode(&[0, 0, 0, 0, 0xFF]).unwrap();
    assert_eq!(decoded._reserved, 0);
    assert_eq!(rest, &[0xFF]);
}

// --- Box<[T]> ---

#[derive(Debug, PartialEq, Codec)]
struct WithBoxedSlice {
    data: Box<[u8]>,
}

#[derive(Debug, PartialEq, Codec)]
struct WithBoxedSliceShortLen {
    #[codec(len = "u8")]
    data: Box<[u16]>,
}

#[derive(Debug, PartialEq, Codec)]
struct WithBoxedSliceMinLen {
    #[codec(min_len = 2)]
    data: Box<[u8]>,
}

#[test]
fn boxed_slice_roundtrip() {
    let val = WithBoxedSlice { data: vec![1, 2, 3].into_boxed_slice() };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // u32 len (4) + 3 bytes = 7
    assert_eq!(buf.len(), 7);
    let (decoded, rest) = WithBoxedSlice::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn boxed_slice_custom_len() {
    let val = WithBoxedSliceShortLen { data: vec![1, 2].into_boxed_slice() };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // u8 len (1) + 2 * u16 (4) = 5
    assert_eq!(buf.len(), 5);
    assert_eq!(buf[0], 2); // u8 length prefix
    let (decoded, _) = WithBoxedSliceShortLen::decode(&buf).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn boxed_slice_min_len_rejects() {
    let val = WithBoxedSliceMinLen { data: vec![1].into_boxed_slice() };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let err = WithBoxedSliceMinLen::decode(&buf).unwrap_err();
    match err {
        CodecError::FieldError { field, source } => {
            assert_eq!(field, "data");
            assert_eq!(*source, CodecError::TooShort { min: 2, actual: 1 });
        }
        other => panic!("expected FieldError, got {other:?}"),
    }
}
