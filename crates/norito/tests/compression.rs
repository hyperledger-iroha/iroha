//! Compression-related Norito roundtrips and corruption coverage.
#![allow(clippy::manual_div_ceil)]
use iroha_schema::IntoSchema;
use norito::core::*;

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
struct Data {
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
    f: u32,
    g: u32,
    h: u32,
}

impl Data {
    fn new(val: u32) -> Self {
        Self {
            a: val,
            b: val,
            c: val,
            d: val,
            e: val,
            f: val,
            g: val,
            h: val,
        }
    }
}

#[test]
fn zstd_roundtrip() {
    let d = Data::new(1);
    let cfg = CompressionConfig { level: 1 };
    let bytes = to_compressed_bytes(&d, Some(cfg)).unwrap();
    assert!(bytes.len() < to_bytes(&d).unwrap().len());
    let archived = from_compressed_bytes::<Data>(&bytes).unwrap();
    let decoded = <Data as NoritoDeserialize>::deserialize(&archived);
    assert_eq!(decoded.a, 1);
}

#[test]
fn deterministic_compression() {
    let d = Data::new(2);
    let cfg = CompressionConfig { level: 1 };
    let b1 = to_compressed_bytes(&d, Some(cfg)).unwrap();
    let b2 = to_compressed_bytes(&d, Some(cfg)).unwrap();
    assert_eq!(b1, b2);
}

#[test]
fn no_compression() {
    let d = Data::new(3);
    let bytes = to_compressed_bytes(&d, None).unwrap();
    let archived = from_compressed_bytes::<Data>(&bytes).unwrap();
    let decoded = <Data as NoritoDeserialize>::deserialize(&archived);
    assert_eq!(decoded.d, 3);
}

#[test]
fn compressed_vec_roundtrip() {
    let payload: Vec<u32> = (0..128).collect();
    let bytes = to_compressed_bytes(&payload, Some(CompressionConfig { level: 1 })).unwrap();
    let archived = from_compressed_bytes::<Vec<u32>>(&bytes).unwrap();
    let decoded = <Vec<u32> as NoritoDeserialize>::deserialize(&archived);
    assert_eq!(decoded, payload);
}

#[test]
fn corrupt_data() {
    let d = Data::new(4);
    let cfg = CompressionConfig { level: 0 };
    let mut bytes = to_compressed_bytes(&d, Some(cfg)).unwrap();
    bytes[Header::SIZE + 1] ^= 0xFF; // corrupt the payload
    assert!(from_compressed_bytes::<Data>(&bytes).is_err());
}

#[cfg(feature = "gpu-compression")]
#[test]
fn gpu_zstd_roundtrip() {
    if !norito::core::gpu_zstd::available() {
        // No GPU backend; skip the test
        return;
    }
    let d = Data::new(5);
    let cfg = CompressionConfig { level: 1 };
    let bytes = to_compressed_bytes(&d, Some(cfg)).unwrap();
    let archived = from_compressed_bytes::<Data>(&bytes).unwrap();
    let decoded = <Data as NoritoDeserialize>::deserialize(&archived);
    assert_eq!(decoded.a, 5);
}
