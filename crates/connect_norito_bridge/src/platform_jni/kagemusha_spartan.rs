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
            // Three empty arrays are the buffer-free open of the owner's own installed
            // programs; three complete originals are a raw offer. The shared classifier
            // refuses every partial tuple before any owner is looked up.
            let signature = read(env, &signature, 64)?;
            let manifest = read(env, &manifest, 16384)?;
            let bundle = read(env, &bundle, native::MAX_PROGRAM_BUNDLE_BYTES)?;
            spartan::call(
                handle as u64,
                spartan::install_action(&bundle, &manifest, &signature)?,
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
            // The same two argument forms as the C entry. Four originals are the split
            // credential, certificate, account and asset. An empty certificate and an empty
            // asset make the first array one complete E6 result: the signed selected owner
            // supplies its own asset and Enrollment certificate. The shared classifier
            // refuses every other tuple before any owner is looked up.
            let certificate = read(env, &certificate, 172)?;
            let account = read(env, &account, wallet::open::BOUNDS[2])?;
            let asset = read(env, &asset, wallet::open::BOUNDS[3])?;
            let credential = read(
                env,
                &credential,
                spartan::begin_account_first_bound(certificate.len(), asset.len()),
            )?;
            spartan::call(
                handle as u64,
                spartan::begin_account_action([&credential, &certificate, &account, &asset])?,
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_time(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    selector: jint,
    token: jlong,
    first: JByteArray<'_>,
    second: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let first = read(env, &first, 512)?;
            let second = read(env, &second, 512)?;
            spartan::call(
                handle as u64,
                Action::DirectTime {
                    slot,
                    selector: selector as u32,
                    token: token as u64,
                    first: &first,
                    second: &second,
                },
            )
        });
        reply(env, result)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_confirm(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    token: jlong,
    message: JByteArray<'_>,
    der: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let message = read(env, &message, 32)?;
            let der = read(env, &der, 72)?;
            spartan::call(
                handle as u64,
                Action::Confirm {
                    token: token as u64,
                    message: &message,
                    der: &der,
                },
            )
        });
        reply(env, result)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_creditStatus(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    selector: jint,
    operation: JByteArray<'_>,
    request: JByteArray<'_>,
    status: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let operation = fixed(env, &operation)?;
            let (request_max, status_max) = spartan::credit_status_input_limits(selector as u32)?;
            let request = read(env, &request, request_max)?;
            let status = read(env, &status, status_max)?;
            spartan::call(
                handle as u64,
                Action::CreditStatus {
                    slot,
                    selector: selector as u32,
                    operation,
                    request: &request,
                    status: &status,
                },
            )
        });
        reply(env, result)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_issueOpenRequest(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    request_id: JByteArray<'_>,
    amount: JByteArray<'_>,
    fee: JByteArray<'_>,
    issuer: JByteArray<'_>,
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let id = fixed(env, &request_id)?;
            let amount = u128::from_le_bytes(
                read(env, &amount, 16)?
                    .try_into()
                    .map_err(|_| wallet::Failure::code(wallet::INVALID))?,
            );
            let fee = read(env, &fee, 255)?;
            let issuer = read(env, &issuer, 172)?;
            spartan::call(
                handle as u64,
                Action::IssueOpenRequest {
                    slot,
                    id,
                    amount,
                    fee: &fee,
                    issuer: &issuer,
                },
            )
        });
        reply(env, result)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_installBlacklist(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    original: JByteArray<'_>,
    certificates: JByteArray<'_>,
) -> jobject {
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
    };
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let original = read(env, &original, KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1)?;
            let certificates = read(env, &certificates, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
            spartan::call(
                handle as u64,
                Action::InstallBlacklist {
                    slot,
                    original: &original,
                    certificates: &certificates,
                },
            )
        });
        reply(env, result)
    })
}


#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_unloadSettlement(
    env:jni::EnvUnowned<'_>, _class:JClass<'_>, handle:jlong, slot:JByteArray<'_>, operation:JByteArray<'_>, signed:JByteArray<'_>, proof:JByteArray<'_>
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let operation = fixed(env, &operation)?;
            let signed = read(env, &signed, 64*1024)?;
            let proof = read(env, &proof, iroha_data_model::kagemusha::KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1)?;
            spartan::call(handle as u64, Action::UnloadSettlement { slot, operation, signed:&signed, proof:&proof })
        });
        reply(env, result)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_activationSettlement(
    env: jni::EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    slot: JByteArray<'_>,
    signed: JByteArray<'_>,
    proof: JByteArray<'_>,
) -> jobject {
    use iroha_data_model::kagemusha::KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1;
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let signed = read(env, &signed, 64 * 1024)?;
            let proof = read(env, &proof, KAGEMUSHA_WALLET_LEDGER_FINALITY_MAX_BYTES_V1)?;
            spartan::call(
                handle as u64,
                Action::ActivationSettlement { slot, signed: &signed, proof: &proof },
            )
        });
        reply(env, result)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_NativeSpartanBridge_epoch(
    env:jni::EnvUnowned<'_>, _class:JClass<'_>, handle:jlong, slot:JByteArray<'_>, expected:jlong, boundary:JByteArray<'_>
) -> jobject {
    crate::platform_jni::with_java_env(env, |env| {
        let result = wallet::run(|| {
            let slot = fixed(env, &slot)?;
            let boundary = read(env, &boundary, iroha_data_model::sumeragi_finality::MAX_COMMIT_CERTIFICATE_BYTES_V1)?;
            spartan::call(handle as u64, Action::Epoch { slot, expected:expected as u64, boundary:&boundary })
        });
        reply(env, result)
    })
}
