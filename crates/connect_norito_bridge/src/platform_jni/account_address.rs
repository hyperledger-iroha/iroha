//! JNI admission of complete canonical account controllers through the Rust model owner.

use super::{catch_unwind_to_java, read_java_byte_array_bounded, throw_java_illegal_argument};
use iroha_data_model::account::address::AccountAddress;

/// Maximum raw account-address input accepted by the SDK JNI resource boundary.
const MAX_CANONICAL_ADDRESS_BYTES_V1: usize = 64 * 1024 * 1024;

fn validate_canonical_address(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_CANONICAL_ADDRESS_BYTES_V1 {
        return Err("canonical account address exceeds the JNI input bound".to_owned());
    }
    let address = AccountAddress::from_canonical_bytes(bytes).map_err(|error| error.to_string())?;
    let encoded = address.canonical_hex().map_err(|error| error.to_string())?;
    let expected_len = 2 + bytes.len() * 2;
    let hex = b"0123456789abcdef";
    if encoded.len() != expected_len
        || !encoded.starts_with("0x")
        || !encoded.as_bytes()[2..]
            .chunks_exact(2)
            .zip(bytes)
            .all(|(pair, byte)| {
                pair[0] == hex[usize::from(byte >> 4)] && pair[1] == hex[usize::from(byte & 0x0f)]
            })
    {
        return Err("account address is not its exact canonical representation".to_owned());
    }
    Ok(())
}

/// Validate every controller key and policy and return the exact admitted canonical bytes.
///
/// # Safety
/// The JVM must provide the native environment and a valid byte-array handle for this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeValidateAccountAddressCanonical(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    input: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let Some(bytes) = read_java_byte_array_bounded(
        &mut env,
        &input,
        "canonicalAccountAddress",
        MAX_CANONICAL_ADDRESS_BYTES_V1,
    ) else {
        return std::ptr::null_mut();
    };
    let Some(result) = catch_unwind_to_java(&mut env, "canonical account validation", || {
        validate_canonical_address(&bytes)
    }) else {
        return std::ptr::null_mut();
    };
    if let Err(message) = result {
        throw_java_illegal_argument(&mut env, message);
        return std::ptr::null_mut();
    }
    match env.byte_array_from_slice(&bytes) {
        Ok(value) => value.into_raw(),
        Err(error) => {
            super::throw_java_illegal_state(
                &mut env,
                format!("canonical account output allocation failed: {error}"),
            );
            std::ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_controller_fixture_is_accepted_without_byte_normalization() {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../fixtures/account/multisig_wire_v1.json"
        ))
        .unwrap();
        let cases = fixture.get("positive").unwrap().as_array().unwrap();
        assert_eq!(cases.len(), 16);
        for case in cases {
            let encoded = case.get("canonical_address_hex").unwrap().as_str().unwrap();
            let bytes = hex::decode(encoded.strip_prefix("0x").unwrap_or(encoded)).unwrap();
            validate_canonical_address(&bytes).unwrap();
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(validate_canonical_address(&trailing).is_err());
        }
    }

    #[test]
    fn malformed_points_are_rejected_for_every_controller_algorithm() {
        let mut vectors = vec![
            (1, [vec![1], vec![0; 31]].concat()),
            (1, vec![0xff; 32]),
            (4, [vec![2], vec![0xff; 32]].concat()),
            (3, vec![0xff; 48]),
            (5, vec![0xff; 96]),
            (2, vec![0; 1952]),
            (15, [vec![0, 0, 4], vec![0xff; 64]].concat()),
        ];
        for curve in [10, 11, 12, 13, 14] {
            vectors.push((curve, vec![0xff; if curve < 13 { 64 } else { 128 }]));
        }
        assert_eq!(vectors.len(), 12);
        for (curve, key) in vectors {
            let mut encoded = vec![2, if key.len() <= 255 { 0 } else { 2 }, curve];
            if key.len() <= 255 {
                encoded.push(u8::try_from(key.len()).unwrap());
            } else {
                encoded.extend_from_slice(&u16::try_from(key.len()).unwrap().to_be_bytes());
            }
            encoded.extend_from_slice(&key);
            assert!(
                validate_canonical_address(&encoded).is_err(),
                "curve {curve}"
            );
        }
        assert!(validate_canonical_address(&[]).is_err());
        assert!(validate_canonical_address(&vec![0; MAX_CANONICAL_ADDRESS_BYTES_V1 + 1]).is_err());
    }
}
