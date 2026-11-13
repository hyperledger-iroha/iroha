//! JNI projections of the same authenticated experimental native owner.
use super::kagemusha_wallet_advance::{read, response_class};
use crate::kagemusha_wallet_ffi::{
    self as wallet,
    spartan::{self, Action},
};
use iroha_core_zk::kagemusha_spartan_native::native;
use jni::{
    Env,
    objects::{JByteArray, JClass},
    sys::{jint, jlong, jobject},
};
fn reply(env: &mut Env<'_>, result: wallet::Result<wallet::Response>) -> jobject {
    response_class(
        env,
        result,
        jni::jni_str!("org/hyperledger/iroha/sdk/offline/wallet/KagemushaSpartanReplyV1"),
    )
}
fn fixed(env: &mut Env<'_>, bytes: &JByteArray<'_>) -> wallet::Result<[u8; 32]> {
    read(env, bytes, 32)?
        .try_into()
        .map_err(|_| wallet::Failure::code(wallet::INVALID))
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_install(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    bundle: JByteArray<'_>,
    manifest: JByteArray<'_>,
    signature: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let signature: [u8; 64] = read(env, &signature, 64)?
                .try_into()
                .map_err(|_| wallet::Failure::code(wallet::INVALID))?;
            let manifest = read(env, &manifest, 16384)?;
            let bundle = read(env, &bundle, native::MAX_PROGRAM_BUNDLE_BYTES)?;
            spartan::call(
                handle as u64,
                Action::Install {
                    bundle: &bundle,
                    manifest: &manifest,
                    signature: &signature,
                },
            )
        });
        reply(env, result)
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_prepare(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    witness: JByteArray<'_>,
    head: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let witness = zeroize::Zeroizing::new(read(env, &witness, native::MAX_RECOVERY_BYTES)?);
            let head = read(env, &head, native::MAX_OUTPUT_BYTES)?;
            spartan::call(
                handle as u64,
                Action::Prepare {
                    slot,
                    witness: &witness,
                    head: &head,
                },
            )
        });
        reply(env, result)
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_execute(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    token: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        reply(
            env,
            wallet::run(|| spartan::call(handle as u64, Action::Execute(token as u64))),
        )
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_output(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    operation: JByteArray<'_>,
    part: jint,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            spartan::call(
                handle as u64,
                Action::Output {
                    slot: fixed(env, &slot)?,
                    operation: fixed(env, &operation)?,
                    part: part as u32,
                },
            )
        });
        reply(env, result)
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_verifyPayment(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    payment: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let payment = read(env, &payment, native::MAX_OUTPUT_BYTES)?;
            spartan::call(handle as u64, Action::VerifyPayment(&payment))
        });
        reply(env, result)
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_discard(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    token: jlong,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        reply(
            env,
            wallet::run(|| spartan::call(handle as u64, Action::Discard(token as u64))),
        )
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_beginAccount(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    credential: JByteArray<'_>,
    certificate: JByteArray<'_>,
    account: JByteArray<'_>,
    asset: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let credential = read(env, &credential, 596)?;
            let certificate = read(env, &certificate, 172)?;
            let account = read(env, &account, wallet::open::BOUNDS[2])?;
            let asset = read(env, &asset, wallet::open::BOUNDS[3])?;
            spartan::call(
                handle as u64,
                Action::BeginAccount([&credential, &certificate, &account, &asset]),
            )
        });
        reply(env, result)
    })
}
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_authorizeAccount(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    signature: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let signature = read(env, &signature, 4096)?;
            spartan::call(handle as u64, Action::AuthorizeAccount(&signature))
        });
        reply(env, result)
    })
}
