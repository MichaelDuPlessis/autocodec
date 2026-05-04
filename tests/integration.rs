use autocodec::{Codec, CodecError};

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

#[derive(Debug, PartialEq, Codec)]
enum Message {
    Ping,
    Data { id: u32, payload: Vec<u8> },
    Ack(u64),
}

#[derive(Debug, PartialEq, Codec)]
struct Nested {
    header: Header,
    name: String,
}

#[derive(Debug, PartialEq, Codec)]
struct WithOptional {
    id: u32,
    label: Option<String>,
}

#[derive(Debug, PartialEq, Codec)]
struct WithArray {
    data: [u8; 4],
    values: [u16; 3],
}

#[derive(Debug, PartialEq, Codec)]
struct MixedEndian {
    #[codec(endian = "big")]
    big: u32,
    #[codec(endian = "little")]
    little: u32,
}

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
    let u = Unit;
    let mut buf = Vec::new();
    u.encode(&mut buf);
    assert!(buf.is_empty());
    let (decoded, rest) = Unit::decode(&buf).unwrap();
    assert_eq!(decoded, u);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_enum_unit() {
    let msg = Message::Ping;
    let mut buf = Vec::new();
    msg.encode(&mut buf);
    assert_eq!(buf, vec![0]);
    let (decoded, rest) = Message::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_enum_named() {
    let msg = Message::Data { id: 99, payload: vec![1, 2, 3] };
    let mut buf = Vec::new();
    msg.encode(&mut buf);
    let (decoded, rest) = Message::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_enum_tuple() {
    let msg = Message::Ack(0xDEADBEEF);
    let mut buf = Vec::new();
    msg.encode(&mut buf);
    let (decoded, rest) = Message::decode(&buf).unwrap();
    assert_eq!(decoded, msg);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_nested() {
    let n = Nested {
        header: Header { version: 2, length: 100, flags: 0x01 },
        name: "hello".to_string(),
    };
    let mut buf = Vec::new();
    n.encode(&mut buf);
    let (decoded, rest) = Nested::decode(&buf).unwrap();
    assert_eq!(decoded, n);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_option_some() {
    let val = WithOptional { id: 7, label: Some("test".into()) };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, rest) = WithOptional::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_option_none() {
    let val = WithOptional { id: 7, label: None };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, rest) = WithOptional::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_fixed_array() {
    let val = WithArray { data: [0xAA, 0xBB, 0xCC, 0xDD], values: [1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // 4 bytes for data + 6 bytes for values (3 * u16)
    assert_eq!(buf.len(), 10);
    let (decoded, rest) = WithArray::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn mixed_endian_encoding() {
    let val = MixedEndian { big: 0x01020304, little: 0x01020304 };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // big-endian: 01 02 03 04
    assert_eq!(&buf[0..4], &[0x01, 0x02, 0x03, 0x04]);
    // little-endian: 04 03 02 01
    assert_eq!(&buf[4..8], &[0x04, 0x03, 0x02, 0x01]);
    let (decoded, rest) = MixedEndian::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn error_not_enough_bytes() {
    let result = Header::decode(&[0x00]);
    assert_eq!(result, Err(CodecError::NotEnoughBytes { needed: 2, available: 1 }));
}

#[test]
fn error_unknown_discriminant() {
    let result = Message::decode(&[255]);
    assert_eq!(result, Err(CodecError::UnknownDiscriminant { value: 255 }));
}

#[test]
fn error_invalid_utf8() {
    let data = [0, 0, 0, 2, 0xFF, 0xFE];
    let result = String::decode(&data);
    assert_eq!(result, Err(CodecError::InvalidUtf8));
}

#[test]
fn roundtrip_vec_string() {
    let v: Vec<String> = vec!["foo".into(), "bar".into()];
    let mut buf = Vec::new();
    v.encode(&mut buf);
    let (decoded, rest) = <Vec<String>>::decode(&buf).unwrap();
    assert_eq!(decoded, v);
    assert!(rest.is_empty());
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

#[test]
fn roundtrip_custom_len_u8() {
    let val = ShortVec { items: vec![1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // u8 len (1 byte) + 3 * u16 (6 bytes) = 7 bytes
    assert_eq!(buf.len(), 7);
    assert_eq!(buf[0], 3); // u8 length prefix
    let (decoded, rest) = ShortVec::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn roundtrip_custom_len_u16_string() {
    let val = ShortString { name: "hello".into() };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    // u16 len (2 bytes) + 5 bytes = 7 bytes
    assert_eq!(buf.len(), 7);
    assert_eq!(&buf[0..2], &[0, 5]); // u16 big-endian length prefix
    let (decoded, rest) = ShortString::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[derive(Debug, PartialEq, Codec)]
struct NonEmpty {
    #[codec(min_len = 1)]
    items: Vec<u8>,
}

#[test]
fn min_len_accepts_valid() {
    let val = NonEmpty { items: vec![1, 2, 3] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let (decoded, rest) = NonEmpty::decode(&buf).unwrap();
    assert_eq!(decoded, val);
    assert!(rest.is_empty());
}

#[test]
fn min_len_rejects_empty() {
    let val = NonEmpty { items: vec![] };
    let mut buf = Vec::new();
    val.encode(&mut buf);
    let result = NonEmpty::decode(&buf);
    assert_eq!(result, Err(CodecError::TooShort { min: 1, actual: 0 }));
}
