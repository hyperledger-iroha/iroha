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
    let key_type = if cfg!(any(target_os = "ios", target_os = "android")) {
        "device hardware"
    } else {
        "host software"
    };
    let (bundle, manifest, signature) = read_artifacts(artifacts)?;
    measurements.program_bundle(&bundle);
    let parameters = install_device_parameters(artifacts, &bundle, &manifest, signature)
        .map_err(|e| format!("authenticate signed device parameters: {e:?}"))?;
    let scheme = native::device_fixture::scheme();
    let mut f = advance::engineering_fixture::installed_fixture(
        scheme.clone(),
        InstallationV1 {
            scheme_id: scheme.scheme_id(),
            manifest_digest: *parameters.manifest_digest(),
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
            .map_err(|e| format!("create device custody directory: {e}"))?;
    }
    let mut provider: NativeProviderV1<_, _> = advance::KagemushaWalletProviderV1::open(
        advance::KagemushaWalletStdFsV1::open(&custody)
            .map_err(|e| format!("open device custody directory: {e:?}"))?,
        platform.clone(),
        scheme.scheme_id(),
        advance::KagemushaWalletProviderOptionsV1::default(),
    )
    .map_err(|e| format!("open device custody provider: {e:?}"))?;
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
        let mut enrollment = EnrollmentOwnerV1::new(provider, f.config.clone())
            .map_err(|(_, e)| format!("open device enrollment: {e:?}"))?;
        let account = norito::encode_canonical(&f.account).map_err(fail)?;
        let asset = norito::encode_canonical(&f.asset).map_err(fail)?;
        let dispatch = PreKeyDispatchV1::decode(
            &enrollment
                .begin(&[11; 32], &account, &asset)
                .map_err(|e| format!("begin device enrollment: {e:?}"))?,
        )
        .map_err(fail)?;
        let challenge = enrollment
            .accept_permit(
                &advance::engineering_fixture::permit(&f, &dispatch)
                    .encode_canonical()
                    .map_err(fail)?,
            )
            .map_err(|e| format!("accept device enrollment permit: {e:?}"))?;
        let progress = enrollment
            .authorize(&advance::engineering_fixture::sign(&f, &challenge))
            .map_err(|e| format!("authorize device enrollment: {e:?}"))?;
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
    let runtime =
        FutureRuntime::from_installed(provider, parameters, native::device_fixture::scheme(), ());
    let five_families = native::AuthenticatedParameters::is_five_manifest(&manifest);
    let verification = runtime.authenticated_parameters();
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
    #[cfg(test)]
    if let Ok(stage) = std::env::var("KAGEMUSHA_SPARTAN_HOST_LAUNCH")
        && matches!(
            stage.as_str(),
            "ordinary-receive" | "ordinary-recover" | "ordinary-verify-prepare" | "ordinary-prepare"
        )
    {
        let wallet = bootstrap.input.credential.body[66..98]
            .try_into()
            .expect("credential wallet");
        drop(verification);
        return host_tests::ordinary_receive::exercise(
            root, &mut runtime, &slot.0, &wallet, &stage, measurements,
        );
    }
    #[cfg(test)]
    if let Ok(stage) = std::env::var("KAGEMUSHA_SPARTAN_HOST_LAUNCH")
        && matches!(
            stage.as_str(),
            "refresh-prepare" | "refresh-execute" | "refresh-recover"
        )
    {
        let wallet = bootstrap.input.credential.body[66..98]
            .try_into()
            .expect("credential wallet");
        return host_tests::retained_refresh::check(root, &mut runtime, &slot.0, wallet, &stage);
    }
    #[cfg(test)]
    if std::env::var_os("KAGEMUSHA_SPARTAN_HOST_LAUNCH").as_deref()
        == Some(std::ffi::OsStr::new("retained-credit"))
    {
        return host_tests::verify_retained_credit(root, &mut runtime, &slot.0, &verification);
    }
    #[cfg(test)]
    if let Ok(stage) = std::env::var("KAGEMUSHA_SPARTAN_HOST_LAUNCH")
        && matches!(stage.as_str(), "retained-archive-refusal-v4" | "archive-execute-v5" | "archive-recover-v5")
    {
        let stage = match stage.as_str() {
            "archive-execute-v5" => ArchiveStage::Execute,
            "archive-recover-v5" => ArchiveStage::Recover,
            _ => ArchiveStage::Unavailable,
        };
        return verify_retained_archive(root, &mut runtime, &slot.0, &verification, stage, measurements);
    }
    let archive_ready = root.join("archive-ready-after-receive.bin");
    if five_families && archive_ready.try_exists().map_err(fail)? {
        let operation = read_fixture_digest(&archive_ready, "saved Receive operation")?;
        let receive = runtime.output(&slot.0, &operation, 2)?;
        if receive.status != 1 {
            return Err("ArchiveSent fixture requires the exact retained Receive".into());
        }
        let stage = if root.join("archive-completion.norito").try_exists().map_err(fail)? {
            ArchiveStage::Recover
        } else {
            ArchiveStage::Execute
        };
        return verify_retained_archive(root, &mut runtime, &slot.0, &verification, stage, measurements);
    }
    drop(verification);
    let operation = operation_id(&witness);
    let exact = complete(
        &mut runtime,
        &slot,
        &witness,
        &[],
        &wallet_action(1, &[])?,
        None,
        None,
        measurements,
    )?
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
    measurements.measure("Bootstrap", "activation_recovery_drill", || {
        let exact = runtime.activation(&slot.0)?;
        if exact[..32] != scheme.scheme_id()
            || exact[32..64] != credential[34..66]
            || exact[64..96] != *verification_parameters.manifest_digest()
            || exact[96..128] != credential[66..98]
            || runtime.activation(&slot.0)? != exact
        {
            return Err("signed activation response changed its exact scope or original".into());
        }
        let saved = root.join("bootstrap-activation.bin");
        if saved.exists() {
            if std::fs::read(&saved).map_err(fail)? != exact {
                return Err("cold restart changed the wallet-signed activation".into());
            }
        } else {
            save_fixture_input(&saved, &exact)?;
        }
        Ok(())
    })?;
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
    let native::ExactOperationWitness::Load {
        receipt_original,
        finality_original,
        ..
    } = &load
    else {
        return Err("missing fixture Load originals".into());
    };
    let load_input = wallet_action(2, &[receipt_original, finality_original])?;
    let loaded = complete(
        &mut runtime,
        &slot,
        &load,
        &bootstrap_head,
        &load_input,
        None,
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
    let request_signature = counterparty.sign_request(&b_request).map_err(fail)?;
    let open = counterparty
        .open_request_wire(&b_request, request_signature)
        .map_err(fail)?;
    let send_input = wallet_action(3, &[&open])?;
    let send =
        native::device_fixture::send10_to(&load, counterparty.credential(), request_signature)
            .map_err(fail)?;
    // A second signed Request has a competing unselected review at the same predecessor.
    // If the first debit wins, this independent Request must remain payable on the new head.
    let still_unspent = root.join("competing-send-request.bin");
    let competing_review = root.join("competing-send-first-review.bin");
    if !still_unspent.exists() {
        let mut alternative = b_request.clone();
        alternative[386..418].copy_from_slice(&[0x68; 32]);
        let signed = counterparty.sign_request(&alternative).map_err(fail)?;
        let alternative = counterparty
            .open_request_wire(&alternative, signed)
            .map_err(fail)?;
        let action = wallet_action(3, &[&alternative])?;
        let draft = runtime.prepare(&slot.0, &action, &[])?;
        runtime.discard(draft.token)?;
        save_fixture_input(&competing_review, &draft.review)?;
        save_fixture_input(&still_unspent, &action)?;
    }
    let sent = complete(
        &mut runtime,
        &slot,
        &send,
        &load_head,
        &send_input,
        send.counterparty_account(),
        approval,
        measurements,
    )?;
    let alternative = std::fs::read(&still_unspent).map_err(fail)?;
    let competing_before = std::fs::read(&competing_review).map_err(fail)?;
    let rebound = runtime.prepare(&slot.0, &alternative, &[])?;
    if rebound.review[99..131] == competing_before[99..131] {
        return Err(
            "an abandoned competing Send did not rebind to the actual released head".into(),
        );
    }
    runtime.discard(rebound.token)?;
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
    // The receiver issues its signed Request from its actual Released head. The same stable
    // fixture action id must reproduce the original across process launches and a later Receive.
    let return_request = measurements.measure("Request", "issue_or_retry", || {
        let original = runtime.open_request(&slot.0, &[0x39; 32], 10)?;
        if runtime.open_request(&slot.0, &[0x39; 32], 10)? != original
            || runtime.open_request(&slot.0, &[0x39; 32], 11).is_ok()
        {
            return Err("open Request retry changed its terms or signed bytes".into());
        }
        Ok(original)
    })?;
    let incoming_path = root.join("incoming-payment.bin");
    if !incoming_path.exists() {
        let (bytes, outgoing_status) = counterparty_round(
            root,
            &engine,
            &counterparty,
            &outgoing_payment,
            &verification_parameters,
            &open,
            &return_request,
            witness.credential(),
            measurements,
        )?;
        match outgoing_status {
            Ok(wire) => save_fixture_input(&root.join("outgoing-credit-status.bin"), &wire)?,
            Err(reason) => {
                // Finish the independent payment and receiver recovery checks before reporting
                // a transport refusal. The original encoder diagnostic includes the actual size.
                eprintln!("counterparty CreditStatus refused: {reason}");
                save_fixture_input(
                    &root.join("outgoing-credit-status-error.txt"),
                    reason.as_bytes(),
                )?;
            }
        }
        save_fixture_input(&incoming_path, &bytes)?;
        measurements.incoming_payment(&bytes);
        // A receives in the next launch. This process still holds what B's
        // three proofs left resident, and a phone wallet receiving a Payment
        // has not just proved for a counterparty; that memory and those
        // caches would be counted in the measured Receive.
        runtime.close()?;
        return Ok(format!(
            "stage 1 of 2: {key_type} keys. Bootstrap, account admission, cold startup, finalized Load100 and Send10 passed. The counterparty bootstrapped, received the payment and returned 10 under the wallet-signed Request. Retained outgoing and incoming Payments are {} and {} bytes.",
            outgoing.len(),
            bytes.len()
        ));
    }
    // B shares the installed programs. Release its last reference before A reopens custody.
    drop(counterparty);
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
    let send_head = sent.output_part(0).map_err(fail)?;
    let receive_result = complete(
        &mut runtime,
        &slot,
        &send,
        &send_head,
        &wallet_action(4, &[&[0x39; 32], &payment_bytes])?,
        Some(
            payment.payer().body[98..130]
                .try_into()
                .expect("fixture payer account"),
        ),
        None,
        measurements,
    );
    measurements.receive_flow(receive_start, instrumentation_before);
    let received = receive_result?;
    if !received.output_part(1).map_err(fail)?.is_empty() {
        return Err("Receive unexpectedly emits Payment".into());
    }
    // The exact outgoing object remains retrievable after the subsequent Receive.
    let send_operation = *sent.operation_id();
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
    let receive_operation = *received.operation_id();
    let (mut runtime, verification_parameters) =
        measurements.measure("Receive", "cold_start_drill", || owner.open())?;
    measurements.measure("Receive", "cold_start_output_drill", || {
        if runtime.next_load_ordinal(&slot.0)? != 1 {
            return Err("cold wallet changed the consumed Load ordinal".into());
        }
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
    #[cfg(test)]
    let credit_status_check = if std::env::var_os("KAGEMUSHA_SPARTAN_HOST_LAUNCH").as_deref()
        == Some(std::ffi::OsStr::new("3"))
    {
        measurements.measure(
            "CreditStatus",
            "cold_receiver_source_and_independent_peer",
            || {
                let status = runtime.credit_status_for_payment(
                    slot.0,
                    [0x39; 32],
                    receive_operation,
                    &payment_bytes,
                )?;
                let verify =
                    |wire: &[u8], request: &[u8], payer: &native::VerifiedIncomingPayment| {
                        engine.verify_credit_status_peer(
                            wire,
                            request,
                            payer,
                            &verification_parameters,
                        )
                    };
                if verify(&status, &return_request, &payment).map_err(fail)?
                    != native::VerifiedCreditStatus::Credited
                    || verify(&status, &return_request, &outgoing_payment).is_ok()
                    || verify(&status, &[], &payment).is_ok()
                {
                    return Err(
                        "CreditStatus failed to bind the returned Payment and its open Request"
                            .into(),
                    );
                }
                let mut changed = status.clone();
                *changed.last_mut().ok_or("empty credit status")? ^= 1;
                if verify(&changed, &return_request, &payment).is_ok() {
                    return Err("altered credit status accepted".into());
                }
                if runtime
                    .credit_status(slot.0, [0x38; 32], receive_operation)
                    .is_ok()
                    || runtime
                        .credit_status(slot.0, [0x39; 32], send_operation)
                        .is_ok()
                    || runtime
                        .credit_status_for_payment(slot.0, [0x39; 32], receive_operation, &outgoing)
                        .is_ok()
                {
                    return Err("unissued or unrelated source yielded a credit status".into());
                }
                let outgoing_status = match std::fs::read(root.join("outgoing-credit-status.bin")) {
                    Ok(bytes) => bytes,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        let capsule = native::RecoveryCapsule::decode(
                            &std::fs::read(root.join("counterparty-receive-capsule.norito"))
                                .map_err(fail)?,
                            &scheme.scheme_id(),
                        )
                        .map_err(fail)?;
                        let completed = native::CompletionRecord::decode(
                            &std::fs::read(root.join("counterparty-receive-completion.norito"))
                                .map_err(fail)?,
                            capsule.wallet(),
                        )
                        .map_err(fail)?;
                        let bytes = engine
                            .encode_credit_status_peer(
                                &capsule,
                                &completed,
                                &open,
                                &outgoing_payment,
                                &verification_parameters,
                            )
                            .map_err(fail)?;
                        save_fixture_input(&root.join("outgoing-credit-status.bin"), &bytes)?;
                        bytes
                    }
                    Err(error) => return Err(fail(error)),
                };
                if !runtime.verify_credit_status(slot.0, send_operation, &open, &outgoing_status)?
                    || !runtime.verify_credit_status(
                        slot.0,
                        send_operation,
                        &[],
                        &outgoing_status,
                    )?
                    || runtime
                        .verify_credit_status(
                            slot.0,
                            send_operation,
                            &return_request,
                            &outgoing_status,
                        )
                        .is_ok()
                    || runtime
                        .verify_credit_status(slot.0, receive_operation, &open, &outgoing_status)
                        .is_ok()
                {
                    return Err(
                        "payer C verification did not bind its own Send, Request and peer".into(),
                    );
                }
                let mut altered = outgoing_status.clone();
                *altered
                    .last_mut()
                    .ok_or("empty counterparty credit status")? ^= 1;
                if runtime
                    .verify_credit_status(slot.0, send_operation, &open, &altered)
                    .is_ok()
                {
                    return Err("payer C verifier accepted altered acknowledgement".into());
                }
                eprintln!(
                    "Verified credited receipt: {} full IPM1 bytes, {} payload bytes",
                    status.len(),
                    status.len() - 84
                );
                Ok(())
            },
        )
    } else {
        Ok(())
    };
    // Confirm the financial effect under the actual released recursive head.
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
    #[cfg(test)]
    if std::env::var_os("KAGEMUSHA_SPARTAN_HOST_LAUNCH").as_deref()
        == Some(std::ffi::OsStr::new("3"))
    {
        host_tests::recovery::check(
            &owner,
            &receive_operation,
            &send_operation,
            &outgoing,
            &send,
            &load_head,
        )?;
    }
    #[cfg(test)]
    credit_status_check?;
    save_fixture_input(&root.join("complete-payment.bin"), &outgoing)?;
    // Handset launches four and five exercise ArchiveSent only after launch three
    // has recovered Receive and both exact Payments without another transfer.
    if five_families && measurements.was_retained("Receive") {
        save_fixture_input(&archive_ready, &receive_operation)?;
    }
    Ok(format!(
        "{key_type} keys. C dispatch, account admission and its rejection cases, finalized Load100, Send10, receipt of the returned Payment and cold recovery passed. Native verified the credit and both original Payments. Outgoing and incoming Payments are {} and {} bytes.",
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
            install_device_parameters(self.artifacts, &bundle, &manifest, signature)?;
        let provider: NativeProviderV1<_, _> = advance::KagemushaWalletProviderV1::open(
            advance::KagemushaWalletStdFsV1::open(self.custody).map_err(fail)?,
            self.platform.clone(),
            self.scheme,
            advance::KagemushaWalletProviderOptionsV1::default(),
        )
        .map_err(fail)?;
        let runtime = FutureRuntime::from_installed(
            provider,
            parameters,
            native::device_fixture::scheme(),
            (),
        );
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

fn read_fixture_digest(path: &Path, name: &str) -> Result<[u8; 32]> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("{name}: {e}"))?;
    let mut digest = [0; 32];
    file.read_exact(&mut digest).map_err(fail)?;
    if file.read(&mut [0; 1]).map_err(fail)? != 0 {
        return Err(format!("{name} length"));
    }
    Ok(digest)
}

fn install_device_parameters(
    artifacts: &Path,
    bundle: &[u8],
    manifest: &[u8],
    signature: [u8; 64],
) -> Result<native::AuthenticatedParameters> {
    let selected = if native::AuthenticatedParameters::is_five_manifest(manifest) {
        Some(read_fixture_digest(&artifacts.join("world-manifest-digest.bin"), "fixture World manifest digest")?)
    } else {
        None
    };
    native::device_fixture::install_selected(bundle, manifest, signature, selected.as_ref())
        .map_err(fail)
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
fn counterparty_round(
    root: &Path,
    engine: &native::NativeEngine,
    counterparty: &native::device_fixture::Counterparty,
    payment: &native::VerifiedIncomingPayment,
    parameters: &native::AuthenticatedParameters,
    outgoing_request: &[u8],
    open_request: &[u8],
    payee: &native::Credential,
    measurements: &mut Measurements,
) -> Result<(Vec<u8>, Result<Vec<u8>>)> {
    let bootstrap = counterparty.bootstrap();
    let origin = counterparty.origin(engine, parameters).map_err(fail)?;
    let (_, enrolled) = software_advance(
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
    let (receive_capsule, received) = software_advance(
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
    // Preserve B's actual selected originals so a transport failure can be retried.
    save_fixture_input(
        &root.join("counterparty-receive-capsule.norito"),
        &receive_capsule.encode().map_err(fail)?,
    )?;
    save_fixture_input(
        &root.join("counterparty-receive-completion.norito"),
        &received.completion().encode().map_err(fail)?,
    )?;
    let outgoing_status = engine
        .encode_credit_status_peer(
            &receive_capsule,
            received.completion(),
            outgoing_request,
            payment,
            parameters,
        )
        .map_err(fail);
    let origin =
        native::device_fixture::SendOrigin::after_receive(&receive, received.head().profile())
            .map_err(fail)?;
    let back =
        native::device_fixture::send10_from_open(&origin, payee, open_request).map_err(fail)?;
    let (_, sent) = software_advance(
        "CounterpartySend",
        engine,
        counterparty,
        &back,
        received.head(),
        parameters,
        measurements,
    )?;
    Ok((
        sent.completion().output_part(1).map_err(fail)?,
        outgoing_status,
    ))
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
) -> Result<(native::RecoveryCapsule, native::CompletedOperation)> {
    let prepared = measurements.measure(operation, "prepare", || {
        engine.prepare(witness, head, parameters).map_err(fail)
    })?;
    let receipt = measurements.measure(operation, "software_signature", || {
        wallet.sign(&prepared).map_err(fail)
    })?;
    let capsule = prepared.capsule().clone();
    let completed = measurements.measure(operation, "finalize", || {
        wallet
            .finalize(engine, prepared, &receipt)
            .map_err(fail)
    })?;
    Ok((capsule, completed))
}

#[derive(Clone, Copy)]
enum ArchiveStage {
    Execute,
    Recover,
    Unavailable,
}

/// Exercise the public ArchiveSent action against the selected fixture generation.
fn verify_retained_archive(
    root: &Path,
    runtime: &mut SessionAdapter,
    slot: &[u8; 32],
    parameters: &native::AuthenticatedParameters,
    stage: ArchiveStage,
    measurements: &mut Measurements,
) -> Result<String> {
    let outgoing = std::fs::read(root.join("complete-payment.bin")).map_err(fail)?;
    let credited = std::fs::read(root.join("outgoing-credit-status.bin")).map_err(fail)?;
    let payment = native::NativeEngine::default()
        .verify_payment(&outgoing, parameters)
        .map_err(fail)?;
    let send = payment.receipt().body[114..146]
        .try_into()
        .expect("fixed Send operation");
    if !runtime.verify_credit_status(*slot, send, &[], &credited)? {
        return Err("retained CreditStatus must verify as credited".into());
    }
    let before = runtime.output(slot, &[0; 32], 3)?;
    if before.status != 101 || before.bytes.len() != 196 || before.bytes[2] != 3 {
        return Err("ArchiveSent fixture requires a released wallet".into());
    }
    let command = wallet_action(5, &[&send, &credited])?;
    if !matches!(stage, ArchiveStage::Unavailable) {
        let recovered = matches!(stage, ArchiveStage::Recover);
        let completion_path = root.join("archive-completion.norito");
        if recovered != completion_path.is_file() {
            return Err("ArchiveSent fixture stage does not match its saved completion".into());
        }
        let mut damaged = credited;
        *damaged.last_mut().ok_or("empty CreditStatus")? ^= 1;
        if runtime.prepare(slot, &wallet_action(5, &[&send, &damaged])?, &[]).is_ok()
            || before.bytes != runtime.output(slot, &[0; 32], 3)?.bytes
        {
            return Err("altered CreditStatus was admitted or moved the released head".into());
        }
        let prepared = measurements.measure("ArchiveSent", "prepare", || runtime.prepare(slot, &command, &[]))?;
        let review = prepared.review;
        if review.len() != 291 || review[..3] != [1, 0, 5] || review[131..227] != [0; 96] {
            return Err("ArchiveSent must have zero monetary and destination review terms".into());
        }
        let operation: [u8; 32] = review[67..99].try_into().expect("fixed financial review");
        let selected = before.bytes[68..100] == operation;
        if (recovered && !selected) || (!selected && review[227..259] != before.bytes[100..132]) {
            return Err("ArchiveSent review does not follow the released state".into());
        }
        let phase = if selected { "exact_retry" } else { "execute" };
        let reply = measurements.measure("ArchiveSent", phase, || runtime.execute(prepared.token))?;
        runtime.discard(prepared.token)?;
        let after = runtime.output(slot, &[0; 32], 3)?;
        let retained = runtime.output(slot, &operation, 2)?;
        if reply.status != 1 || retained.status != 1 || retained.bytes != reply.bytes
            || after.status != 101 || after.bytes.len() != 196 || after.bytes[2] != 3
            || after.bytes[68..100] != operation || after.bytes[100..132] != review[259..291]
            || before.bytes[148..196] != after.bytes[148..196]
        {
            return Err("ArchiveSent did not retain the reviewed successor and wallet value".into());
        }
        let wallet = review[35..67].try_into().expect("fixed review wallet");
        let completion = native::CompletionRecord::decode(&reply.bytes, &wallet).map_err(fail)?;
        if completion.operation_id() != &operation || !completion.output_part(1).map_err(fail)?.is_empty() {
            return Err("ArchiveSent completion changed operation or produced a Payment".into());
        }
        if selected && after.bytes != before.bytes {
            return Err("exact ArchiveSent retry moved the released head".into());
        }
        if recovered {
            if reply.bytes != std::fs::read(completion_path).map_err(fail)? {
                return Err("cold ArchiveSent replay changed the selected state or exact completion".into());
            }
        } else {
            let sequence = |head: &[u8]| u128::from_le_bytes(head[132..148].try_into().unwrap());
            if !selected && sequence(&before.bytes).checked_add(1) != Some(sequence(&after.bytes)) {
                return Err("ArchiveSent must advance the wallet sequence exactly once".into());
            }
            save_fixture_input(&completion_path, &reply.bytes)?;
        }
        runtime.close()?;
        return Ok(format!("Verified ArchiveSent {phase}, {} exact completion bytes, sequence {}.",
            reply.bytes.len(), u128::from_le_bytes(after.bytes[132..148].try_into().unwrap())));
    }
    for _ in 0..2 {
        runtime.require_invalid_prepare(slot, &command, &[])?;
        let head = runtime.output(slot, &[0; 32], 3)?;
        let retry = runtime.output(slot, &send, 1)?;
        if head.status != before.status
            || head.bytes != before.bytes
            || retry.status != 1
            || retry.bytes != outgoing
        {
            return Err("ArchiveSent refusal changed the selected head or outgoing Payment".into());
        }
    }
    Ok("The signed four-kind wallet retained the original head and exact Payment after two ArchiveSent refusals.".into())
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
    fn confirm(&self, _review: &[u8]) -> Result<()> {
        Err("this platform requires an exact Native confirmation adapter".into())
    }

    #[cfg(target_os = "android")]
    fn confirm_android(&self, _owner: u64, _token: u64, _slot: &[u8; 32], _review: &[u8]) -> Result<()> {
        Err("Android hardware confirmation adapter unavailable".into())
    }
}
#[cfg(target_os = "android")]
struct AndroidConfirmation {
    vm: jni::JavaVM,
    object: jni::objects::Global<jni::objects::JObject<'static>>,
}
#[cfg(target_os = "android")]
impl Confirmation for AndroidConfirmation {
    fn confirm_android(&self, owner: u64, token: u64, slot: &[u8; 32], review: &[u8]) -> Result<()> {
        self.vm.attach_current_thread_for_scope(|env| {
            let result = env.with_local_frame(8, |env| -> jni::errors::Result<()> {
                let slot = env.byte_array_from_slice(slot)?;
                let review = env.byte_array_from_slice(review)?;
                env.call_method(
                    self.object.as_obj(),
                    jni::jni_str!("confirm"),
                    jni::jni_sig!("(JJ[B[B)V"),
                    &[
                        jni::objects::JValue::Long(owner as i64),
                        jni::objects::JValue::Long(token as i64),
                        jni::objects::JValue::Object(&slot),
                        jni::objects::JValue::Object(&review),
                    ],
                )?.v()
            });
            if result.is_err() {
                env.exception_clear();
            }
            result
        }).map_err(|error| format!("Android protected Send confirmation refused: {error}"))
    }
}

fn operation_id(witness: &native::ExactOperationWitness) -> [u8; 32] {
    let body = match witness {
        native::ExactOperationWitness::Bootstrap(w) => &w.input.receipt.body,
        native::ExactOperationWitness::Load { step, .. } => &step.receipt.body,
        native::ExactOperationWitness::RefreshTimeAnchor(step)
        | native::ExactOperationWitness::Unload { step, .. } => &step.receipt.body,
        native::ExactOperationWitness::Send(step) => &step.receipt.body,
        native::ExactOperationWitness::Receive { step, .. } => &step.receipt.body,
        native::ExactOperationWitness::ArchiveSent { step, .. } => &step.receipt.body,
    };
    body[114..146].try_into().unwrap()
}
fn wallet_action(kind: u8, originals: &[&[u8]]) -> Result<Vec<u8>> {
    let mut out = b"KSO1".to_vec();
    out.push(kind);
    for original in originals {
        let size = u32::try_from(original.len()).map_err(fail)?;
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(original);
    }
    Ok(out)
}
fn complete(
    runtime: &mut SessionAdapter,
    slot: &advance::KagemushaWalletSlotIdV1,
    witness: &native::ExactOperationWitness,
    head: &[u8],
    action: &[u8],
    counterparty: Option<[u8; 32]>,
    approval: Option<&dyn Confirmation>,
    measurements: &mut Measurements,
) -> Result<native::CompletionRecord> {
    let kind = action[4];
    let name = match kind {
        1 => "Bootstrap",
        2 => "Load",
        3 => "Send",
        4 => "Receive",
        _ => return Err("unsupported fixture action".into()),
    };
    #[cfg(not(test))]
    let _ = counterparty;
    let unused = measurements.measure(name, "prepare", || runtime.prepare(&slot.0, action, &[]))?;
    if unused.review.get(2) != Some(&kind) {
        return Err("wrong prepared wallet action".into());
    }
    let operation: [u8; 32] = unused.review[67..99]
        .try_into()
        .expect("Native review operation");
    if kind != 4 && operation != operation_id(witness) {
        return Err("wallet action changed its financial operation identity".into());
    }
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
                let historical = runtime.prepare(&slot.0, action, &[])?;
                if historical.review != unused.review {
                    return Err("retained wallet action changed its exact capsule".into());
                }
                runtime.discard(historical.token)?;
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
    measurements.measure(name, "load_ordinal_before_drill", || {
        if kind == 1 {
            if runtime.next_load_ordinal(&slot.0).is_ok() || runtime.activation(&slot.0).is_ok() {
                return Err("enrollment exposed an uncommitted ordinal or activation".into());
            }
        } else {
            // The certified Load100 is ordinal zero. Send and Receive follow it.
            let expected = u128::from(kind != 2);
            let actual = runtime.next_load_ordinal(&slot.0)?;
            if actual != expected {
                return Err(format!("before {name}, expected Load ordinal {expected}, got {actual}"));
            }
        }
        if runtime.output(&slot.0, &operation, 4).is_ok() {
            return Err("operation-scoped lookup supplied a current Load ordinal".into());
        }
        if runtime.output(&slot.0, &operation, 5).is_ok() {
            return Err("operation-scoped lookup supplied a Bootstrap activation".into());
        }
        Ok(())
    })?;
    let prepared = measurements.measure(name, "discard_reprepare_drill", || {
        runtime.discard(unused.token)?;
        if runtime.execute(unused.token).is_ok() {
            return Err("discarded C review token remained executable".into());
        }
        runtime.prepare(&slot.0, action, &[])
    })?;
    if prepared.token == unused.token {
        return Err("fresh C review reused a discarded token".into());
    }
    // Before the first selection, a discarded Bootstrap has no durable capsule.
    // A new secret blinds its successor. The enrollment, predecessor and displayed
    // terms must still match. Other wallet actions already reserve a frozen capsule.
    let same_review = if kind == 1 {
        prepared.review.len() == 291
            && unused.review.len() == 291
            && prepared.review[..99] == unused.review[..99]
            && prepared.review[131..259] == unused.review[131..259]
    } else {
        prepared.review == unused.review
    };
    if !same_review {
        return Err("fresh C review changed the exact prepared capsule".into());
    }
    #[cfg(test)]
    let prepared = match counterparty {
        Some(counterparty) => measurements.measure(name, "blacklist_reprepare_drill", || {
            let initial_version = if kind == 3 { 1 } else { 3 };
            host_tests::local_blacklist::check(
                runtime,
                &slot.0,
                prepared.token,
                action,
                &[],
                counterparty,
                initial_version,
            )?;
            let fresh = runtime.prepare(&slot.0, action, &[])?;
            if fresh.token == prepared.token || fresh.review != prepared.review {
                return Err(
                    "a changed list must permit a fresh review of the same exact terms".into(),
                );
            }
            runtime.discard(prepared.token)?;
            Ok(fresh)
        })?,
        None => prepared,
    };
    if kind == 3 {
        let approval = approval.ok_or("actual hardware Send confirmation adapter is required")?;
        measurements.measure(name, "confirmation_wait", || {
            #[cfg(target_os = "ios")]
            {
                let mut display = runtime.apple_debit_asset(prepared.token)?.to_vec();
                display.extend_from_slice(&prepared.review);
                approval.confirm(&display)?;
                runtime.confirm_apple_debit(prepared.token)?;
            }
            #[cfg(target_os = "android")]
            approval.confirm_android(
                runtime.android_confirmation_handle(), prepared.token, &slot.0, &prepared.review,
            )?;
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            approval.confirm(&prepared.review)?;
            Ok(())
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
        let expected = u128::from(kind != 1);
        let actual = runtime.next_load_ordinal(&slot.0)?;
        if actual != expected {
            return Err(format!("after {name}, expected Load ordinal {expected}, got {actual}"));
        }
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

// Forward through the retail JNI decoder and the same live registry. No
// independent response parser or unsigned approval bit enters the device.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_SpartanDeviceRuntimeNative_confirm(
    env: jni::EnvUnowned<'_>,
    class: jni::objects::JClass<'_>,
    owner: jni::sys::jlong,
    token: jni::sys::jlong,
    message: jni::objects::JByteArray<'_>,
    der: jni::objects::JByteArray<'_>,
) -> jni::sys::jobject {
    connect_norito_bridge::android_fixture_debit_confirm(env, class, owner, token, message, der)
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
