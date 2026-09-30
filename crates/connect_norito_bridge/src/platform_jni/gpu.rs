//! Canonical Kotlin acceleration JNI exports and owned batch conversion.

#[path = "gpu/bn254.rs"]
mod bn254;
#[path = "gpu/poseidon.rs"]
mod poseidon;

use super::{catch_unwind_to_java, throw_java_illegal_argument};

/// Return whether the native CUDA implementation is available.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativeCudaAvailable(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let available =
        catch_unwind_to_java(&mut env, "cuda_available", ivm::cuda_available).unwrap_or(false);
    if available { JNI_TRUE } else { JNI_FALSE }
}
/// Return whether the native CUDA implementation has been disabled.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativeCudaDisabled(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let disabled =
        catch_unwind_to_java(&mut env, "cuda_disabled", ivm::cuda_disabled).unwrap_or(false);
    if disabled { JNI_TRUE } else { JNI_FALSE }
}

fn ensure_min_array_length(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    required: i32,
    context: &str,
) -> bool {
    match env.get_array_length(array) {
        Ok(len) if len >= required => true,
        Ok(len) => {
            throw_java_illegal_argument(
                env,
                format!("{context} expects an output array with length >= {required}, got {len}"),
            );
            false
        }
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            false
        }
    }
}
