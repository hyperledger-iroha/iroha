//! Bounded DATA copies through the existing Native observation owner.
use super::kagemusha_wallet_advance::{read, response_class};
use crate::kagemusha_wallet_ffi as wallet;
use jni::{
    Env,
    objects::{JByteArray, JClass},
    sys::{jint, jlong, jobject},
};

fn response(env: &mut Env<'_>, result: wallet::Result<wallet::Response>) -> jobject {
    response_class(
        env,
        result,
        jni::jni_str!("org/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletObservationReplyV1"),
    )
}

/// Copy admission, released output, prepared Load DATA or public fold diagnostics from
/// the same live owner. Each selector keeps its own fixed input and output bounds.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaWalletObservationNativeV1_observe(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    selector: jint,
    identity: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |mut env| {
        let result = wallet::run(|| {
            let selector =
                u32::try_from(selector).map_err(|_| wallet::Failure::code(wallet::INVALID))?;
            let identity = read(&mut env, &identity, 32)?;
            wallet::observation::observe(handle as u64, selector, &identity)
        });
        response(&mut env, result)
    })
}
