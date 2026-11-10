//! Folding genuine Receive signature leaves (`QSignatureCircuit`, three P-256 +
//! SHA-256 slots: own receipt hard, incoming receipt and Request soft), each with
//! distinct genuine signatures. Heavy: run optimized.

use ff::Field;
use iroha_fold::{
    Accumulator, CommitKey, FoldedRelation,
    decide::{decide, decide_fresh},
    fold::{fold, fresh_instance, verify_fold},
};
use iroha_kagemusha_proof::q_signature::{
    QSignatureCircuit, QSignaturePlan, SignatureKey, SignatureSlot, SignatureWitness,
};
use iroha_pasta::{Ep, Fp, Fq};
use iroha_plonk::{frontend::synthesize, pcs::ipa::PinnedParams};
use iroha_plonk_gadgets::{
    p256::{
        VerifyMode,
        native::{self, Affine, HALF_N, ORDER, verify_prehashed},
    },
    sha256::native::sha256_of_digest,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

const DOMAIN: [u8; 32] = *b"iroha_fold genuine Q1 test leaf!";

/// A genuine `SHA256withECDSA` signature over the 32-byte encoding of `digest`.
fn signature(digest: u64, secret: u64, nonce: u64) -> SignatureWitness {
    let digest = Fp::from(digest);
    let hash = sha256_of_digest(&digest);
    let message = core::array::from_fn(|i| {
        u64::from_be_bytes(hash[(3 - i) * 8..(4 - i) * 8].try_into().unwrap())
    });
    let secret = [
        secret,
        0x1234_5678_9abc_def0,
        0x0fed_cba9_8765_4321,
        0x0123_4567,
    ];
    let nonce = [
        nonce,
        0x0bad_cafe_dead_beef,
        0x1357_9bdf_2468_ace0,
        0x0765_4321,
    ];
    let key = native::mul(&Affine::GENERATOR, &secret).unwrap();
    let r = ORDER.reduce_once(&native::mul(&Affine::GENERATOR, &nonce).unwrap().x);
    let mut s = ORDER.mul(
        &ORDER.inverse(&nonce),
        &ORDER.add(&ORDER.reduce_once(&message), &ORDER.mul(&r, &secret)),
    );
    if native::words_cmp(&s, &HALF_N).is_gt() {
        s = ORDER.neg(&s);
    }
    assert!(verify_prehashed(&message, &r, &s, &key));
    SignatureWitness {
        digest,
        key: [key.x, key.y],
        signature: [r, s],
    }
}

fn plan() -> QSignaturePlan {
    let slot = |mode| SignatureSlot {
        mode,
        key: SignatureKey::Variable,
    };
    QSignaturePlan::new(vec![
        slot(VerifyMode::Hard),
        slot(VerifyMode::Soft),
        slot(VerifyMode::Soft),
    ])
    .unwrap()
}

/// Receive `i`: the wallet's own key signs its receipt and its Request; a payer
/// key signs the incoming receipt.
fn leaf(i: u64) -> QSignatureCircuit {
    let own = 0x5eed_0001;
    let witnesses = vec![
        signature(0x1111_0000 + i, own, 0xa100 + i),
        signature(0x2222_0000 + i, 0x5eed_1000 + i, 0xa200 + i),
        signature(0x3333_0000 + i, own, 0xa300 + i),
    ];
    QSignatureCircuit::new(plan(), witnesses).unwrap()
}

fn fold_leaves(count: u64) {
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let key = CommitKey::lagrange(params.params());
    let layout = synthesize(&leaf(0), 16, None).unwrap();
    let relation = FoldedRelation::from_synthesized(&layout).unwrap();
    assert_eq!(relation.degree(), 4);
    let mut rng = ChaCha20Rng::seed_from_u64(16);
    let fresh = |i: u64, rng: &mut ChaCha20Rng| {
        let circuit = leaf(i);
        let instances = circuit.instances(&[true, true, true]).unwrap();
        let synthesized = synthesize(&circuit, 16, Some(&instances)).unwrap();
        let (advice, public) = relation.stage_witness(&synthesized).unwrap();
        fresh_instance(&relation, &key, &DOMAIN, advice, public, rng).unwrap()
    };
    let (first, mut witness) = fresh(0, &mut rng);
    decide_fresh(&relation, &key, &DOMAIN, &first, &witness).expect("genuine leaf");
    let mut acc = Accumulator::from_fresh(&relation, &DOMAIN, first).unwrap();
    for i in 1..count {
        let (instance, fresh_witness) = fresh(i, &mut rng);
        let (next, next_witness, proof) = fold(
            &relation,
            &DOMAIN,
            &acc,
            &witness,
            &instance,
            &fresh_witness,
        )
        .unwrap();
        assert_eq!(
            verify_fold(&relation, &DOMAIN, &acc, &instance, &proof).unwrap(),
            next
        );
        acc = next;
        witness = next_witness;
    }
    decide(&relation, &key, &acc, &witness).expect("folded genuine leaves decide");
    // A changed cell of the folded witness does not decide.
    witness.columns[0][100] += Fq::ONE;
    assert!(decide(&relation, &key, &acc, &witness).is_err());
}

#[test]
#[ignore = "genuine P-256 leaves; run optimized"]
fn three_genuine_receive_signature_leaves_fold_and_decide() {
    fold_leaves(3);
}

#[test]
#[ignore = "fifty genuine P-256 leaves (F1 exit test); run optimized"]
fn fifty_genuine_receive_signature_leaves_fold_and_decide() {
    fold_leaves(50);
}
