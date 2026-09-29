//! BN254 JNI batches with charged snapshots and caller-owned Java publication.

use super::super::{catch_unwind_to_java, throw_java_illegal_argument, throw_java_illegal_state};
use jni::{
    JNIEnv,
    objects::{JClass, JLongArray},
    sys::{JNI_FALSE, JNI_TRUE, jboolean},
};

type Batch = fn(&[[u64; 4]], &[[u64; 4]], &mut [[u64; 4]]) -> bool;

fn field_at(env: &mut JNIEnv<'_>, array: &JLongArray<'_>, index: usize) -> Option<[u64; 4]> {
    let offset = i32::try_from(index.checked_mul(4)?).ok()?;
    let mut words = [0i64; 4];
    if let Err(error) = env.get_long_array_region(array, offset, &mut words) {
        throw_java_illegal_state(env, format!("BN254 input read failed: {error}"));
        return None;
    }
    let field = words.map(|word| word as u64);
    if !ivm::bn254_vec::FieldElem(field).is_canonical() {
        throw_java_illegal_argument(env, "BN254 input must be below the field modulus".into());
        return None;
    }
    Some(field)
}

fn reserve(env: &mut JNIEnv<'_>, count: usize) -> Option<ivm::AccelerationOutput<[u64; 4]>> {
    match ivm::try_acceleration_output(count) {
        Ok(output) => Some(output),
        Err(ivm::AccelerationOutputError::InvalidLayout) => {
            throw_java_illegal_argument(env, "BN254 native allocation layout is invalid".into());
            None
        }
        Err(error) => {
            throw_java_illegal_state(env, format!("BN254 native resource refusal: {error}"));
            None
        }
    }
}

fn run(
    env: &mut JNIEnv<'_>,
    lhs: &JLongArray<'_>,
    rhs: &JLongArray<'_>,
    out: &JLongArray<'_>,
    operation: Batch,
) -> jboolean {
    let left_len = match env.get_array_length(lhs) {
        Ok(len) => len,
        Err(error) => {
            throw_java_illegal_argument(env, format!("BN254 lhs length: {error}"));
            return JNI_FALSE;
        }
    };
    let right_len = match env.get_array_length(rhs) {
        Ok(len) => len,
        Err(error) => {
            throw_java_illegal_argument(env, format!("BN254 rhs length: {error}"));
            return JNI_FALSE;
        }
    };
    if left_len != right_len || left_len % 4 != 0 {
        throw_java_illegal_argument(
            env,
            "BN254 batches require equal lengths divisible by four".into(),
        );
        return JNI_FALSE;
    }
    if !super::ensure_min_array_length(env, out, left_len, "BN254") {
        return JNI_FALSE;
    }
    let count = left_len as usize / 4;
    for index in 0..count {
        if field_at(env, lhs, index).is_none() || field_at(env, rhs, index).is_none() {
            return JNI_FALSE;
        }
    }
    let Some(mut left) = reserve(env, count) else {
        return JNI_FALSE;
    };
    let Some(mut right) = reserve(env, count) else {
        return JNI_FALSE;
    };
    let Some(mut output) = reserve(env, count) else {
        return JNI_FALSE;
    };
    for index in 0..count {
        let Some(a) = field_at(env, lhs, index) else {
            return JNI_FALSE;
        };
        let Some(b) = field_at(env, rhs, index) else {
            return JNI_FALSE;
        };
        left[index] = a;
        right[index] = b;
    }
    let result = catch_unwind_to_java(env, "BN254 automatic batch", || {
        operation(&left, &right, &mut output)
    });
    if result != Some(true) {
        return JNI_FALSE;
    }
    // All Rust computation succeeded before the first Java write. No flattened
    // Rust Vec detaches the native output charge; only four stack limbs are used.
    for (index, field) in output.as_slice().iter().enumerate() {
        let words = field.map(|word| word as i64);
        let Ok(offset) = i32::try_from(index * 4) else {
            throw_java_illegal_state(
                env,
                "BN254 output offset exceeded validated Java bounds".into(),
            );
            return JNI_FALSE;
        };
        if let Err(error) = env.set_long_array_region(out, offset, &words) {
            throw_java_illegal_state(env, format!("BN254 output write failed: {error}"));
            return JNI_FALSE;
        }
    }
    JNI_TRUE
}

/// Add an ordered BN254 batch with automatic CPU/GPU selection.
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativeBn254Add(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    lhs: JLongArray<'_>,
    rhs: JLongArray<'_>,
    out: JLongArray<'_>,
) -> jboolean {
    run(&mut env, &lhs, &rhs, &out, ivm::bn254_vec::add_batch_into)
}

/// Subtract an ordered BN254 batch with automatic CPU/GPU selection.
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativeBn254Sub(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    lhs: JLongArray<'_>,
    rhs: JLongArray<'_>,
    out: JLongArray<'_>,
) -> jboolean {
    run(&mut env, &lhs, &rhs, &out, ivm::bn254_vec::sub_batch_into)
}

/// Multiply an ordered BN254 batch with automatic CPU/GPU selection.
/// # Safety
/// Arguments must be valid JNI handles supplied by the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativeBn254Mul(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    lhs: JLongArray<'_>,
    rhs: JLongArray<'_>,
    out: JLongArray<'_>,
) -> jboolean {
    run(&mut env, &lhs, &rhs, &out, ivm::bn254_vec::mul_batch_into)
}
