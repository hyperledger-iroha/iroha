//! Exact incoming objects with explicitly public recursive-verifier fixture
//! ports. These exercise the objects predicate, not a recursive proof verifier.
use super::*;
use crate::{
    bootstrap,
    full::{self, IncomingProofFacts, WalletKey},
    ipa::Gens,
    monetary_state,
    operation_auth::{self, tests as auth_tests},
    r1cs::{Instance, Witness},
    receipt,
    receive_lineage::PriorCredentialOpening,
    spartan, statement,
    uniform::UniformCircuit,
};
use iroha_pasta::Eq as Vesta;

#[derive(Clone)]
pub(crate) struct Fixture {
    pub(crate) receiver: bootstrap::Inputs,
    pub(crate) own: [Fp; 64],
    pub(crate) payer: crate::credential::Credential,
    pub(crate) quoted: crate::credential::Credential,
    pub(crate) request: CompletedRequest,
    pub(crate) statement: [Fp; 26],
    pub(crate) receipt: bootstrap::Receipt,
    pub(crate) incoming: [Fp; 64],
    pub(crate) pair: Vec<u8>,
}
pub(crate) fn fixture() -> Fixture {
    let gens = Gens::<Vesta>::new(1 << 14);
    let receiver_key = WalletKey::labelled("Receive original receiver");
    let payer_key = WalletKey::labelled("Receive original payer");
    let (context, receiver) = bootstrap::fixture(&receiver_key, &gens);
    let (_, payer_input) = bootstrap::fixture(&payer_key, &gens);
    let payer = payer_input.credential.clone();
    let quoted = receiver.credential.clone();
    let mut request = CompletedRequest {
        body: [0; 418],
        signature: [0; 64],
    };
    request.body[0] = 1;
    request.body[2..130].copy_from_slice(&payer.body[2..130]);
    request.body[130..194].copy_from_slice(&quoted.body[66..130]);
    request.body[194..210].copy_from_slice(&7u128.to_le_bytes());
    request.body[210..242].copy_from_slice(crate::credential::digest(&quoted).to_repr().as_ref());
    request.body[242..258].copy_from_slice(&100u128.to_le_bytes());
    request.body[346..354].copy_from_slice(&110u64.to_le_bytes());
    request.body[354] = 1;
    request.body[386..].fill(0x66);
    request.signature = bootstrap::sign(
        request::commitments(&request).unwrap().message,
        &receiver_key.d,
        "original Request fixture",
    );
    let rq = request::commitments(&request).unwrap();
    let mut statement = payer_input.statement;
    statement[9] = Fp::from(9);
    statement[10] = Fp::ONE;
    statement[12] = Fp::from(30);
    statement[13] = payer_input.core[18];
    statement[14] = Fp::from(900);
    statement[15] = Fp::from(901);
    statement[16] = Fp::from(3);
    statement[17] = rq.credit;
    statement[18] = rq.words[word::RECEIVER_WALLET[0]];
    statement[19] = rq.words[word::RECEIVER_WALLET[1]];
    statement[20] = rq.words[word::ORDINAL];
    statement[21] = rq.words[word::AMOUNT];
    statement[22] = rq.words[word::FEE];
    statement[23] = rq.digest;
    statement[24] = Fp::from(110);
    statement[25] = Fp::from(120);
    let operation = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[
            payer_input.core[5],
            payer_input.core[6],
            Fp::from(3),
            rq.credit,
        ],
    );
    let mut receipt = payer_input.receipt.clone();
    receipt.body[98..114].copy_from_slice(&statement[9].to_repr()[..16]);
    for (offset, v) in [
        (114, operation),
        (146, statement[14]),
        (178, statement[15]),
        (210, statement::digest(&statement)),
    ] {
        receipt.body[offset..offset + 32].copy_from_slice(v.to_repr().as_ref());
    }
    receipt.signature = bootstrap::sign(
        bootstrap::receipt_message(&receipt),
        &payer_key.d,
        "original Send receipt fixture",
    );
    let own = bootstrap::public_profile(&context, &receiver);
    let mut incoming = bootstrap::public_profile(&context, &payer_input);
    incoming[full::HEAD_IO] = statement[15];
    incoming[full::STATEMENT_IO] = statement::digest(&statement);
    incoming[full::RECEIPT_IO] = bootstrap::receipt_digest(&receipt);
    incoming[full::KIND_IO] = Fp::ONE;
    incoming[full::KIND_IO + 1] = Fp::ZERO;
    Fixture {
        receiver,
        own,
        payer,
        quoted,
        request,
        statement,
        receipt,
        incoming,
        pair: (0..257).map(|i| i as u8).collect(),
    }
}
type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);
fn build(input: &Fixture) -> Built {
    let mut w = Wiring::default();
    let family = w.family(0, 1);
    let prior = auth_tests::predecessor(&mut w, family, input.own);
    let incoming = IncomingProofFacts::fixture(
        &mut w,
        family,
        input.incoming,
        Fp::ONE,
        crate::payment::proof_digest(&input.pair).unwrap(),
    );
    // Bind every candidate field that the object predicate reads. The fixture
    // does not pretend that a proof has authenticated these public values.
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
    auth_tests::expose(&mut w, family, 256, &candidate);
    let before =
        monetary_state::append_state(&mut w, family, &input.receiver.core, &input.receiver.rest);
    let current =
        operation_auth::append_current(&mut w, family, &input.receiver.credential, &before, &prior)
            .unwrap();
    let payer = operation_auth::append_opening_total(&mut w, family, &input.payer);
    let quoted = operation_auth::append_opening_total(&mut w, family, &input.quoted);
    let request = append_request(&mut w, family, &input.request);
    let statement = append_statement(&mut w, family, &input.statement);
    let receipt = receipt::append_opening_total(&mut w, family, &input.receipt);
    let payment = crate::payment::append_digest(
        &mut w,
        family,
        request.request(),
        &payer,
        &statement,
        &receipt,
        &incoming,
    );
    auth_tests::expose(
        &mut w,
        family,
        16,
        &[
            payment.package_digest(),
            payment.digest(),
            incoming.proof_digest(),
        ],
    );
    let mut empty =
        poseidon::hash_with_domain(crate::receive_lineage::PRIOR_LEAF_DOMAIN, &[Fp::ZERO]);
    let siblings = std::array::from_fn(|_| {
        let value = empty;
        empty =
            poseidon::hash_with_domain(crate::receive_lineage::PRIOR_NODE_DOMAIN, &[empty, empty]);
        value
    });
    let objects = append_objects(
        &mut w,
        family,
        &request,
        &payer,
        &quoted,
        &receipt,
        &statement,
        &current,
        &before,
        &prior,
        &incoming,
        &PriorCredentialOpening {
            digest: Fp::ZERO,
            siblings,
        },
    );
    auth_tests::expose(
        &mut w,
        family,
        4,
        &[
            objects.valid(),
            objects.quoted_continuity(),
            payer.digest(),
            quoted.digest(),
        ],
    );
    auth_tests::expose(
        &mut w,
        family,
        8,
        &[receipt.digest(), statement.digest(), request.valid()],
    );
    // Exact Request bytes stay bound even when a noncanonical atom selects a
    // safe parsed zero; its semantic hash alone is not original provenance.
    let bytes: Vec<_> = (0..482).map(|i| request.byte(i)).collect();
    auth_tests::expose(&mut w, family, 512, &bytes);
    let (a, p, n, t, c) = w.finish();
    UniformCircuit::build_with_copies(
        "incoming original objects",
        a,
        p,
        n,
        t,
        c,
        14,
        1024,
        b"incoming original objects",
    )
}

#[test]
fn valid_and_wrong_asset_originals_have_one_verdict_and_matrix() {
    let good = fixture();
    let mut wrong = good.clone();
    wrong.request.body[34] ^= 1;
    let a = build(&good);
    let b = build(&wrong);
    assert!(auth_tests::holds(&a) && auth_tests::holds(&b));
    assert_eq!(a.1.io[4], Fp::ONE);
    assert_eq!(b.1.io[4], Fp::ZERO);
    assert_eq!(
        a.0.entries(&[Fp::from(31), Fp::from(47)]),
        b.0.entries(&[Fp::from(31), Fp::from(47)])
    );
    let gens = Gens::<Vesta>::new(a.0.shape.cols());
    for built in [&a, &b] {
        let (proof, _) = spartan::prove_circuit(
            &a.0.shape,
            &gens,
            &built.1,
            &built.2,
            &mut SpikeRng::new(b"original objects fixed matrix"),
            &a.0,
        );
        assert!(
            spartan::verify_circuit(&a.0.shape, &gens, &built.1, &proof.to_bytes(), &a.0).is_ok()
        );
        let mut tampered = built.1.clone();
        tampered.io[4] = Fp::ONE - tampered.io[4];
        assert!(
            spartan::verify_circuit(&a.0.shape, &gens, &tampered, &proof.to_bytes(), &a.0).is_err()
        );
    }
}

#[test]
fn every_incoming_authority_and_original_mismatch_is_a_derived_false() {
    let original = fixture();
    for case in 0..16 {
        let mut input = original.clone();
        match case {
            0 => input.request.body[130] ^= 1,
            1 => input.quoted.signature[0] ^= 1,
            2 => input.incoming[18] += Fp::ONE,
            3 => input.incoming[full::HEAD_IO] += Fp::ONE,
            4 => input.incoming[full::RECEIPT_IO] += Fp::ONE,
            5 => input.incoming[full::KIND_IO] = Fp::ZERO,
            6 => input.receipt.body[114] ^= 1,
            7 => input.statement[15] += Fp::ONE,
            8 => input.statement[17] += Fp::ONE,
            9 => input.statement[18] += Fp::ONE,
            10 => input.payer.body[131] ^= 1,
            11 => input.payer.body[259] ^= 1,
            12 => input.receipt.body[0] = 2,
            13 => input.request.body[386..].fill(0),
            14 => input.incoming[full::TABLE_IO] += Fp::ONE,
            _ => input.request.body[98] ^= 1,
        }
        // A noncurrent quote needs the authentic value at its actual prior
        // array index. bootstrap's all-zero prior array is opened below.
        let mut built = build(&input);
        assert!(auth_tests::holds(&built), "total original case {case}");
        assert_eq!(built.1.io[4], Fp::ZERO, "case {case}");
        built.1.io[4] = Fp::ONE;
        assert!(!auth_tests::holds(&built), "forged true case {case}");
    }
}

#[test]
fn complete_payment_digest_matches_native_and_binds_each_original_source() {
    let original = fixture();
    let base = build(&original);
    assert!(auth_tests::holds(&base));
    for case in 0..6 {
        let mut input = original.clone();
        match case {
            0 => {}
            1 => input.request.signature[0] ^= 1,
            2 => input.payer.signature[0] ^= 1,
            3 => input.statement[15] += Fp::ONE,
            4 => input.receipt.body[242] ^= 1,
            _ => *input.pair.last_mut().unwrap() ^= 1,
        }
        let native = crate::payment::digests(
            &input.request,
            &input.payer,
            &input.statement,
            &input.receipt,
            &input.pair,
        )
        .unwrap();
        let mut built = build(&input);
        assert!(auth_tests::holds(&built), "Payment source case {case}");
        assert_eq!(built.1.io[16], native.package);
        assert_eq!(built.1.io[17], native.payment);
        assert_eq!(built.1.io[18], native.proof);
        if case != 0 {
            assert_ne!(built.1.io[17], base.1.io[17], "changed original {case}");
        }
        for index in [16, 17, 18, 285] {
            built.1.io[index] += Fp::ONE;
            assert!(!auth_tests::holds(&built), "unbound Payment source {index}");
            built.1.io[index] -= Fp::ONE;
        }
    }
}
