//! Canonical Kotlin CUDA JNI exports and owned batch conversion.

use super::{catch_unwind_to_java, throw_java_illegal_argument, throw_java_illegal_state};

/// Return whether the native CUDA implementation is available.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativeCudaAvailable(
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
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativeCudaDisabled(
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
fn read_long_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: &str,
) -> Option<Vec<i64>> {
    let len = match env.get_array_length(array) {
        Ok(value) => value,
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            return None;
        }
    } as usize;
    let mut buf = vec![0i64; len];
    if let Err(err) = env.get_long_array_region(array, 0, &mut buf) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to read array contents: {err}"),
        );
        return None;
    }
    Some(buf)
}
fn write_long_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    values: &[i64],
    context: &str,
) -> bool {
    if let Err(err) = env.set_long_array_region(array, 0, values) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to write output array: {err}"),
        );
        return false;
    }
    true
}
fn convert_field_elems<L: Into<String>>(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: L,
) -> Option<Vec<[u64; 4]>> {
    let context = context.into();
    let buf = read_long_array(env, array, &context)?;
    if buf.len() % 4 != 0 {
        throw_java_illegal_argument(
            env,
            format!("{context} expects a flattened array with a length multiple of 4"),
        );
        return None;
    }
    let mut elems = Vec::with_capacity(buf.len() / 4);
    for chunk in buf.chunks_exact(4) {
        let mut limbs = [0u64; 4];
        for (dst, src) in limbs.iter_mut().zip(chunk.iter()) {
            *dst = *src as u64;
        }
        elems.push(limbs);
    }
    Some(elems)
}
/// Compute an ordered batch of two-word Poseidon inputs.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativePoseidon2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let buf = match read_long_array(&mut env, &inputs, "poseidon2 inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() % 2 != 0 {
        throw_java_illegal_argument(
            &mut env,
            "poseidon2 inputs must contain an even number of elements".into(),
        );
        return JNI_FALSE;
    }
    let batch_size = (buf.len() / 2) as i32;
    if !ensure_min_array_length(&mut env, &out, batch_size, "poseidon2") {
        return JNI_FALSE;
    }
    let mut tuples = Vec::with_capacity(batch_size as usize);
    for chunk in buf.chunks_exact(2) {
        tuples.push((chunk[0] as u64, chunk[1] as u64));
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon2_cuda_many", || {
        ivm::poseidon2_cuda_many(&tuples)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(outputs) = result {
        let values: Vec<i64> = outputs.into_iter().map(|value| value as i64).collect();
        if write_long_array(&mut env, &out, &values, "poseidon2") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
/// Compute an ordered batch of six-word Poseidon inputs.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativePoseidon6(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let buf = match read_long_array(&mut env, &inputs, "poseidon6 inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() % 6 != 0 {
        throw_java_illegal_argument(&mut env, "poseidon6 inputs must be multiples of six".into());
        return JNI_FALSE;
    }
    let batch_size = (buf.len() / 6) as i32;
    if !ensure_min_array_length(&mut env, &out, batch_size, "poseidon6") {
        return JNI_FALSE;
    }
    let mut states = Vec::with_capacity(batch_size as usize);
    for chunk in buf.chunks_exact(6) {
        let mut state = [0u64; 6];
        for (dst, src) in state.iter_mut().zip(chunk.iter()) {
            *dst = *src as u64;
        }
        states.push(state);
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon6_cuda_many", || {
        ivm::poseidon6_cuda_many(&states)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(outputs) = result {
        let values: Vec<i64> = outputs.into_iter().map(|value| value as i64).collect();
        if write_long_array(&mut env, &out, &values, "poseidon6") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
/// Add an ordered batch of canonical BN254 field elements.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativeBn254Add(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let lhs = match convert_field_elems(&mut env, &lhs, "bn254Add lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254Add rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(&mut env, "bn254Add expects matching batch lengths".into());
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254Add output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254Add") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_add_batch_cuda", || {
        ivm::bn254_add_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254Add") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
/// Subtract an ordered batch of canonical BN254 field elements.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativeBn254Sub(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let lhs = match convert_field_elems(&mut env, &lhs, "bn254Sub lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254Sub rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(&mut env, "bn254Sub expects matching batch lengths".into());
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254Sub output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254Sub") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_sub_batch_cuda", || {
        ivm::bn254_sub_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254Sub") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
/// Multiply an ordered batch of canonical BN254 field elements.
///
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_CudaAccelerators_nativeBn254Mul(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let lhs = match convert_field_elems(&mut env, &lhs, "bn254Mul lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254Mul rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(&mut env, "bn254Mul expects matching batch lengths".into());
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254Mul output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254Mul") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_mul_batch_cuda", || {
        ivm::bn254_mul_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254Mul") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
