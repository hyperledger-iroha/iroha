//! JNI for the additive suffix API. Every method uses the dedicated closed result holder.
use super::kagemusha_wallet_advance::{read, response_class};
use crate::kagemusha_wallet_ffi::{self as wallet, exchange, review, suffix};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1, KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
};
use jni::{
    objects::{JByteArray, JClass},
    sys::{jlong, jobject},
};
fn reply(env: &mut jni::Env<'_>, value: wallet::Result<wallet::Response>) -> jobject {
    response_class(
        env,
        value,
        jni::jni_str!(
            "org/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletSuffixUnloadReplyV1"
        ),
    )
}

/// Read bounded Native suffix scheduling data.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadStatus(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        reply(
            env,
            wallet::run(|| suffix::read(handle as u64, suffix::Read::Status)),
        )
    })
}

/// Prepare one bounded suffix verification turn for exact caller terms.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadPrepare(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    request_id: JByteArray<'_>,
    amount_low: jlong,
    amount_high: jlong,
    quote: JByteArray<'_>,
    certificates: JByteArray<'_>,
    beneficiary: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let mut admitted = None;
        let mut delivery = review::CapabilityDelivery::default();
        let result = wallet::run(|| {
            let request_id = read(env, &request_id, 32)?;
            let quote = read(env, &quote, KAGEMUSHA_WALLET_CHARGE_QUOTE_MAX_BYTES_V1)?;
            let certificates = read(env, &certificates, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
            let beneficiary = read(env, &beneficiary, suffix::ACCOUNT_MAX)?;
            let amount = u128::from(amount_low as u64) | (u128::from(amount_high as u64) << 64);
            let input = suffix::request(&request_id, amount, &quote, &certificates, &beneficiary)?;
            admitted = Some(exchange::Invocation::admit(
                handle as u64,
                exchange_id as u64,
            )?);
            suffix::prepare(
                admitted
                    .as_ref()
                    .ok_or(wallet::Failure::code(wallet::INTERNAL))?,
                input,
                &mut delivery,
            )
        });
        // A constructor/array-allocation exception is a failed foreign handoff. The exact
        // origin-bound result73 token is removed even when Kotlin never receives an object.
        review::deliver(delivery, result, |result| {
            let object = reply(env, result);
            (object, !object.is_null())
        })
    })
}

/// Resume retained suffix preparation or exact selected Advance recovery.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadResume(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    request_id: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let mut admitted = None;
        let mut delivery = review::CapabilityDelivery::default();
        let result = wallet::run(|| {
            let request = suffix::identity(&read(env, &request_id, 32)?)?;
            admitted = Some(exchange::Invocation::admit(
                handle as u64,
                exchange_id as u64,
            )?);
            suffix::prepare(
                admitted
                    .as_ref()
                    .ok_or(wallet::Failure::code(wallet::INTERNAL))?,
                suffix::Input::Resume(request),
                &mut delivery,
            )
        });
        review::deliver(delivery, result, |result| {
            let object = reply(env, result);
            (object, !object.is_null())
        })
    })
}

/// Consume one exact origin-bound Native suffix review after hardware approval.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_executeReviewedSuffixUnload(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    token: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let mut admitted = None;
        let mut delivery = None;
        let result = wallet::run(|| {
            if token <= 0 {
                return Err(wallet::Failure::code(wallet::INVALID));
            }
            admitted = Some(exchange::Invocation::admit(
                handle as u64,
                exchange_id as u64,
            )?);
            let invocation = admitted
                .as_ref()
                .ok_or(wallet::Failure::code(wallet::INTERNAL))?;
            delivery = Some(review::CapabilityDelivery::financial(
                invocation,
                token as u64,
            ));
            suffix::execute(invocation, token as u64)
        });
        let retained = result.as_ref().is_ok_and(|value| value.kind == 65);
        let object = reply(env, result);
        if let Some(delivery) = delivery.as_mut() {
            delivery.complete(retained && !object.is_null());
        }
        object
    })
}

/// Read one exact retained suffix stage instruction.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadInstruction(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    request_id: JByteArray<'_>,
    ordinal_low: jlong,
    ordinal_high: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            suffix::read(
                handle as u64,
                suffix::Read::Instruction {
                    request: suffix::identity(&read(env, &request_id, 32)?)?,
                    ordinal: suffix::ordinal(ordinal_low as u64, ordinal_high as u64)?,
                },
            )
        });
        reply(env, result)
    })
}

/// Read one exact retained signed attempt without reconstructing its envelope.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadSignedWire(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    request_id: JByteArray<'_>,
    ordinal_low: jlong,
    ordinal_high: jlong,
    attempt_index: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let attempt =
                u64::try_from(attempt_index).map_err(|_| wallet::Failure::code(wallet::INVALID))?;
            suffix::read(
                handle as u64,
                suffix::Read::SignedWire {
                    request: suffix::identity(&read(env, &request_id, 32)?)?,
                    ordinal: suffix::ordinal(ordinal_low as u64, ordinal_high as u64)?,
                    attempt,
                },
            )
        });
        reply(env, result)
    })
}

/// Retain and inspect one exact signed suffix stage attempt.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadAttemptOutcome(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    request_id: JByteArray<'_>,
    ordinal_low: jlong,
    ordinal_high: jlong,
    signed_wire: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let wire = read(env, &signed_wire, SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1)?;
            if wire.is_empty() {
                return Err(wallet::Failure::code(wallet::INVALID));
            }
            suffix::read(
                handle as u64,
                suffix::Read::Attempt {
                    request: suffix::identity(&read(env, &request_id, 32)?)?,
                    ordinal: suffix::ordinal(ordinal_low as u64, ordinal_high as u64)?,
                    wire,
                },
            )
        });
        reply(env, result)
    })
}

/// Verify direct ledger finality for one exact retained suffix stage attempt.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_suffixUnloadIngestFinality(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    request_id: JByteArray<'_>,
    ordinal_low: jlong,
    ordinal_high: jlong,
    signed_wire: JByteArray<'_>,
    finality: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let mut admitted = None;
        let result = wallet::run(|| {
            let request = suffix::identity(&read(env, &request_id, 32)?)?;
            let ordinal = suffix::ordinal(ordinal_low as u64, ordinal_high as u64)?;
            let wire = read(env, &signed_wire, SUFFIX_UNLOAD_SIGNED_WIRE_MAX_BYTES_V1)?;
            let finality = read(
                env,
                &finality,
                KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1,
            )?;
            admitted = Some(exchange::Invocation::admit(
                handle as u64,
                exchange_id as u64,
            )?);
            suffix::ingest(
                admitted
                    .as_ref()
                    .ok_or(wallet::Failure::code(wallet::INTERNAL))?,
                request,
                ordinal,
                &wire,
                &finality,
            )
        });
        reply(env, result)
    })
}
