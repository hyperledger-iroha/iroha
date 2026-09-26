//! Signed-bootstrap-only JNI startup of the native Experimental mobile host.

use super::*;
use iroha_data_model::kagemusha::KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1;

/// Return the exact ABI version and bounded signed-bootstrap size.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTestnetNativeStartupJniV1_nativeContractV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jintArray {
    let contract = KAGEMUSHA_TESTNET_NATIVE_STARTUP_CONTRACT_V1.map(|word| word as i32);
    let Ok(output) = env.new_int_array(contract.len() as jni::sys::jsize) else {
        return ptr::null_mut();
    };
    if env.set_int_array_region(&output, 0, &contract).is_err() {
        return ptr::null_mut();
    }
    output.into_raw()
}

fn valid_bootstrap_length(length: jni::sys::jsize) -> bool {
    length > 0 && (length as usize) <= KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1
}

/// Activate using only a bounded signed checkpoint; all authority stays native-owned.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTestnetNativeStartupJniV1_nativeActivateV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bootstrap: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(length) = env.get_array_length(&bootstrap) else {
            return ERR_KAGEMUSHA_V1;
        };
        if !valid_bootstrap_length(length) {
            return ERR_KAGEMUSHA_V1;
        }
        let Ok(archive) = env.convert_byte_array(&bootstrap) else {
            return ERR_KAGEMUSHA_V1;
        };
        if archive.len() != length as usize {
            return ERR_KAGEMUSHA_V1;
        }
        // No Java allocation or output conversion follows native activation.
        unsafe {
            connect_norito_kagemusha_testnet_native_startup_activate_v1(
                archive.as_ptr(),
                archive.len(),
            )
        }
    }))
    .unwrap_or(ERR_KAGEMUSHA_V1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_jni_bounds_match_the_native_contract_before_allocation() {
        assert_eq!(KAGEMUSHA_TESTNET_NATIVE_STARTUP_CONTRACT_V1, [1, 1_048_576]);
        for length in [-1, 0, 1_048_577, i32::MAX] {
            assert!(!valid_bootstrap_length(length));
        }
        assert!(valid_bootstrap_length(1));
        assert!(valid_bootstrap_length(1_048_576));
    }
}
