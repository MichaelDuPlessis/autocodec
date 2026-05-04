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
    assert_eq!(NonEmpty::decode(&buf), Err(CodecError::TooShort { min: 1, actual: 0 }));
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
    assert_eq!(Bounded::decode(&buf), Err(CodecError::TooLong { max: 3, actual: 4 }));
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
    let result = WithMagic::decode(&buf);
    assert_eq!(result, Err(CodecError::BadMagic));
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
    assert_eq!(Validated::decode(&buf), Err(CodecError::ValidationFailed));
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
    assert_eq!(Header::decode(&[0x00]), Err(CodecError::NotEnoughBytes { needed: 2, available: 1 }));
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
