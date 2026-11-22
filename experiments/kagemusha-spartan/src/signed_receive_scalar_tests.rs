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
    result: (UniformCircuit<S>, Instance<S>, Witness<S>, Vec<crate::r1cs::RowFill>),
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
        let challenges: Vec<S> = (0..built.phase2_challenges()).map(|_| rng.scalar()).collect();
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
    assert!(allocated < copied, "at least one authored row must be reused");
    eprintln!(
        "signed Receive {} copied {} rows, mutable {} rows, avoided {} bytes",
        built.label, copied, allocated, (copied - allocated) * cols * std::mem::size_of::<S>()
    );
}

fn signed_receive_fixture() -> (AuthenticatedParameters, RecoveryCapsule, PathBuf) {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let wallet = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
    let read = |dir: &PathBuf, file: &str| std::fs::read(dir.join(file)).unwrap();
    let parameters = crate::native::device_fixture::install(
        &read(&bundle, "program-bundle.norito"),
        &read(&bundle, "parameter-manifest.norito"),
        read(&bundle, "parameter-signature.bin").try_into().unwrap(),
    )
    .unwrap();
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
