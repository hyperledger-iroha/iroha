//! Receive terminal closure under one immutable native pinned-key Omega catalog.
//!
//! Funded Bootstrap/Load ancestry requires explicit native-BLS-authorized Load originals.
//! Bootstrap-only malformed-payment burn is unfunded.
//! Execution checks component closure; it does not
//! establish release admission, mobile qualification or a complete terminal catalog.
//! Funded cases in `native_load_tests` require a fresh `KAGEMUSHA_LOAD_OUTPUT`
//! directory and run individually in release with `--ignored --exact`.
#![allow(clippy::duplicate_mod)]
#[path = "receive_omega/native_load_tests.rs"]
mod native_load_tests;
/// Genuine staged Receive and its exact native terminal/source exports.
#[path = "a_receive_recursive.rs"]
pub mod receive_chain;

mod common;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    omega::native::{Output, terminal_fold_inputs},
    operation_relation::map_effects::CREDIT_DOMAIN,
    q_sigma::native::IncomingMode,
    tree::{INDEXED_DEPTH, INDEXED_NODE_DOMAIN, IndexedLeaf, path_root},
};
use iroha_pasta::{Ep, Eq, Fp, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{DescriptorBinding, VerifyingKey, verifier::verify_full};
use iroha_plonk_recursion::{FoldConfig, create_fold, obligation::ledger::Variant};
use receive_chain::compact_catalog::{self, Terminal};

/// Exact resulting receiver sources retained for subsequent `ArchiveStatus` tests.
/// This component artifact has not been admitted to the complete release catalog.
#[allow(dead_code)]
pub(crate) struct CompactReceiveOmega {
    pub(crate) source: receive_chain::AuthenticatedReceive,
    pub(crate) binding: DescriptorBinding,
    pub(crate) key: VerifyingKey<Ep>,
    pub(crate) output: Output,
    /// Native proof || P544 || V544, with no fabricated or re-encoded proof bytes.
    pub(crate) transport: Vec<u8>,
    /// Original public320 followed by the exact native transport.
    pub(crate) raw: Vec<u8>,
    pub(crate) credit_leaf: IndexedLeaf<Fp>,
    pub(crate) credit_slot: u32,
    pub(crate) credit_siblings: [Fp; INDEXED_DEPTH],
}

/// Build both signed source chains again after extending the immutable catalog,
/// then prove every Receive stage and close its exact four-slot native outer fold.
pub(crate) fn compact_receive(
    variant: Variant,
    corrected: bool,
    fixture: compact_catalog::LoadFixture,
) -> CompactReceiveOmega {
    let (seed, initial_program, initial_wallets) =
        compact_catalog::compact_shared_wallet_seed(fixture);
    let (expected_binding, expected_key) =
        receive_chain::receive_terminal_identity(&initial_wallets, variant);
    let (program, wallets) = seed.extend(
        &initial_program,
        &initial_wallets,
        &expected_binding,
        vec![expected_key.clone()],
    );
    drop(initial_program);
    drop(initial_wallets);
    drop(seed);
    let (wallets, correction) = if corrected {
        let (wallets, correction) = program.with_nondeciding_payer(wallets);
        (wallets, Some(correction))
    } else {
        (wallets, None)
    };
    let source = receive_chain::authenticated_receive_from_wallets(wallets, variant, correction);
    assert_eq!(source.binding, expected_binding);
    assert_eq!(source.key.to_bytes(), expected_key.to_bytes());
    assert_eq!(program.catalog().len(), 3);
    wrap_receive(&program, source)
}

/// Close an already authenticated Receive under the supplied immutable program.
/// The caller can extend the common catalog with Send/Archive without duplicating
/// the four-slot selector, native outer producer or exact retained credit path.
pub(crate) fn wrap_receive(
    program: &compact_catalog::Program,
    source: receive_chain::AuthenticatedReceive,
) -> CompactReceiveOmega {
    assert_eq!(source.state.lineage[17], program.digest());
    let terminal = &source.terminal;
    let corrected = terminal.incoming_mode == IncomingMode::Corrected;
    let frame: [Fp; 69] = terminal.instances.as_slice().try_into().unwrap();
    let slots = terminal_fold_inputs(&frame, terminal.opening.clone()).unwrap();
    assert_eq!(slots[0], terminal.vesta_part.as_input());
    assert_eq!(slots[1], terminal.opening);
    assert_eq!(slots[2], terminal.predecessor_vesta.as_input());
    let params = iroha_plonk::pcs::ipa::PinnedParams::derive(16).unwrap();
    for slot in &slots {
        slot.decide(&params, MemoryBudget::DEFAULT).unwrap();
    }
    match terminal.incoming_mode {
        IncomingMode::Corrected => {
            assert!(
                terminal
                    .incoming_vesta
                    .decide(&params, MemoryBudget::DEFAULT)
                    .is_err()
            );
            assert_eq!(slots[3].g(), &terminal.incoming_correction);
            assert_ne!(slots[3].g(), terminal.incoming_vesta.g());
            assert_eq!(slots[3].challenges(), terminal.incoming_vesta.challenges());
        }
        IncomingMode::Accept => {
            assert_eq!(slots[3], terminal.incoming_vesta.as_input());
        }
        IncomingMode::Trivial => {
            assert!(!source.accepted_credit);
            let trivial =
                iroha_plonk_recursion::AccumulatorT::trivial(&params, MemoryBudget::DEFAULT)
                    .unwrap();
            assert_eq!(slots[3], trivial.as_input());
        }
    }
    // Retain the same successfully parsed source owner for an equation mutation.
    // The four-slot context, D/L/R and tape remain unchanged; only terminal c/G
    // are replaced to satisfy the equation with the fourth term omitted.
    let plan = iroha_plonk_recursion::terminal::TerminalPlan::new(
        source.binding.encoded(),
        &[source.key.to_bytes().to_vec()],
        params.clone(),
    )
    .unwrap();
    let frame = plan
        .read(
            0,
            std::slice::from_ref(&terminal.instances),
            &terminal.proof,
            &FoldConfig::default(),
        )
        .unwrap();
    let (fold, expected) = frame
        .create(common::public_fixture_tape(917), &FoldConfig::default())
        .unwrap();
    expected.decide(&params, MemoryBudget::DEFAULT).unwrap();
    let retained_frame = plan
        .read(
            0,
            std::slice::from_ref(&terminal.instances),
            &terminal.proof,
            &FoldConfig::default(),
        )
        .unwrap();
    assert_eq!(
        retained_frame
            .verify(&fold, &FoldConfig::default())
            .unwrap(),
        expected
    );
    let omitted = omit_terminal_fourth_term(&params, &slots, &fold, &expected);
    assert!(matches!(
        retained_frame.verify(&omitted, &FoldConfig::default()),
        Err(iroha_plonk_recursion::terminal::Error::Fold(
            iroha_plonk_recursion::Error::FoldEquation
        ))
    ));
    assert_eq!(
        retained_frame
            .verify(&fold, &FoldConfig::default())
            .unwrap(),
        expected,
        "failed omission must preserve exact original replay"
    );
    for index in [42, 46, 62, 63, 64, 65] {
        let mut changed = terminal.instances.clone();
        changed[index] += Fp::ONE;
        assert!(
            verify_full(
                &params,
                &source.binding,
                &source.key,
                &[changed],
                &terminal.proof,
                MemoryBudget::DEFAULT,
            )
            .is_err()
        );
    }
    let outer = program.prove(
        Terminal {
            key: &source.key,
            binding: &source.binding,
            proof: &terminal.proof,
            instances: &terminal.instances,
            public: &source.state.lineage,
            pallas: &terminal.pallas,
            opening: &terminal.opening,
        },
        match terminal.incoming_mode {
            IncomingMode::Corrected => "Receive-corrected-burn",
            IncomingMode::Trivial => "Receive-trivial-burn",
            IncomingMode::Accept => "Receive-accept",
        },
    );
    let output = Output {
        proof: outer.proof,
        instances: outer.public,
        pallas: terminal.pallas.clone(),
        vesta: outer.vesta,
        opening: outer.opening,
    };
    let transport = output.transport();
    assert_eq!(transport.len(), 5120);
    compact_catalog::assert_omega_peer_budget(&transport);
    let mut raw = receive_chain::public_bytes(&source.state.lineage);
    raw.extend_from_slice(&transport);
    assert_eq!(raw.len(), 5440);
    let credit_leaf = IndexedLeaf {
        key: source.statement[17],
        value: hash_with_domain(
            CREDIT_DOMAIN,
            &[
                source.statement[17],
                source.payment_digest,
                Fp::from(u64::from(!source.accepted_credit)),
            ],
        ),
        next_key: source.credit.leaf.next_key,
    };
    let credit_slot = source.credit.slot;
    let credit_siblings = source.credit.slot_siblings;
    assert_eq!(
        path_root(
            INDEXED_NODE_DOMAIN,
            credit_leaf.hash(),
            u64::from(credit_slot),
            &credit_siblings,
        ),
        source.state.lineage[16]
    );
    eprintln!(
        "RECEIVE_OMEGA_CLOSURE corrected={corrected} accepted_credit={} incoming_mode={:?} actual_proof=4032 transport=5120 four_inputs_decide=true fourth_slot_equation_omission_rejected=true immutable_native_catalog=true exact_terminal_VK=true credit_membership=true full_catalog=false release_qualified=false",
        source.accepted_credit, terminal.incoming_mode
    );
    CompactReceiveOmega {
        source,
        binding: program.binding().clone(),
        key: program.verifying_key().clone(),
        output,
        transport,
        raw,
        credit_leaf,
        credit_slot,
        credit_siblings,
    }
}

// Test-only transcript observations; none grants a frame, catalog or proof verdict.
#[derive(Debug)]
struct TerminalFoldTrace {
    prelude: [Fp; 3],
    rounds: [Fp; iroha_plonk_recursion::K],
    round_terms: Eq,
    gamma: Fp,
    coefficient: Fp,
    generator: iroha_pasta::EqAffine,
}

fn terminal_fold_trace(
    params: &iroha_plonk::pcs::ipa::PinnedParams<Eq>,
    bytes: &[u8],
) -> TerminalFoldTrace {
    use iroha_plonk::transcript::{
        BasePoseidonHash, Transcript, TranscriptRead, TranscriptReader, pipa_r::nonzero_challenge,
    };
    use iroha_plonk_recursion::{FOLD_BODY_BYTES, FoldPurpose, FoldTape, K};
    let tape = FoldTape::<Eq>::from_private_bytes(&bytes[FOLD_BODY_BYTES..]).unwrap();
    let mut reader = TranscriptReader::<Eq, _>::new(
        BasePoseidonHash::with_domain(*b"pipa-as1"),
        &bytes[..FOLD_BODY_BYTES],
    );
    // Always four, including for the malformed equation below. No generic
    // WrapperVesta API or caller-selected raw terminal prefix is introduced.
    for word in iroha_plonk_recursion::accumulation::context(params, FoldPurpose::WrapperVesta, 4) {
        reader.common_base(&word).unwrap();
    }
    let _commitment = reader.read_point().unwrap();
    let prelude = core::array::from_fn(|_| reader.squeeze_challenge());
    let mut rounds = [Fp::ZERO; K];
    let mut round_terms = Eq::default();
    let mut gamma = Fp::ZERO;
    for (round, challenge) in rounds.iter_mut().enumerate() {
        let left = reader.read_point().unwrap();
        let right = reader.read_point().unwrap();
        *challenge = nonzero_challenge(reader.squeeze_challenge());
        let inverse = challenge.invert().unwrap();
        round_terms += Eq::from(left) * inverse + Eq::from(right) * *challenge;
        gamma += tape.masks()[2 * round] * inverse + tape.masks()[2 * round + 1] * *challenge;
    }
    let coefficient = reader.read_scalar().unwrap();
    let generator = reader.read_unabsorbed_generator().unwrap();
    reader.finish().unwrap();
    TerminalFoldTrace {
        prelude,
        rounds,
        round_terms,
        gamma,
        coefficient,
        generator,
    }
}

fn omit_terminal_fourth_term(
    params: &iroha_plonk::pcs::ipa::PinnedParams<Eq>,
    slots: &[iroha_plonk_recursion::FoldInput<Eq>; 4],
    original: &iroha_plonk_recursion::FoldWitness<Eq>,
    expected: &iroha_plonk_recursion::AccumulatorT<Eq>,
) -> iroha_plonk_recursion::FoldWitness<Eq> {
    use iroha_plonk::{pcs::ipa::fold_evaluation, transcript::encode_point};
    use iroha_plonk_recursion::{FOLD_BODY_BYTES, FoldWitness, K, PrivateWitnessBytes};
    let retained = original.to_private_bytes();
    let trace = terminal_fold_trace(params, &retained);
    assert_eq!(&trace.rounds, expected.challenges());
    assert_eq!(&trace.generator, expected.g());
    let [alpha, z, zeta] = trace.prelude;
    let g0 = Eq::from(params.params().g()[0]);
    let mut three = Eq::default();
    // Independent Horner evaluation of exactly the first three source claims.
    for slot in slots[..3].iter().rev() {
        three = three * alpha + Eq::from(*slot.g()) - g0 * fold_evaluation(z, slot.challenges());
    }
    let fourth = (Eq::from(*slots[3].g()) - g0 * fold_evaluation(z, slots[3].challenges()))
        * alpha.pow_vartime([3]);
    let fixed = trace.round_terms - Eq::from(params.params().w()) * trace.gamma;
    let auxiliary = Eq::from(params.params().u()) * (fold_evaluation(z, &trace.rounds) * zeta);
    assert_eq!(
        three + fourth + fixed
            - Eq::from(trace.generator) * trace.coefficient
            - auxiliary * trace.coefficient,
        Eq::default(),
        "independent reconstruction of the original full equation"
    );
    assert_ne!(
        fourth,
        Eq::default(),
        "the fixture must distinguish the omitted term"
    );
    let replacement = iroha_pasta::EqAffine::from(three + fixed - auxiliary);
    // This zero is only the deliberately incomplete equation, never a decide
    // verdict or an accepted terminal proof. Identity G uses its canonical codec.
    assert_eq!(
        three + fixed - Eq::from(replacement) - auxiliary,
        Eq::default()
    );
    let complete_residual = three + fourth + fixed - Eq::from(replacement) - auxiliary;
    assert_eq!(complete_residual, fourth);
    assert_ne!(
        complete_residual,
        Eq::default(),
        "the complete four-input equation must reject this same tape"
    );
    let coefficient_offset = 32 * (2 * K + 1);
    let changed = PrivateWitnessBytes::try_write::<std::io::Error>(retained.len(), |writer| {
        writer.write_all(&retained[..coefficient_offset])?;
        writer.write_all(Fp::ONE.to_repr().as_ref())?;
        writer.write_all(&encode_point::<Eq>(&replacement))?;
        writer.write_all(&retained[FOLD_BODY_BYTES..])
    })
    .unwrap();
    assert_eq!(
        &changed[..coefficient_offset],
        &retained[..coefficient_offset]
    );
    assert_eq!(&changed[FOLD_BODY_BYTES..], &retained[FOLD_BODY_BYTES..]);
    let forged = terminal_fold_trace(params, &changed);
    assert_eq!(forged.prelude, trace.prelude);
    assert_eq!(forged.rounds, trace.rounds);
    assert_eq!(forged.round_terms, trace.round_terms);
    assert_eq!(forged.gamma, trace.gamma);
    assert_eq!(forged.coefficient, Fp::ONE);
    assert_eq!(forged.generator, replacement);
    assert_eq!(&*original.to_private_bytes(), &*retained);
    FoldWitness::from_private_bytes(&changed).unwrap()
}

/// No funded source or Load trust is needed to prove the malformed-input burn.
/// Both Bootstrap and Receive are rebuilt under one final two-terminal key.
#[test]
#[ignore = "genuine immutable Bootstrap+Receive catalog, all5A4W and final native Omega; run optimized"]
fn bootstrap_only_invalid_sigma_burn_closes_all_four_outer_obligations() {
    let (seed, initial_program, initial_receiver) =
        compact_catalog::compact_bootstrap_catalog_seed();
    let (binding, key) = receive_chain::bootstrap_burn_terminal_identity(&initial_receiver);
    let (program, receiver) = seed.extend(
        &initial_program,
        &initial_receiver,
        &binding,
        vec![key.clone()],
    );
    drop(initial_program);
    drop(initial_receiver);
    drop(seed);
    assert_eq!(program.catalog().len(), 2);
    let source = receive_chain::authenticated_bootstrap_burn(receiver);
    assert_eq!(source.binding, binding);
    assert_eq!(source.key.to_bytes(), key.to_bytes());
    assert!(!source.accepted_credit);
    let result = wrap_receive(&program, source);
    assert_eq!(
        result.source.state.core[8] - result.source.state.lineage[14],
        Fp::ZERO
    );
    assert_eq!(result.source.terminal.incoming_mode, IncomingMode::Trivial);
    eprintln!(
        "BOOTSTRAP_ONLY_INVALID_SIGMA_RECEIVE_OMEGA full5A4W=true canonical_custody_all9=true original_sigma_soft_false=true no_Load_trust=true adjusted_spendable_zero=true terminal_omega=true immutable_catalog_size=2 full_catalog=false release_qualified=false"
    );
}

/// Run the retained composition assertions with genuine native Load originals.
#[allow(dead_code)] // Other component harnesses include this shared builder.
pub fn genuine_receive_acceptance_closes_all_four_outer_obligations(
    fixture: compact_catalog::LoadFixture,
) {
    let _ = compact_receive(Variant::Receive, false, fixture);
}

/// Close genuine funded ReceiveRenewed under its exact rebuilt native catalog.
#[allow(dead_code)] // Other component harnesses include this shared builder.
pub fn genuine_renewed_receive_acceptance_closes_all_four_outer_obligations(
    fixture: compact_catalog::LoadFixture,
) {
    let result = compact_receive(Variant::ReceiveRenewed, false, fixture);
    assert!(result.source.accepted_credit);
    assert_eq!(result.source.terminal.incoming_mode, IncomingMode::Accept);
}

/// Run the retained composition assertions with genuine native Load originals.
#[allow(dead_code)] // Other component harnesses include this shared builder.
pub fn genuine_receive_corrected_burn_closes_all_four_outer_obligations(
    fixture: compact_catalog::LoadFixture,
) {
    let _ = compact_receive(Variant::Receive, true, fixture);
}

/// Equal original claim bytes still occupy four prescribed obligation slots.
/// These fabricated frame cells test selection/folding only, not A admission.
#[test]
fn four_slot_selection_never_deduplicates_equal_claims() {
    let params = iroha_plonk::pcs::ipa::PinnedParams::<Eq>::derive(16).unwrap();
    let trivial =
        iroha_plonk_recursion::AccumulatorT::trivial(&params, MemoryBudget::DEFAULT).unwrap();
    let (x, y) = iroha_plonk::transcript::generator_coordinates::<iroha_pasta::Eq>(trivial.g());
    let coordinates: Vec<_> = [x, y]
        .into_iter()
        .flat_map(|value| iroha_plonk_gadgets::statement::foreign_limbs(&value).map(Fp::from_u128))
        .collect();
    let mut frame = [Fp::ZERO; 69];
    frame[1] = Fp::from(16);
    for (point, challenges) in [(2, 6), (22, 26), (42, 46)] {
        frame[point..point + 4].copy_from_slice(&coordinates);
        frame[challenges..challenges + 16].copy_from_slice(trivial.challenges());
    }
    frame[62] = Fp::ONE;
    frame[65..69].copy_from_slice(&coordinates);
    let inputs = terminal_fold_inputs(&frame, trivial.as_input()).unwrap();
    assert_eq!(inputs, core::array::from_fn(|_| trivial.as_input()));
    // Four parsed slots remain four even when their values coincide. A caller
    // still cannot create a WrapperVesta fold without the opaque verified frame.
    assert!(matches!(
        create_fold(
            &params,
            &inputs,
            common::public_fixture_tape(919),
            iroha_plonk_recursion::FoldPurpose::WrapperVesta,
            &FoldConfig::default(),
        ),
        Err(iroha_plonk_recursion::Error::Purpose)
    ));
}
