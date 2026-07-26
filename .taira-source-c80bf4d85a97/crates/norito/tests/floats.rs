//! Float roundtrip tests (f32, f64) covering strict-safe paths.

use norito::core::*;

#[test]
fn f32_roundtrip() {
    let vals: [f32; 5] = [
        0.0,
        1.5,
        -2.25,
        std::f32::consts::PI,
        f32::from_bits(0x7f800000),
    ];
    for &v in &vals {
        let bytes = to_bytes(&v).expect("encode f32");
        let decoded: f32 = decode_from_bytes(&bytes).expect("decode f32");
        // For NaN bit patterns, direct equality is false; compare bits
        if v.is_nan() {
            let arch = from_bytes::<f32>(&bytes).unwrap();
            let got = <f32 as NoritoDeserialize>::deserialize(arch);
            assert!(got.is_nan());
        } else {
            assert_eq!(v, decoded);
        }
    }
}

#[test]
fn f64_roundtrip() {
    let vals: [f64; 5] = [
        0.0,
        1.5,
        -2.25,
        std::f64::consts::PI,
        f64::from_bits(0x7ff0000000000000),
    ];
    for &v in &vals {
        let bytes = to_bytes(&v).expect("encode f64");
        let decoded: f64 = decode_from_bytes(&bytes).expect("decode f64");
        if v.is_nan() {
            let arch = from_bytes::<f64>(&bytes).unwrap();
            let got = <f64 as NoritoDeserialize>::deserialize(arch);
            assert!(got.is_nan());
        } else {
            assert_eq!(v, decoded);
        }
    }
}
