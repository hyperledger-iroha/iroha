//! Isolated device integration fixture. Fixed public test issuers can never be
//! selected by the production SDK. Every journal action and hardware receipt
//! below uses the real canonical provider and complete experimental engine.
use iroha_core_zk::{
    kagemusha_spartan_native::{FutureRuntime, native},
    kagemusha_wallet_advance_v1::{self as advance, KagemushaWalletPlatformV1},
    kagemusha_wallet_artifacts_v1::InstallationV1,
    kagemusha_wallet_enrollment_v1::{
        EnrollmentOwnerV1, EnrollmentProgressV1, PlatformEvidenceV1, PreKeyDispatchV1,
        RequestPreparationV1,
    },
    kagemusha_wallet_state_v1::NativeProviderV1,
};
use std::{path::Path, sync::Arc};
mod account_checks;
type Result<T> = std::result::Result<T, String>;
fn fail(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

fn exercise<P: KagemushaWalletPlatformV1 + 'static>(
    root: &Path,
    artifacts: &Path,
    platform: Arc<P>,
    apple: Option<&apple::Callbacks>,
    approval: Option<&dyn Confirmation>,
) -> Result<String> {
    let bundle = std::fs::read(artifacts.join("program-bundle.norito")).map_err(fail)?;
    let manifest = std::fs::read(artifacts.join("parameter-manifest.norito")).map_err(fail)?;
    let signature: [u8; 64] = std::fs::read(artifacts.join("parameter-signature.bin"))
        .map_err(fail)?
        .try_into()
        .map_err(|_| "fixture signature length")?;
    let parameters =
        native::device_fixture::install(&bundle, &manifest, signature).map_err(fail)?;
    let scheme = native::device_fixture::scheme();
    let mut f = advance::engineering_fixture::installed_fixture(
        scheme.clone(),
        InstallationV1 {
            scheme_id: scheme.scheme_id(),
            manifest_digest: *parameters.id(),
        },
    );
    if apple.is_some() {
        apple::configure(&mut f)?;
    }
    let custody = root.join("custody");
    if !custody.exists() {
        std::fs::create_dir(&custody).map_err(fail)?;
    }
    let mut provider: NativeProviderV1<_, _> = advance::KagemushaWalletProviderV1::open(
        advance::KagemushaWalletStdFsV1::open(&custody).map_err(fail)?,
        platform.clone(),
        scheme.scheme_id(),
        advance::KagemushaWalletProviderOptionsV1::default(),
    )
    .map_err(fail)?;
    let saved = root.join("bootstrap-witness.norito");
    let (provider, witness) = if saved.exists() {
        let bytes = std::fs::read(&saved).map_err(fail)?;
        let witness = native::ExactOperationWitness::decode(&bytes).map_err(fail)?;
        let credential = [
            witness.credential().body.as_slice(),
            witness.credential().signature.as_slice(),
        ]
        .concat();
        let key = iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1::from_sec1_bytes(
            &witness.credential().body[130..195],
        )
        .map_err(fail)?;
        let mut matching = 0;
        for slot in provider.slots().map_err(fail)? {
            if provider
                .read_intent(&slot)
                .map_err(fail)?
                .is_some_and(|intent| {
                    intent.challenge.wallet_id(&key) == witness.credential().body[66..98]
                })
            {
                matching += 1;
                match provider.credential(&slot, 0).map_err(fail)? {
                    Some(retained) if retained == credential => {}
                    None => provider
                        .store_credential(&slot, 0, &credential)
                        .map_err(fail)?,
                    Some(_) => return Err("retained fixture credential differs".into()),
                }
            }
        }
        if matching != 1 {
            return Err("fixture retained intent must be unique".into());
        }
        (provider, witness)
    } else {
        let mut enrollment =
            EnrollmentOwnerV1::new(provider, f.config.clone()).map_err(|(_, e)| fail(e))?;
        let account = norito::encode_canonical(&f.account).map_err(fail)?;
        let asset = norito::encode_canonical(&f.asset).map_err(fail)?;
        let dispatch = PreKeyDispatchV1::decode(
            &enrollment
                .begin(&[11; 32], &account, &asset)
                .map_err(fail)?,
        )
        .map_err(fail)?;
        let challenge = enrollment
            .accept_permit(
                &advance::engineering_fixture::permit(&f, &dispatch)
                    .encode_canonical()
                    .map_err(fail)?,
            )
            .map_err(fail)?;
        let progress = enrollment
            .authorize(&advance::engineering_fixture::sign(&f, &challenge))
            .map_err(fail)?;
        let EnrollmentProgressV1::Evidence {
            slot,
            marker,
            challenge_digest,
        } = progress
        else {
            return Err("fixture enrollment did not produce actual hardware evidence".into());
        };
        let slot = advance::KagemushaWalletSlotIdV1(slot);
        let evidence = if let Some(callbacks) = apple {
            apple::collect(
                callbacks,
                &mut enrollment,
                &slot.0,
                marker.payment_key.as_sec1_bytes(),
                &challenge_digest,
            )?
        } else {
            let chain = match platform.key_attestation_chain(&slot) {
                advance::KagemushaWalletProbeV1::Present(chain) => chain,
                other => return Err(format!("actual hardware chain unavailable: {other:?}")),
            };
            norito::encode_canonical(&PlatformEvidenceV1::Android {
                certificates: chain,
            })
            .map_err(fail)?
        };
        let RequestPreparationV1::AccountChallenge(challenge) =
            enrollment.prepare_request(&evidence).map_err(fail)?
        else {
            return Err("fixture request already selected unexpectedly".into());
        };
        enrollment
            .retain_request(&advance::engineering_fixture::sign(&f, &challenge))
            .map_err(fail)?;
        let mut provider = enrollment.into_provider();
        let intent = provider
            .read_intent(&slot)
            .map_err(fail)?
            .ok_or("actual enrollment intent missing")?;
        let witness =
            native::device_fixture::bootstrap(&parameters, &intent.challenge, &marker.payment_key)
                .map_err(fail)?;
        let credential = [
            witness.credential().body.as_slice(),
            witness.credential().signature.as_slice(),
        ]
        .concat();
        let bytes = witness.encode().map_err(fail)?;
        // The private fixture input precedes E6 so either side of a process exit
        // resumes the same exact enrollment credential; monetary custody is Native.
        save_fixture_input(&saved, &bytes)?;
        provider
            .store_credential(&slot, 0, &credential)
            .map_err(fail)?;
        (provider, witness)
    };
    let native::ExactOperationWitness::Bootstrap(bootstrap) = &witness else {
        return Err("fixture bootstrap input kind".into());
    };
    let credential = [
        bootstrap.input.credential.body.as_slice(),
        bootstrap.input.credential.signature.as_slice(),
    ]
    .concat();
    let certificate = [
        bootstrap.input.certificate.body.as_slice(),
        bootstrap.input.certificate.signature.as_slice(),
    ]
    .concat();
    let account = norito::encode_canonical(&f.account).map_err(fail)?;
    let asset = norito::encode_canonical(&f.asset).map_err(fail)?;
    let mut runtime = FutureRuntime::from_installed(provider, parameters, ());
    let admission = account_checks::authorize(
        &mut runtime,
        [&credential, &certificate, &account, &asset],
        |challenge| advance::engineering_fixture::sign(&f, challenge),
        None,
    )?;
    let slot = admission.slot;
    let operation: [u8; 32] = bootstrap.input.receipt.body[114..146].try_into().unwrap();
    let exact = match runtime.lookup(&slot, &operation).map_err(fail)? {
        advance::KagemushaWalletLookupV1::Retained(retained) => {
            retained.record.encode().map_err(fail)?
        }
        advance::KagemushaWalletLookupV1::Unknown => {
            let prepared = runtime
                .prepare(&slot, &witness.encode().map_err(fail)?, &[])
                .map_err(fail)?;
            match runtime.advance(&slot, prepared).map_err(fail)? {
                advance::KagemushaWalletAdvanceOutcomeV1::Released { retained, .. } => {
                    retained.record.encode().map_err(fail)?
                }
                other => {
                    return Err(format!(
                        "fixture Bootstrap remains pending: {}",
                        match other {
                            advance::KagemushaWalletAdvanceOutcomeV1::Pending { .. } => "selected",
                            _ => "not released",
                        }
                    ));
                }
            }
        }
        _ => return Err("fixture current Bootstrap output unavailable".into()),
    };
    drop(runtime);
    let parameters =
        native::device_fixture::install(&bundle, &manifest, signature).map_err(fail)?;
    let provider: NativeProviderV1<_, _> = advance::KagemushaWalletProviderV1::open(
        advance::KagemushaWalletStdFsV1::open(&custody).map_err(fail)?,
        platform.clone(),
        scheme.scheme_id(),
        advance::KagemushaWalletProviderOptionsV1::default(),
    )
    .map_err(fail)?;
    let mut runtime = FutureRuntime::from_installed(provider, parameters, ());
    account_checks::authorize(
        &mut runtime,
        [&credential, &certificate, &account, &asset],
        |challenge| advance::engineering_fixture::sign(&f, challenge),
        Some(&admission),
    )?;
    let advance::KagemushaWalletLookupV1::Retained(retained) =
        runtime.lookup(&slot, &operation).map_err(fail)?
    else {
        return Err("cold start did not recover exact completion".into());
    };
    if retained.record.encode().map_err(fail)? != exact {
        return Err("cold-start output changed".into());
    }
    let bootstrap_head = retained.record.output_part(0).map_err(fail)?;
    let load = native::device_fixture::load100(&witness).map_err(fail)?;
    let loaded = complete(&mut runtime, &slot, &load, &bootstrap_head, None)?;
    let load_head = loaded.output_part(0).map_err(fail)?;
    let request = native::device_fixture::request_self(&load).map_err(fail)?;
    let key = iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1::from_sec1_bytes(
        &witness.credential().body[130..195],
    )
    .map_err(fail)?;
    let request_path = root.join("request-signature.bin");
    let signature: [u8; 64] = if request_path.exists() {
        std::fs::read(&request_path)
            .map_err(fail)?
            .try_into()
            .map_err(|_| "retained request signature length")?
    } else {
        let signature = advance::kagemusha_wallet_sign_domain_v1(
            platform.as_ref(),
            &slot,
            &key,
            iroha_data_model::kagemusha::KagemushaWalletSigningDomainV1::Request,
            &request,
        )
        .map_err(fail)?;
        let raw = *signature.as_raw_bytes();
        save_fixture_input(&request_path, &raw)?;
        raw
    };
    let send = native::device_fixture::send10(&load, signature).map_err(fail)?;
    let sent = complete(&mut runtime, &slot, &send, &load_head, approval)?;
    let payment_bytes = sent.output_part(1).map_err(fail)?;
    let payment = runtime.verify_payment(&payment_bytes).map_err(fail)?;
    let receive = native::device_fixture::receive_self(&witness, &send, &payment).map_err(fail)?;
    let send_head = sent.output_part(0).map_err(fail)?;
    let received = complete(&mut runtime, &slot, &receive, &send_head, None)?;
    if !received.output_part(1).map_err(fail)?.is_empty() {
        return Err("Receive unexpectedly emits Payment".into());
    }
    // The exact outgoing object remains retrievable after the subsequent Receive.
    let operation = operation_id(&send);
    let advance::KagemushaWalletLookupV1::Retained(retry) =
        runtime.lookup(&slot, &operation).map_err(fail)?
    else {
        return Err("outgoing Payment retry missing after Receive".into());
    };
    if retry.record.output_part(1).map_err(fail)? != payment_bytes {
        return Err("outgoing retry changed complete Payment bytes".into());
    }
    std::fs::write(root.join("complete-payment.bin"), &payment_bytes).map_err(fail)?;
    Ok(format!(
        "actual hardware Bootstrap/account/cold-start with invalid-signature, changed-credential and old-challenge rejection, BLS-finalized fixture Load100, confirmed self-Send10, complete Payment verification, Receive and exact outgoing retry passed; Payment {} bytes",
        payment_bytes.len()
    ))
}

fn save_fixture_input(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = path.with_extension("pending");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(fail)?;
    file.write_all(bytes).map_err(fail)?;
    file.sync_all().map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)?;
    std::fs::File::open(path.parent().ok_or("fixture parent path")?)
        .map_err(fail)?
        .sync_all()
        .map_err(fail)?;
    Ok(())
}

trait Confirmation {
    fn confirm(&self, review: &[u8]) -> Result<()>;
}
#[cfg(target_os = "android")]
struct AndroidConfirmation {
    vm: jni::JavaVM,
    object: jni::objects::Global<jni::objects::JObject<'static>>,
}
#[cfg(target_os = "android")]
impl Confirmation for AndroidConfirmation {
    fn confirm(&self, review: &[u8]) -> Result<()> {
        let result = self
            .vm
            .attach_current_thread_for_scope(|env| {
                let result = env.with_local_frame(8, |env| -> jni::errors::Result<bool> {
                    let review = env.byte_array_from_slice(review)?;
                    env.call_method(
                        self.object.as_obj(),
                        jni::jni_str!("confirm"),
                        jni::jni_sig!("([B)Z"),
                        &[jni::objects::JValue::Object(&review)],
                    )?
                    .z()
                });
                if result.is_err() {
                    env.exception_clear();
                }
                result
            })
            .map_err(fail)?;
        if result {
            Ok(())
        } else {
            Err("hardware Send confirmation refused".into())
        }
    }
}

fn operation_id(witness: &native::ExactOperationWitness) -> [u8; 32] {
    let body = match witness {
        native::ExactOperationWitness::Bootstrap(w) => &w.input.receipt.body,
        native::ExactOperationWitness::Load { step, .. } => &step.receipt.body,
        native::ExactOperationWitness::Send(step) => &step.receipt.body,
        native::ExactOperationWitness::Receive { step, .. } => &step.receipt.body,
    };
    body[114..146].try_into().unwrap()
}
fn complete<F: advance::KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static>(
    runtime: &mut FutureRuntime<F, P, ()>,
    slot: &advance::KagemushaWalletSlotIdV1,
    witness: &native::ExactOperationWitness,
    head: &[u8],
    approval: Option<&dyn Confirmation>,
) -> Result<native::CompletionRecord> {
    let operation = operation_id(witness);
    match runtime.lookup(slot, &operation).map_err(fail)? {
        advance::KagemushaWalletLookupV1::Retained(retained) => return Ok(retained.record),
        advance::KagemushaWalletLookupV1::Unknown => {}
        _ => return Err("operation remains pending; retained capsule must recover".into()),
    }
    let prepared = runtime
        .prepare(slot, &witness.encode().map_err(fail)?, head)
        .map_err(fail)?;
    if matches!(witness, native::ExactOperationWitness::Send(_)) {
        let approval = approval.ok_or("actual hardware Send confirmation adapter is required")?;
        approval.confirm(&prepared.review_bytes().map_err(fail)?)?;
    }
    match runtime.advance(slot, prepared).map_err(fail)? {
        advance::KagemushaWalletAdvanceOutcomeV1::Released { retained, .. } => Ok(retained.record),
        _ => Err("selected operation remains pending; retry exact retained capsule".into()),
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_SpartanDeviceRuntimeNative_run(
    mut unowned: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    platform: jni::objects::JObject<'_>,
    root: jni::objects::JString<'_>,
    artifacts: jni::objects::JString<'_>,
    confirmation: jni::objects::JObject<'_>,
) -> jni::sys::jstring {
    unowned
        .with_env(|env| {
            let root: String = env.get_string(&root)?.into();
            let artifacts: String = env.get_string(&artifacts)?.into();
            let approval = AndroidConfirmation {
                vm: env.get_java_vm()?,
                object: env.new_global_ref(&confirmation)?,
            };
            let result =
                connect_norito_bridge::kagemusha_wallet_ffi::AndroidPlatform::new(env, &platform)
                    .map_err(|_| "actual Android platform unavailable".to_string())
                    .and_then(|platform| {
                        exercise(
                            Path::new(&root),
                            Path::new(&artifacts),
                            Arc::new(platform),
                            None,
                            Some(&approval),
                        )
                    });
            let result = match result {
                Ok(value) => format!("PASS: {value}"),
                Err(error) => format!("FAIL: {error}"),
            };
            Ok::<_, jni::errors::Error>(env.new_string(result)?.into_raw())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

mod apple;
