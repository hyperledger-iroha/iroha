//! Genuine Archive sigma, hard2V1F/soft1V Q originals and oversized predecessor rejection.
//!
//! The local leaf fixture supplies no pending-map or acknowledgement authority.
//! Complete native Archive A/W positives require authentic retained Payment/evidence
//! and compact installed Omega artifacts. No fixture head is admitted here.

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
#[path = "bootstrap_omega.rs"]
mod bootstrap_outer;
mod common;

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    a_relation::{
        AProofPlan, QProofPlan, native::archive, own::OwnPolicy, schedule::sigma_selector,
    },
    admin_sigma::{ArchiveCircuit, ArchiveWitness, StateWitness},
    operation_relation::objects::ObjectKind,
    q_sigma::{QSigmaPlan, SigmaClass, SigmaSlotWitness, native::QSigmaProver},
    q_signature::{QSignatureCircuit, QSignaturePlan},
    witness::{CORE_DOMAIN, REST_DOMAIN, core_index as core},
};
use iroha_pasta::{Ep, Eq, Fp, Fq, msm::MemoryBudget, poseidon::hash_with_domain};
use iroha_plonk::{
    ProverConfig, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    frontend::{Circuit, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::{bytes::p_bytes_native, statement::STATEMENT_DOMAIN};
use iroha_plonk_recursion::{FoldConfig, obligation::ledger::Variant, verifier::VerifierPlan};

fn rebind(w: &mut ArchiveWitness) {
    for state in [&mut w.predecessor, &mut w.successor] {
        let mut preimage = state.core.to_vec();
        preimage.push(hash_with_domain(REST_DOMAIN, &state.rest));
        state.lineage[5] = hash_with_domain(CORE_DOMAIN, &preimage);
        state.lineage[1..3].copy_from_slice(&state.core[core::SCHEME..core::SCHEME + 2]);
        state.lineage[6..8].copy_from_slice(&state.core[core::WALLET..core::WALLET + 2]);
        state.lineage[8] = state.core[core::CREDENTIAL];
        state.lineage[13] = state.core[core::LIFECYCLE]
            + Fp::from(256) * state.core[core::POLICY_EPOCH]
            + Fp::from_u128(1 << 72) * state.core[core::ENABLED_CONTROLS];
    }
    w.statement[3..7].copy_from_slice(&w.successor.core[core::SCHEME..core::ASSET + 2]);
    for (field, index) in [
        (7, core::CREDENTIAL),
        (8, core::LIFECYCLE),
        (9, core::SEQUENCE),
        (10, core::NEXT_LOAD),
    ] {
        w.statement[field] = w.successor.core[index];
    }
    w.statement[12] = Fp::ZERO;
    w.statement[13] = Fp::ZERO;
    w.statement[14] = w.predecessor.lineage[5];
    w.statement[15] = w.successor.lineage[5];
}

fn witness() -> ArchiveWitness {
    let initial = bootstrap_objects::enrollment().0;
    let predecessor = StateWitness::from(&initial);
    let mut successor = predecessor;
    successor.core[core::SEQUENCE] += Fp::ONE;
    successor.core[core::STATE_NONCE] += Fp::ONE;
    successor.core[core::PENDING_OUTGOING_ROOT] = Fp::from(99);
    let mut statement = initial.statement;
    statement[17..].fill(Fp::ZERO);
    statement[16] = Fp::from(5);
    statement[17] = Fp::from(43);
    statement[18] = Fp::from(44);
    let mut w = ArchiveWitness {
        predecessor,
        successor,
        statement,
    };
    rebind(&mut w);
    w
}

fn frame(raw: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(raw.len()).unwrap().to_le_bytes().to_vec();
    out.extend(raw);
    out
}

#[test]
#[ignore = "real k12 Archive/Q_sigma/hard2V1F/soft1V Q and actual oversized Bootstrap descriptor rejection; run optimized"]
fn genuine_archive_status_sources_reject_an_oversized_predecessor() {
    let budget = MemoryBudget::DEFAULT;
    let witness = witness();
    let leaf = ArchiveCircuit::new(&witness);
    let public = leaf.instances();
    assert!(
        check_circuit(&leaf, 12, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let known = synthesize(&leaf, 12, Some(&public)).unwrap();
    let unknown = synthesize(&leaf.without_witnesses(), 12, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
    let leaf_params = PinnedParams::<Eq>::derive(12).unwrap();
    let leaf_key = keygen_pk_v2(
        &leaf_params,
        &leaf,
        &KeygenConfigV2::pipa_r(ArchiveCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let leaf_output = create_proof_owned_with_claim(
        &leaf_params,
        &leaf_key,
        Witness::from_circuit(&leaf_key, &leaf, &public).unwrap(),
        common::recovery(221),
        ProverConfig::default(),
    )
    .unwrap();
    verify_full(
        &leaf_params,
        leaf_key.binding(),
        leaf_key.vk(),
        &public,
        &leaf_output.proof,
        budget,
    )
    .unwrap();
    leaf_output.opening.decide(&leaf_params, budget).unwrap();
    assert!(leaf_output.proof.len() <= 3_456);
    for mutation in 0..2 {
        let mut instances = public.clone();
        let mut proof = leaf_output.proof.clone();
        if mutation == 0 {
            instances[0][0] += Fp::ONE;
        } else {
            proof[0] ^= 1;
        }
        assert!(
            verify_full(
                &leaf_params,
                leaf_key.binding(),
                leaf_key.vk(),
                &instances,
                &proof,
                budget
            )
            .is_err()
        );
    }
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    let vparams = PinnedParams::<Eq>::derive(16).unwrap();
    let selector = sigma_selector(5, 0).unwrap();
    assert_eq!(selector, 12);
    let sigma_plan = QSigmaPlan::new(
        SigmaClass::new(
            VerifierPlan::new(leaf_key.binding().clone(), leaf_params).unwrap(),
            vec![(
                selector,
                leaf_key.vk().kagemusha_digest(leaf_key.binding()).unwrap(),
            )],
        )
        .unwrap(),
        None,
        &vparams,
    )
    .unwrap();
    let prepared = sigma_plan
        .prepare(
            SigmaSlotWitness {
                key: leaf_key.vk().clone(),
                statement: public[0][0],
                proof: leaf_output.proof.clone(),
                length: leaf_output.proof.len().try_into().unwrap(),
            },
            None,
            &vparams,
            Fq::from(131),
            &FoldConfig::default(),
        )
        .unwrap();
    let q = QSigmaProver::keygen_serialized_foreign(&prepared, params.clone(), 4).unwrap();
    let qproof = q
        .prove(&prepared, common::recovery(222), ProverConfig::default())
        .unwrap();
    verify_full(
        &params,
        q.binding(),
        q.verifying_key(),
        &qproof.instances,
        &qproof.bytes,
        budget,
    )
    .unwrap();
    let qopening = accumulate_generator(
        &params,
        q.binding(),
        q.verifying_key(),
        &qproof.instances,
        &qproof.bytes,
        budget,
    )
    .unwrap();
    qopening.decide(&params, budget).unwrap();
    qproof.part.decide(&vparams, budget).unwrap();
    let mut wrong_selector = qproof.instances.clone();
    wrong_selector[2][0] = Fq::from(15);
    assert!(
        verify_full(
            &params,
            q.binding(),
            q.verifying_key(),
            &wrong_selector,
            &qproof.bytes,
            budget
        )
        .is_err()
    );
    let rooted = bootstrap_outer::rooted_bootstrap_omega(false);
    verify_full(
        &params,
        &rooted.binding,
        &rooted.key,
        &rooted.instances,
        &rooted.proof,
        budget,
    )
    .unwrap();
    rooted.source.pallas.decide(&params, budget).unwrap();
    rooted.vesta.decide(&vparams, budget).unwrap();
    assert!(rooted.proof.len() + 1_088 + 320 > 10_000);
    // This genuine Bootstrap source does not prove an outstanding Send descriptor.
    // This original is retained solely to prove the actual descriptor-size gate
    // rejects a real outer proof instead of fabricating a compact replacement.
    let (initial, certificate, credential) = bootstrap_objects::enrollment();
    assert_eq!(
        initial.core[core::CREDENTIAL],
        witness.predecessor.core[core::CREDENTIAL]
    );
    // Exact genuine original public320 || proof || both full claims, following
    // the canonical consuming lineage transcript. It is not an Archive pending head.
    let f = rooted.source.state.lineage;
    let mut omega = 1_u16.to_le_bytes().to_vec();
    for i in [1, 2, 3, 4] {
        omega.extend(&f[i].to_repr()[..16]);
    }
    omega.extend(f[5].to_repr());
    for i in [6, 7] {
        omega.extend(&f[i].to_repr()[..16]);
    }
    omega.extend(f[8].to_repr());
    omega.push(4);
    for i in [10, 9, 12, 11] {
        omega.extend(f[i].to_repr()[..16].iter().rev());
    }
    omega.extend(&f[13].to_repr()[..13]);
    omega.extend(&f[14].to_repr()[..16]);
    omega.extend(f[15].to_repr());
    omega.extend(f[16].to_repr());
    assert_eq!(omega.len(), 320);
    omega.extend(&rooted.proof);
    omega.extend(rooted.source.pallas.to_bytes());
    omega.extend(rooted.vesta.to_bytes());
    // The local receipt/signature-Q fixture uses Archive's genuine sigma-only
    // digest domain. It does not establish a retained descriptor or acknowledgement.
    let proof_digest = p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &frame(&leaf_output.proof));
    let mut body = 1_u16.to_le_bytes().to_vec();
    body.extend(bootstrap_objects::id(
        witness.successor.core[1],
        witness.successor.core[2],
    ));
    body.extend(bootstrap_objects::id(
        witness.successor.core[5],
        witness.successor.core[6],
    ));
    body.extend(bootstrap_objects::small_id(31, 32));
    body.extend(&witness.statement[9].to_repr()[..16]);
    let operation = hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[
            witness.successor.core[5],
            witness.successor.core[6],
            Fp::from(5),
            witness.statement[18],
        ],
    );
    for value in [
        operation,
        witness.statement[14],
        witness.statement[15],
        hash_with_domain(STATEMENT_DOMAIN, &witness.statement),
        proof_digest,
    ] {
        body.extend(value.to_repr());
    }
    body.extend(bootstrap_objects::small_id(101, 102));
    body.extend(Fp::ZERO.to_repr());
    let receipt = bootstrap_objects::sign(ObjectKind::Receipt, body, 29, 59);
    let policy = OwnPolicy::new([1, 2], [31, 32], bootstrap_objects::key(23)).unwrap();
    let schemas = archive::Plan::signature_schemas(policy).unwrap();
    let schema = schemas[0].clone();
    let signatures = QSignatureCircuit::new(
        schema.clone(),
        vec![
            receipt.signature,
            credential.signature,
            certificate.signature,
        ],
    )
    .unwrap();
    let instances = signatures.instances(&[true; 3]).unwrap();
    let signature_key = keygen_pk_v2(
        &params,
        &signatures,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    let signature_output = create_proof_owned_with_claim(
        &params,
        &signature_key,
        Witness::from_circuit(&signature_key, &signatures, &instances).unwrap(),
        common::recovery(223),
        ProverConfig::default(),
    )
    .unwrap();
    verify_full(
        &params,
        signature_key.binding(),
        signature_key.vk(),
        &instances,
        &signature_output.proof,
        budget,
    )
    .unwrap();
    signature_output.opening.decide(&params, budget).unwrap();
    let mut changed = signature_output.proof.clone();
    changed[0] ^= 1;
    assert!(
        verify_full(
            &params,
            signature_key.binding(),
            signature_key.vk(),
            &instances,
            &changed,
            budget
        )
        .is_err()
    );
    // A distinct real soft receipt Q is mandatory even though this fixed artifact
    // size rejection happens before any Archive input or map witness is admitted.
    let incoming = QSignatureCircuit::new(schemas[1].clone(), vec![receipt.signature]).unwrap();
    let incoming_instances = incoming.instances(&[true]).unwrap();
    let incoming_key = keygen_pk_v2(
        &params,
        &incoming,
        &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
    )
    .unwrap();
    let incoming_output = create_proof_owned_with_claim(
        &params,
        &incoming_key,
        Witness::from_circuit(&incoming_key, &incoming, &incoming_instances).unwrap(),
        common::recovery(224),
        ProverConfig::default(),
    )
    .unwrap();
    verify_full(
        &params,
        incoming_key.binding(),
        incoming_key.vk(),
        &incoming_instances,
        &incoming_output.proof,
        budget,
    )
    .unwrap();
    incoming_output.opening.decide(&params, budget).unwrap();
    let operation = AProofPlan::new(
        Variant::ArchiveStatus,
        sigma_plan,
        vec![
            QProofPlan::new(
                VerifierPlan::new(q.binding().clone(), params.clone()).unwrap(),
                q.verifying_key().clone(),
            )
            .unwrap(),
            QProofPlan::new(
                VerifierPlan::new(signature_key.binding().clone(), params.clone()).unwrap(),
                signature_key.vk().clone(),
            )
            .unwrap(),
            QProofPlan::new(
                VerifierPlan::new(incoming_key.binding().clone(), params.clone()).unwrap(),
                incoming_key.vk().clone(),
            )
            .unwrap(),
        ],
        Some(VerifierPlan::new(rooted.binding.clone(), params.clone()).unwrap()),
        &params,
    )
    .unwrap();
    assert!(matches!(
        archive::Plan::new(
            operation, false, policy, schemas, rooted.key, 10_000, 3_456, params, vparams
        ),
        Err(archive::Error::Artifact)
    ));
}
