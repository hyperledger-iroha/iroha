//! Complete Receive composition over explicit public predecessor/verdict test
//! ports. Actual recursive proof provenance is exercised by the full owner.
use super::*;
use crate::{
    full::{self, IncomingProofFacts, WalletKey},
    indexed_map::{self, SearchWitness},
    ipa::Gens,
    monetary_state,
    operation_auth::tests::{self as auth, Built},
    payment, receipt, receive_lineage as lineage,
    receive_originals::object_tests,
    request, spartan,
    uniform::UniformCircuit,
};
use ff::PrimeField;
use iroha_pasta::Eq as Vesta;

struct Fixture {
    witness: super::Witness,
    own: [Fp; 64],
    incoming: [Fp; 64],
    proof_valid: Fp,
    proof_digest: Fp,
    payment_digest: Fp,
}
fn fixture(proof_valid: bool, signature_valid: bool) -> Fixture {
    let mut input = object_tests::fixture();
    let receiver_key = WalletKey::labelled("Receive original receiver");
    let payer_key = WalletKey::labelled("Receive original payer");
    if !signature_valid {
        input.request.signature[..32].fill(0);
        let rq = request::commitments(&input.request).unwrap();
        input.statement[23] = rq.digest;
        input.receipt =
            receipt::tests::signed(&payer_key, &input.payer, &input.statement, Fp::ZERO);
        input.incoming[full::STATEMENT_IO] = crate::statement::digest(&input.statement);
        input.incoming[full::RECEIPT_IO] = receipt::digest(&input.receipt);
    }
    let payment = payment::digests(
        &input.request,
        &input.payer,
        &input.statement,
        &input.receipt,
        &input.pair,
    )
    .unwrap();
    let rq = request::commitments(&input.request).unwrap();
    let valid = proof_valid && signature_valid;
    let mut before = input.receiver.core;
    let rest = input.receiver.rest;
    before[9] = Fp::from(2000);
    before[11] = Fp::from(5);
    let mut after = before;
    after[9] += rq.words[word::AMOUNT];
    after[11] += Fp::ONE;
    after[16] = poseidon::hash_with_domain(
        CHAIN_DOMAIN,
        &[
            before[16],
            rq.credit,
            rq.words[word::PAYER_WALLET[0]],
            rq.words[word::PAYER_WALLET[1]],
            rq.words[word::AMOUNT],
        ],
    );
    after[29] += Fp::ONE;
    let consumed_value = poseidon::hash_with_domain(
        lineage::CONSUMED_DOMAIN,
        &[rq.credit, rq.words[word::AMOUNT], after[11]],
    );
    let (consumed, consumed_root) = indexed_map::empty_insert::<32>(rq.credit, consumed_value);
    assert_eq!(consumed.old_root, before[17]);
    after[17] = consumed_root;
    let consumed_search = SearchWitness {
        root: consumed.old_root,
        key: rq.credit,
        leaf: consumed.low,
        path: consumed.low_path.clone(),
    };
    let credit_value = poseidon::hash_with_domain(
        lineage::CREDIT_DOMAIN,
        &[rq.credit, payment.payment, Fp::from((!valid) as u64)],
    );
    let (credit, _) = indexed_map::empty_insert::<32>(rq.credit, credit_value);
    let credit_search = SearchWitness {
        root: credit.old_root,
        key: rq.credit,
        leaf: credit.low,
        path: credit.low_path.clone(),
    };
    let mut statement = input.receiver.statement;
    statement[9] = after[11];
    statement[10] = after[13];
    statement[11] = before[22];
    statement[14] = monetary_state::commitment(&before, &rest);
    statement[15] = monetary_state::commitment(&after, &rest);
    statement[16] = Fp::from(4);
    statement[17..].fill(Fp::ZERO);
    statement[17] = rq.credit;
    statement[18] = rq.words[word::PAYER_WALLET[0]];
    statement[19] = rq.words[word::PAYER_WALLET[1]];
    statement[20] = rq.words[word::AMOUNT];
    let own_receipt = receipt::tests::signed(
        &receiver_key,
        &input.receiver.credential,
        &statement,
        payment.payment,
    );
    let mut own = input.own;
    own[full::HEAD_IO] = statement[14];
    own[35] = Fp::from(30);
    own[36] = before[18];
    own[37] = credit.old_root;
    let mut empty = poseidon::hash_with_domain(lineage::PRIOR_LEAF_DOMAIN, &[Fp::ZERO]);
    let siblings = std::array::from_fn(|_| {
        let value = empty;
        empty = poseidon::hash_with_domain(lineage::PRIOR_NODE_DOMAIN, &[empty, empty]);
        value
    });
    Fixture {
        witness: super::Witness {
            before_core: before,
            before_rest: rest,
            after_core: after,
            after_rest: rest,
            credential: input.receiver.credential,
            request: input.request,
            payer: input.payer,
            quoted_receiver: input.quoted,
            incoming_statement: input.statement,
            incoming_receipt: input.receipt,
            prior_credential: lineage::PriorCredentialOpening {
                digest: Fp::ZERO,
                siblings,
            },
            statement,
            receipt: own_receipt,
            maps: lineage::ReceiveMapWitness {
                consumed_search,
                consumed_insert: consumed,
                insert: true,
                credit_search,
                credit_insert: credit,
            },
        },
        own,
        incoming: input.incoming,
        proof_valid: Fp::from(proof_valid as u64),
        proof_digest: payment.proof,
        payment_digest: payment.payment,
    }
}
fn build(input: &Fixture) -> Result<Built, &'static str> {
    build_relation(input, append, |receipt| receipt.digest())
}
fn build_preparation(input: &Fixture) -> Result<Built, &'static str> {
    build_relation(input, append_preparation, |body| body.message())
}
fn build_relation<R>(
    input: &Fixture,
    construct: impl FnOnce(
        &mut Wiring<Fp>,
        usize,
        &super::Witness,
        &crate::full::VerifiedPredecessor,
        &IncomingProofFacts,
    ) -> Result<ReceiveRelation<R>, &'static str>,
    receipt_port: impl FnOnce(&R) -> Source,
) -> Result<Built, &'static str> {
    let mut w = Wiring::default();
    let family = w.family(0, 1);
    let predecessor = auth::predecessor(&mut w, family, input.own);
    let incoming = IncomingProofFacts::fixture(
        &mut w,
        family,
        input.incoming,
        input.proof_valid,
        input.proof_digest,
    );
    let mut candidate = vec![
        incoming.head(),
        incoming.statement_digest(),
        incoming.receipt_digest(),
    ];
    candidate.extend(incoming.context());
    candidate.extend(incoming.lineage());
    candidate.extend(incoming.table());
    candidate.extend(incoming.kind());
    candidate.extend([incoming.validity(), incoming.proof_digest()]);
    auth::expose(&mut w, family, 256, &candidate);
    let completed = construct(&mut w, family, &input.witness, &predecessor, &incoming)?;
    auth::expose(
        &mut w,
        family,
        4,
        &[
            completed.head(),
            port(completed.statement.local, completed.statement.digest),
            receipt_port(&completed.receipt),
        ],
    );
    auth::expose(&mut w, family, 16, &completed.context());
    auth::expose(&mut w, family, 32, &completed.lineage());
    auth::expose(
        &mut w,
        family,
        64,
        &[
            completed.valid(),
            completed.objects_valid(),
            completed.signature_valid(),
            completed.nonmembership(),
            completed.payment().digest(),
            completed.payment().package_digest(),
        ],
    );
    let (a, p, n, t, c) = w.finish();
    Ok(UniformCircuit::build_with_copies(
        "complete Receive",
        a,
        p,
        n,
        t,
        c,
        17,
        512,
        b"complete Receive",
    ))
}

#[test]
fn receive_preparation_needs_no_own_signature_and_keeps_all_admission_constraints() {
    let mut good = fixture(true, true);
    good.witness.receipt.signature.fill(0);
    let prepared = build_preparation(&good).unwrap();
    assert!(auth::holds(&prepared));
    assert_eq!(prepared.1.io[64], Fp::ONE);
    let mut forged = good;
    forged.witness.after_core[9] += Fp::ONE;
    assert!(!auth::holds(&build_preparation(&forged).unwrap()));
    let mut invalid = fixture(true, false);
    invalid.witness.receipt.signature.fill(0);
    let prepared = build_preparation(&invalid).unwrap();
    assert!(auth::holds(&prepared));
    assert_eq!(prepared.1.io[64], Fp::ZERO);
    assert_eq!(prepared.1.io[66], Fp::ZERO);
}

#[test]
fn accepted_invalid_proof_and_invalid_request_signature_share_full_receive_matrix() {
    let good = fixture(true, true);
    let bad_proof = fixture(false, true);
    let bad_signature = fixture(true, false);
    let fingerprint = |circuit: &UniformCircuit<Fp>| {
        let mut hash = blake3::Hasher::new();
        // Stream one array at a time rather than allocating the entire sparse
        // matrix alongside the circuit and proof witness.
        for array in 0..circuit.arrays.len() {
            for (matrix, row, col, value) in
                circuit.entries_of(&[Fp::from(31), Fp::from(47)], Some(array))
            {
                for number in [matrix, row, col] {
                    hash.update(&(number as u64).to_le_bytes());
                }
                hash.update(value.to_repr().as_ref());
            }
        }
        *hash.finalize().as_bytes()
    };
    let mut expected_matrix = None;
    let mut expected_key = None;
    for (i, input) in [&good, &bad_proof, &bad_signature].into_iter().enumerate() {
        let start = std::time::Instant::now();
        let candidate = build(input).unwrap();
        let gens = Gens::<Vesta>::new(candidate.0.shape.cols());
        eprintln!("full Receive case {i} memory {:?}", candidate.0.proving_memory_accounting());
        let key = spartan::fixed_commitments(&candidate.0.shape, &gens, &candidate.0);
        let matrix = fingerprint(&candidate.0);
        if let Some(expected) = &expected_matrix {
            assert_eq!(&matrix, expected, "identical complete matrix {i}");
            assert_eq!(
                Some(&key),
                expected_key.as_ref(),
                "identical verification key {i}"
            );
        } else {
            expected_matrix = Some(matrix);
            expected_key = Some(key);
            eprintln!("full Receive shape {:?}", candidate.0.shape);
        }
        assert!(auth::holds(&candidate), "full Receive case {i}");
        assert_eq!(candidate.1.io[64], Fp::from((i == 0) as u64));
        assert_eq!(candidate.1.io[65], Fp::ONE);
        assert_eq!(candidate.1.io[66], Fp::from((i != 2) as u64));
        assert_eq!(candidate.1.io[67], Fp::ONE);
        assert_eq!(candidate.1.io[35], Fp::from(if i == 0 { 30 } else { 130 }));
        assert_eq!(candidate.1.io[68], input.payment_digest);
        let (proof, _) = spartan::prove_circuit(
            &candidate.0.shape,
            &gens,
            &candidate.1,
            &candidate.2,
            &mut SpikeRng::new(b"complete Receive actual proof"),
            &candidate.0,
        );
        assert!(
            spartan::verify_circuit(
                &candidate.0.shape,
                &gens,
                &candidate.1,
                &proof.to_bytes(),
                &candidate.0
            )
            .is_ok()
        );
        for slot in [
            4, 5, 6, 24, 32, 33, 34, 35, 36, 37, 56, 64, 65, 66, 67, 68, 69, 132, 152, 284, 285,
        ] {
            let mut forged = candidate.1.clone();
            forged.io[slot] += Fp::ONE;
            assert!(
                spartan::verify_circuit(
                    &candidate.0.shape,
                    &gens,
                    &forged,
                    &proof.to_bytes(),
                    &candidate.0
                )
                .is_err(),
                "public slot {slot}"
            );
        }
        eprintln!(
            "full Receive proof case {i} verified in {:?}",
            start.elapsed()
        );
    }
}

#[test]
fn full_receive_rejects_state_receipt_map_and_false_burn_substitutions() {
    let original = fixture(true, true);
    for case in 0..8 {
        let mut input = fixture(true, true);
        match case {
            0 => input.witness.after_core[9] += Fp::ONE,
            1 => input.witness.after_core[10] += Fp::ONE,
            2 => input.witness.maps.insert = false,
            3 => input.witness.maps.consumed_search.leaf.value += Fp::ONE,
            4 => input.witness.maps.consumed_insert.value += Fp::ONE,
            5 => input.witness.maps.credit_insert.value += Fp::ONE,
            6 => input.witness.receipt.body[306] ^= 1,
            _ => input.witness.before_core[8] += Fp::ONE,
        }
        if let Ok(b) = build(&input) {
            assert!(!auth::holds(&b), "forged full Receive case {case}");
        }
    }
    let mut honest = build(&original).unwrap();
    honest.1.io[64] = Fp::ZERO;
    honest.1.io[35] += Fp::from(100);
    assert!(
        !auth::holds(&honest),
        "valid originals cannot be voluntarily burned"
    );
}
