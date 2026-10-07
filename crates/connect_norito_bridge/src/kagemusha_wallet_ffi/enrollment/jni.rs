//! Bounded JNI DATA into the same Native enrollment owner, without a foreign slot selector.
use super::*;
use ::jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObject, JValue},
    sys::{jint, jlong, jobject},
};
/// Progress retained canonical E1/policy/account and original dates; no offered freshness.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletEnrollmentNativeV1_enroll(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    runtime: jlong,
    selector: jint,
    challenge: JByteArray<'_>,
    policy: JByteArray<'_>,
    account: JByteArray<'_>,
    original: JByteArray<'_>,
    certificates: JByteArray<'_>,
    issued_at_ms: jlong,
    expires_at_ms: jlong,
) -> jobject {
    let answer = run(|| {
        if runtime <= 0 || !(0..=3).contains(&selector) || issued_at_ms <= 0 || expires_at_ms <= 0 {
            return Err(Failure::code(INVALID));
        }
        let arrays = [&challenge, &policy, &account, &original, &certificates];
        for (array, bound) in arrays.into_iter().zip(BOUNDS) {
            let length = env
                .get_array_length(array)
                .map_err(|_| Failure::code(INVALID))?;
            if length < 0 || length as usize > bound {
                return Err(Failure::code(INVALID));
            }
        }
        let values = arrays
            .into_iter()
            .map(|array| {
                env.convert_byte_array(array)
                    .map_err(|_| Failure::code(INVALID))
            })
            .collect::<Result<Vec<_>>>()?;
        let input = Input {
            selector: selector as u32,
            challenge: &values[0],
            policy: &values[1],
            account: &values[2],
            original: &values[3],
            certificates: &values[4],
            issued_at_ms: issued_at_ms as u64,
            expires_at_ms: expires_at_ms as u64,
        };
        input.validate()?;
        open::enroll(runtime as u64, input)
    });
    let (status, reason, code, value) = match answer {
        Ok(value) => (value.kind, -1, 0, value),
        Err(error) => (
            error.status,
            error.reason,
            error.platform_code,
            Response::default(),
        ),
    };
    let output = (|| -> ::jni::errors::Result<JObject<'_>> {
        let key = env.byte_array_from_slice(&value.payment_key)?;
        let account = env.byte_array_from_slice(&value.account_frame)?;
        let binding = env.byte_array_from_slice(&value.binding)?;
        let chain = env.new_object_array(value.chain.len() as jint, "[B", JObject::null())?;
        for (index, certificate) in value.chain.iter().enumerate() {
            let original = env.byte_array_from_slice(certificate)?;
            env.set_object_array_element(&chain, index as jint, &original)?;
        }
        let bytes = env.byte_array_from_slice(&value.bytes)?;
        env.new_object(
            "org/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletEnrollmentReplyV1",
            "(III[B[B[B[[B[B)V",
            &[
                JValue::Int(status),
                JValue::Int(reason),
                JValue::Int(code),
                JValue::Object(&key),
                JValue::Object(&account),
                JValue::Object(&binding),
                JValue::Object(&chain),
                JValue::Object(&bytes),
            ],
        )
    })();
    // Failed delivery does not remove the actual durable intent/request or regenerate a key.
    output.map_or(std::ptr::null_mut(), |object| object.into_raw())
}
