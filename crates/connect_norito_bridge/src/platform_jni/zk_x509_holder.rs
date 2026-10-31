//! JNI adapter for the same canonical public X.509 signing preparation as C.

use super::super::zk_x509_holder_ffi as holder;
use jni::{
    Env,
    objects::{JByteArray, JClass, JObject, JThrowable, JValue},
    sys::{jint, jobjectArray},
};
use zeroize::Zeroizing;

fn throw_code(env: &mut Env<'_>, code: jint) {
    // Preserve an already-pending JVM allocation/array exception.
    if env.exception_check() {
        return;
    }
    match env.new_object(
        jni::jni_str!("org/hyperledger/iroha/sdk/privacy/ZkX509SigningPreparationException"),
        jni::jni_sig!("(I)V"),
        &[JValue::Int(code)],
    ) {
        Ok(value) => {
            if let Ok(throwable) = JThrowable::cast_local(env, value) {
                let _ = env.throw(throwable);
            }
        }
        Err(_) => {
            if !env.exception_check() {
                let _ = env.throw_new(
                    jni::jni_str!("java/lang/IllegalStateException"),
                    jni::jni_str!("X.509 signing preparation failed"),
                );
            }
        }
    }
}

/// Prepare message and digest without accepting a subject signing key.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_ZkX509SigningPreparationNative_prepare(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    statement: JByteArray<'_>,
) -> jobjectArray {
    crate::platform_jni::with_java_env(env, |env| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if statement.is_null() {
                return Err(holder::NULL);
            }
            let length = statement.len(env).map_err(|_| holder::INTERNAL)?;
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
                .new_object_array(2, jni::jni_str!("[B"), JObject::null())
                .map_err(|_| holder::INTERNAL)?;
            let message = env
                .byte_array_from_slice(&prepared.message)
                .map_err(|_| holder::INTERNAL)?;
            let digest = env
                .byte_array_from_slice(&prepared.digest)
                .map_err(|_| holder::INTERNAL)?;
            outputs
                .set_element(env, 0, message)
                .map_err(|_| holder::INTERNAL)?;
            outputs
                .set_element(env, 1, digest)
                .map_err(|_| holder::INTERNAL)?;
            Ok(outputs.into_raw())
        }))
        .unwrap_or(Err(holder::INTERNAL));
        match result {
            Ok(outputs) => outputs,
            Err(code) => {
                throw_code(env, code);
                std::ptr::null_mut()
            }
        }
    })
}
