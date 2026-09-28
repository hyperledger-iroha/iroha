//! Automatic Poseidon batches with charged native snapshots and publication.

use super::super::{catch_unwind_to_java, throw_java_illegal_argument, throw_java_illegal_state};
use jni::{
    JNIEnv,
    objects::{JClass, JLongArray},
    sys::{JNI_FALSE, JNI_TRUE, jboolean},
};

fn run<T: Copy + Default>(
    env: &mut JNIEnv<'_>,
    input: &JLongArray<'_>,
    out: &JLongArray<'_>,
    width: usize,
    row: impl Fn(&[i64]) -> T,
    operation: fn(&[T], &mut [u64]) -> bool,
) -> jboolean {
    let length = match env.get_array_length(input) {
        Ok(length) => length as usize,
        Err(error) => {
            throw_java_illegal_argument(env, format!("Poseidon input length: {error}"));
            return JNI_FALSE;
        }
    };
    if length % width != 0 {
        throw_java_illegal_argument(
            env,
            format!("Poseidon inputs must contain {width} words per row"),
        );
        return JNI_FALSE;
    }
    let count = length / width;
    if !super::ensure_min_array_length(env, out, count as i32, "Poseidon") {
        return JNI_FALSE;
    }
    let mut inputs = match ivm::try_acceleration_output::<T>(count) {
        Ok(owner) => owner,
        Err(error) => {
            resource_error(env, error);
            return JNI_FALSE;
        }
    };
    let mut output = match ivm::try_acceleration_output::<u64>(count) {
        Ok(owner) => owner,
        Err(error) => {
            resource_error(env, error);
            return JNI_FALSE;
        }
    };
    for (index, destination) in inputs.iter_mut().enumerate() {
        let mut words = [0i64; 6];
        if let Err(error) =
            env.get_long_array_region(input, (index * width) as i32, &mut words[..width])
        {
            throw_java_illegal_state(env, format!("Poseidon input read failed: {error}"));
            return JNI_FALSE;
        }
        *destination = row(&words[..width]);
    }
    if catch_unwind_to_java(env, "automatic Poseidon batch", || {
        operation(&inputs, &mut output)
    }) != Some(true)
    {
        return JNI_FALSE;
    }
    // The common process input/output owners remain charged until Java owns all
    // initialized result words. No Rust Vec or CUDA-availability result is used.
    for (index, &value) in output.as_slice().iter().enumerate() {
        if let Err(error) = env.set_long_array_region(out, index as i32, &[value as i64]) {
            throw_java_illegal_state(env, format!("Poseidon output write failed: {error}"));
            return JNI_FALSE;
        }
    }
    JNI_TRUE
}
fn resource_error(env: &mut JNIEnv<'_>, error: ivm::AccelerationOutputError) {
    match error {
        ivm::AccelerationOutputError::InvalidLayout => {
            throw_java_illegal_argument(env, "Poseidon native allocation layout is invalid".into())
        }
        _ => throw_java_illegal_state(env, format!("Poseidon native resource refusal: {error}")),
    }
}

/// Compute two-word Poseidon rows with automatic qualified CPU/GPU selection.
/// # Safety
/// Arguments must be valid handles from the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativePoseidon2(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    inputs: JLongArray<'_>,
    out: JLongArray<'_>,
) -> jboolean {
    run(
        &mut env,
        &inputs,
        &out,
        2,
        |row| (row[0] as u64, row[1] as u64),
        ivm::poseidon2_many_into,
    )
}
/// Compute six-word Poseidon rows with automatic qualified CPU/GPU selection.
/// # Safety
/// Arguments must be valid handles from the canonical Kotlin bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_gpu_Accelerators_nativePoseidon6(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    inputs: JLongArray<'_>,
    out: JLongArray<'_>,
) -> jboolean {
    run(
        &mut env,
        &inputs,
        &out,
        6,
        |row| std::array::from_fn(|index| row[index] as u64),
        ivm::poseidon6_many_into,
    )
}
