#![no_main]

use iroha_primitives::{
    bigint::BigInt,
    numeric::{Numeric, Quantity},
    numeric_abi::{DecimalValueV1, IntValueV1, QuantityValueV1},
};
use ivm::numeric_tlv::{self, MAX_QUANTITY_ENVELOPE_BYTES_V1};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&mode, payload)) = data.split_first() else {
        return;
    };

    match mode {
        b'I' => fuzz_valid_int(payload),
        b'D' => fuzz_valid_decimal(payload),
        b'Q' => fuzz_valid_quantity(payload),
        _ => match mode % 7 {
            0 => fuzz_envelope(payload),
            1 => fuzz_int_frame(payload),
            2 => fuzz_decimal_frame(payload),
            3 => fuzz_quantity_frame(payload),
            4 => fuzz_valid_int(payload),
            5 => fuzz_valid_decimal(payload),
            6 => fuzz_valid_quantity(payload),
            _ => unreachable!("modulo seven is exhaustive"),
        },
    }
});

fn fuzz_envelope(envelope: &[u8]) {
    // Feed the complete attacker-controlled slice. Truncating it at the
    // largest valid envelope size would turn an oversized/trailing-data input
    // into a different potentially valid message and leave the rejection path
    // unfuzzed.
    if envelope.len() > MAX_QUANTITY_ENVELOPE_BYTES_V1 {
        assert!(numeric_tlv::decode_int_bytes(envelope).is_err());
        assert!(numeric_tlv::decode_decimal_bytes(envelope).is_err());
        assert!(numeric_tlv::decode_quantity_bytes(envelope).is_err());
        return;
    }

    if let Ok(value) = numeric_tlv::decode_int_bytes(envelope) {
        let canonical = numeric_tlv::encode_int(&value).expect("decoded int must re-encode");
        assert_eq!(canonical, envelope);
        assert_eq!(numeric_tlv::decode_int_bytes(&canonical), Ok(value));
    }

    if let Ok(value) = numeric_tlv::decode_decimal_bytes(envelope) {
        let canonical =
            numeric_tlv::encode_decimal(&value).expect("decoded decimal must re-encode");
        assert_eq!(canonical, envelope);
        assert_eq!(numeric_tlv::decode_decimal_bytes(&canonical), Ok(value));
    }

    if let Ok(value) = numeric_tlv::decode_quantity_bytes(envelope) {
        let canonical =
            numeric_tlv::encode_quantity(&value).expect("decoded quantity must re-encode");
        assert_eq!(canonical, envelope);
        assert_eq!(numeric_tlv::decode_quantity_bytes(&canonical), Ok(value));
    }
}

fn fuzz_int_frame(frame: &[u8]) {
    if let Ok(value) = IntValueV1::decode_frame(frame) {
        assert_eq!(
            value.encode_frame().expect("decoded int must re-encode"),
            frame
        );
    }
}

fn fuzz_decimal_frame(frame: &[u8]) {
    if let Ok(value) = DecimalValueV1::decode_frame(frame) {
        assert_eq!(
            value
                .encode_frame()
                .expect("decoded decimal must re-encode"),
            frame
        );
    }
}

fn fuzz_quantity_frame(frame: &[u8]) {
    if let Ok(value) = QuantityValueV1::decode_frame(frame) {
        assert_eq!(
            value
                .encode_frame()
                .expect("decoded quantity must re-encode"),
            frame
        );
    }
}

fn bounded_mantissa(payload: &[u8]) -> BigInt {
    // Every 64-byte two's-complement value is inside the signed 512-bit
    // language domain, including both endpoints and every logical limb width.
    BigInt::from_twos_bytes(&payload[..payload.len().min(64)])
        .expect("the bounded byte slice always fits the primitive bigint")
}

fn fuzz_valid_int(payload: &[u8]) {
    let value = bounded_mantissa(payload);
    let frame = IntValueV1::try_new(value.clone())
        .expect("bounded mantissa is a V1 int")
        .encode_frame()
        .expect("valid int frame encodes");
    assert_eq!(
        IntValueV1::decode_frame(&frame).map(IntValueV1::into_int),
        Ok(value.clone())
    );

    let envelope = numeric_tlv::encode_int(&value).expect("valid int envelope encodes");
    assert_eq!(numeric_tlv::decode_int_bytes(&envelope), Ok(value));
}

fn valid_decimal(payload: &[u8]) -> Numeric {
    let scale = payload.first().map_or(0, |byte| u32::from(*byte % 29));
    Numeric::try_new(
        bounded_mantissa(payload.get(1..).unwrap_or_default()),
        scale,
    )
    .expect("bounded mantissa and scale form a decimal")
    .canonicalize_decimal()
    .expect("bounded decimal canonicalizes")
}

fn fuzz_valid_decimal(payload: &[u8]) {
    let value = valid_decimal(payload);
    let frame = DecimalValueV1::from_canonical_numeric(value.clone())
        .expect("derived decimal is canonical")
        .encode_frame()
        .expect("valid decimal frame encodes");
    assert_eq!(
        DecimalValueV1::decode_frame(&frame).map(DecimalValueV1::into_numeric),
        Ok(value.clone())
    );

    let envelope = numeric_tlv::encode_decimal(&value).expect("valid decimal envelope encodes");
    assert_eq!(numeric_tlv::decode_decimal_bytes(&envelope), Ok(value));
}

fn fuzz_valid_quantity(payload: &[u8]) {
    let decimal = valid_decimal(payload);
    let non_negative = if decimal.mantissa().is_negative() {
        // The absolute value of the signed minimum is not representable. Map
        // that single input to zero so this structured-valid branch remains
        // total; malformed/end-point rejection is exercised by the raw paths.
        Numeric::try_new(
            decimal
                .mantissa()
                .checked_abs()
                .unwrap_or_else(|_| BigInt::zero()),
            decimal.scale(),
        )
        .expect("absolute mantissa retains the valid scale")
        .canonicalize_decimal()
        .expect("absolute decimal canonicalizes")
    } else {
        decimal
    };
    let value = Quantity::from_canonical_numeric(non_negative)
        .expect("non-negative canonical decimal is a quantity");
    let frame = QuantityValueV1::new(value.clone())
        .encode_frame()
        .expect("valid quantity frame encodes");
    assert_eq!(
        QuantityValueV1::decode_frame(&frame).map(QuantityValueV1::into_quantity),
        Ok(value.clone())
    );

    let envelope = numeric_tlv::encode_quantity(&value).expect("valid quantity envelope encodes");
    assert_eq!(numeric_tlv::decode_quantity_bytes(&envelope), Ok(value));
}
