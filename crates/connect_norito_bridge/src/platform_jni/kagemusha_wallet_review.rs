//! JNI financial-review DATA uses the same single Native owner as the C boundary.
use super::kagemusha_wallet_advance::{read, response, response_class};
use crate::kagemusha_wallet_ffi::{self as wallet, exchange, review};
use jni::{
    objects::{JByteArray, JClass},
    sys::{jint, jlong, jobject},
};

/// Synchronous short control: reserve before foreign worker or UI queueing.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_beginExchange(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |mut env| {
        let mut retained = None;
        let result = wallet::run(|| {
            let issued = exchange::begin(handle as u64)?;
            let result = issued.response();
            retained = Some(issued);
            Ok(result)
        });
        exchange::deliver_issued(retained, result, |result| {
            let object = response(&mut env, result);
            (object, !object.is_null())
        })
    })
}

/// Fence fresh calls without invalidating children already admitted by the same owner.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_endExchange(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |mut env| {
        let result = wallet::run(|| {
            Ok(exchange::end_response(exchange::end(
                handle as u64,
                exchange_id as u64,
            )?))
        });
        response(&mut env, result)
    })
}
/// Request-bound Send1/Unload8: Ready18, existing66 or tokenless maintenance67.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_review(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    request_id: JByteArray<'_>,
    selector: jint,
    amount_low: jlong,
    amount_high: jlong,
    expectation: jint,
    expected_projection: JByteArray<'_>,
    first: JByteArray<'_>,
    second: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |mut env| {
        let mut admitted = None;
        let mut delivery = review::CapabilityDelivery::default();
        let result = wallet::run(|| {
            let selector =
                u32::try_from(selector).map_err(|_| wallet::Failure::code(wallet::INVALID))?;
            let expectation =
                u32::try_from(expectation).map_err(|_| wallet::Failure::code(wallet::INVALID))?;
            let request_id = read(&mut env, &request_id, 32)?;
            let expected_projection =
                read(&mut env, &expected_projection, review::PROJECTION_MAX_BYTES)?;
            let bounds = review::bounds(selector)?;
            let first = read(&mut env, &first, bounds[0])?;
            let second = read(&mut env, &second, bounds[1])?;
            let amount = u128::from(amount_low as u64) | (u128::from(amount_high as u64) << 64);
            let input = review::request(
                &request_id,
                selector,
                amount,
                expectation,
                &expected_projection,
                &first,
                &second,
            )?;
            admitted = Some(exchange::Invocation::admit(
                handle as u64,
                exchange_id as u64,
            )?);
            review::review(
                admitted
                    .as_ref()
                    .ok_or(wallet::Failure::code(wallet::INTERNAL))?,
                input,
                &mut delivery,
            )
        });
        review::deliver(delivery, result, |result| {
            let object = response_class(
                &mut env,
                result,
                jni::jni_str!(
                    "org/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletReviewReplyV1"
                ),
            );
            (object, !object.is_null())
        })
    })
}
/// Actual retained review supplies the privately bound request ID and all financial inputs.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_executeReviewed(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    token: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |mut env| {
        let mut admitted = None;
        let mut delivery = None;
        let result = wallet::run(|| {
            admitted = Some(exchange::Invocation::admit(
                handle as u64,
                exchange_id as u64,
            )?);
            delivery = Some(review::CapabilityDelivery::financial(
                admitted
                    .as_ref()
                    .ok_or(wallet::Failure::code(wallet::INTERNAL))?,
                token as u64,
            ));
            let response = review::execute_reviewed(
                admitted
                    .as_ref()
                    .ok_or(wallet::Failure::code(wallet::INTERNAL))?,
                token as u64,
            )?;
            Ok(response)
        });
        let retained = result.as_ref().is_ok_and(|response| response.kind == 65);
        let object = response(&mut env, result);
        if let Some(delivery) = delivery.as_mut() {
            delivery.complete(retained && !object.is_null());
        }
        object
    })
}
/// Cancellation drops only the in-memory actual review; never durable monetary data.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletNativeV1_discardReview(
    _env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    exchange_id: jlong,
    token: jlong,
) -> jint {
    wallet::run(|| {
        review::discard_review(
            &exchange::Invocation::cleanup(handle as u64, exchange_id as u64)?,
            token as u64,
        )
    })
    .err()
    .map_or(0, |error| error.status)
}
