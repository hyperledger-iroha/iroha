//! Public signature binding, total failure paths and actual native Q proofs.

mod common;
use ff::Field;
use iroha_kagemusha_proof::q_signature::{
    QSignatureCircuit, QSignaturePlan, SignatureKey, SignatureSlot, SignatureWitness,
};
use iroha_pasta::{Fq, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    frontend::{Circuit, configure, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    verifier::accumulate_generator,
};
use iroha_plonk_gadgets::{
    ff::Nat,
    p256::{
        VerifyMode,
        native::{self, Affine, HALF_N, N, ORDER, P},
    },
    sha256::native::sha256_of_digest,
};

fn words(bytes: [u8; 32]) -> [u64; 4] {
    core::array::from_fn(|i| {
        u64::from_be_bytes(bytes[(3 - i) * 8..(4 - i) * 8].try_into().unwrap())
    })
}
fn witness() -> SignatureWitness {
    let digest = iroha_pasta::Fp::from(123);
    let message = words(sha256_of_digest(&digest));
    let secret = [17, 0, 0, 0];
    let nonce = [19, 0, 0, 0];
    let key = native::mul(&Affine::GENERATOR, &secret).unwrap();
    let r = ORDER.reduce_once(&native::mul(&Affine::GENERATOR, &nonce).unwrap().x);
    let mut s = ORDER.mul(
        &ORDER.inverse(&nonce),
        &ORDER.add(&ORDER.reduce_once(&message), &ORDER.mul(&r, &secret)),
    );
    if native::words_cmp(&s, &HALF_N).is_gt() {
        s = ORDER.neg(&s);
    }
    assert!(native::verify_prehashed(&message, &r, &s, &key));
    SignatureWitness {
        digest,
        key: [key.x, key.y],
        signature: [r, s],
    }
}
fn oracle(w: &SignatureWitness) -> bool {
    native::verify_prehashed(
        &words(sha256_of_digest(&w.digest)),
        &w.signature[0],
        &w.signature[1],
        &Affine {
            x: w.key[0],
            y: w.key[1],
        },
    )
}
fn circuit(w: SignatureWitness, mode: VerifyMode, fixed: bool) -> QSignatureCircuit {
    let key = if fixed {
        SignatureKey::Fixed(Affine {
            x: w.key[0],
            y: w.key[1],
        })
    } else {
        SignatureKey::Variable
    };
    QSignatureCircuit::new(
        QSignaturePlan::new(vec![SignatureSlot { mode, key }]).unwrap(),
        vec![w],
    )
    .unwrap()
}
fn satisfied(c: &QSignatureCircuit, public: &[Vec<Fq>]) -> bool {
    check_circuit(c, 16, public, CheckMode::Strict)
        .unwrap()
        .is_satisfied()
}
#[test]
fn signature_plan_rejects_invalid_shapes_and_fixed_keys() {
    assert!(QSignaturePlan::new(vec![]).is_err());
    let slot = SignatureSlot {
        mode: VerifyMode::Soft,
        key: SignatureKey::Variable,
    };
    assert!(QSignaturePlan::new(vec![slot; 6]).is_err());
    assert!(QSignaturePlan::new(vec![slot; 7]).is_err());
    assert!(
        QSignaturePlan::new(vec![SignatureSlot {
            key: SignatureKey::Fixed(Affine { x: P, y: [0; 4] }),
            ..slot
        }])
        .is_err()
    );
    let keys = (1..=3)
        .map(|d| SignatureSlot {
            key: SignatureKey::Fixed(native::mul(&Affine::GENERATOR, &[d, 0, 0, 0]).unwrap()),
            ..slot
        })
        .collect();
    assert!(QSignaturePlan::new(keys).is_err());
    let plan = QSignaturePlan::new(vec![slot]).unwrap();
    assert!(QSignatureCircuit::new(plan, vec![]).is_err());
    assert!(
        circuit(witness(), VerifyMode::Hard, false)
            .instances(&[])
            .is_err()
    );
}
#[test]
fn signature_honest_layout_and_all_public_inputs_bound() {
    let w = witness();
    for fixed in [false, true] {
        let c = circuit(w, VerifyMode::Hard, fixed);
        let public = c.instances(&[true]).unwrap();
        assert!(satisfied(&c, &public));
        let (cs, _) = configure(&c).unwrap();
        assert_eq!(cs.num_advice_columns(), 17);
        assert_eq!(cs.lookups().len(), 10);
        assert_eq!(cs.permutation().columns().len(), 12);
        let known = synthesize(&c, 16, Some(&public)).unwrap();
        let unknown = synthesize(&c.without_witnesses(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(
            known.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
        for i in 0..10 {
            let mut wrong = public.clone();
            wrong[0][i] += Fq::ONE;
            assert!(!satisfied(&c, &wrong), "public slot{i} fixed{fixed}");
        }
        if fixed {
            let mut other = w;
            other.key[0][0] ^= 1;
            let forged = QSignatureCircuit::new(c.plan().clone(), vec![other]).unwrap();
            assert!(!satisfied(&forged, &forged.instances(&[true]).unwrap()));
        }
        let flags = known.tables.advice_assigned();
        let rows = flags
            .iter()
            .filter_map(|col| col.iter().rposition(|v| *v))
            .max()
            .unwrap()
            + 1;
        let cells = flags.iter().flatten().filter(|v| **v).count();
        eprintln!("signature fixed{fixed}: rows{rows} cells{cells}");
    }
}
#[test]
fn signature_soft_raw_256_bit_boundaries_and_consistent_aliases() {
    let original = witness();
    let mut cases = vec![original];
    for component in 0..4 {
        for boundary in [
            [0; 4],
            [u64::MAX; 4],
            if component < 2 { P } else { N },
            [0, 0, 0, 1 << 63],
        ] {
            let mut altered = original;
            if component < 2 {
                altered.key[component] = boundary;
            } else {
                altered.signature[component - 2] = boundary;
            }
            cases.push(altered);
        }
    }
    let mut high_s = original;
    high_s.signature[1] = ORDER.neg(&high_s.signature[1]);
    cases.push(high_s);
    let mut off_curve = original;
    off_curve.key[1][0] ^= 1;
    cases.push(off_curve);
    let mut wrong_digest = original;
    wrong_digest.digest += iroha_pasta::Fp::ONE;
    cases.push(wrong_digest);
    for (i, w) in cases.into_iter().enumerate() {
        let verdict = oracle(&w);
        let c = circuit(w, VerifyMode::Soft, false);
        assert!(satisfied(&c, &c.instances(&[verdict]).unwrap()), "case{i}");
        assert!(
            !satisfied(&c, &c.instances(&[!verdict]).unwrap()),
            "forged verdict{i}"
        );
    }
    // A raw integer changed by the native Fq modulus has the same field
    // residue, but distinct exported 128-bit halves and must not alias.
    let modulus = Nat::from_words(iroha_plonk_gadgets::ff::ForeignModulus::PASTA_FQ.words());
    let mut a = original;
    a.signature[0] = [7, 0, 0, 0];
    let mut b = a;
    b.signature[0] = modulus.wrapping_add(&Nat::from_u64(7)).low_words();
    let a = circuit(a, VerifyMode::Soft, false);
    let b = circuit(b, VerifyMode::Soft, false);
    assert!(!satisfied(&b, &a.instances(&[false]).unwrap()));
    let mut bad = original;
    bad.signature[1] = [0; 4];
    let hard = circuit(bad, VerifyMode::Hard, false);
    assert!(!satisfied(&hard, &hard.instances(&[false]).unwrap()));
}
#[test]
fn signature_five_variable_one_fixed_fits_shared_k16() {
    let w = witness();
    let mut slots = vec![
        SignatureSlot {
            mode: VerifyMode::Soft,
            key: SignatureKey::Variable
        };
        5
    ];
    slots.push(SignatureSlot {
        mode: VerifyMode::Hard,
        key: SignatureKey::Fixed(Affine {
            x: w.key[0],
            y: w.key[1],
        }),
    });
    let c = QSignatureCircuit::new(QSignaturePlan::new(slots).unwrap(), vec![w; 6]).unwrap();
    let instances = c.instances(&[true; 6]).unwrap();
    assert!(satisfied(&c, &instances));
    let s = synthesize(&c, 16, Some(&instances)).unwrap();
    let flags = s.tables.advice_assigned();
    let rows = flags
        .iter()
        .filter_map(|col| col.iter().rposition(|v| *v))
        .max()
        .unwrap()
        + 1;
    let cells = flags.iter().flatten().filter(|v| **v).count();
    eprintln!("signature5V1F: rows{rows} cells{cells}");
    assert!(rows <= 65_530);
}
#[test]
#[ignore = "optimized actual native keygen/proof qualification component"]
fn signature_actual_native_proof_binds_inputs_and_retains_opening() {
    let c = circuit(witness(), VerifyMode::Hard, false);
    let public = c.instances(&[true]).unwrap();
    let params = iroha_plonk::pcs::ipa::PinnedParams::<iroha_pasta::Ep>::derive(16).unwrap();
    let mut config = KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec());
    config.compress_selectors = false;
    let key = keygen_pk_v2(&params, &c, &config).unwrap();
    let proof = create_proof_owned_with_claim(
        &params,
        &key,
        Witness::from_circuit(&key, &c, &public).unwrap(),
        common::recovery(119),
        ProverConfig::default(),
    )
    .unwrap();
    let claim = accumulate_generator(
        &params,
        key.binding(),
        key.vk(),
        &public,
        &proof.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    claim.decide(&params, MemoryBudget::DEFAULT).unwrap();
    for index in [0, 1, 5, 9] {
        let mut wrong = public.clone();
        wrong[0][index] += Fq::ONE;
        assert!(
            accumulate_generator(
                &params,
                key.binding(),
                key.vk(),
                &wrong,
                &proof.proof,
                MemoryBudget::DEFAULT
            )
            .is_err()
        );
    }
    eprintln!("signature native proof{}B", proof.proof.len());
}
