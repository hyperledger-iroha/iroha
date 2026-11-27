//! Positive signed Receive fixture for the nine shared verifier scalars.
use super::*;
use crate::{
    ipa::F,
    uniform::{Array, Lin, Src},
    verifier::{Kind, Trace},
};
use iroha_pasta::PastaCurve;
use std::path::PathBuf;

struct ProofWords {
    challenges: usize,
    encodings: [[u8; 32]; 9],
    after_scalars: usize,
    extra_canonical_rows: usize,
    modulus_minus_one: BigUint,
}

impl ProofWords {
    fn from_trace<C: PastaCurve>(
        trace: &Trace<C>,
        proof: &spartan::Proof<C>,
        claims: usize,
    ) -> Self {
        let challenges = trace
            .challenges
            .iter()
            .map(|(_, k, _)| if *k == Kind::Full { 2 } else { 1 })
            .sum();
        let late = usize::from(
            trace
                .circuit
                .as_ref()
                .is_some_and(|c| c.late_digest.is_some()),
        );
        let link = usize::from(matches!(trace.link, Some((true, ..))));
        let two = F::<C>::from(2);
        let half_z1 = (proof.ipa.z1 - two.pow_vartime([255]) - F::<C>::ONE) * two.invert().unwrap();
        let encodings = [
            trace.io_digest,
            proof.v[0],
            proof.v[1],
            proof.v[2],
            trace.kappa_claims,
            trace.kappa_claims * trace.zeta,
            half_z1,
            proof.ipa.z1 * trace.b_final * trace.zeta,
            proof.ipa.z2,
        ]
        .map(|x| x.to_repr());
        Self {
            challenges,
            encodings,
            after_scalars: 4 * claims + 2 * (late + link),
            extra_canonical_rows: late + link,
            modulus_minus_one: BigUint::from_bytes_le(&(-F::<C>::ONE).to_repr()),
        }
    }

    fn words(&self) -> usize {
        self.challenges + 2 * self.encodings.len() + self.after_scalars
    }
}

// Each short top chunk uses a second lookup scaled into the same signed 11-bit table.
fn check_lookups<S: Scalar>(a: &Array<S>, start: usize, widths: &[u16]) {
    let actual = a
        .lookups
        .iter()
        .map(|(table, tuple)| {
            assert_eq!(*table, crate::nonnative::RANGE_TABLE);
            let [lin] = tuple.as_slice() else {
                panic!("range tuple arity")
            };
            let [term] = lin.as_slice() else {
                panic!("range source arity")
            };
            let Src::Own(cell) = term.src else {
                panic!("range source cell")
            };
            assert!(term.coef.iter().all(|c| c == &term.coef[0]));
            (cell, term.coef[0])
        })
        .collect::<Vec<_>>();
    let mut expected = Vec::new();
    for (i, width) in widths.iter().enumerate() {
        expected.push((start + i, S::ONE));
        if *width < 11 {
            expected.push((start + i, S::from(1 << (11 - width))));
        }
    }
    assert_eq!(actual, expected);
}

fn evaluate<S: Scalar>(lin: &Lin<S>, w: &full::Wiring<S>, a: &Array<S>, r: usize) -> S {
    lin.iter()
        .map(|term| {
            let value = match &term.src {
                Src::One => S::ONE,
                Src::Own(cell) => a.values[*cell][r],
                Src::Link { array, cell, map } => w.arrays[*array].values[*cell][map[r].unwrap()],
                Src::All { .. } => panic!("unexpected aggregate in range row"),
            };
            value * term.coef[r]
        })
        .fold(S::ZERO, |acc, term| acc + term)
}

fn check_side<S: Scalar>(
    w: &full::Wiring<S>,
    shared: usize,
    slots: &[[ProofWords; 2]; 2],
    points: usize,
) {
    let binding = &w.arrays[shared];
    let mut found = w
        .arrays
        .iter()
        .filter(|a| a.name == "canonical full-width limbs");
    let a = found.next().expect("joined Receive canonical range array");
    assert!(found.next().is_none());
    let mut chunks = vec![11; 12];
    chunks[11] = 7; // 128 low bits
    if points == 1 {
        assert_eq!(binding.name, "binding shared");
        assert_eq!(binding.widths[0], 128);
        assert_eq!(&binding.widths[1..], &chunks);
        check_lookups(binding, 1, &chunks);
    } else {
        assert_eq!(binding.name, "binding native");
        assert_eq!(binding.widths, [128]);
    }
    chunks.extend([11; 11]);
    chunks.push(6); // 127 high bits
    assert_eq!(&a.widths[..24], &chunks);
    assert_eq!(&a.widths[24..], &[1]);
    check_lookups(a, 0, &chunks);
    assert_eq!(a.rows.len(), 3);
    let mut base = 0;
    let mut r = 0;
    for (slot, pair) in slots.iter().enumerate() {
        let proof = &pair[points];
        let full = base + if points == 1 { pair[0].words() } else { 0 } + proof.challenges;
        for (s, repr) in proof.encodings.iter().enumerate() {
            let value = BigUint::from_bytes_le(repr);
            let remainder = &proof.modulus_minus_one - &value;
            let mut shift = 0;
            for (cell, width) in chunks.iter().enumerate() {
                let part = (&remainder >> shift) & BigUint::from((1u32 << width) - 1);
                assert_eq!(BigUint::from_bytes_le(&a.values[cell][r].to_repr()), part);
                shift += usize::from(*width);
            }
            for half in 0..2 {
                let off = full + 2 * s + half;
                let limb = u128::from_le_bytes(repr[16 * half..16 * half + 16].try_into().unwrap());
                assert_eq!(binding.values[0][off], S::from_u128(limb));
                let mut links = a.rows[half][0].iter().filter_map(|t| match &t.src {
                    Src::Link { array, cell, map } => Some((*array, *cell, map[r], t.coef[r])),
                    _ => None,
                });
                assert_eq!(links.next(), Some((shared, 0, Some(off), S::ONE)));
                assert!(links.next().is_none());
            }
            for [left, right, out] in &a.rows {
                assert_eq!(
                    evaluate(left, w, a, r) * evaluate(right, w, a, r),
                    evaluate(out, w, a, r)
                );
            }
            r += 1;
        }
        eprintln!(
            "SIGNED_RECEIVE_FULL_SCALARS points_side={} slot={slot} first_shared={full} count=9",
            if points == 1 { "P" } else { "S" }
        );
        r += proof.extra_canonical_rows;
        base += pair[0].words() + pair[1].words();
    }
    assert_eq!(r, a.r_count);
}

// A verifier-only import lacks the prover's row placement. Compare the bounds
// on each reconstructed wallet circuit whose digest matches the signed original.
fn check_built_program<S: Scalar>(built: &UniformCircuit<S>, signed: &UniformCircuit<S>) {
    use std::time::Instant;

    assert_eq!(built.program_digest(), signed.program_digest());
    let start = Instant::now();
    let full = built.phase_memory_accounting();
    let full_elapsed = start.elapsed();
    let start = Instant::now();
    let preparation = built.preparation_memory_bytes();
    let preparation_elapsed = start.elapsed();
    let start = Instant::now();
    let first_phase = built.first_phase_memory_bytes();
    let first_phase_elapsed = start.elapsed();
    assert_eq!(preparation, full.preparation_bytes);
    assert_eq!(first_phase, full.phase1_bytes);
    eprintln!(
        "signed Receive preparation={preparation} first_phase={first_phase} bytes; \
         full={full_elapsed:?} preparation_only={preparation_elapsed:?} first_phase_only={first_phase_elapsed:?}"
    );
}

// A fresh proof writes masks, its link phase and its late witness helpers.
// Compare the old immutable input and the guarded mutable input at the exact
// issued dimensions. Only row counts and their storage requirements are printed.
fn check_owned_writes<S: Scalar + zeroize::Zeroize>(
    result: (
        UniformCircuit<S>,
        Instance<S>,
        Witness<S>,
        Vec<crate::r1cs::RowFill>,
    ),
    signed: &UniformCircuit<S>,
) {
    use crate::{fs::SpikeRng, kern::Rows, r1cs::Circuit as _};
    let (built, _, witness, fill) = result;
    check_built_program(&built, signed);
    let cols = built.shape.cols();
    let mutate = |rows: &mut Rows<'_, S>| {
        let mut rng = SpikeRng::new(b"signed Receive witness row differential");
        built.randomize_masks_rows(rows, &mut rng);
        if built.link_rows(&built.shape) > 0 {
            built.fill_link_rows(&[0x17; 32], rows);
        }
        let challenges: Vec<S> = (0..built.phase2_challenges())
            .map(|_| rng.scalar())
            .collect();
        drop(built.instantiate_rows(&built.shape, &challenges, rows));
        let mut digest = blake3::Hasher::new();
        for row in rows.slices() {
            digest.update(&(row.len() as u64).to_le_bytes());
            for x in row {
                digest.update(x.to_repr().as_ref());
            }
        }
        let evidence = (rows.owned(), digest.finalize());
        rows.zeroize_owned();
        evidence
    };
    let (copied, prior) = mutate(&mut Rows::with_fill(&witness.w, cols, &fill));
    let mut witness = crate::r1cs::WrittenWitness::new(witness, fill);
    let (witness, fill) = witness.parts_mut();
    let (allocated, current) = mutate(&mut Rows::with_mutable_values(&mut witness.w, cols, fill));
    assert_eq!(prior, current);
    assert!(
        allocated < copied,
        "at least one authored row must be reused"
    );
    eprintln!(
        "signed Receive {} copied {} rows, mutable {} rows, avoided {} bytes",
        built.label,
        copied,
        allocated,
        (copied - allocated) * cols * std::mem::size_of::<S>()
    );
}

fn signed_receive_fixture() -> (AuthenticatedParameters, RecoveryCapsule, PathBuf) {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let wallet = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
    let read = |dir: &PathBuf, file: &str| std::fs::read(dir.join(file)).unwrap();
    let parameters = crate::native::device_fixture::install_test_bundle(&bundle).unwrap();
    let capsule = RecoveryCapsule::decode(
        &read(&wallet, "counterparty-receive-capsule.norito"),
        &crate::native::device_fixture::scheme().scheme_id(),
    )
    .unwrap();
    (parameters, capsule, wallet)
}

#[test]
#[ignore = "requires the signed device bundle and retained counterparty Receive originals"]
fn resumed_predecessor_matches_full_verification_only_for_bound_original_and_owner() {
    let (mut parameters, capsule, _) = signed_receive_fixture();
    let engine = NativeEngine::default();
    let mut prepared = engine.resume(&capsule, &parameters).unwrap();
    let original = Arc::clone(&prepared.predecessor.pair);
    let mut altered = original.to_vec();
    altered[0] ^= 1;
    prepared.predecessor.pair = altered.into();
    assert!(
        engine
            .predecessor_for_proof(&mut prepared, &parameters)
            .is_err()
    );
    prepared.predecessor.pair = original;
    let other_engine = NativeEngine::default();
    let installation = std::mem::replace(&mut parameters.installation, Arc::new(()));
    assert!(!engine.resumed_here(&prepared, &parameters));
    drop(
        engine
            .predecessor_for_proof(&mut prepared, &parameters)
            .unwrap(),
    );
    assert!(
        prepared
            .resumed_under
            .as_ref()
            .unwrap()
            .predecessor
            .is_some()
    );
    parameters.installation = installation;
    assert!(!other_engine.resumed_here(&prepared, &parameters));
    assert!(engine.resumed_here(&prepared, &parameters));
    let start = std::time::Instant::now();
    let independently_verified = other_engine
        .predecessor_for_proof(&mut prepared, &parameters)
        .unwrap();
    let independently_verified = independently_verified.pair;
    let full_check = start.elapsed();
    assert!(
        prepared
            .resumed_under
            .as_ref()
            .unwrap()
            .predecessor
            .is_some()
    );
    let start = std::time::Instant::now();
    let pred = engine
        .predecessor_for_proof(&mut prepared, &parameters)
        .unwrap();
    let pred = pred.pair;
    eprintln!(
        "signed Receive predecessor full check {full_check:?}, retained {:?}",
        start.elapsed()
    );
    assert!(
        prepared
            .resumed_under
            .as_ref()
            .unwrap()
            .predecessor
            .is_none()
    );
    assert_eq!(independently_verified.kind, pred.kind);
    assert_eq!(independently_verified.ip.io, pred.ip.io);
    assert_eq!(independently_verified.is.io, pred.is.io);
    assert_eq!(independently_verified.pp.to_bytes(), pred.pp.to_bytes());
    assert_eq!(independently_verified.ps.to_bytes(), pred.ps.to_bytes());
    assert_eq!(
        (
            independently_verified.d,
            independently_verified.rho,
            independently_verified.r
        ),
        (pred.d, pred.rho, pred.r)
    );
    assert!(Arc::ptr_eq(&independently_verified.cp, &pred.cp));
    assert!(Arc::ptr_eq(&independently_verified.cs, &pred.cs));
    drop(independently_verified);
}

#[test]
#[ignore = "requires the signed device bundle and retained counterparty Receive originals"]
fn signed_joined_receive_encodes_all_nine_scalars_on_both_curves() {
    let (parameters, capsule, wallet) = signed_receive_fixture();
    let record = CompletionRecord::decode(
        &std::fs::read(wallet.join("counterparty-receive-completion.norito")).unwrap(),
        capsule.wallet(),
    )
    .unwrap();
    let engine = NativeEngine::default();
    let (prepared, _) = engine
        .verify_released_operation(&capsule, &record, &parameters)
        .unwrap();
    let mut witness = prepared.witness.as_ref().clone();
    let receipt = receipt_mut(&mut witness);
    assert_eq!(&receipt.body[..], &record.exact_receipt()[..338]);
    receipt
        .signature
        .copy_from_slice(&record.exact_receipt()[338..]);
    let ExactOperationWitness::Receive {
        step,
        incoming_payment,
    } = &witness
    else {
        panic!("retained Receive")
    };
    assert_eq!(
        incoming_payment.as_ref(),
        std::fs::read(wallet.join("complete-payment.bin")).unwrap()
    );
    let incoming = prepared.receive.as_ref().unwrap();
    assert!(incoming.can_credit);
    let (_, _, _, _, pred) = decode_head(&prepared.predecessor.pair, &parameters).unwrap();
    let p = &parameters.inner;
    let pairs = [
        full::trace_uniform(pred, &p.generators),
        full::trace_incoming_candidate(
            incoming.pair.clone(),
            &p.generators,
            incoming.pair_bytes.clone(),
        ),
    ];
    assert_eq!(pairs[1].kind, full::SEND);
    let slots = pairs.each_ref().map(|pair| {
        [
            ProofWords::from_trace(&pair.tr_p, &pair.proof_p, pair.claims_p.len()),
            ProofWords::from_trace(&pair.tr_s, &pair.proof_s, pair.claims_s.len()),
        ]
    });
    let entropy = ProofEntropy::fresh().unwrap();
    let folds = full::GeneratorFolds::prepare(&p.generators, &pairs, &entropy).unwrap();
    let signed = &p.programs[full::RECEIVE];
    let (w, idx) = full::p_operation(
        full::OperationWitness::Receive(step),
        &p.generators,
        &pairs,
        &folds,
    )
    .unwrap();
    check_side(&w, idx.shared, &slots, 1);
    check_owned_writes(
        build(w, &signed.primary, &entropy, b"primary witness").unwrap(),
        &signed.primary,
    );
    let (w, idx) = full::s_operation(full::RECEIVE, None, &p.generators, &pairs, &folds);
    check_side(&w, idx.shared, &slots, 0);
    check_owned_writes(
        build(w, &signed.secondary, &entropy, b"secondary witness").unwrap(),
        &signed.secondary,
    );
}

fn same_fold<C: PastaCurve>(
    a: &crate::generator_claim::Fold<C>,
    b: &crate::generator_claim::Fold<C>,
    case: &str,
) {
    assert_eq!(a.sources.len(), b.sources.len(), "{case}: source count");
    for (x, y) in a.sources.iter().zip(&b.sources) {
        assert!(x.g == y.g && x.us == y.us, "{case}: sources");
    }
    assert_eq!(a.masks, b.masks, "{case}: masks");
    assert_eq!(a.d, b.d, "{case}: D");
    assert_eq!(a.lr, b.lr, "{case}: L/R");
    assert_eq!(a.c, b.c, "{case}: c");
    assert_eq!(a.output.us, b.output.us, "{case}: output challenges");
    assert_eq!(a.output.g, b.output.g, "{case}: output G");
}

fn same_fold_trace<C: PastaCurve>(
    a: &crate::generator_claim::Trace<C>,
    b: &crate::generator_claim::Trace<C>,
    case: &str,
) {
    assert_eq!(a.relation, b.relation, "{case}: relation");
    assert_eq!(
        format!("{:?}", a.layout),
        format!("{:?}", b.layout),
        "{case}: layout"
    );
    assert_eq!(a.shared, b.shared, "{case}: shared limbs");
    assert_eq!(a.input_header, b.input_header, "{case}: input header");
    assert_eq!(a.fold_header, b.fold_header, "{case}: fold header");
    assert_eq!(a.digest, b.digest, "{case}: digest");
    assert_eq!(a.challenges, b.challenges, "{case}: challenges");
    assert_eq!(a.mu, b.mu, "{case}: mu");
    assert_eq!(a.point, b.point, "{case}: point");
    assert_eq!(a.zeta, b.zeta, "{case}: zeta");
    assert_eq!(a.gamma, b.gamma, "{case}: gamma");
    assert_eq!(a.value, b.value, "{case}: value");
    assert_eq!(a.b_final, b.b_final, "{case}: b_final");
    assert_eq!(a.commitment, b.commitment, "{case}: C-star");
}

/// Opus review: the signed counterparty Receive through the verified fold.
/// Its native verdict binds the exact frozen Payment and proof bytes; every
/// generator claim the verified fold reuses (own and inherited, head and
/// incoming, both curves) is decided true exactly as traced. The checked blocks
/// produce the same folds and fold traces on both curves as
/// the stock decomposition from the same entropy. Prints wall and process CPU
/// seconds, stock and verified alternating (stock, verified, verified, stock).
#[test]
#[ignore = "requires the signed device bundle and retained counterparty Receive originals"]
fn signed_receive_verified_folds_match_stock_on_both_curves() {
    let (mut parameters, capsule, wallet) = signed_receive_fixture();
    let record = CompletionRecord::decode(
        &std::fs::read(wallet.join("counterparty-receive-completion.norito")).unwrap(),
        capsule.wallet(),
    )
    .unwrap();
    let engine = NativeEngine::default();
    let (prepared, _) = engine
        .verify_released_operation(&capsule, &record, &parameters)
        .unwrap();
    let ExactOperationWitness::Receive {
        incoming_payment, ..
    } = prepared.witness.as_ref()
    else {
        panic!("retained Receive")
    };
    let incoming = prepared.receive.as_ref().unwrap();
    assert!(incoming.can_credit);
    let payment = incoming
        .verified
        .as_ref()
        .expect("a creditable Receive carries the native verdict");
    payment.require_parameters(&parameters).unwrap();
    assert_eq!(payment.exact_bytes(), incoming_payment.as_ref());
    assert_eq!(payment.exact_pair(), incoming.pair_bytes.as_slice());
    let (_, _, _, _, pred) = decode_head(&prepared.predecessor.pair, &parameters).unwrap();
    let p = &parameters.inner;
    let mut pairs = [
        full::trace_uniform(pred, &p.generators),
        full::trace_incoming_candidate(
            incoming.pair.clone(),
            &p.generators,
            incoming.pair_bytes.clone(),
        ),
    ];
    assert_eq!(
        pairs[1].incoming_original.as_deref(),
        Some(payment.exact_pair())
    );
    for (side, pair) in ["head", "incoming"].into_iter().zip(&pairs) {
        let own_p = crate::ipa::Claim {
            g: pair.proof_p.ipa.g_final,
            us: pair.tr_p.us.clone(),
        };
        let inherited_p = full::inherited_generator_claim(&pair.tr_p, &p.generators.p).unwrap();
        let own_s = crate::ipa::Claim {
            g: pair.proof_s.ipa.g_final,
            us: pair.tr_s.us.clone(),
        };
        let inherited_s = full::inherited_generator_claim(&pair.tr_s, &p.generators.s).unwrap();
        assert!(
            crate::generator_claim::decide(&p.generators.p, &own_p),
            "{side} own P claim"
        );
        assert!(
            crate::generator_claim::decide(&p.generators.p, &inherited_p),
            "{side} inherited P claim"
        );
        assert!(
            crate::generator_claim::decide(&p.generators.s, &own_s),
            "{side} own S claim"
        );
        assert!(
            crate::generator_claim::decide(&p.generators.s, &inherited_s),
            "{side} inherited S claim"
        );
    }
    let original = incoming_payment.as_ref();
    let head = &prepared
        .resumed_under
        .as_ref()
        .unwrap()
        .predecessor
        .as_ref()
        .unwrap()
        .blocks;
    assert!(matches!(
        VerifiedFoldSources::bind(&parameters, head, payment, original, &pairs[..1]),
        Err(Error::Malformed("verified Receive fold proof count"))
    ));
    assert!(matches!(
        VerifiedFoldSources::bind(&parameters, head, payment, &original[1..], &pairs),
        Err(Error::AuthorityMismatch("verified Receive fold sources"))
    ));
    let mut other_manifest = payment.clone();
    other_manifest.manifest_digest[0] ^= 1;
    assert!(matches!(
        VerifiedFoldSources::bind(&parameters, head, &other_manifest, original, &pairs),
        Err(Error::AuthorityMismatch(
            "verified Payment parameter manifest"
        ))
    ));
    let genesis = parameters.genesis_commit_verifier();
    let checkpoint = genesis
        .export_epoch_checkpoint(genesis.initial_commit_epoch())
        .unwrap();
    let reselected = engine
        .with_protected_epoch(&parameters, &checkpoint)
        .unwrap();
    assert!(reselected.accepted_payment(original, &parameters).is_none());
    let independent = reselected
        .payment_for_fold(payment, original, &parameters)
        .unwrap();
    assert!(!Arc::ptr_eq(&payment.blocks, &independent.blocks));
    assert!(matches!(
        VerifiedFoldSources::bind(&parameters, head, &independent, original, &pairs),
        Err(Error::AuthorityMismatch("generator fold installation"))
    ));
    pairs.swap(0, 1);
    assert!(matches!(
        VerifiedFoldSources::bind(&parameters, head, payment, original, &pairs),
        Err(Error::AuthorityMismatch("verified Receive fold sources"))
    ));
    pairs.swap(0, 1);
    let original_pair = pairs[1].incoming_original.take().unwrap();
    assert!(matches!(
        VerifiedFoldSources::bind(&parameters, head, payment, original, &pairs),
        Err(Error::AuthorityMismatch("verified Receive fold sources"))
    ));
    pairs[1].incoming_original = Some(original_pair);
    let entropy = ProofEntropy::fresh().unwrap();
    let general = full::RecursiveInputs::new(&p.generators, &pairs).incoming_witness_values();
    assert_eq!(general.0, [1; 10]);
    {
        let sources =
            VerifiedFoldSources::bind(&parameters, head, payment, original, &pairs).unwrap();
        let mut other = full::RecursiveInputs::new(&p.generators, &pairs[..1]);
        assert!(matches!(
            other.fold_verified_receive(&sources, &entropy),
            Err("verified Receive recursive inputs")
        ));
    }

    // The original bytes are held constant here to ensure a mismatched
    // traced claim cannot qualify the sibling generator predicates.
    pairs[1].tr_p.us[0] += Fp::ONE;
    {
        let sources =
            VerifiedFoldSources::bind(&parameters, head, payment, original, &pairs).unwrap();
        let mut refused = full::RecursiveInputs::new(&p.generators, &pairs);
        assert!(matches!(
            refused.fold_verified_receive(&sources, &entropy),
            Err("verified generator claim mismatch")
        ));
        let (verdicts, primary_matrix, secondary_matrix) = refused.incoming_witness_values();
        assert_eq!(verdicts[4], 0);
        assert_eq!(verdicts[5..], general.0[5..]);
        assert_eq!((primary_matrix, secondary_matrix), (general.1, general.2));
    }
    pairs[1].tr_p.us[0] -= Fp::ONE;

    let stock = || full::GeneratorFolds::prepare(&p.generators, &pairs, &entropy).unwrap();
    let verified = || {
        let mut inputs = full::RecursiveInputs::new(&p.generators, &pairs);
        let sources =
            VerifiedFoldSources::bind(&parameters, head, payment, original, &pairs).unwrap();
        let folds = inputs.fold_verified_receive(&sources, &entropy).unwrap();
        (folds, inputs)
    };
    let (s1, s1_wall, s1_cpu) = measured(stock);
    let ((v1, first), v1_wall, v1_cpu) = measured(verified);
    let ((v2, second), v2_wall, v2_cpu) = measured(verified);
    assert_eq!(first.incoming_witness_values(), general);
    assert_eq!(second.incoming_witness_values(), general);
    let (s2, s2_wall, s2_cpu) = measured(stock);
    for (other, label) in [(&v1, "verified 1"), (&v2, "verified 2"), (&s2, "stock 2")] {
        same_fold(&s1.p, &other.p, &format!("primary fold, {label}"));
        same_fold(&s1.s, &other.s, &format!("secondary fold, {label}"));
        same_fold_trace(&s1.tp, &other.tp, &format!("primary fold trace, {label}"));
        same_fold_trace(&s1.ts, &other.ts, &format!("secondary fold trace, {label}"));
    }
    eprintln!(
        "opus-review signed Receive GeneratorFolds: stock wall {s1_wall:.3} {s2_wall:.3} s cpu {s1_cpu:.3} {s2_cpu:.3} s; verified wall {v1_wall:.3} {v2_wall:.3} s cpu {v1_cpu:.3} {v2_cpu:.3} s"
    );
    let single = VerifiedFoldSources::single(&parameters, head, &pairs[..1]).unwrap();
    let single = full::GeneratorFolds::prepare_verified(&single, &entropy).unwrap();
    let repeated = full::GeneratorFolds::prepare(&p.generators, &pairs[..1], &entropy).unwrap();
    same_fold(&repeated.p, &single.p, "single-parent primary");
    same_fold(&repeated.s, &single.s, "single-parent secondary");
    same_fold_trace(&repeated.tp, &single.tp, "single-parent primary trace");
    same_fold_trace(&repeated.ts, &single.ts, "single-parent secondary trace");

    // A preparation retained under the same manifest may finalize after a
    // reinstall. Rebind the exact head and Payment before deriving its folds.
    parameters.installation = Arc::new(());
    let remounted = engine
        .payment_for_fold(payment, original, &parameters)
        .unwrap();
    let (_, own) = engine
        .verified_head_and_pair(
            &prepared.predecessor.pair,
            &prepared.predecessor.credential,
            &parameters,
        )
        .unwrap();
    assert!(!Arc::ptr_eq(&payment.blocks, &remounted.blocks));
    assert!(!Arc::ptr_eq(head, &own.blocks));
    let sources =
        VerifiedFoldSources::bind(&parameters, &own.blocks, &remounted, original, &pairs).unwrap();
    let fresh = full::GeneratorFolds::prepare_verified(&sources, &entropy).unwrap();
    same_fold(&s1.p, &fresh.p, "remounted primary");
    same_fold(&s1.s, &fresh.s, "remounted secondary");
    same_fold_trace(&s1.tp, &fresh.tp, "remounted primary trace");
    same_fold_trace(&s1.ts, &fresh.ts, "remounted secondary trace");
}

/// Process CPU seconds (user + system, every thread) for the review timings.
fn process_cpu_seconds() -> f64 {
    #[cfg(target_os = "macos")]
    type Usec = i32;
    #[cfg(not(target_os = "macos"))]
    type Usec = i64;
    #[repr(C)]
    struct TimeVal {
        sec: i64,
        usec: Usec,
    }
    #[repr(C)]
    struct RUsage {
        user: TimeVal,
        system: TimeVal,
        rest: [i64; 14],
    }
    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut RUsage) -> i32;
    }
    let mut usage = std::mem::MaybeUninit::<RUsage>::zeroed();
    // SAFETY: getrusage(RUSAGE_SELF) writes one complete struct rusage.
    assert_eq!(unsafe { getrusage(0, usage.as_mut_ptr()) }, 0);
    // SAFETY: zero-initialized and then fully written by getrusage.
    let usage = unsafe { usage.assume_init() };
    let seconds = |t: &TimeVal| t.sec as f64 + t.usec as f64 * 1e-6;
    seconds(&usage.user) + seconds(&usage.system)
}

/// Wall and process CPU seconds of `f`.
fn measured<T>(f: impl FnOnce() -> T) -> (T, f64, f64) {
    let (wall, cpu) = (std::time::Instant::now(), process_cpu_seconds());
    let out = f();
    (
        out,
        wall.elapsed().as_secs_f64(),
        process_cpu_seconds() - cpu,
    )
}
