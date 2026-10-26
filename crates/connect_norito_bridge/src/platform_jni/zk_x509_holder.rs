//! JNI adapter for the same canonical public X.509 signing preparation as C.

use super::super::zk_x509_holder_ffi as holder;
use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObject, JThrowable, JValue},
    sys::{jint, jobjectArray},
};
use zeroize::Zeroizing;

fn throw_code(env: &mut JNIEnv<'_>, code: jint) {
    // Preserve an already-pending JVM allocation/array exception.
    if env.exception_check().unwrap_or(true) {
        return;
    }
    match env.new_object(
        "org/hyperledger/iroha/sdk/privacy/ZkX509SigningPreparationException",
        "(I)V",
        &[JValue::Int(code)],
    ) {
        Ok(value) => {
            let _ = env.throw(JThrowable::from(value));
        }
        Err(_) => {
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new(
                    "java/lang/IllegalStateException",
                    "X.509 signing preparation failed",
                );
            }
        }
    }
}

/// Prepare message and digest without accepting a subject signing key.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ZkX509SigningPreparationNative_prepare(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    statement: JByteArray<'_>,
) -> jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if statement.is_null() {
            return Err(holder::NULL);
        }
        let length = env
            .get_array_length(&statement)
            .map_err(|_| holder::INTERNAL)?;
        let length = usize::try_from(length).map_err(|_| holder::TOO_LARGE)?;
        if length == 0 {
            return Err(holder::EMPTY);
        }
        if length > holder::MAX_STATEMENT_BYTES {
            return Err(holder::TOO_LARGE);
        }
        let input = Zeroizing::new(
            env.convert_byte_array(&statement)
                .map_err(|_| holder::INTERNAL)?,
        );
        let prepared = holder::prepare(&input)?;
        let outputs = env
            .new_object_array(2, "[B", JObject::null())
            .map_err(|_| holder::INTERNAL)?;
        let message = env
            .byte_array_from_slice(&prepared.message)
            .map_err(|_| holder::INTERNAL)?;
        let digest = env
            .byte_array_from_slice(&prepared.digest)
            .map_err(|_| holder::INTERNAL)?;
        env.set_object_array_element(&outputs, 0, message)
            .map_err(|_| holder::INTERNAL)?;
        env.set_object_array_element(&outputs, 1, digest)
            .map_err(|_| holder::INTERNAL)?;
        Ok(outputs.into_raw())
    }))
    .unwrap_or(Err(holder::INTERNAL));
    match result {
        Ok(outputs) => outputs,
        Err(code) => {
            throw_code(&mut env, code);
            std::ptr::null_mut()
        }
    }
}
