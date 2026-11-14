//! Complete native containment checks, invoked only after exact program closure.
//! Software fixture signatures exercise the sealed Engine, not device custody.
use super::*;
use group::{Curve, Group};

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
            let signature = bootstrap::sign(
                crate::payment::byte_digest(u64::from_le_bytes(*b"kgwparm1"), &bytes),
                &BigUint::from_bytes_be(&[0x11; 32]),
                "native wrong-program manifest fixture",
            );
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
        let chain =
            crate::chain_fixture::with_relation(&parameters.inner.generators.p, *parameters.id());
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
        let origin = engine.enrollment_origin(&sender_bootstrap, [0x31; 32], &parameters)?;
        let sender_witness = ExactOperationWitness::Bootstrap(Box::new(sender_bootstrap));
        let sender = finish(
            "fresh sender Bootstrap",
            &engine,
            &parameters,
            &chain.sender_key,
            &origin,
            &sender_witness,
        )?;
        let origin = engine.enrollment_origin(&receiver_bootstrap, [0x32; 32], &parameters)?;
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

    // Isolate the receiver signature predicate from proof verification: produce
    // a genuinely valid Send pair whose exact Request has an invalid signature.
    // All updated statement/map/receipt bindings are proven by the real engine.
    let mut bad_request_send = actual_send.clone();
    bad_request_send.request.signature.fill(0);
    let request =
        crate::request::commitments(&bad_request_send.request).map_err(Error::Malformed)?;
    bad_request_send.statement[23] = request.digest;
    let pending_value = iroha_pasta::poseidon::hash_with_domain(
        crate::send_maps::PENDING_DOMAIN,
        &bad_request_send.statement[17..24],
    );
    let (pending, pending_root) = crate::indexed_map::empty_insert(request.credit, pending_value);
    bad_request_send.maps.pending = pending;
    bad_request_send.after_core[18] = pending_root;
    bad_request_send.after_core[15] = iroha_pasta::poseidon::hash_with_domain(
        crate::send_arithmetic::CHAIN_DOMAIN,
        &[
            bad_request_send.before_core[15],
            request.credit,
            bad_request_send.statement[18],
            bad_request_send.statement[19],
            bad_request_send.before_core[12],
            bad_request_send.statement[21],
            bad_request_send.statement[22],
            request.digest,
        ],
    );
    bad_request_send.statement[15] = crate::monetary_state::commitment(
        &bad_request_send.after_core,
        &bad_request_send.after_rest,
    );
    bad_request_send.receipt = crate::chain_fixture::own_receipt(
        &chain.sender_key,
        &bad_request_send.credential,
        &bad_request_send.statement,
        Fp::ZERO,
    );
    let signed_invalid_request = finish(
        "valid Send proof containing invalid receiver Request signature",
        engine,
        parameters,
        &chain.sender_key,
        sender_loaded,
        &ExactOperationWitness::Send(Box::new(bad_request_send)),
    )?;
    let bytes = signed_invalid_request.completion().output_part(1)?;
    engine.verify_payment(&bytes, parameters)?;
    cases.push((
        "isolated Request signature with valid Send proof",
        bytes,
        true,
    ));

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
        let burned = finish(
            label,
            engine,
            parameters,
            &chain.receiver_key,
            loaded.head(),
            &ExactOperationWitness::Receive {
                step: Box::new(witness.clone()),
                incoming_payment: bytes.into(),
            },
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
            &loaded_witness,
            &witness,
            burned.head().profile(),
            &chain.receiver_key,
            &chain.send.credential,
        )?;
        let ExactOperationWitness::Send(step) = &successor else { unreachable!() };
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

fn post_burn_send(
    loaded_witness: &ExactOperationWitness,
    witness: &crate::receive::Witness,
    profile: &[Fp; 64],
    key: &full::WalletKey,
    payee: &crate::credential::Credential,
) -> Result<ExactOperationWitness> {
    // Reuse the witness builder only; this synthetic Load-shaped input is
    // never submitted as a Load or accepted as authority. The resulting Send
    // must verify against the actual completed Receive head below. It pays
    // another wallet: the completed-Request relation refuses payer == receiver.
    let mut builder = loaded_witness.clone();
    let ExactOperationWitness::Load { step, .. } = &mut builder else {
        unreachable!()
    };
    step.after_core = witness.after_core;
    step.after_rest = witness.after_rest;
    step.statement = witness.statement;
    let request = CompletedRequest {
        body: super::super::device_fixture::request_to(&builder, payee)?
            .try_into()
            .unwrap(),
        signature: [0; 64],
    };
    let signature = bootstrap::sign(
        crate::request::commitments(&request)
            .map_err(Error::Malformed)?
            .message,
        &key.d,
        "post-Burn receiver Request fixture",
    );
    let mut successor = super::super::device_fixture::send10_to(&builder, payee, signature)?;
    let ExactOperationWitness::Send(step) = &mut successor else {
        unreachable!()
    };
    step.after_core[10] = profile[35];
    step.statement[12] = profile[35];
    step.statement[15] = crate::monetary_state::commitment(&step.after_core, &step.after_rest);
    step.receipt = crate::chain_fixture::own_receipt(
        key,
        &step.credential,
        &step.statement,
        Fp::ZERO,
    );
    Ok(successor)
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
    let ExactOperationWitness::Load { step, .. } = &loaded else { unreachable!() };
    let mut receiver = chain.receiver_bootstrap.clone();
    receiver.core = step.after_core;
    receiver.rest = step.after_rest;
    receiver.statement = step.statement;
    let mut profile = crate::bootstrap::public_profile(&chain.receiver_context, &chain.receiver_bootstrap);
    profile[full::HEAD_IO] = step.statement[15];
    let received = crate::receive_fixture::from_send_with_validity(
        &receiver, &chain.receiver_key, &chain.send, b"relation-only incoming pair", profile, false,
    ).unwrap();
    profile[full::HEAD_IO] = received.statement[15];
    profile[full::STATEMENT_IO] = crate::statement::digest(&received.statement);
    profile[full::RECEIPT_IO] = crate::receipt::digest(&received.receipt);
    profile[35] = Fp::from(10);
    let (_, credit_root) = crate::indexed_map::empty_insert::<32>(received.maps.credit_insert.key, received.maps.credit_insert.value);
    profile[37] = credit_root;
    let successor =
        post_burn_send(&loaded, &received, &profile, &chain.receiver_key, &chain.send.credential).unwrap();
    let ExactOperationWitness::Send(step) = &successor else { unreachable!() };
    let mut w = full::Wiring::default();
    let family = w.family(0, 1);
    let predecessor = full::VerifiedPredecessor::fixture(&mut w, family, profile);
    crate::send::append_preparation(&mut w, family, step, &predecessor).unwrap();
    // Identify local witness mistakes before the expensive full circuit check.
    for (a, array) in w.arrays.iter().enumerate() {
        let evaluate = |lin: &crate::uniform::Lin<Fp>, i: usize| {
            let (p, r) = (i >> array.rho, i & ((1 << array.rho) - 1));
            lin.iter().map(|term| {
                use crate::uniform::Src;
                let value = match &term.src {
                    Src::One => Fp::ONE,
                    Src::Own(cell) => array.values[*cell][i],
                    Src::Link { array: target, cell, map } => match map[r] {
                        Some(rr) => { let target = &w.arrays[*target]; let pp = if w.pi[target.family] == 0 { 0 } else { p }; target.values[*cell][(pp << target.rho) | rr] },
                        None => Fp::ZERO,
                    },
                    Src::All { .. } => panic!("no pre-instantiation sum rows"),
                };
                term.coef[r] * value
            }).sum::<Fp>()
        };
        for (row, relation) in array.rows.iter().enumerate() {
            for p in 0..w.counts[array.family] {
                for r in 0..array.r_count {
                    let i = (p << array.rho) | r;
                    assert_eq!(evaluate(&relation[0], i) * evaluate(&relation[1], i), evaluate(&relation[2], i), "array {a} {} row {row} instance {i}", array.name);
                }
            }
        }
    }
    check_constraints(w, 18, None).unwrap();
    assert_eq!(step.after_core[9] - profile[35], Fp::from(90));
    assert_eq!(step.after_core[10], profile[35]);
}
