//! Isolated device integration fixture. Fixed public test issuers can never be
//! selected by the production SDK. Every journal action and hardware receipt
//! below uses the real canonical provider and complete experimental engine.
use connect_norito_bridge::kagemusha_wallet_ffi::spartan_device_fixture::SessionAdapter;
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
use native::Engine as _;
use std::{path::Path, sync::Arc};
mod account_checks;
mod measurements;
use measurements::Measurements;
use std::time::Instant;
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
    let start = Instant::now();
    let mut measurements = Measurements::new();
    let result = exercise_measured(
        root,
        artifacts,
        platform,
        apple,
        approval,
        &mut measurements,
    );
    let report = measurements.finish(root, start, result.is_ok())?;
    result
        .map(|summary| format!("{summary}\n{report}"))
        .map_err(|error| format!("{error}\n{report}"))
}

fn exercise_measured<P: KagemushaWalletPlatformV1 + 'static>(
    root: &Path,
    artifacts: &Path,
    platform: Arc<P>,
    apple: Option<&apple::Callbacks>,
    approval: Option<&dyn Confirmation>,
    measurements: &mut Measurements,
) -> Result<String> {
    let (bundle, manifest, signature) = read_artifacts(artifacts)?;
    measurements.program_bundle(&bundle);
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
        // Native custody admits only an owner-only root, whatever the umask.
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&custody)
            .map_err(fail)?;
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
    let originals = [
        credential.as_slice(),
        certificate.as_slice(),
        account.as_slice(),
        asset.as_slice(),
    ];
    let runtime = FutureRuntime::from_installed(provider, parameters, ());
    let mut runtime = SessionAdapter::new(runtime, &bundle, &manifest, &signature)?;
    // Each later owner re-reads the immutable artifact files, as a restarted
    // process would. Release the encoded transport buffers before proving.
    drop(bundle);
    drop(manifest);
    let admission = account_checks::authorize(
        &mut runtime,
        originals,
        |challenge| advance::engineering_fixture::sign(&f, challenge),
        None,
    )?;
    let slot = admission.slot;
    let operation = operation_id(&witness);
    let exact = complete(&mut runtime, &slot, &witness, &[], None, measurements)?
        .encode()
        .map_err(fail)?;
    runtime.close()?;
    drop(runtime);
    let owner = Owner {
        artifacts,
        custody: &custody,
        platform: &platform,
        scheme: scheme.scheme_id(),
        fixture: &f,
        originals,
        admission: &admission,
    };
    let (mut runtime, verification_parameters) =
        measurements.measure("Bootstrap", "cold_start_drill", || owner.open())?;
    let retained = runtime.output(&slot.0, &operation, 2)?;
    if retained.status != 1 {
        return Err("cold start did not recover exact completion".into());
    }
    if retained.bytes != exact {
        return Err("cold-start output changed".into());
    }
    let wallet: [u8; 32] = credential[66..98].try_into().unwrap();
    let retained = native::CompletionRecord::decode(&retained.bytes, &wallet).map_err(fail)?;
    let bootstrap_head = retained.output_part(0).map_err(fail)?;
    let load = native::device_fixture::load100(&witness).map_err(fail)?;
    let loaded = complete(
        &mut runtime,
        &slot,
        &load,
        &bootstrap_head,
        None,
        measurements,
    )?;
    let load_head = loaded.output_part(0).map_err(fail)?;
    // Wallet B, the fixture's software counterparty under this wallet's scheme
    // and asset. The completed-Request relation refuses payer == receiver, so
    // the two wallets pay each other: this hardware wallet A pays B 10, and B
    // pays the same 10 back. A device holds one hardware payment key, so B's
    // key is a public fixture scalar and B has no platform custody. B's
    // Bootstrap, Receive and Send are proved in this process by the same
    // engine under the same installed programs.
    let engine = native::NativeEngine::default();
    let counterparty =
        native::device_fixture::Counterparty::new(&verification_parameters, witness.credential())
            .map_err(fail)?;
    // B makes the Request for exactly this payer state. Its fixture signature
    // is deterministic, so a restart rebuilds the same Send witness.
    let b_request =
        native::device_fixture::request_to(&load, counterparty.credential()).map_err(fail)?;
    let send = native::device_fixture::send10_to(
        &load,
        counterparty.credential(),
        counterparty.sign_request(&b_request).map_err(fail)?,
    )
    .map_err(fail)?;
    let sent = complete(
        &mut runtime,
        &slot,
        &send,
        &load_head,
        approval,
        measurements,
    )?;
    let outgoing = sent.output_part(1).map_err(fail)?;
    measurements.outgoing_payment(&outgoing);
    // The independent proof reader's view of A's own Payment: B receives it,
    // and its payer profile is A's Send head, the predecessor of A's Receive.
    let outgoing_payment = measurements.measure("CounterpartyReceive", "verify_payment", || {
        engine
            .verify_payment(&outgoing, &verification_parameters)
            .map_err(fail)
    })?;
    // B's return Payment is retained before A's Receive starts, so a restart
    // receives exactly the same bytes. A's hardware Request signature is not
    // deterministic; it travels inside these bytes.
    let incoming_path = root.join("incoming-payment.bin");
    if !incoming_path.exists() {
        let bytes = counterparty_round(
            &engine,
            &counterparty,
            &outgoing_payment,
            &verification_parameters,
            platform.as_ref(),
            &slot,
            witness.credential(),
            measurements,
        )?;
        save_fixture_input(&incoming_path, &bytes)?;
        measurements.incoming_payment(&bytes);
        // A receives in the next launch. This process still holds what B's
        // three proofs left resident, and a phone wallet receiving a Payment
        // has not just proved for a counterparty; that memory and those
        // caches would be counted in the measured Receive.
        runtime.close()?;
        return Ok(format!(
            "stage 1 of 2: hardware Bootstrap/account/cold-start with invalid-signature, changed-credential and old-challenge rejection, BLS-finalized fixture Load100, confirmed Send10 to the counterparty wallet, and counterparty Bootstrap/Receive and Send10 back under a hardware-signed Request passed; outgoing Payment {} bytes, incoming Payment {} bytes retained; launch again to receive it",
            outgoing.len(),
            bytes.len()
        ));
    }
    // The measured Receive, in a launch that did not prove for B: retained
    // incoming bytes to a verified, durable output in A's platform custody.
    let receive_start = Instant::now();
    let instrumentation_before = measurements.instrumentation_us();
    let payment_bytes = measurements.measure("Receive", "retained_incoming_bytes", || {
        std::fs::read(&incoming_path).map_err(fail)
    })?;
    measurements.incoming_payment(&payment_bytes);
    let projection = measurements.measure("Receive", "c_payment_verification", || {
        runtime.verify_payment(&payment_bytes)
    })?;
    let payment = measurements.measure("Receive", "independent_verifier_drill", || {
        engine
            .verify_payment(&payment_bytes, &verification_parameters)
            .map_err(fail)
    })?;
    measurements.measure("Receive", "projection_comparison_drill", || {
        let mut expected_projection = payment.digest_bytes().to_vec();
        expected_projection.extend_from_slice(&payment.request().body);
        expected_projection.extend_from_slice(&payment.request().signature);
        if projection != expected_projection {
            return Err(
                "C Payment verification projection differs from the native proof reader".into(),
            );
        }
        Ok(())
    })?;
    let (receive, send_head) = measurements.measure("Receive", "witness_construction", || {
        Ok((
            native::device_fixture::receive_from(
                &witness,
                &send,
                outgoing_payment.profile(),
                &payment,
            )
            .map_err(fail)?,
            sent.output_part(0).map_err(fail)?,
        ))
    })?;
    let receive_result = complete(
        &mut runtime,
        &slot,
        &receive,
        &send_head,
        None,
        measurements,
    );
    measurements.receive_flow(receive_start, instrumentation_before);
    let received = receive_result?;
    if !received.output_part(1).map_err(fail)?.is_empty() {
        return Err("Receive unexpectedly emits Payment".into());
    }
    // The exact outgoing object remains retrievable after the subsequent Receive.
    let send_operation = operation_id(&send);
    let retry = runtime.output(&slot.0, &send_operation, 1)?;
    if retry.status != 1 {
        return Err("outgoing Payment retry missing after Receive".into());
    }
    if retry.bytes != outgoing {
        return Err("outgoing retry changed complete Payment bytes".into());
    }
    runtime.require_all_entries()?;
    runtime.close()?;
    drop(runtime);
    drop(verification_parameters);
    // Durable and releasable: a new owner of the same custody, as after a
    // process restart, returns the Receive's exact completion through the
    // production C output entry, and still the exact outgoing Payment.
    let receive_operation = operation_id(&receive);
    let (mut runtime, verification_parameters) =
        measurements.measure("Receive", "cold_start_drill", || owner.open())?;
    measurements.measure("Receive", "cold_start_output_drill", || {
        for part in 0..=2 {
            let output = runtime.output(&slot.0, &receive_operation, part)?;
            if output.status != 1 || output.bytes != received.output_part(part).map_err(fail)? {
                return Err("cold start changed the Receive completion".into());
            }
        }
        let retry = runtime.output(&slot.0, &send_operation, 1)?;
        if retry.status != 1 || retry.bytes != outgoing {
            return Err("cold start changed the outgoing Payment".into());
        }
        Ok(())
    })?;
    // Native admission burns a Payment it cannot credit instead of refusing
    // the Receive; the new head must show the credit and no Burn.
    measurements.measure("Receive", "credit_check_drill", || {
        let head = engine
            .verify_head(
                &received.output_part(0).map_err(fail)?,
                witness.credential(),
                &verification_parameters,
            )
            .map_err(fail)?;
        native::device_fixture::require_credited(outgoing_payment.profile(), head.profile())
            .map_err(fail)
    })?;
    runtime.close()?;
    std::fs::write(root.join("complete-payment.bin"), &outgoing).map_err(fail)?;
    Ok(format!(
        "actual C dispatch/registry/buffer lifecycle, hardware Bootstrap/account/cold-start with invalid-signature, changed-credential and old-challenge rejection, BLS-finalized fixture Load100, confirmed Send10 to the counterparty wallet, counterparty Bootstrap/Receive and Send10 back under a hardware-signed Request, complete Payment verification, Receive of the returned Payment in custody, credited without Burn and recovered exactly by a cold-started owner, and exact outgoing retry passed; outgoing Payment {} bytes, incoming Payment {} bytes",
        outgoing.len(),
        payment_bytes.len()
    ))
}

/// What a new owner of this wallet's custody needs, as after a process
/// restart: the immutable artifact files, the custody directory, the platform,
/// and the account originals and previous admission.
struct Owner<'a, P> {
    artifacts: &'a Path,
    custody: &'a Path,
    platform: &'a Arc<P>,
    scheme: [u8; 32],
    fixture: &'a advance::engineering_fixture::Fixture,
    originals: [&'a [u8]; 4],
    admission: &'a account_checks::Admitted,
}
impl<P: KagemushaWalletPlatformV1 + 'static> Owner<'_, P> {
    /// Fresh fixture-root parameter authentication, provider, runtime and C
    /// session, then account admission that refuses the previous owner's
    /// challenge signature and must select the same custody slot.
    fn open(&self) -> Result<(SessionAdapter, Arc<native::AuthenticatedParameters>)> {
        let (bundle, manifest, signature) = read_artifacts(self.artifacts)?;
        let parameters =
            native::device_fixture::install(&bundle, &manifest, signature).map_err(fail)?;
        let provider: NativeProviderV1<_, _> = advance::KagemushaWalletProviderV1::open(
            advance::KagemushaWalletStdFsV1::open(self.custody).map_err(fail)?,
            self.platform.clone(),
            self.scheme,
            advance::KagemushaWalletProviderOptionsV1::default(),
        )
        .map_err(fail)?;
        let runtime = FutureRuntime::from_installed(provider, parameters, ());
        let verification = runtime.authenticated_parameters();
        let mut runtime = SessionAdapter::new(runtime, &bundle, &manifest, &signature)?;
        drop(bundle);
        drop(manifest);
        account_checks::authorize(
            &mut runtime,
            self.originals,
            |challenge| advance::engineering_fixture::sign(self.fixture, challenge),
            Some(self.admission),
        )?;
        Ok((runtime, verification))
    }
}

fn read_artifacts(artifacts: &Path) -> Result<(Vec<u8>, Vec<u8>, [u8; 64])> {
    let bundle = std::fs::read(artifacts.join("program-bundle.norito")).map_err(fail)?;
    let manifest = std::fs::read(artifacts.join("parameter-manifest.norito")).map_err(fail)?;
    let signature = std::fs::read(artifacts.join("parameter-signature.bin"))
        .map_err(fail)?
        .try_into()
        .map_err(|_| "fixture signature length")?;
    Ok((bundle, manifest, signature))
}

/// Wallet B receives A's Payment and pays the same 10 back under a Request
/// that A's payment key signs through the canonical Request domain. Returns
/// B's complete return Payment, the carrier of A's measured Receive.
#[allow(clippy::too_many_arguments)]
fn counterparty_round<P: KagemushaWalletPlatformV1 + ?Sized>(
    engine: &native::NativeEngine,
    counterparty: &native::device_fixture::Counterparty,
    payment: &native::VerifiedIncomingPayment,
    parameters: &native::AuthenticatedParameters,
    platform: &P,
    slot: &advance::KagemushaWalletSlotIdV1,
    payee: &native::Credential,
    measurements: &mut Measurements,
) -> Result<Vec<u8>> {
    let bootstrap = counterparty.bootstrap();
    let origin = counterparty.origin(engine, parameters).map_err(fail)?;
    let enrolled = software_advance(
        "CounterpartyBootstrap",
        engine,
        counterparty,
        bootstrap,
        &origin,
        parameters,
        measurements,
    )?;
    let receive = native::device_fixture::receive_from(
        bootstrap,
        bootstrap,
        enrolled.head().profile(),
        payment,
    )
    .map_err(fail)?;
    let received = software_advance(
        "CounterpartyReceive",
        engine,
        counterparty,
        &receive,
        enrolled.head(),
        parameters,
        measurements,
    )?;
    native::device_fixture::require_credited(enrolled.head().profile(), received.head().profile())
        .map_err(fail)?;
    let origin =
        native::device_fixture::SendOrigin::after_receive(&receive, received.head().profile())
            .map_err(fail)?;
    // A makes the return Request with its hardware key.
    let request = native::device_fixture::request_from(&origin, payee);
    let key =
        iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1::from_sec1_bytes(&payee.body[130..195])
            .map_err(fail)?;
    let request_signature = measurements.measure("Request", "hardware_signature", || {
        advance::kagemusha_wallet_sign_domain_v1(
            platform,
            slot,
            &key,
            iroha_data_model::kagemusha::KagemushaWalletSigningDomainV1::Request,
            &request,
        )
        .map(|signature| *signature.as_raw_bytes())
        .map_err(fail)
    })?;
    let back = native::device_fixture::send10_from(&origin, payee, request_signature)
        .map_err(fail)?;
    let sent = software_advance(
        "CounterpartySend",
        engine,
        counterparty,
        &back,
        received.head(),
        parameters,
        measurements,
    )?;
    sent.completion().output_part(1).map_err(fail)
}

/// One operation of the software counterparty: prepared, signed with its
/// public fixture scalar and finalized by the engine, without platform custody.
fn software_advance(
    operation: &str,
    engine: &native::NativeEngine,
    wallet: &native::device_fixture::Counterparty,
    witness: &native::ExactOperationWitness,
    head: &native::VerifiedWalletHead,
    parameters: &native::AuthenticatedParameters,
    measurements: &mut Measurements,
) -> Result<native::CompletedOperation> {
    let prepared = measurements.measure(operation, "prepare", || {
        engine.prepare(witness, head, parameters).map_err(fail)
    })?;
    let receipt = measurements.measure(operation, "software_signature", || {
        wallet.sign(&prepared).map_err(fail)
    })?;
    measurements.measure(operation, "finalize", || {
        engine
            .finalize(prepared, &receipt, parameters)
            .map_err(fail)
    })
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
fn complete(
    runtime: &mut SessionAdapter,
    slot: &advance::KagemushaWalletSlotIdV1,
    witness: &native::ExactOperationWitness,
    head: &[u8],
    approval: Option<&dyn Confirmation>,
    measurements: &mut Measurements,
) -> Result<native::CompletionRecord> {
    let name = match witness {
        native::ExactOperationWitness::Bootstrap(_) => "Bootstrap",
        native::ExactOperationWitness::Load { .. } => "Load",
        native::ExactOperationWitness::Send(_) => "Send",
        native::ExactOperationWitness::Receive { .. } => "Receive",
    };
    let operation = operation_id(witness);
    let wallet: [u8; 32] = witness.credential().body[66..98].try_into().unwrap();
    let retained =
        measurements.measure(name, "lookup", || runtime.output(&slot.0, &operation, 2))?;
    match retained.status {
        1 => {
            measurements.retained(name);
            // Repeated process launches use retained output. Even a replayed
            // Prepare must refuse the obsolete predecessor, without a new debit.
            measurements.measure(name, "retained_replay_drill", || {
                if runtime
                    .prepare(&slot.0, &witness.encode().map_err(fail)?, head)
                    .is_ok()
                {
                    return Err("completed operation admitted a stale C preparation".into());
                }
                runtime.discard(0)?;
                if runtime.execute(0).is_ok() {
                    return Err("unknown C review token became executable after discard".into());
                }
                Ok(())
            })?;
            return native::CompletionRecord::decode(&retained.bytes, &wallet).map_err(fail);
        }
        0 => {}
        _ => {
            return Err(format!(
                "operation remains pending; C output status {}",
                retained.status
            ));
        }
    }
    let encoded =
        measurements.measure(name, "witness_encoding", || witness.encode().map_err(fail))?;
    // Measure the first real preparation before the extra retry drill can warm
    // any process caches. The replacement review selects identical inputs.
    let unused =
        measurements.measure(name, "prepare", || runtime.prepare(&slot.0, &encoded, head))?;
    let prepared = measurements.measure(name, "discard_reprepare_drill", || {
        runtime.discard(unused.token)?;
        if runtime.execute(unused.token).is_ok() {
            return Err("discarded C review token remained executable".into());
        }
        runtime.prepare(&slot.0, &encoded, head)
    })?;
    if prepared.token == unused.token || prepared.review != unused.review {
        return Err("fresh C review changed the exact prepared capsule".into());
    }
    if matches!(witness, native::ExactOperationWitness::Send(_)) {
        let approval = approval.ok_or("actual hardware Send confirmation adapter is required")?;
        measurements.measure(name, "confirmation_wait", || {
            approval.confirm(&prepared.review)
        })?;
    }
    let completion = measurements.measure(name, "execute", || runtime.execute(prepared.token))?;
    if completion.status != 1 {
        return Err(format!(
            "selected operation remains pending; C execute status {} detail {}",
            completion.status, completion.detail
        ));
    }
    measurements.measure(name, "exact_execute_retry_drill", || {
        let retried = runtime.execute(prepared.token)?;
        if retried.status != completion.status
            || retried.detail != completion.detail
            || retried.bytes != completion.bytes
        {
            return Err("lost C execute reply changed the exact completion".into());
        }
        Ok(())
    })?;
    let record = native::CompletionRecord::decode(&completion.bytes, &wallet).map_err(fail)?;
    measurements.measure(name, "output_verification_drill", || {
        for part in 0..=2 {
            let output = runtime.output(&slot.0, &operation, part)?;
            if output.status != 1 || output.bytes != record.output_part(part).map_err(fail)? {
                return Err("C output differs from the exact completion".into());
            }
        }
        runtime.discard(prepared.token)?;
        Ok(())
    })?;
    Ok(record)
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
#[cfg(test)]
mod host_tests;
