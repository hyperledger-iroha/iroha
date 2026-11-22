//! Complete native containment checks, invoked only after exact program closure.
//! Software fixture signatures exercise the sealed Engine, not device custody.
use super::*;
use group::{Curve, Group};

#[path = "archive_remove_successor_tests.rs"]
mod archive_remove_successor_tests;
#[path = "signed_receive_scalar_tests.rs"]
mod signed_receive_scalar_tests;

#[test]
#[ignore = "requires the signed device bundle and two distinct retained Payments"]
fn repeated_payment_verification_is_exact_and_local_to_an_installation() {
    use std::{path::PathBuf, time::Instant};
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let wallet = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
    let programs = std::fs::read(bundle.join("program-bundle.norito")).unwrap();
    let manifest = std::fs::read(bundle.join("parameter-manifest.norito")).unwrap();
    let signature = std::fs::read(bundle.join("parameter-signature.bin"))
        .unwrap()
        .try_into()
        .unwrap();
    let install =
        || super::super::device_fixture::install(&programs, &manifest, signature).unwrap();
    let parameters = install();
    let first = std::fs::read(wallet.join("complete-payment.bin")).unwrap();
    let second = std::fs::read(wallet.join("incoming-payment.bin")).unwrap();
    assert_ne!(first, second);

    // Receive never needs a time observation, but its available memory may
    // change between receipt-body validation and irreversible selection.
    use iroha_data_model::kagemusha::{KagemushaWalletAnchoredTimeV1, KagemushaWalletMonotonicReadingV1};
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Memory(AtomicU64);
    type TimeReading = (KagemushaWalletAnchoredTimeV1, KagemushaWalletMonotonicReadingV1);
    impl TimeSource for Memory {
        fn available_memory_bytes(&self) -> Result<u64> { Ok(self.0.load(Ordering::Relaxed)) }
        fn prepared(&self, _: &[u8; 32], _: &[u8; 32]) -> Result<TimeReading> {
            Err(Error::Unavailable("no Send clock in Receive fixture"))
        }
        fn observe(&self, _: &[u8; 32], _: &[u8; 32]) -> Result<TimeReading> {
            Err(Error::Unavailable("no Send clock in Receive fixture"))
        }
    }
    let memory = Arc::new(Memory(AtomicU64::new(u64::MAX)));
    let engine = NativeEngine::with_time_source(memory.clone());
    let started = Instant::now();
    let accepted = engine.verify_payment(&first, &parameters).unwrap();
    let cold = started.elapsed();
    let started = Instant::now();
    let repeated = engine.clone().verify_payment(&first, &parameters).unwrap();
    let warm = started.elapsed();
    assert!(Arc::ptr_eq(&accepted.bytes, &repeated.bytes));
    let mut damaged = first.clone();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(engine.verify_payment(&damaged, &parameters).is_err());
    assert!(Arc::ptr_eq(
        &accepted.bytes,
        &engine.verify_payment(&first, &parameters).unwrap().bytes
    ));
    let independent = NativeEngine::default()
        .verify_payment(&first, &parameters)
        .unwrap();
    assert!(!Arc::ptr_eq(&accepted.bytes, &independent.bytes));
    let other = engine.verify_payment(&second, &parameters).unwrap();
    assert_eq!(other.exact_bytes(), second);
    let replaced = engine.verify_payment(&first, &parameters).unwrap();
    assert!(!Arc::ptr_eq(&accepted.bytes, &replaced.bytes));

    // The provider has already checked this exact capsule before signing. A staged
    // readback still has to verify its own receipt, output proof and selected wallet.
    let capsule = RecoveryCapsule::decode(
        &std::fs::read(wallet.join("counterparty-receive-capsule.norito")).unwrap(),
        &parameters.inner.scheme,
    )
    .unwrap();
    let record = CompletionRecord::decode(
        &std::fs::read(wallet.join("counterparty-receive-completion.norito")).unwrap(),
        capsule.wallet(),
    )
    .unwrap();
    let started = Instant::now();
    let prepared = engine.resume(&capsule, &parameters).unwrap();
    let reconstruction = started.elapsed();
    let started = Instant::now();
    engine.clone().recheck_selection(&prepared, &parameters).unwrap();
    let live_recheck = started.elapsed();
    let prepared = engine.retain_reconstruction(prepared, &parameters).unwrap();
    eprintln!("signed Receive reconstruction {reconstruction:?}, same-owner live controls {live_recheck:?}");
    memory.0.store(1, Ordering::Relaxed);
    assert!(matches!(
        engine.recheck_selection(&prepared, &parameters),
        Err(Error::NotEnoughMemory { available: 1, .. })
    ));
    memory.0.store(u64::MAX, Ordering::Relaxed);
    assert!(matches!(
        NativeEngine::default().recheck_selection(&prepared, &parameters),
        Err(Error::AuthorityMismatch("resumed selection authority"))
    ));
    let genesis = parameters.genesis_commit_verifier();
    let checkpoint = genesis.export_epoch_checkpoint(genesis.initial_commit_epoch()).unwrap();
    let reselected = engine.with_protected_epoch(&parameters, &checkpoint).unwrap();
    assert!(matches!(
        reselected.recheck_selection(&prepared, &parameters),
        Err(Error::AuthorityMismatch("resumed selection authority"))
    ));
    let released = engine
        .verify_staged_completion(&prepared, &record, &parameters)
        .unwrap();
    assert_eq!(
        released.completion().encode().unwrap(),
        record.encode().unwrap()
    );
    let mut wrong_wallet = record.clone();
    wrong_wallet.frame.wallet[0] ^= 1;
    assert!(
        engine
            .verify_staged_completion(&prepared, &wrong_wallet, &parameters)
            .is_err()
    );
    let mut wrong_receipt = record.clone();
    *wrong_receipt.frame.receipt.last_mut().unwrap() ^= 1;
    assert!(
        engine
            .verify_staged_completion(&prepared, &wrong_receipt, &parameters)
            .is_err()
    );
    let mut wrong_output = record.clone();
    *wrong_output.frame.output.last_mut().unwrap() ^= 1;
    assert!(
        engine
            .verify_staged_completion(&prepared, &wrong_output, &parameters)
            .is_err()
    );

    drop(parameters);
    let next_installation = install();
    assert!(matches!(
        engine.recheck_selection(&prepared, &next_installation),
        Err(Error::AuthorityMismatch("resumed selection authority"))
    ));
    assert!(engine.retain_reconstruction(prepared, &next_installation).is_none());
    let reinstalled = engine.verify_payment(&first, &next_installation).unwrap();
    assert!(!Arc::ptr_eq(&replaced.bytes, &reinstalled.bytes));
    eprintln!(
        "signed Payment {} bytes, first {cold:?}, same-owner repeat {warm:?}",
        first.len()
    );
}

pub(super) fn reject_wrong_program_digests(
    parameters: &AuthenticatedParameters,
    original: &[u8],
) -> Result<()> {
    for side in 0..2 {
        for kind in 0..4 {
            let mut manifest: ParameterManifest = super::super::decode(original)?;
            if side == 0 {
                manifest.primary[kind][0] ^= 1;
            } else {
                manifest.secondary[kind][0] ^= 1;
            }
            let bytes = super::super::encode(&manifest)?;
            // A correctly signed manifest still cannot select a different program.
            let signature = super::test_sign_manifest(&bytes);
            let programs = std::array::from_fn(|i| Program {
                primary: parameters.inner.programs[i].primary.clone(),
                secondary: parameters.inner.programs[i].secondary.clone(),
            });
            if !matches!(
                ParameterSet::authenticate(
                    programs,
                    &bytes,
                    signature,
                    parameters.inner.root.clone(),
                    parameters.inner.finality.clone(),
                ),
                Err(Error::AuthorityMismatch("exact compiled monetary program"))
            ) {
                return Err(Error::AuthorityMismatch("all eight exact program pins"));
            }
        }
    }
    Ok(())
}

/// G5: construction artifacts at every native accepting boundary. A template
/// pair from the compiler's last warm-up generation verifies as linked proofs
/// under these exact programs, but its accumulators fold toy or earlier-program
/// claims. The head boundary (which also guards preparation, resume, finalize
/// and completion recovery, all through `decode_head`/`verify_head`), the
/// Payment boundary and the Receive candidate verdict each accept such a pair
/// exactly when the caller's independent decision says its accumulators hold.
pub(super) fn reject_template_artifacts(
    engine: &NativeEngine,
    parameters: &AuthenticatedParameters,
    chain: &crate::chain_fixture::Chain,
    templates: &[(full::UniformPair, bool)],
) -> Result<()> {
    let verdict = |label: &str, result: Result<()>, holds: bool| -> Result<bool> {
        match (result, holds) {
            (Ok(()), true) => Ok(false),
            (Err(e), false) if super::is_accumulator_refusal(&e) => Ok(true),
            (other, _) => {
                eprintln!("native template {label}: holds={holds}, result={other:?}");
                Err(Error::AuthorityMismatch("template accumulator verdict"))
            }
        }
    };
    let trusted = parameters.inner.profile(full::SEND)?;
    let payment_profile = payment_wire::Profile {
        pair: &trusted,
        scheme: parameters.inner.scheme,
        relation: parameters.inner.relation,
    };
    let mut refused = 0;
    for (template, holds) in templates {
        let (credential, certificate) = if template.kind == full::RECEIVE {
            (
                &chain.receiver_bootstrap.credential,
                &chain.receiver_bootstrap.certificate,
            )
        } else {
            (&chain.send.credential, &chain.sender_bootstrap.certificate)
        };
        // The pair itself, then the retained-head boundary.
        let label = format!("kind {} pair", template.kind);
        let direct = decide_accumulators(&parameters.inner.generators, template);
        refused += usize::from(verdict(&label, direct, *holds)?);
        let head = encode_head(parameters, template, credential, certificate)?;
        let label = format!("kind {} head", template.kind);
        let result = engine
            .verify_head(&head, credential, parameters)
            .map(|_| ());
        refused += usize::from(verdict(&label, result, *holds)?);
        if template.kind == full::SEND {
            let payment = payment_wire::encode(
                &payment_profile,
                &chain.payment_originals(),
                &template.ip,
                &template.is,
                &template.pp,
                &template.ps,
                template.r,
            )
            .map_err(Error::Rejected)?;
            let result = engine.verify_payment(&payment, parameters).map(|_| ());
            refused += usize::from(verdict("Send Payment", result, *holds)?);
            // At Receive, the same bytes never credit: Native derives Burn.
            if engine.incoming_candidate(&payment, parameters)?.valid != *holds {
                return Err(Error::AuthorityMismatch("template incoming verdict"));
            }
        }
    }
    if !templates.is_empty() && refused == 0 {
        return Err(Error::AuthorityMismatch(
            "no template artifact with a false accumulator was exercised",
        ));
    }
    eprintln!(
        "native template artifacts: {} pairs, {refused} accumulator refusals across pair, head and Payment boundaries",
        templates.len()
    );
    Ok(())
}

/// G6 at the engine boundaries: a credential the real issuer signed, whose
/// certified table commitment is another wallet's, is refused at original
/// intake and inside a retained head.
pub(super) fn reject_wrong_certified_table(
    engine: &NativeEngine,
    parameters: &AuthenticatedParameters,
    chain: &crate::chain_fixture::Chain,
    load: &full::UniformPair,
) -> Result<()> {
    let mut wrong = chain.send.credential.clone();
    wrong.body[195..259].copy_from_slice(&chain.receiver_bootstrap.credential.body[195..259]);
    wrong.signature = bootstrap::sign(
        credential::message(&wrong),
        &BigUint::from_bytes_be(&[0x26; 32]),
        "correctly issued wrong-table credential",
    );
    let certificate = &chain.sender_bootstrap.certificate;
    let table =
        |r: Result<()>| matches!(r, Err(Error::AuthorityMismatch("certified wallet table")));
    let intake = engine
        .authenticate_credential_originals(
            &credential_bytes(&wrong),
            &certificate_bytes(certificate),
            parameters,
        )
        .map(|_| ());
    if !table(intake) {
        return Err(Error::AuthorityMismatch("wrong-table credential intake"));
    }
    let head = encode_head(parameters, load, &wrong, certificate)?;
    if !table(engine.verify_head(&head, &wrong, parameters).map(|_| ())) {
        return Err(Error::AuthorityMismatch("wrong-table credential head"));
    }
    Ok(())
}

fn finish(
    label: &str,
    engine: &NativeEngine,
    parameters: &AuthenticatedParameters,
    key: &full::WalletKey,
    head: &VerifiedWalletHead,
    witness: &ExactOperationWitness,
) -> Result<CompletedOperation> {
    let started = std::time::Instant::now();
    eprintln!("native operation: preparing {label}");
    let prepared = engine.prepare(witness, head, parameters)?;
    finish_prepared(label, engine, parameters, key, prepared, started)
}

fn finish_prepared(
    label: &str,
    engine: &NativeEngine,
    parameters: &AuthenticatedParameters,
    key: &full::WalletKey,
    prepared: PreparedOperation,
    started: std::time::Instant,
) -> Result<CompletedOperation> {
    let prepare_seconds = started.elapsed().as_secs_f64();
    let capsule = prepared.capsule().clone();
    let bytes = capsule.encode()?;
    let resume_started = std::time::Instant::now();
    let resumed = engine.resume(&capsule, parameters)?;
    if resumed.capsule().encode()? != bytes {
        return Err(Error::AuthorityMismatch(
            "adversarial exact capsule recovery",
        ));
    }
    let resume_seconds = resume_started.elapsed().as_secs_f64();
    let signature = bootstrap::sign(
        super::super::field(&prepared.signing_message())?,
        &key.d,
        "native containment fixture hardware boundary",
    );
    let receipt =
        HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), signature)?;
    eprintln!("native operation: finalizing {label}");
    let finalize_started = std::time::Instant::now();
    let completed = engine.finalize(resumed, &receipt, parameters)?;
    let finalize_seconds = finalize_started.elapsed().as_secs_f64();
    let exact = completed.completion().encode()?;
    let recovery_started = std::time::Instant::now();
    let recovered = engine.verify_completion(&capsule, completed.completion(), parameters)?;
    if recovered.completion().encode()? != exact {
        return Err(Error::AuthorityMismatch(
            "adversarial exact completion recovery",
        ));
    }
    eprintln!(
        "native timing {label}: prepare={prepare_seconds:.3}s resume={resume_seconds:.3}s finalize={finalize_seconds:.3}s completion_recovery={:.3}s total={:.3}s",
        recovery_started.elapsed().as_secs_f64(),
        started.elapsed().as_secs_f64(),
    );
    Ok(recovered)
}

/// Reuse the independently authenticated public bundle after the full compiler
/// closure run. Every head/proof below is fresh; no saved witness or unchecked
/// sizing proof is admitted. This avoids repeating compiler warm-up generations
/// when iterating on the native containment boundary.
#[test]
#[ignore = "requires the authenticated full-backend-chain/native-device bundle"]
fn authenticated_native_containment_from_bundle() {
    run_from_bundle(true);
}

#[test]
#[ignore = "candidate timing only; the selected layout must pass authenticated_native_containment_from_bundle"]
fn measure_authenticated_native_chain_from_bundle() {
    run_from_bundle(false);
}

fn run_from_bundle(complete_validation: bool) {
    fn run(complete_validation: bool) -> Result<()> {
        let path = std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("output/full-backend-chain/native-device")
            });
        let read =
            |name: &str| std::fs::read(path.join(name)).map_err(|e| Error::Rejected(e.to_string()));
        let signature: [u8; 64] = read("parameter-signature.bin")?
            .try_into()
            .map_err(|_| Error::Malformed("test parameter signature"))?;
        let manifest = read("parameter-manifest.norito")?;
        let parameters = super::super::device_fixture::install(
            &read("program-bundle.norito")?,
            &manifest,
            signature,
        )?;
        reject_wrong_program_digests(&parameters, &manifest)?;
        let chain = crate::native::device_fixture::chain(&parameters.inner.generators.p);
        let engine = NativeEngine::default();
        let bootstrap =
            |context: &bootstrap::Context, input: &bootstrap::Inputs| BootstrapWitness {
                scheme: context.root.scheme,
                root: context.root.root.clone(),
                relation: context.relation,
                input: input.clone(),
            };
        let sender_bootstrap = bootstrap(&chain.sender_context, &chain.sender_bootstrap);
        let receiver_bootstrap = bootstrap(&chain.receiver_context, &chain.receiver_bootstrap);
        // This host unit fixture supplies explicit test enrollment markers and
        // software signatures. Actual platform custody is a separate phone test.
        let origin = engine.enrollment_origin(&sender_bootstrap, [0x44; 32], &parameters)?;
        let sender_witness = ExactOperationWitness::Bootstrap(Box::new(sender_bootstrap));
        let sender = finish(
            "fresh sender Bootstrap",
            &engine,
            &parameters,
            &chain.sender_key,
            &origin,
            &sender_witness,
        )?;
        let origin = engine.enrollment_origin(&receiver_bootstrap, [0x44; 32], &parameters)?;
        let receiver = finish(
            "fresh receiver Bootstrap",
            &engine,
            &parameters,
            &chain.receiver_key,
            &origin,
            &ExactOperationWitness::Bootstrap(Box::new(receiver_bootstrap)),
        )?;
        let load = super::super::device_fixture::load100(&sender_witness)?;
        let loaded = finish(
            "fresh sender Load100",
            &engine,
            &parameters,
            &chain.sender_key,
            sender.head(),
            &load,
        )?;
        let mut send = chain.send.clone();
        send.upper = send.lower;
        send.statement[25] = Fp::from(send.lower);
        send.receipt = crate::chain_fixture::own_receipt(
            &chain.sender_key,
            &send.credential,
            &send.statement,
            Fp::ZERO,
        );
        let sent = finish(
            "fresh sender Send10",
            &engine,
            &parameters,
            &chain.sender_key,
            loaded.head(),
            &ExactOperationWitness::Send(Box::new(send.clone())),
        )?;
        let bytes = sent.completion().output_part(1)?;
        eprintln!("fresh Native complete Payment: {} B", bytes.len());
        if bytes.len() > payment_wire::QR_BUDGET {
            return Err(Error::Rejected(format!(
                "complete Payment {} exceeds {} bytes",
                bytes.len(),
                payment_wire::QR_BUDGET
            )));
        }
        let payment = engine.verify_payment(&bytes, &parameters)?;
        std::fs::write(path.join("native-compact-companion-payment.ipm1"), &bytes)
            .map_err(|e| Error::Rejected(e.to_string()))?;
        send.receipt = payment.receipt().clone();
        receive_and_successors(
            &engine,
            &parameters,
            &chain,
            receiver.head(),
            loaded.head(),
            &send,
            &bytes,
            complete_validation,
        )
    }
    run(complete_validation).unwrap();
}

/// Each case starts at the same genuinely proved, funded receiver state. The
/// scenarios are independent test histories, never competing production spends.
pub(super) fn burn_and_successors(
    engine: &NativeEngine,
    parameters: &AuthenticatedParameters,
    chain: &crate::chain_fixture::Chain,
    receiver_boot: &VerifiedWalletHead,
    sender_loaded: &VerifiedWalletHead,
    actual_send: &crate::send::Witness,
    payment: &[u8],
) -> Result<()> {
    receive_and_successors(
        engine,
        parameters,
        chain,
        receiver_boot,
        sender_loaded,
        actual_send,
        payment,
        true,
    )
}

fn receive_and_successors(
    engine: &NativeEngine,
    parameters: &AuthenticatedParameters,
    chain: &crate::chain_fixture::Chain,
    receiver_boot: &VerifiedWalletHead,
    sender_loaded: &VerifiedWalletHead,
    actual_send: &crate::send::Witness,
    payment: &[u8],
    complete_validation: bool,
) -> Result<()> {
    let origin = ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
        scheme: chain.receiver_context.root.scheme,
        root: chain.receiver_context.root.root.clone(),
        relation: chain.receiver_context.relation,
        input: chain.receiver_bootstrap.clone(),
    }));
    let loaded_witness = super::super::device_fixture::load100(&origin)?;
    let loaded = finish(
        "receiver Load100",
        engine,
        parameters,
        &chain.receiver_key,
        receiver_boot,
        &loaded_witness,
    )?;
    let ExactOperationWitness::Load { step: load, .. } = &loaded_witness else {
        unreachable!()
    };
    let mut receiver_input = chain.receiver_bootstrap.clone();
    receiver_input.core = load.after_core;
    receiver_input.rest = load.after_rest;
    receiver_input.statement = load.statement;
    let trusted = parameters.inner.profile(full::SEND)?;
    let profile = payment_wire::Profile {
        pair: &trusted,
        scheme: parameters.inner.scheme,
        relation: parameters.inner.relation,
    };
    let decoded = payment_wire::decode(&profile, payment).map_err(Error::Rejected)?;
    let valid = crate::receive_fixture::from_send(
        &receiver_input,
        &chain.receiver_key,
        actual_send,
        &decoded.pair_bytes,
        *loaded.head().profile(),
    )
    .map_err(Error::Malformed)?;
    let forced = crate::receive_fixture::from_send_with_validity(
        &receiver_input,
        &chain.receiver_key,
        actual_send,
        &decoded.pair_bytes,
        *loaded.head().profile(),
        false,
    )
    .map_err(Error::Malformed)?;
    if engine
        .prepare(
            &ExactOperationWitness::Receive {
                step: Box::new(forced),
                incoming_payment: payment.into(),
            },
            loaded.head(),
            parameters,
        )
        .is_ok()
    {
        return Err(Error::AuthorityMismatch(
            "valid Payment cannot be forced to Burn",
        ));
    }
    let accepted = finish(
        "valid Receive control",
        engine,
        parameters,
        &chain.receiver_key,
        loaded.head(),
        &ExactOperationWitness::Receive {
            step: Box::new(valid.clone()),
            incoming_payment: payment.into(),
        },
    )?;
    if accepted.head().profile()[35] != Fp::ZERO || valid.after_core[9] != Fp::from(110) {
        return Err(Error::AuthorityMismatch(
            "valid Payment must credit without Burn",
        ));
    }
    if !complete_validation {
        eprintln!(
            "native candidate measurement: fresh Bootstrap/Load/Send/Receive passed; full containment gate not run"
        );
        return Ok(());
    }

    let p = &parameters.inner.programs[full::SEND];
    let ranges = carrier::frame_projected_pair(&trusted, &decoded.pair_bytes)
        .map_err(Error::Rejected)?
        .ranges;
    // The bool identifies a valid Send proof, not the receiver's full verdict:
    // Send intentionally does not verify the receiver's Request signature.
    let mut cases: Vec<(&str, Vec<u8>, bool)> = Vec::new();
    macro_rules! changed_proof {
        ($label:literal, $proof:expr, $range:expr, $change:expr) => {{
            let mut proof = $proof.clone();
            $change(&mut proof);
            let mut raw = decoded.pair_bytes.clone();
            raw[$range.clone()].copy_from_slice(&proof.to_bytes());
            cases.push((
                $label,
                payment_wire::test_replace_pair(&profile, payment, raw).map_err(Error::Rejected)?,
                false,
            ));
        }};
    }
    let pm = p
        .primary
        .late_slots()
        .iter()
        .position(|&i| i == full::LATE_IO + parameters.inner.generators.defer_p.n())
        .ok_or(Error::Malformed("primary matrix value slot"))?;
    let sm = p
        .secondary
        .late_slots()
        .iter()
        .position(|&i| i == full::LATE_IO + parameters.inner.generators.defer_s.n())
        .ok_or(Error::Malformed("secondary matrix value slot"))?;
    let pg = p
        .primary
        .late_slots()
        .iter()
        .position(|&i| i == crate::generator_claim::IO_START + 4)
        .ok_or(Error::Malformed(
            "primary inherited generator challenge slot",
        ))?;
    let sg = p
        .secondary
        .late_slots()
        .iter()
        .position(|&i| i == crate::generator_claim::IO_START + 4)
        .ok_or(Error::Malformed(
            "secondary inherited generator challenge slot",
        ))?;
    changed_proof!(
        "P matrix",
        decoded.pair.pp,
        ranges.primary,
        |p: &mut spartan::Proof<Eq>| p.late[pm] += Fp::ONE
    );
    changed_proof!(
        "S matrix",
        decoded.pair.ps,
        ranges.secondary,
        |p: &mut spartan::Proof<Ep>| p.late[sm] += Fq::ONE
    );
    changed_proof!(
        "P own generator",
        decoded.pair.pp,
        ranges.primary,
        |p: &mut spartan::Proof<Eq>| p.ipa.g_final =
            (Eq::from(p.ipa.g_final) + Eq::generator()).to_affine()
    );
    changed_proof!(
        "S own generator",
        decoded.pair.ps,
        ranges.secondary,
        |p: &mut spartan::Proof<Ep>| p.ipa.g_final =
            (Ep::from(p.ipa.g_final) + Ep::generator()).to_affine()
    );
    changed_proof!(
        "P inherited generator",
        decoded.pair.pp,
        ranges.primary,
        |p: &mut spartan::Proof<Eq>| p.late[pg] += Fp::ONE
    );
    changed_proof!(
        "S inherited generator",
        decoded.pair.ps,
        ranges.secondary,
        |p: &mut spartan::Proof<Ep>| p.late[sg] += Fq::ONE
    );
    changed_proof!(
        "P group equation",
        decoded.pair.pp,
        ranges.primary,
        |p: &mut spartan::Proof<Eq>| p.ipa.a = (Eq::from(p.ipa.a) + Eq::generator()).to_affine()
    );
    changed_proof!(
        "S group equation",
        decoded.pair.ps,
        ranges.secondary,
        |p: &mut spartan::Proof<Ep>| p.ipa.a = (Ep::from(p.ipa.a) + Ep::generator()).to_affine()
    );
    changed_proof!(
        "P scalar equation",
        decoded.pair.pp,
        ranges.primary,
        |p: &mut spartan::Proof<Eq>| p.ipa.z1 += Fp::ONE
    );
    changed_proof!(
        "S scalar equation",
        decoded.pair.ps,
        ranges.secondary,
        |p: &mut spartan::Proof<Ep>| p.ipa.z1 += Fq::ONE
    );
    for (label, range) in [
        ("P malformed word", ranges.primary),
        ("S malformed word", ranges.secondary),
        ("malformed link R", 42..74),
    ] {
        let mut raw = decoded.pair_bytes.clone();
        raw[range.start..range.start + 32].fill(255);
        cases.push((
            label,
            payment_wire::test_replace_pair(&profile, payment, raw).map_err(Error::Rejected)?,
            false,
        ));
    }
    // The signed Request is also bound by the Send statement/receipt/proof.
    // Reframing admits this malformed Request-bearing candidate but invalidates
    // those bindings as well. Isolated signature verdicts have component tests;
    // this case establishes full native containment of its exact package.
    let mut originals = payment_wire::decode(&profile, payment)
        .map_err(Error::Rejected)?
        .originals;
    originals.request.signature.fill(0);
    originals.statement[23] = crate::request::commitments(&originals.request)
        .map_err(Error::Malformed)?
        .digest;
    originals.receipt.body[210..242]
        .copy_from_slice(&crate::statement::digest(&originals.statement).to_repr());
    cases.push((
        "Request signature with corrupted bindings",
        payment_wire::test_reframe_originals(&profile, &originals, decoded.pair_bytes.clone())
            .map_err(Error::Rejected)?,
        false,
    ));

    // A payer with an otherwise complete private witness must refuse a Request
    // whose receiver never authenticated the terms, before producing a review.
    let mut unsigned = actual_send.clone();
    unsigned.request.signature.fill(0);
    if !matches!(
        engine.prepare(
            &ExactOperationWitness::Send(Box::new(unsigned)),
            sender_loaded,
            parameters
        ),
        Err(Error::AuthorityMismatch("Send Request receiver signature"))
    ) {
        return Err(Error::AuthorityMismatch("unauthenticated outgoing Request"));
    }
    eprintln!("native Send refused a Request with an invalid receiver signature before review");

    for (label, bytes, valid_send_proof) in cases {
        let candidate = engine.incoming_candidate(&bytes, parameters)?;
        if candidate.valid != valid_send_proof
            || engine.verify_payment(&bytes, parameters).is_ok() != valid_send_proof
        {
            return Err(Error::AuthorityMismatch(
                "corrupted Payment proof verdict differs from its test case",
            ));
        }
        let mut send = actual_send.clone();
        send.request = candidate.originals.request.clone();
        send.statement = candidate.originals.statement;
        send.receipt = candidate.originals.receipt.clone();
        let witness = crate::receive_fixture::from_send_with_validity(
            &receiver_input,
            &chain.receiver_key,
            &send,
            &candidate.pair_bytes,
            *loaded.head().profile(),
            false,
        )
        .map_err(Error::Malformed)?;
        let invalid = ExactOperationWitness::Receive {
            step: Box::new(witness.clone()),
            incoming_payment: bytes.into(),
        };
        let started = std::time::Instant::now();
        if !matches!(engine.prepare(&invalid, loaded.head(), parameters), Err(Error::Rejected(_))) {
            return Err(Error::AuthorityMismatch("fresh Receive admission must refuse Burn"));
        }
        // Only a historical selection reaches defensive Burn recovery. The
        // wallet must also refuse to select an unselected restored capsule.
        let historical = engine.prepare_retained(&invalid, loaded.head(), parameters)?;
        if !matches!(engine.prepare_selection(historical.capsule(), parameters), Err(Error::Rejected(_))) {
            return Err(Error::AuthorityMismatch("restored Receive admission must refuse Burn"));
        }
        let burned = finish_prepared(
            label,
            engine,
            parameters,
            &chain.receiver_key,
            historical,
            started,
        )?;
        if burned.head().profile()[35] != Fp::from(10)
            || witness.after_core[9] - burned.head().profile()[35] != Fp::from(100)
            || witness.after_core[18..21] != load.after_core[18..21]
            || witness.after_rest != load.after_rest
        {
            return Err(Error::AuthorityMismatch(
                "Burn preserves unrelated funded value",
            ));
        }
        let successor = post_burn_send(
            &witness,
            burned.head().profile(),
            &chain.receiver_key,
            &chain.send.credential,
            &chain.sender_key,
        )?;
        let ExactOperationWitness::Send(step) = &successor else {
            unreachable!()
        };
        let after = step.after_core;
        let next = finish(
            &format!("{label} -> Send10"),
            engine,
            parameters,
            &chain.receiver_key,
            burned.head(),
            &successor,
        )?;
        let exported = next.completion().output_part(1)?;
        if exported.len() > payment_wire::QR_BUDGET {
            return Err(Error::Rejected(format!(
                "post-Burn complete Payment {} exceeds {} bytes",
                exported.len(),
                payment_wire::QR_BUDGET
            )));
        }
        let checked = engine.verify_payment(&exported, parameters)?;
        if after[9] - next.head().profile()[35] != Fp::from(90)
            || next.head().profile()[37] != burned.head().profile()[37]
            || checked.profile() != next.head().profile()
        {
            return Err(Error::AuthorityMismatch(
                "post-Burn Send preserves credit history and balance",
            ));
        }
        eprintln!(
            "native containment: {label} burned10, preserved100, successor spent10, remaining90"
        );
    }
    Ok(())
}

/// The Send10 that follows a Burn. The fixture builds it from the exact
/// post-Burn state under the Burn Receive's head, so the head's adjusted burned
/// total reaches the statement and the successor Core through the builder. It
/// pays another wallet, whose key signs the Request: the completed-Request
/// relation refuses payer == receiver.
fn post_burn_send(
    received: &crate::receive::Witness,
    head: &[Fp; 64],
    key: &full::WalletKey,
    payee: &crate::credential::Credential,
    payee_key: &full::WalletKey,
) -> Result<ExactOperationWitness> {
    use super::super::device_fixture::{SendOrigin, request_from, send10_from};
    let origin = SendOrigin::after_received(received, head)?;
    let request = CompletedRequest {
        body: request_from(&origin, payee).try_into().unwrap(),
        signature: [0; 64],
    };
    let signature = bootstrap::sign(
        crate::request::commitments(&request)
            .map_err(Error::Malformed)?
            .message,
        &payee_key.d,
        "post-Burn payee Request fixture",
    );
    let mut successor = send10_from(&origin, payee, signature)?;
    let ExactOperationWitness::Send(step) = &mut successor else {
        unreachable!()
    };
    // Software stand-in for the payer's hardware boundary. It signs the body
    // of the statement the fixture built and changes no monetary field.
    step.receipt =
        crate::chain_fixture::own_receipt(key, &step.credential, &step.statement, Fp::ZERO);
    Ok(successor)
}

/// Every violated pre-instantiation row as (array, name, row, instance), so a
/// local witness mistake is named before the expensive full circuit check.
fn violated_rows(w: &full::Wiring<Fp>) -> Vec<(usize, &'static str, usize, usize)> {
    let mut violated = Vec::new();
    for (a, array) in w.arrays.iter().enumerate() {
        let evaluate = |lin: &crate::uniform::Lin<Fp>, i: usize| {
            let (p, r) = (i >> array.rho, i & ((1 << array.rho) - 1));
            lin.iter()
                .map(|term| {
                    use crate::uniform::Src;
                    let value = match &term.src {
                        Src::One => Fp::ONE,
                        Src::Own(cell) => array.values[*cell][i],
                        Src::Link {
                            array: target,
                            cell,
                            map,
                        } => match map[r] {
                            Some(rr) => {
                                let target = &w.arrays[*target];
                                let pp = if w.pi[target.family] == 0 { 0 } else { p };
                                target.values[*cell][(pp << target.rho) | rr]
                            }
                            None => Fp::ZERO,
                        },
                        Src::All { .. } => panic!("no pre-instantiation sum rows"),
                    };
                    term.coef[r] * value
                })
                .sum::<Fp>()
        };
        for (row, relation) in array.rows.iter().enumerate() {
            for p in 0..w.counts[array.family] {
                for r in 0..array.r_count {
                    let i = (p << array.rho) | r;
                    if evaluate(&relation[0], i) * evaluate(&relation[1], i)
                        != evaluate(&relation[2], i)
                    {
                        violated.push((a, array.name, row, i));
                    }
                }
            }
        }
    }
    violated
}

/// Relation-only regression; these explicit fixture ports never confer native
/// predecessor authority. The ignored full containment test supplies real proofs.
#[test]
fn post_burn_successor_preparation_preserves_adjusted_lineage() {
    let gens = Gens::<Eq>::new(1 << 16);
    let chain = crate::chain_fixture::new(&gens);
    let origin = ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
        scheme: chain.receiver_context.root.scheme,
        root: chain.receiver_context.root.root.clone(),
        relation: chain.receiver_context.relation,
        input: chain.receiver_bootstrap.clone(),
    }));
    let loaded = super::super::device_fixture::load100(&origin).unwrap();
    let ExactOperationWitness::Load { step, .. } = &loaded else {
        unreachable!()
    };
    let mut receiver = chain.receiver_bootstrap.clone();
    receiver.core = step.after_core;
    receiver.rest = step.after_rest;
    receiver.statement = step.statement;
    let mut profile =
        crate::bootstrap::public_profile(&chain.receiver_context, &chain.receiver_bootstrap);
    profile[full::HEAD_IO] = step.statement[15];
    let received = crate::receive_fixture::from_send_with_validity(
        &receiver,
        &chain.receiver_key,
        &chain.send,
        b"relation-only incoming pair",
        profile,
        false,
    )
    .unwrap();
    profile[full::HEAD_IO] = received.statement[15];
    profile[full::STATEMENT_IO] = crate::statement::digest(&received.statement);
    profile[full::RECEIPT_IO] = crate::receipt::digest(&received.receipt);
    profile[35] = Fp::from(10);
    let (_, credit_root) = crate::indexed_map::empty_insert::<32>(
        received.maps.credit_insert.key,
        received.maps.credit_insert.value,
    );
    profile[37] = credit_root;
    // The relation always reads the actual verified predecessor `profile`.
    // `fixture_head` is the head the successor's builder is given.
    let prepare = |fixture_head: &[Fp; 64]| {
        let successor = post_burn_send(
            &received,
            fixture_head,
            &chain.receiver_key,
            &chain.send.credential,
            &chain.sender_key,
        )
        .unwrap();
        let ExactOperationWitness::Send(step) = &successor else {
            unreachable!()
        };
        let mut w = full::Wiring::default();
        let family = w.family(0, 1);
        let predecessor = full::VerifiedPredecessor::fixture(&mut w, family, profile);
        let prepared =
            crate::send::append_preparation(&mut w, family, step, &predecessor).map(|_| ());
        ((**step).clone(), prepared, w)
    };
    let (step, prepared, w) = prepare(&profile);
    prepared.unwrap();
    let violated = violated_rows(&w);
    assert!(
        violated.is_empty(),
        "violated rows (array, name, row, instance): {violated:?}"
    );
    check_constraints(w, 18, 3, None).unwrap();
    // The Burn credited 10 into the Core balance and the lineage alone records
    // it as burned: the Core's own burn field is still the stale zero.
    assert_eq!(step.before_core[9], Fp::from(110));
    assert_eq!(step.before_core[10], Fp::ZERO);
    assert_eq!(profile[35], Fp::from(10));
    // The successor carries the adjusted burn forward and spends around it.
    assert_eq!(step.statement[12], profile[35]);
    assert_eq!(step.after_core[9] - profile[35], Fp::from(90));
    assert_eq!(step.after_core[10], profile[35]);
    // It pays the other wallet, and that wallet is what its send chain names.
    assert_eq!(step.statement[18..20], chain.sender_bootstrap.core[5..7]);
    assert_ne!(step.statement[18..20], step.before_core[5..7]);
    // A successor built as if nothing had been burned is refused under the
    // same verified predecessor.
    let mut unburned = profile;
    unburned[35] = Fp::ZERO;
    let (dropped, prepared, w) = prepare(&unburned);
    assert_eq!(dropped.after_core[10], Fp::ZERO);
    assert!(
        prepared.is_err()
            || violated_rows(&w)
                .iter()
                .any(|v| v.1 == "Send checked arithmetic and state preservation"),
        "a successor that drops the adjusted burn must violate the Send relation"
    );
}

#[test]
fn native_private_witness_rebuilds_the_authenticated_sumcheck_group_on_both_curves() {
    use crate::native::program_bundle::tests::{fixture_wiring, original_at};
    fn check<F: Scalar + zeroize::Zeroize, const GROUP: usize>(
        primary: bool,
        width: usize,
        coordinates: usize,
    ) {
        let (encoded, pin) = original_at::<F, GROUP>(primary, 1, coordinates, width, primary, None);
        let circuit = match GROUP {
            3 => UniformCircuit::<F>::import_program(&encoded, primary),
            4 => UniformCircuit::<F>::import_four_variable_program(&encoded, primary),
            _ => unreachable!(),
        }
        .unwrap();
        let wiring = fixture_wiring::<F>(primary, 1, coordinates, width, primary);
        let entropy = ProofEntropy::fresh().unwrap();
        let (derived, _, _, _) = build(wiring, &circuit, &entropy, b"selected group test").unwrap();
        assert_eq!(derived.shape.k, GROUP);
        assert_eq!(derived.program_digest(), pin);
        same_program(&derived, &circuit).unwrap();
    }
    check::<Fp, 3>(true, 13, 55);
    check::<Fq, 3>(false, 10, 54);
    check::<Fp, 4>(true, 13, 55);
    check::<Fq, 4>(false, 13, 54);
}
