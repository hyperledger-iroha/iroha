//! Android's non-authorizing JNI transport for one testnet State-proof observation.

use super::*;

const TESTNET_OBSERVATION_JNI_CONTRACT_V1: [jni::sys::jint; 4] = [
    1,
    KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1 as jni::sys::jint,
    iroha_data_model::kagemusha::KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1 as jni::sys::jint,
    KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1 as jni::sys::jint,
];

fn bounded_nonempty_length(length: jni::sys::jsize, maximum: usize) -> Option<usize> {
    let length = usize::try_from(length).ok()?;
    (length > 0 && length <= maximum).then_some(length)
}

fn bounded_java_archive(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    maximum: usize,
) -> Option<Vec<u8>> {
    let length = bounded_nonempty_length(env.get_array_length(value).ok()?, maximum)?;
    let bytes = env.convert_byte_array(value).ok()?;
    (bytes.len() == length).then_some(bytes)
}

/// Advertise the exact diagnostic JNI ABI; this grants no verifier or monetary authority.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetStateProofObservationJniV1_nativeContractV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jintArray {
    let Ok(output) =
        env.new_int_array(TESTNET_OBSERVATION_JNI_CONTRACT_V1.len() as jni::sys::jsize)
    else {
        return ptr::null_mut();
    };
    if env
        .set_int_array_region(&output, 0, &TESTNET_OBSERVATION_JNI_CONTRACT_V1)
        .is_err()
    {
        return ptr::null_mut();
    }
    output.into_raw()
}

/// Return the positive archive length, or the exact negative C ABI error status.
///
/// The caller provides a preallocated direct buffer. The C observer writes its
/// canonical response into that buffer before it reports success. The JNI entry
/// itself makes no Java allocation after the diagnostic trial head advances.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_probe_KagemushaTestnetStateProofObservationJniV1_nativeObserveV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    public_inputs: jni::objects::JByteArray<'_>,
    paired_proof: jni::objects::JByteArray<'_>,
    output: jni::objects::JByteBuffer<'_>,
) -> jni::sys::jint {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(public_inputs) = bounded_java_archive(
            &mut env,
            &public_inputs,
            KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1,
        ) else {
            return ERR_KAGEMUSHA_V1;
        };
        let Some(paired_proof) = bounded_java_archive(
            &mut env,
            &paired_proof,
            iroha_data_model::kagemusha::KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        ) else {
            return ERR_KAGEMUSHA_V1;
        };
        let Ok(capacity) = env.get_direct_buffer_capacity(&output) else {
            return ERR_KAGEMUSHA_V1;
        };
        if capacity != KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1 {
            return ERR_BUFFER_TOO_SMALL;
        }
        let Ok(output_ptr) = env.get_direct_buffer_address(&output) else {
            return ERR_KAGEMUSHA_V1;
        };
        if output_ptr.is_null() {
            return ERR_KAGEMUSHA_V1;
        }
        let mut output_length = 0;
        let status = unsafe {
            connect_norito_kagemusha_testnet_state_proof_observe_v1(
                public_inputs.as_ptr(),
                public_inputs.len(),
                paired_proof.as_ptr(),
                paired_proof.len(),
                output_ptr,
                capacity,
                &mut output_length,
            )
        };
        if status != 0 {
            return status;
        }
        if output_length == 0 || output_length > capacity {
            return ERR_KAGEMUSHA_V1;
        }
        output_length as jni::sys::jint
    }))
    .unwrap_or(ERR_KAGEMUSHA_V1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_jni_contract_is_exact_and_not_a_monetary_contract() {
        assert_eq!(TESTNET_OBSERVATION_JNI_CONTRACT_V1, [1, 4096, 6528, 256]);
        assert!(
            TESTNET_OBSERVATION_JNI_CONTRACT_V1
                .iter()
                .all(|word| *word > 0)
        );
    }

    #[test]
    fn bounded_jni_archive_length_rejects_empty_negative_and_oversized_arrays() {
        assert_eq!(bounded_nonempty_length(-1, 4096), None);
        assert_eq!(bounded_nonempty_length(0, 4096), None);
        assert_eq!(bounded_nonempty_length(1, 4096), Some(1));
        assert_eq!(bounded_nonempty_length(4096, 4096), Some(4096));
        assert_eq!(bounded_nonempty_length(4097, 4096), None);
    }
}
