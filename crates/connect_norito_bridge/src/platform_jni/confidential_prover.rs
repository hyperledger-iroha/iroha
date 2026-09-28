//! JNI marshalling for the same opaque Core wallet owner exposed by the C ABI.

use super::super::confidential_prover_ffi as wallet;
use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JThrowable, JValue},
    sys::{jbyteArray, jint, jlong},
};
use zeroize::Zeroizing;

fn run<T>(f: impl FnOnce() -> Result<T, jint>) -> Result<T, jint> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(Err(wallet::INTERNAL))
}
fn read(
    env: &mut JNIEnv<'_>,
    value: &JByteArray<'_>,
    max: usize,
) -> Result<Zeroizing<Vec<u8>>, jint> {
    let len = env.get_array_length(value).map_err(|_| wallet::INVALID)?;
    if len < 0 || len as usize > max {
        return Err(wallet::INVALID);
    }
    env.convert_byte_array(value)
        .map(Zeroizing::new)
        .map_err(|_| wallet::INVALID)
}
fn handle(result: Result<u64, jint>) -> jlong {
    result.map(|id| id as jlong).unwrap_or_else(i64::from)
}
fn status(result: Result<(), jint>) -> jint {
    result.err().unwrap_or(0)
}
fn throw_code(env: &mut JNIEnv<'_>, code: jint) {
    match env.new_object(
        "org/hyperledger/iroha/sdk/privacy/ConfidentialProverException",
        "(I)V",
        &[JValue::Int(code)],
    ) {
        Ok(value) => {
            let _ = env.throw(JThrowable::from(value));
        }
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "confidential prover boundary failed",
            );
        }
    }
}
/// Native local-prover contract revision; requires the already trusted SDK library loader.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_revision(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    1
}
/// Copy one spend key directly into a clearing owner after bounded JNI admission.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_create(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    network: JByteArray<'_>,
    asset: JByteArray<'_>,
    key: JByteArray<'_>,
) -> jlong {
    handle(run(|| {
        let key = read(&mut env, &key, 32)?;
        let network = read(&mut env, &network, 32)?;
        let asset = read(&mut env, &asset, 512)?;
        wallet::create(&network, &asset, &key)
    }))
}
/// Release the caller's context while preserving accepted jobs.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_close(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
) -> jint {
    status(run(|| wallet::close(id as u64)))
}
/// Retain a prover into one consumable native job before asynchronous execution.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobCreate(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
    operation: jint,
    root: JByteArray<'_>,
    lo: jlong,
    hi: jlong,
) -> jlong {
    handle(run(|| {
        let operation = u8::try_from(operation).map_err(|_| wallet::INVALID)?;
        let root = read(&mut env, &root, 32)?;
        wallet::job_create(id as u64, operation, &root, lo as u64, hi as u64)
    }))
}
/// Add one bounded input; all JNI-owned note copies clear on every return.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobInput(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
    lo: jlong,
    hi: jlong,
    rho: JByteArray<'_>,
    diversifier: JByteArray<'_>,
    index: jlong,
) -> jint {
    status(run(|| {
        if index < 0 {
            return Err(-15);
        }
        let rho = read(&mut env, &rho, 32)?;
        let diversifier = read(&mut env, &diversifier, 32)?;
        wallet::job_input(
            id as u64,
            lo as u64,
            hi as u64,
            &rho,
            &diversifier,
            index as u64,
        )
    }))
}
/// Add one bounded output or change note using the shared native owner.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobOutput(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
    lo: jlong,
    hi: jlong,
    rho: JByteArray<'_>,
    owner: JByteArray<'_>,
) -> jint {
    status(run(|| {
        let rho = read(&mut env, &rho, 32)?;
        let owner = read(&mut env, &owner, 32)?;
        wallet::job_output(id as u64, lo as u64, hi as u64, &rho, &owner)
    }))
}
/// Admit a complete bounded commitment prefix before native tree work.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobCommitments(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
    leaves: JByteArray<'_>,
) -> jint {
    status(run(|| {
        let leaves = read(&mut env, &leaves, 65_536 * 32)?;
        wallet::job_commitments(id as u64, &leaves)
    }))
}
/// Admit one path per actual input, clearing copied sibling/direction scratch.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobPaths(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
    siblings: JByteArray<'_>,
    directions: JByteArray<'_>,
) -> jint {
    status(run(|| {
        let siblings = read(&mut env, &siblings, 1024)?;
        let directions = read(&mut env, &directions, 32)?;
        wallet::job_paths(id as u64, &siblings, &directions)
    }))
}
/// Consume one native job and return public Norito JSON, or a stable typed failure code.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobProve(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
) -> jbyteArray {
    match run(|| wallet::job_prove(id as u64)) {
        Ok(result) => match env.byte_array_from_slice(&result) {
            Ok(value) => value.into_raw(),
            Err(_) => {
                throw_code(&mut env, wallet::RESOURCE);
                std::ptr::null_mut()
            }
        },
        Err(code) => {
            throw_code(&mut env, code);
            std::ptr::null_mut()
        }
    }
}
/// Close a prepared job that will not be executed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ConfidentialProverNative_jobClose(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    id: jlong,
) -> jint {
    status(run(|| wallet::job_close(id as u64)))
}
