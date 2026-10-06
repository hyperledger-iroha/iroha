//! Archive schema/dispatch metadata tests; no fixture key is admitted.

use super::*;
use crate::{
    a_relation::{AProofPlan, QProofPlan, own::OwnPolicy},
    q_sigma::{QSigmaPlan, SigmaClass},
    q_signature::QSignaturePlan,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaField};
use iroha_plonk::{
    cs::{ConstraintSystem, InstanceType},
    frontend::{Circuit, Layouter, SimpleFloorPlanner},
    keys::{KeygenConfigV2, keygen_vk_with_binding_v2},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::p256::native::Affine;
use iroha_plonk_recursion::verifier::VerifierPlan;
use stage::ArchiveStagePlan;
use std::marker::PhantomData;

// This constructor-only fixture declares exact instance columns. Its keys and
// nonexistent proofs never authenticate operation execution or wallet artifacts.
#[derive(Clone)]
struct Metadata<F>(Vec<usize>, PhantomData<F>);
impl<F: PastaField> Circuit<F> for Metadata<F> {
    type Config = ();
    type Params = Vec<usize>;
    type FloorPlanner = SimpleFloorPlanner;
    fn params(&self) -> Self::Params {
        self.0.clone()
    }
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<F>) {
        Self::configure_with_params(meta, vec![1]);
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, lengths: Vec<usize>) {
        for n in lengths {
            meta.instance_column(n);
        }
    }
    fn synthesize(&self, (): (), _: impl Layouter<F>) -> Result<(), Error> {
        Ok(())
    }
}

fn tasks() -> Vec<Vec<OperationTask>> {
    use OperationTask::*;
    vec![
        vec![ArchiveRetainedPayment, ArchiveProofs, ArchiveOwnProof],
        vec![ArchiveAuthorization],
        vec![ArchiveSignatures],
        vec![ArchiveEvidence],
        vec![ArchiveEffects],
    ]
}
fn operation(variant: Variant) -> (AProofPlan, OwnPolicy) {
    let policy = OwnPolicy::new([1, 2], [31, 32], Affine::GENERATOR).unwrap();
    let p = PinnedParams::<Ep>::derive(16).unwrap();
    let v = PinnedParams::<Eq>::derive(16).unwrap();
    let leaf_params = PinnedParams::<Eq>::derive(12).unwrap();
    let (binding, key) = keygen_vk_with_binding_v2(
        &leaf_params,
        &Metadata::<Fp>(vec![1], PhantomData),
        &KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]),
    )
    .unwrap();
    let digest = key.kagemusha_digest(&binding).unwrap();
    let leaf = VerifierPlan::new(binding, leaf_params).unwrap();
    let own = SigmaClass::new(leaf, vec![(12, digest)]).unwrap();
    let incoming = (variant == Variant::ArchiveReceive).then(|| {
        // Distinct selector indices require distinct authenticated key digests.
        // A different constructor-only domain supplies that distinction without
        // weakening the production selector/digest bijection.
        let params = PinnedParams::<Eq>::derive(14).unwrap();
        let (binding, key) = keygen_vk_with_binding_v2(
            &params,
            &Metadata::<Fp>(vec![1], PhantomData),
            &KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]),
        )
        .unwrap();
        let digest = key.kagemusha_digest(&binding).unwrap();
        SigmaClass::new(
            VerifierPlan::new(binding, params).unwrap(),
            vec![(10, digest)],
        )
        .unwrap()
    });
    let sigma = QSigmaPlan::new(own, incoming, &v).unwrap();
    let schemas = authorization::ArchiveAuthorizationObjects::signature_schemas(policy).unwrap();
    let mut q = Vec::new();
    for (lengths, types) in std::iter::once((
        sigma.instance_lengths().to_vec(),
        QSigmaPlan::instance_types().to_vec(),
    ))
    .chain(schemas.iter().map(|s| {
        (
            vec![s.instance_length()],
            QSignaturePlan::instance_types().to_vec(),
        )
    })) {
        let (binding, key) = keygen_vk_with_binding_v2(
            &p,
            &Metadata::<Fq>(lengths, PhantomData),
            &KeygenConfigV2::pipa_r(types),
        )
        .unwrap();
        q.push(QProofPlan::new(VerifierPlan::new(binding, p.clone()).unwrap(), key).unwrap());
    }
    let (binding, _) = keygen_vk_with_binding_v2(
        &p,
        &Metadata::<Fq>(vec![1, 2, 16], PhantomData),
        &KeygenConfigV2::pipa_r(vec![
            InstanceType::Bounded,
            InstanceType::Field,
            InstanceType::Bounded,
        ]),
    )
    .unwrap();
    let omega = VerifierPlan::new(binding, p.clone()).unwrap();
    (
        AProofPlan::new(variant, sigma, q, Some(omega), &p).unwrap(),
        policy,
    )
}

#[test]
fn archive_stage_schema_requires_all_original_categories_results_and_q_owners() {
    for variant in [Variant::ArchiveReceive, Variant::ArchiveStatus] {
        let (operation, policy) = operation(variant);
        let specs = ArchiveStagePlan::context_specs(variant, 5120, 3456, 5120).unwrap();
        let partition = vec![vec![0], vec![1], vec![2], vec![], vec![]];
        let make = |operation, partition, specs, groups| {
            ContextPlan::with_schedule(operation, partition, Some(0), specs)
                .and_then(|p| p.with_operation_tasks(groups))
                .and_then(|p| ArchiveStagePlan::new(p, policy))
        };
        let plan = make(operation.clone(), partition.clone(), specs.clone(), tasks()).unwrap();
        assert_eq!(plan.context().stage_count(), 5);
        assert_eq!(
            plan.context().object_specs().len(),
            if variant == Variant::ArchiveReceive {
                17
            } else {
                19
            }
        );
        assert_eq!(plan.results().spec(), *specs.last().unwrap());
        assert!(plan.signature_schema(0).is_none());
        assert!(plan.signature_schema(3).is_none());
        assert_eq!(plan.signature_schema(1).unwrap().slots().len(), 3);
        assert_eq!(plan.signature_schema(2).unwrap().slots().len(), 1);
        for i in 0..specs.len() {
            if [9, 10, 13].contains(&i) {
                continue;
            }
            let mut changed = specs.clone();
            changed[i].capacity += 1;
            assert!(
                make(operation.clone(), partition.clone(), changed, tasks()).is_err(),
                "capacity{i}"
            );
        }
        for index in 0..specs.len() {
            let mut changed = specs.clone();
            changed.remove(index);
            assert!(
                make(operation.clone(), partition.clone(), changed, tasks()).is_err(),
                "missing source{index}"
            );
        }
        for task in OperationTask::required(variant) {
            for duplicate in [false, true] {
                let mut changed = tasks();
                if duplicate {
                    changed[4].push(*task);
                    changed[4].sort_unstable();
                } else {
                    for group in &mut changed {
                        group.retain(|t| t != task);
                    }
                }
                assert!(
                    make(operation.clone(), partition.clone(), specs.clone(), changed).is_err(),
                    "task{task:?} duplicate{duplicate}"
                );
            }
        }
        for changed in [
            vec![vec![0], vec![2], vec![1], vec![], vec![]],
            vec![vec![0], vec![1], vec![], vec![2], vec![]],
            vec![vec![0], vec![1], vec![2], vec![], vec![2]],
        ] {
            assert!(make(operation.clone(), changed, specs.clone(), tasks()).is_err());
        }
        let p = PinnedParams::<Ep>::derive(16).unwrap();
        let wrong = AProofPlan::new(
            variant,
            operation.sigma.clone(),
            vec![
                operation.q(0).unwrap().clone(),
                operation.q(2).unwrap().clone(),
                operation.q(1).unwrap().clone(),
            ],
            operation.omega().cloned(),
            &p,
        )
        .unwrap();
        assert!(make(wrong, partition, specs, tasks()).is_err());
    }
    for variant in Variant::ALL {
        assert_eq!(
            ArchiveStagePlan::context_specs(variant, 1, 1, 1).is_ok(),
            matches!(variant, Variant::ArchiveReceive | Variant::ArchiveStatus)
        );
    }
    for sizes in [[0, 1, 1], [1, 0, 1], [1, 1, 0], [1, 1, usize::MAX]] {
        assert!(
            ArchiveStagePlan::context_specs(Variant::ArchiveStatus, sizes[0], sizes[1], sizes[2])
                .is_err()
        );
    }
}
