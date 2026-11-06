//! Unchanged Q/A receipt-digest continuation subgate over a genuine funded predecessor.
//!
//! This uses the existing component catalog. It deliberately does not claim the
//! production phone's complete catalog, a Receive suffix, or final Omega qualification.
//! The installed-profile suffix gate additionally requires all of those checks.

use super::*;
use iroha_kagemusha_proof::{
    a_relation::native::consuming as native, operation_relation::objects::ObjectKind,
    q_signature::QSignaturePlan,
};
use iroha_plonk::{
    cs::InstanceType,
    keys::{CosetCachePolicy, pk::artifact::ReadConfig},
};
use iroha_plonk_gadgets::bytes::p_bytes_native;

const RECEIPT_PROOF_DIGEST_OFFSET: usize = 2 + 3 * 32 + 16 + 4 * 32;

fn inputs(source: &Sources) -> native::Inputs {
    native::Inputs {
        state: source.maps.witness,
        objects: [source.objects[0].bytes.clone()],
        recovery: source.maps.recovery,
        q: [native::QInput {
            proof: source.q_proofs[0].clone(),
            instances: source.q_instances[0].clone(),
        }],
        predecessor: native::PredecessorInput {
            proof: source.predecessor.proof.clone(),
            pallas: source.predecessor.pallas.to_bytes(),
            vesta: source.predecessor.vesta.to_bytes(),
        },
    }
}

fn signed_digest(source: &Sources, digest: Fp) -> bootstrap_objects::Signed {
    assert_ne!(digest, Fp::ZERO);
    let mut body = source.objects[0].bytes[..ObjectKind::Receipt.body_len()].to_vec();
    body[RECEIPT_PROOF_DIGEST_OFFSET..RECEIPT_PROOF_DIGEST_OFFSET + 32]
        .copy_from_slice(&digest.to_repr());
    // This is the existing public component-fixture payment key, not a device key.
    bootstrap_objects::sign(ObjectKind::Receipt, body, 29, 971)
}

fn accepts(plan: &native::Plan, input: native::Inputs) -> bool {
    let Ok(prepared) = plan.prepare(input, MemoryBudget::DEFAULT) else {
        return false;
    };
    let Ok((circuit, public)) =
        prepared.terminal_circuit(common::public_fixture_tape(972), &FoldConfig::default())
    else {
        return false;
    };
    iroha_plonk::check::check_circuit(&circuit, 16, &[public], CheckMode::Strict)
        .is_ok_and(|report| report.is_satisfied())
}

#[test]
#[ignore = "genuine funded component predecessor plus unchanged Q/A suffix-digest proofs; run alone optimized with fresh KAGEMUSHA_LOAD_OUTPUT; not the full installed-phone gate"]
fn distinct_suffix_digest_proves_under_the_same_q_and_a_originals() {
    let (fixture, rebuilds) = super::native_load_tests::first_load_fixture::first_load_fixture(2);
    let rooted = catalog::compact_payer_load(&fixture);
    assert_eq!(rebuilds.load(std::sync::atomic::Ordering::Relaxed), 2);
    let source = sources(&Head::loaded(&rooted), false);
    let original = inputs(&source);
    let params = source.params.clone();
    let vparams = common::vesta_params(16);
    let budget = MemoryBudget::DEFAULT;
    let plan = native::Plan::new(
        source.plan.clone(),
        load_objects::policy(),
        source.signature_schema.clone(),
        source.predecessor.key.clone(),
        params.clone(),
        vparams.clone(),
    )
    .unwrap();

    // Choose a distinct domain-separated carrier commitment before any consuming
    // proof. A complete native suffix-authority codec still needs its own tests;
    // this subgate isolates the existing relation's opaque nonzero digest rule.
    let mut carrier = b"unchanged Q/A suffix authority subgate".to_vec();
    for field in original.state.statement {
        carrier.extend(field.to_repr());
    }
    carrier.extend(&original.predecessor.proof);
    let digest = p_bytes_native(u64::from_le_bytes(*b"kgwsuf_1"), &carrier);
    let receipt = signed_digest(&source, digest);
    assert_ne!(receipt.bytes, original.objects[0]);
    assert_ne!(
        &receipt.bytes[RECEIPT_PROOF_DIGEST_OFFSET..RECEIPT_PROOF_DIGEST_OFFSET + 32],
        &original.objects[0][RECEIPT_PROOF_DIGEST_OFFSET..RECEIPT_PROOF_DIGEST_OFFSET + 32]
    );

    let (q, q_public) = load_objects::own_signature(receipt.signature);
    let q_key = keygen_pk_v2(
        &params,
        &q,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    let old_q = source.plan.q(0).unwrap();
    assert_eq!(q_key.binding(), old_q.verifier().binding());
    let (ordinary_q, _) = load_objects::own_signature(source.objects[0].signature);
    let (ordinary_binding, ordinary_key) = keygen_vk_with_binding_v2(
        &params,
        &ordinary_q,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    assert_eq!(q_key.binding(), &ordinary_binding);
    assert_eq!(q_key.vk().to_bytes(), ordinary_key.to_bytes());
    let q_output = create_proof_owned_with_claim(
        &params,
        &q_key,
        Witness::from_circuit(&q_key, &q, &q_public).unwrap(),
        common::fresh_randomness(),
        ProverConfig::default(),
    )
    .unwrap();
    q_output.opening.decide(&params, budget).unwrap();
    let mut suffix = original.clone();
    suffix.objects[0] = receipt.bytes;
    suffix.q[0] = native::QInput {
        proof: q_output.proof,
        instances: q_public.to_vec(),
    };

    // Install exactly one ordinary consuming original, before preparing the
    // suffix input. No changed digest, statement or source can select a new key.
    let mut key_config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    key_config.compress_selectors = false;
    let a_key = keygen_pk_v2(&vparams, &plan.source_circuit().unwrap(), &key_config).unwrap();
    let artifact = NativeConsumingOriginal::from_key(&a_key);
    let installed =
        native::Prover::from_artifacts(plan.clone(), artifact.metadata().unwrap()).unwrap();
    let seal = installed
        .import_terminal(
            &artifact.pk,
            ReadConfig {
                maximum_bytes: artifact.pk.len(),
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: budget,
            },
        )
        .unwrap();
    let installed_key = installed.bind_terminal(&seal, None).unwrap();
    assert_eq!(installed_key.binding(), a_key.binding());
    assert_eq!(
        installed_key.verifying_key().to_bytes(),
        a_key.vk().to_bytes()
    );
    assert!(accepts(&plan, original.clone()));
    assert!(accepts(&plan, suffix.clone()));
    assert_eq!(
        suffix.state.predecessor.lineage,
        original.state.predecessor.lineage
    );
    assert_eq!(suffix.state.successor.core, original.state.successor.core);
    assert_eq!(suffix.state.successor.rest, original.state.successor.rest);
    assert_eq!(
        suffix.state.successor.lineage,
        original.state.successor.lineage
    );

    let session = installed.prepare(suffix.clone(), budget).unwrap();
    let terminal = session
        .terminal(
            &installed_key,
            common::public_fixture_tape(973),
            &FoldConfig::default(),
            common::fresh_randomness(),
            ProverConfig::default(),
        )
        .unwrap();
    terminal.opening.decide(&vparams, budget).unwrap();
    terminal.pallas.decide(&params, budget).unwrap();
    terminal.predecessor_vesta.decide(&vparams, budget).unwrap();
    let checkpoint = session
        .encode_terminal_checkpoint(&terminal, budget)
        .unwrap();
    let restarted = installed.prepare(suffix.clone(), budget).unwrap();
    let restored = restarted
        .restore_terminal_checkpoint(&checkpoint, budget)
        .unwrap();
    assert_eq!(restored.proof, terminal.proof);
    assert_eq!(restored.instances, terminal.instances);
    assert_eq!(
        restarted
            .encode_terminal_checkpoint(&restored, budget)
            .unwrap(),
        checkpoint
    );
    assert!(
        installed
            .prepare(original, budget)
            .unwrap()
            .restore_terminal_checkpoint(&checkpoint, budget)
            .is_err(),
        "a different, independently valid signed source must not adopt this checkpoint"
    );

    for mutation in 0..6 {
        let mut wrong = suffix.clone();
        match mutation {
            0 => wrong.objects[0][RECEIPT_PROOF_DIGEST_OFFSET] ^= 1,
            1 => wrong.state.statement[19] += Fp::ONE,
            2 => wrong.state.predecessor.lineage[5] += Fp::ONE,
            3 => {
                wrong.state.successor.core
                    [iroha_kagemusha_proof::witness::core_index::LOAD_REDEEM_ROOT] += Fp::ONE
            }
            4 => wrong.recovery.as_mut().unwrap().leaf_siblings[0] += Fp::ONE,
            _ => wrong.q[0].proof[0] ^= 1,
        }
        assert!(
            !accepts(&plan, wrong),
            "suffix digest subgate mutation {mutation}"
        );
    }
    eprintln!(
        "SUFFIX_DIGEST_SUBGATE genuine_funded_predecessor=true unchanged_q_key=true unchanged_a_key=true exact_receipt_checkpoint_replay=true complete_installed_phone_gate=false final_omega=false"
    );
}
