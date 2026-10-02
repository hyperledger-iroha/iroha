//! Dedicated first-device JNI; no financial coordinator or UI authority selectors.
use super::*;
use crate::kagemusha_hardware_evidence_v1 as service;

/// Return the exact closed hardware-evidence contract, distinct from monetary JNI.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaFirstDeviceHardwareEvidenceJniV1_nativeContractV1(
    env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jintArray {
    let Ok(array) = env.new_int_array(service::frame::CONTRACT.len() as i32) else {
        return ptr::null_mut();
    };
    if env
        .set_int_array_region(&array, 0, &service::frame::CONTRACT)
        .is_err()
    {
        return ptr::null_mut();
    }
    array.into_raw()
}
/// Open only the fixed directory already selected by independently authenticated Rust startup.
/// An absent startup policy returns zero; no financial owner, reply pin or fallback is created.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaFirstDeviceHardwareEvidenceJniV1_nativeOpenV1<
    'a,
>(
    mut env: jni::JNIEnv<'a>,
    _class: jni::objects::JClass<'a>,
    application: jni::objects::JObject<'a>,
    path: jni::objects::JString<'a>,
) -> jni::sys::jlong {
    let Ok(raw) = env.get_string(&path) else {
        return 0;
    };
    let Ok(path) = raw.to_str() else {
        return 0;
    };
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return 0;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        #[cfg(target_os = "android")]
        if !service::android_startup::initialize_if_packaged(
            &mut env,
            &application,
            std::path::Path::new(path),
        )? {
            return Err(
                iroha_core_zk::kagemusha_v1_state::KagemushaHardwareEvidenceErrorV1::Custody,
            );
        }
        service::open(std::path::Path::new(path))
    }))
    .ok()
    .and_then(|r| r.ok())
    .map(|h| h as i64)
    .unwrap_or(0);
    // Startup refusal must not leak an AssetManager/JNI exception into a UI crash.
    if env.exception_check().unwrap_or(true) {
        let _ = env.exception_clear();
        return 0;
    }
    result
}
/// Close only the view handle. Native retains original WAL/alias/pending platform outcomes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaFirstDeviceHardwareEvidenceJniV1_nativeCloseV1(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) -> jni::sys::jint {
    match service::close(u64::from_ne_bytes(handle.to_ne_bytes())) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
/// Invoke only the closed eighteen nonmonetary methods. Complete inputs are bounded before
/// transfer/owner intake. Failed response allocation leaves Native originals/fences retained.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaFirstDeviceHardwareEvidenceJniV1_nativeInvokeV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    method: jni::sys::jint,
    fields: jni::objects::JObjectArray<'_>,
) -> jni::sys::jobjectArray {
    let Ok(count) = env.get_array_length(&fields) else {
        return ptr::null_mut();
    };
    if count < 0 || count as usize > service::frame::MAX_FIELDS {
        return ptr::null_mut();
    }
    let mut input = zeroize::Zeroizing::new(Vec::<Vec<u8>>::new());
    for index in 0..count {
        let Ok(value) = env.get_object_array_element(&fields, index) else {
            return ptr::null_mut();
        };
        if value.is_null() {
            return ptr::null_mut();
        }
        let value = jni::objects::JByteArray::from(value);
        let Ok(size) = env.get_array_length(&value) else {
            return ptr::null_mut();
        };
        if size < 0 || size as usize > service::frame::MAX_FIELD {
            return ptr::null_mut();
        }
        let Ok(bytes) = env.convert_byte_array(&value) else {
            return ptr::null_mut();
        };
        if bytes.len() != size as usize {
            return ptr::null_mut();
        }
        input.push(bytes);
    }
    if !service::frame::valid_request(method, &input) {
        return ptr::null_mut();
    }
    let handle = u64::from_ne_bytes(handle.to_ne_bytes());
    let Ok(Ok(output)) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        service::invoke(handle, method, std::mem::take(&mut *input))
    })) else {
        return ptr::null_mut();
    };
    let output = zeroize::Zeroizing::new(output);
    let Ok(class) = env.find_class("[B") else {
        return ptr::null_mut();
    };
    let Ok(array) = env.new_object_array(output.len() as i32, class, jni::objects::JObject::null())
    else {
        return ptr::null_mut();
    };
    for (index, field) in output.iter().enumerate() {
        let Ok(value) = env.byte_array_from_slice(field) else {
            return ptr::null_mut();
        };
        if env
            .set_object_array_element(&array, index as i32, &value)
            .is_err()
        {
            return ptr::null_mut();
        }
    }
    array.into_raw()
}
