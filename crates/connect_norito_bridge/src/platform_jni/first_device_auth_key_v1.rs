//! JNI front end of the independent, nonmonetary durable authentication-key owner.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
};

use crate::first_device_auth_key_v1 as owner;
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue},
    sys::jobject,
};

const PACKAGE: &str = "org/hyperledger/iroha/sdk/crypto/keystore/";

struct AndroidPlatform {
    vm: JavaVM,
    object: GlobalRef,
}

impl AndroidPlatform {
    fn new(env: &mut JNIEnv<'_>, object: &JObject<'_>) -> owner::Result<Self> {
        let result = (|| -> jni::errors::Result<Self> {
            if !env.is_instance_of(object, format!("{PACKAGE}FirstDeviceAuthNativePlatformV1"))? {
                return Err(jni::errors::Error::WrongJValueType(
                    "auth platform",
                    "exact auth platform",
                ));
            }
            Ok(Self {
                vm: env.get_java_vm()?,
                object: env.new_global_ref(object)?,
            })
        })();
        if result.is_err() {
            let _ = env.exception_clear();
        }
        result.map_err(|_| owner::Error::Invalid)
    }

    fn call<T>(
        &self,
        function: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> jni::errors::Result<T>,
    ) -> owner::Result<T> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|_| owner::Error::Unavailable)?;
        let value = env.with_local_frame(16, |env| function(env, self.object.as_obj()));
        if value.is_err() {
            let _ = env.exception_clear();
        }
        value.map_err(|_| owner::Error::Unavailable)
    }

    fn key_call(
        &self,
        generate: bool,
        slot: &[u8; 32],
        digest: &[u8],
        strong_box: bool,
    ) -> owner::Result<(i32, Vec<u8>)> {
        self.call(|env, object| {
            let slot = env.byte_array_from_slice(slot)?;
            let reply = if generate {
                let digest = env.byte_array_from_slice(digest)?;
                env.call_method(object, "generateFromConsumedNativeIntent",
                    "([B[BZ)Lorg/hyperledger/iroha/sdk/crypto/keystore/NativeFirstDeviceAuthPlatformReplyV1;",
                    &[JValue::Object(&slot), JValue::Object(&digest), JValue::Bool(u8::from(strong_box))])?.l()?
            } else {
                env.call_method(object, "probeFromNative",
                    "([B)Lorg/hyperledger/iroha/sdk/crypto/keystore/NativeFirstDeviceAuthPlatformReplyV1;",
                    &[JValue::Object(&slot)])?.l()?
            };
            let status = env.get_field(&reply, "status", "I")?.i()?;
            let bytes = JByteArray::from(env.call_method(&reply, "bytes", "()[B", &[])?.l()?);
            let length = env.get_array_length(&bytes)?;
            if !(0..=65).contains(&length) {
                return Err(jni::errors::Error::WrongJValueType("auth point", "bounded auth point"));
            }
            Ok((status, env.convert_byte_array(&bytes)?))
        })
    }
}

impl owner::Platform for AndroidPlatform {
    fn require_original(&self) -> owner::Result<()> {
        self.call(|env, object| {
            env.call_method(object, "requireOriginalFromNative", "()V", &[])
                .map(|_| ())
        })
        .map_err(|_| owner::Error::Changed)
    }
    fn transcript(&self) -> owner::Result<Vec<u8>> {
        self.call(|env, object| {
            let bytes = JByteArray::from(
                env.call_method(object, "transcriptFromNative", "()[B", &[])?
                    .l()?,
            );
            if env.get_array_length(&bytes)? != 276 {
                return Err(jni::errors::Error::WrongJValueType(
                    "transcript",
                    "full auth transcript",
                ));
            }
            env.convert_byte_array(&bytes)
        })
    }
    fn no_backup_root(&self) -> owner::Result<PathBuf> {
        self.call(|env, object| {
            let string = JString::from(
                env.call_method(
                    object,
                    "noBackupRootFromNative",
                    "()Ljava/lang/String;",
                    &[],
                )?
                .l()?,
            );
            let value: String = env.get_string(&string)?.into();
            if value.is_empty() || value.len() > 4096 || value.as_bytes().contains(&0) {
                return Err(jni::errors::Error::WrongJValueType(
                    "root",
                    "bounded protected path",
                ));
            }
            Ok(PathBuf::from(value))
        })
    }
    fn api_level(&self) -> owner::Result<u32> {
        let value = self.call(|env, object| {
            env.call_method(object, "apiLevelFromNative", "()I", &[])?
                .i()
        })?;
        u32::try_from(value).map_err(|_| owner::Error::Invalid)
    }
    fn probe(&self, slot: &[u8; 32]) -> owner::Probe {
        match self.key_call(false, slot, &[], false) {
            Ok((0, bytes)) => owner::Probe::Present(bytes),
            _ => owner::Probe::Unavailable,
        }
    }
    fn generate(&self, slot: &[u8; 32], digest: &[u8; 32], strong_box: bool) -> owner::Generated {
        match self.key_call(true, slot, digest, strong_box) {
            Ok((0, bytes)) => owner::Generated::Present(bytes),
            Ok((1, bytes)) if bytes.is_empty() && strong_box => {
                owner::Generated::StrongBoxUnavailable
            }
            _ => owner::Generated::Unavailable,
        }
    }
}

fn response(env: &mut JNIEnv<'_>, result: owner::Result<owner::KeyData>) -> jobject {
    let (status, slot, public) = match result {
        Ok(data) => (0, data.slot.to_vec(), data.public_key),
        Err(error) => (
            match error {
                owner::Error::Invalid => -1,
                owner::Error::Unavailable => -2,
                owner::Error::Busy => -3,
                owner::Error::Changed => -4,
                owner::Error::ExistingIntent => -5,
            },
            vec![],
            vec![],
        ),
    };
    let result = (|| -> jni::errors::Result<JObject<'_>> {
        let slot = env.byte_array_from_slice(&slot)?;
        let public = env.byte_array_from_slice(&public)?;
        env.new_object(
            format!("{PACKAGE}NativeFirstDeviceAuthKeyReplyV1"),
            "(I[B[B)V",
            &[
                JValue::Int(status),
                JValue::Object(&slot),
                JValue::Object(&public),
            ],
        )
    })();
    match result {
        Ok(object) => object.into_raw(),
        Err(_) => {
            let _ = env.exception_clear();
            std::ptr::null_mut()
        }
    }
}

fn invoke(
    mut env: JNIEnv<'_>,
    platform: JObject<'_>,
    transcript: JByteArray<'_>,
    original: bool,
) -> jobject {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if env
            .get_array_length(&transcript)
            .map_err(|_| owner::Error::Invalid)?
            != 276
        {
            return Err(owner::Error::Invalid);
        }
        let transcript = env
            .convert_byte_array(&transcript)
            .map_err(|_| owner::Error::Invalid)?;
        let platform = AndroidPlatform::new(&mut env, &platform)?;
        if original {
            owner::reserve_original(&platform, &transcript)
        } else {
            owner::restore_original(&platform, &transcript)
        }
    }))
    .unwrap_or(Err(owner::Error::Unavailable));
    let _ = env.exception_clear();
    response(&mut env, result)
}

/// Original auth operation; durable Native state alone supplies the private generation grant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_keystore_NativeFirstDeviceAuthKeyJniV1_reserve(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
    platform: JObject<'_>,
    transcript: JByteArray<'_>,
) -> jobject {
    invoke(env, platform, transcript, true)
}

/// Read-only auth-key recovery, without intent repair or generation.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_keystore_NativeFirstDeviceAuthKeyJniV1_restore(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
    platform: JObject<'_>,
    transcript: JByteArray<'_>,
) -> jobject {
    invoke(env, platform, transcript, false)
}
