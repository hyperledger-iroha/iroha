//! JNI is a bounded carrier into the same Native runtime enrollment owner.
use super::*;
use ::jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JObject, JValue},
    sys::{jint, jlong, jobject},
};

/// Progress exact enrollment originals; this declaration grants no generation authority.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletEnrollmentNativeV1_enroll(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    runtime: jlong,
    selector: jint,
    slot: JByteArray<'_>,
    challenge: JByteArray<'_>,
    policy: JByteArray<'_>,
    account: JByteArray<'_>,
    original: JByteArray<'_>,
    certificates: JByteArray<'_>,
) -> jobject {
    let answer = run(|| {
        if runtime <= 0 || !(0..=3).contains(&selector) {
            return Err(Failure::code(INVALID));
        }
        let arrays = [
            &slot,
            &challenge,
            &policy,
            &account,
            &original,
            &certificates,
        ];
        for (array, bound) in arrays.into_iter().zip(BOUNDS) {
            let length = env
                .get_array_length(array)
                .map_err(|_| Failure::code(INVALID))?;
            if length < 0 || length as usize > bound {
                return Err(Failure::code(INVALID));
            }
        }
        let originals = arrays
            .into_iter()
            .map(|array| {
                env.convert_byte_array(array)
                    .map_err(|_| Failure::code(INVALID))
            })
            .collect::<Result<Vec<_>>>()?;
        let input = Input {
            selector: selector as u32,
            slot: &originals[0],
            challenge: &originals[1],
            policy: &originals[2],
            account: &originals[3],
            original: &originals[4],
            certificates: &originals[5],
        };
        input.validate()?;
        open::enroll(runtime as u64, input)
    });
    let (status, reason, code, slot, key, bytes) = match answer {
        Ok(value) => (
            value.kind,
            -1,
            0,
            value.slot.to_vec(),
            value.payment_key,
            value.bytes,
        ),
        Err(error) => (
            error.status,
            error.reason,
            error.platform_code,
            vec![],
            vec![],
            vec![],
        ),
    };
    let output = (|| -> ::jni::errors::Result<JObject<'_>> {
        let slot = env.byte_array_from_slice(&slot)?;
        let key = env.byte_array_from_slice(&key)?;
        let bytes = env.byte_array_from_slice(&bytes)?;
        env.new_object(
            "org/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletEnrollmentReplyV1",
            "(III[B[B[B)V",
            &[
                JValue::Int(status),
                JValue::Int(reason),
                JValue::Int(code),
                JValue::Object(&slot),
                JValue::Object(&key),
                JValue::Object(&bytes),
            ],
        )
    })();
    output.map_or(std::ptr::null_mut(), |object| object.into_raw())
}
