//! Fixed Receive task/Q/object metadata, without admitting fixture lineage keys.

use super::*;
use crate::{
    a_relation::{AProofPlan, QProofPlan, receive::tests::recursive_program_fixture},
    q_signature::{QSignatureCircuit, SignatureWitness},
};
use ff::Field;
use iroha_plonk::{
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::p256::native::Affine;
use iroha_plonk_recursion::verifier::VerifierPlan;

fn tasks() -> Vec<Vec<OperationTask>> {
    use OperationTask::*;
    vec![
        vec![ReceiveProofs, ReceiveOwnProof, ReceiveProofDigest],
        vec![ReceiveAuthorization],
        vec![
            ReceiveObjects,
            ReceiveSignatures,
            ReceiveNonmembership,
            ReceiveBlacklist,
        ],
        vec![ReceiveEffects],
    ]
}

#[test]
fn stage_plan_pins_all_tapes_signature_slots_and_q_owners() {
    let (base, policy) = recursive_program_fixture();
    assert_eq!(base.q_count(), 1, "unused sigma key is a metadata fixture");
    let params = PinnedParams::<Ep>::derive(16).unwrap();
    for variant in [Variant::Receive, Variant::ReceiveRenewed] {
        let schemas = ReceiveStagePlan::signature_schemas(variant, policy).unwrap();
        let keys = schemas
            .iter()
            .map(|schema| {
                let placeholder = SignatureWitness {
                    digest: Fp::ZERO,
                    key: [Affine::GENERATOR.x, Affine::GENERATOR.y],
                    signature: [[0; 4]; 2],
                };
                // Key generation fixes the real signature relation without making
                // any claim that these unused placeholder signatures are valid.
                let circuit =
                    QSignatureCircuit::new(schema.clone(), vec![placeholder; schema.slots().len()])
                        .unwrap();
                let key = keygen_pk_v2(
                    &params,
                    &circuit,
                    &KeygenConfigV2::pipa_r(QSignaturePlan::instance_types().to_vec()),
                )
                .unwrap();
                QProofPlan::new(
                    VerifierPlan::new(key.binding().clone(), params.clone()).unwrap(),
                    key.vk().clone(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let operation = AProofPlan::new(
            variant,
            base.sigma.clone(),
            vec![base.q(0).unwrap().clone(), keys[0].clone(), keys[1].clone()],
            base.omega().cloned(),
            &params,
        )
        .unwrap();
        let specs = ReceiveStagePlan::context_specs(variant, 8132, 10000).unwrap();
        let context = |operation: AProofPlan, partition, specs| {
            ContextPlan::with_schedule(operation, partition, Some(0), specs)
                .unwrap()
                .with_operation_tasks(tasks())
                .unwrap()
        };
        let partition = vec![vec![0], vec![1], vec![2], vec![]];
        let honest = context(operation.clone(), partition.clone(), specs.clone());
        for duplicate in [false, true] {
            let mut incomplete = tasks();
            if duplicate {
                incomplete[1].push(OperationTask::ReceiveProofDigest);
            } else {
                incomplete[0].retain(|task| *task != OperationTask::ReceiveProofDigest);
            }
            assert!(
                ContextPlan::with_schedule(
                    operation.clone(),
                    partition.clone(),
                    Some(0),
                    specs.clone()
                )
                .unwrap()
                .with_operation_tasks(incomplete)
                .is_err(),
                "the hard digest owner must occur exactly once before terminal admission",
            );
        }
        let plan = ReceiveStagePlan::new(honest, policy).unwrap();
        assert_eq!(plan.context().stage_count(), 4);
        assert!(plan.signature_schema(0).is_none());
        assert!(plan.signature_schema(3).is_none());
        assert_eq!(plan.signature_schema(1).unwrap().slots().len(), 3);
        assert_eq!(
            plan.signature_schema(2).unwrap().slots().len(),
            if variant == Variant::Receive { 2 } else { 4 }
        );
        assert_eq!(plan.context().object_specs().len(), 11);
        for mutation in 0..4 {
            let mut wrong = specs.clone();
            match mutation {
                0 => wrong[1].capacity += 1,
                1 => wrong.swap(0, 1),
                2 => {
                    wrong.pop();
                }
                _ => wrong[10].capacity -= 1,
            }
            assert!(
                ReceiveStagePlan::new(context(operation.clone(), partition.clone(), wrong), policy)
                    .is_err(),
                "object schema {mutation}"
            );
        }
        let wrong_owner = context(
            operation.clone(),
            vec![vec![0], vec![2], vec![1], vec![]],
            specs.clone(),
        );
        assert!(ReceiveStagePlan::new(wrong_owner, policy).is_err());
        let wrong_slots = AProofPlan::new(
            variant,
            base.sigma.clone(),
            vec![base.q(0).unwrap().clone(), keys[1].clone(), keys[0].clone()],
            base.omega().cloned(),
            &params,
        )
        .unwrap();
        assert!(ReceiveStagePlan::new(context(wrong_slots, partition, specs), policy).is_err());
    }
}
