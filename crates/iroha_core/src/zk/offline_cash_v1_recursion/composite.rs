//! Paired recursive aggregate-state circuits.
//!
//! Each parity verifies the predecessor and the normalized GuardBundle in its scalar field, folds
//! the predecessor's carried BGH19 history, and consumes the opposite parity's complete curve
//! equation audit through the dedicated dense MSM machine. Bootstrap preserves the same parser
//! shape but selector-disables only the nonexistent monetary predecessor; GuardBundle authority
//! remains enabled for every operation.

use ff::Field as _;
use halo2_base::{
    AssignedValue,
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt, fe_to_biguint},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::{
        CurveExt as _,
        group::Curve as _,
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    util::arithmetic::{Domain, root_of_unity},
    verifier::plonk::PlonkProtocol,
};

use super::{
    DigestV1, OfflineCashEpAccumulatorV1, OfflineCashEpFoldProofV1, OfflineCashEqAccumulatorV1,
    OfflineCashEqFoldProofV1, OfflineCashGuardBundleRelationWitnessV1, OfflineCashOperationV1,
    OfflineCashPastaParityV1, OfflineCashStateRelationWitnessV1,
    commit_wrapper::{
        COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1, COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1,
        TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1, public_instance as wrapper_public_instance,
    },
    deferred_parent::{
        DeferredLoader, DeferredScalar, OfflineCashDeferredParentOutputV1,
        OfflineCashDeferredParentWitnessV1, accumulator_limb_count, bind_accumulator_limbs,
        constrain_parent_and_history_into_loader_v1, constrain_reciprocal_parent_pass_v1,
        deferred_field_chips_v1, deferred_loader_v1, finalize_deferred_audit_plan_v1,
        load_and_constrain_parent_protocol_if_v1, load_and_constrain_parent_protocol_v1,
        load_native_accumulator, native_parent_protocol_digest_v1,
        offline_cash_protocol_structure_digest_v1, select_accumulator_v1, verify_fold,
        verify_ordinary_proof_v1,
    },
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, OfflineCashAssignedGuardBundleV1, assign_bytes,
        constant_bytes, constrain_guard_bundle_semantics_v1, digest_limbs_assigned, hash,
    },
    mint_authority::{
        OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
        public_instance as mint_public_instance,
    },
    state_relation::{self, public_instance},
};
use crate::zk::{
    offline_cash_v1_poseidon::OfflineCashPoseidonFieldV1,
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_PASTA_STATE_COMMITMENT_DOMAIN_V1, OfflineCashPastaStateCommitmentV1,
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const PARENT_EQUATION_TAG: u32 = 1;
const INCOMING_CREDIT_EQUATION_TAG: u32 = 2;
const GUARD_BUNDLE_EQUATION_TAG: u32 = 3;
const MINT_FINALITY_EQUATION_TAG: u32 = 4;
const RECEIVE_BATCH_BINDING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:receive-fold-batch\0";
const SUITE_UPGRADE_BRIDGE_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:suite-upgrade-bridge\0";

/// Exact old-to-new recursive verifier bridge authorized for one `SuiteUpgrade`.
///
/// The governance proof digest is authenticated by the paired GuardBundle helper. These values
/// are private candidate witnesses; only their common bridge digest is carried by the candidate
/// proof and subsequently hidden by the terminal wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashSuiteUpgradeBridgeWitnessV1 {
    pub(crate) old_eq_protocol_digest: DigestV1,
    pub(crate) old_ep_protocol_digest: DigestV1,
    pub(crate) old_suite_id: DigestV1,
    pub(crate) old_vk_digest: DigestV1,
    pub(crate) new_suite_id: DigestV1,
    pub(crate) new_vk_digest: DigestV1,
    pub(crate) governance_authorization_proof_digest: DigestV1,
}

impl OfflineCashSuiteUpgradeBridgeWitnessV1 {
    /// Canonical inactive padding carried by every non-upgrade transition.
    pub(crate) const ZERO: Self = Self {
        old_eq_protocol_digest: [0; 32],
        old_ep_protocol_digest: [0; 32],
        old_suite_id: [0; 32],
        old_vk_digest: [0; 32],
        new_suite_id: [0; 32],
        new_vk_digest: [0; 32],
        governance_authorization_proof_digest: [0; 32],
    };

    pub(crate) fn canonical_digest(self) -> DigestV1 {
        let mut hasher = Sha256::new();
        hasher.update(SUITE_UPGRADE_BRIDGE_DOMAIN_V1);
        for digest in [
            self.old_eq_protocol_digest,
            self.old_ep_protocol_digest,
            self.old_suite_id,
            self.old_vk_digest,
            self.new_suite_id,
            self.new_vk_digest,
            self.governance_authorization_proof_digest,
        ] {
            hasher.update(digest);
        }
        hasher.finalize().into()
    }

    fn validate_against(&self, state: &OfflineCashStateRelationWitnessV1) -> Result<(), String> {
        let is_upgrade = state.operation == OfflineCashOperationV1::SuiteUpgrade;
        if !is_upgrade {
            return (*self == Self::ZERO).then_some(()).ok_or_else(|| {
                "suite-upgrade bridge must be canonical zero outside upgrade".to_owned()
            });
        }
        let predecessor = state
            .predecessor
            .as_ref()
            .ok_or_else(|| "suite-upgrade bridge predecessor is absent".to_owned())?;
        if [
            self.old_eq_protocol_digest,
            self.old_ep_protocol_digest,
            self.old_suite_id,
            self.old_vk_digest,
            self.new_suite_id,
            self.new_vk_digest,
            self.governance_authorization_proof_digest,
        ]
        .contains(&[0; 32])
            || self.old_eq_protocol_digest == self.old_ep_protocol_digest
            || self.old_suite_id != predecessor.suite_id
            || self.old_vk_digest != predecessor.vk_digest
            || self.new_suite_id != state.successor.suite_id
            || self.new_vk_digest != state.successor.vk_digest
            || self.canonical_digest() != state.suite_upgrade_authorization_digest
        {
            return Err("suite-upgrade bridge authorization mismatch".to_owned());
        }
        Ok(())
    }
}

/// One Eq/Fp incoming sender proof slot consumed by `ReceiveFoldBatch`.
///
/// Inactive positions carry the release-pinned valid padding proof and history; only their
/// semantic slot data are canonical zero. This keeps proof verification shape fixed.
pub(super) struct OfflineCashRecursiveIncomingEqWitnessV1<'a> {
    pub(super) instances: &'a [Vec<Fp>],
    pub(super) proof: &'a [u8],
    pub(super) history: &'a OfflineCashEqAccumulatorV1,
    pub(super) history_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(super) merge_fold_proof: &'a OfflineCashEqFoldProofV1,
}

/// One Ep/Fq incoming sender proof slot consumed by `ReceiveFoldBatch`.
pub(super) struct OfflineCashRecursiveIncomingEpWitnessV1<'a> {
    pub(super) instances: &'a [Vec<Fq>],
    pub(super) proof: &'a [u8],
    pub(super) history: &'a OfflineCashEpAccumulatorV1,
    pub(super) history_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(super) merge_fold_proof: &'a OfflineCashEpFoldProofV1,
}

struct OfflineCashRecursiveIncomingParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    instances: &'a [Vec<C::ScalarExt>],
    proof: &'a [u8],
    history: &'a IpaAccumulator<C, NativeLoader>,
    history_fold_proof: &'a [u8],
    merge_fold_proof: &'a [u8],
}

fn validate_incoming_commit_wrapper_shape_v1(
    protocol_num_instance: &[usize],
    slot_instance_lengths: impl IntoIterator<Item = usize>,
) -> Result<(), String> {
    if protocol_num_instance != [COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1] {
        return Err(
            "Offline Cash incoming CommitWrapper protocol has wrong public shape".to_owned(),
        );
    }
    if !slot_instance_lengths
        .into_iter()
        .eq(protocol_num_instance.iter().copied())
    {
        return Err("Offline Cash incoming CommitWrapper proof has wrong public shape".to_owned());
    }
    Ok(())
}

/// One parity's predecessor and GuardBundle proof material consumed by the recursive wrapper.
pub(super) struct OfflineCashRecursiveParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(super) suite_upgrade_bridge: OfflineCashSuiteUpgradeBridgeWitnessV1,
    pub(super) parent_protocol: &'a PlonkProtocol<C>,
    pub(super) parent_instances: &'a [Vec<C::ScalarExt>],
    pub(super) parent_proof: &'a [u8],
    pub(super) predecessor_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) parent_fold_proof: &'a [u8],
    pub(super) successor_history: &'a [u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
    pub(super) incoming_protocol: &'a PlonkProtocol<C>,
    pub(super) incoming_eq_protocol_digest: DigestV1,
    pub(super) incoming_ep_protocol_digest: DigestV1,
    pub(super) incoming_slots: &'a [OfflineCashRecursiveIncomingParityWitnessV1<'a, C>],
    pub(super) guard_protocol: &'a PlonkProtocol<C>,
    pub(super) guard_proof: &'a [u8],
    pub(super) guard_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) guard_history_bytes: &'a [u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
    pub(super) guard_history_fold_proof: &'a [u8],
    pub(super) guard_merge_fold_proof: &'a [u8],
    pub(super) mint_protocol: &'a PlonkProtocol<C>,
    pub(super) mint_instances: &'a [Vec<C::ScalarExt>],
    pub(super) mint_proof: &'a [u8],
    pub(super) mint_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) mint_history_fold_proof: &'a [u8],
    pub(super) mint_merge_fold_proof: &'a [u8],
}

/// Complete paired witness used to construct one Eq/Ep recursive transition circuit pair.
pub(super) struct OfflineCashRecursiveStateWitnessV1<'a> {
    pub(super) state: OfflineCashStateRelationWitnessV1,
    pub(super) guard_relation: OfflineCashGuardBundleRelationWitnessV1,
    pub(super) suite_upgrade_bridge: OfflineCashSuiteUpgradeBridgeWitnessV1,
    pub(super) eq_parent_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_parent_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_parent_instances: &'a [Vec<Fp>],
    pub(super) ep_parent_instances: &'a [Vec<Fq>],
    pub(super) eq_parent_proof: &'a [u8],
    pub(super) ep_parent_proof: &'a [u8],
    pub(super) eq_predecessor_history: &'a OfflineCashEqAccumulatorV1,
    pub(super) ep_predecessor_history: &'a OfflineCashEpAccumulatorV1,
    pub(super) eq_parent_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(super) ep_parent_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(super) eq_incoming_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_incoming_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_incoming_slots: &'a [OfflineCashRecursiveIncomingEqWitnessV1<'a>;
            state_relation::OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1],
    pub(super) ep_incoming_slots: &'a [OfflineCashRecursiveIncomingEpWitnessV1<'a>;
            state_relation::OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1],
    pub(super) eq_successor_history: &'a OfflineCashEqAccumulatorV1,
    pub(super) ep_successor_history: &'a OfflineCashEpAccumulatorV1,
    pub(super) eq_guard_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_guard_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_guard_proof: &'a [u8],
    pub(super) ep_guard_proof: &'a [u8],
    pub(super) eq_guard_history: &'a OfflineCashEqAccumulatorV1,
    pub(super) ep_guard_history: &'a OfflineCashEpAccumulatorV1,
    pub(super) eq_guard_history_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(super) ep_guard_history_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(super) eq_guard_merge_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(super) ep_guard_merge_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(super) eq_mint_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_mint_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_mint_instances: &'a [Vec<Fp>],
    pub(super) ep_mint_instances: &'a [Vec<Fq>],
    pub(super) eq_mint_proof: &'a [u8],
    pub(super) ep_mint_proof: &'a [u8],
    pub(super) eq_mint_history: &'a OfflineCashEqAccumulatorV1,
    pub(super) ep_mint_history: &'a OfflineCashEpAccumulatorV1,
    pub(super) eq_mint_history_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(super) ep_mint_history_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(super) eq_mint_merge_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(super) ep_mint_merge_fold_proof: &'a OfflineCashEpFoldProofV1,
}

/// Shared Base plus reciprocal dense-MSM configuration.
#[derive(Clone, Debug)]
pub(super) struct OfflineCashRecursiveStateConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp half of one production recursive aggregate-state proof.
#[derive(Clone)]
pub(super) struct OfflineCashRecursiveStateEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq half of one production recursive aggregate-state proof.
#[derive(Clone)]
pub(super) struct OfflineCashRecursiveStateEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_recursive_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = OfflineCashRecursiveStateConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    sha_jobs: self.sha_jobs.unknown(),
                    dense_jobs: self.dense_jobs.unknown(),
                }
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                OfflineCashRecursiveStateConfigV1 {
                    base,
                    sha: PastaSha256ConfigV1::configure(meta),
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let usable_rows = (1_usize << self.builder.config_params.k) - MINIMUM_UNUSABLE_ROWS;
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )?;
                self.sha_jobs.synthesize(
                    &config.sha,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    usable_rows,
                )?;
                self.dense_jobs.synthesize(
                    &config.dense,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                    usable_rows,
                )
            }
        }
    };
}

impl_recursive_circuit!(
    OfflineCashRecursiveStateEqCircuitV1,
    Fp,
    EpAffine,
    "Offline Cash Eq recursive state"
);
impl_recursive_circuit!(
    OfflineCashRecursiveStateEpCircuitV1,
    Fq,
    EqAffine,
    "Offline Cash Ep recursive state"
);

/// Build both mutually-audited recursive circuits from one exact state transition.
pub(super) fn build_offline_cash_recursive_state_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: OfflineCashRecursiveStateWitnessV1<'_>,
) -> Result<
    (
        OfflineCashRecursiveStateEqCircuitV1,
        OfflineCashRecursiveStateEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    witness.state.validate()?;
    witness.guard_relation.validate()?;
    witness
        .suite_upgrade_bridge
        .validate_against(&witness.state)?;
    let eq_history = witness
        .eq_predecessor_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_history = witness
        .ep_predecessor_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_incoming_histories = witness
        .eq_incoming_slots
        .iter()
        .map(|slot| slot.history.to_native().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let ep_incoming_histories = witness
        .ep_incoming_slots
        .iter()
        .map(|slot| slot.history.to_native().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let eq_guard_history = witness
        .eq_guard_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_guard_history = witness
        .ep_guard_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_mint_history = witness
        .eq_mint_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_mint_history = witness
        .ep_mint_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_svk = eq_succinct_vk(eq_params);
    let ep_svk = ep_succinct_vk(ep_params);
    let eq_incoming_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq_incoming_protocol,
        OfflineCashPastaParityV1::Eq,
    )?;
    let ep_incoming_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep_incoming_protocol,
        OfflineCashPastaParityV1::Ep,
    )?;
    let eq_incoming_slots = witness
        .eq_incoming_slots
        .iter()
        .zip(&eq_incoming_histories)
        .map(
            |(slot, history)| OfflineCashRecursiveIncomingParityWitnessV1 {
                instances: slot.instances,
                proof: slot.proof,
                history,
                history_fold_proof: slot.history_fold_proof.as_bytes(),
                merge_fold_proof: slot.merge_fold_proof.as_bytes(),
            },
        )
        .collect::<Vec<_>>();
    let ep_incoming_slots = witness
        .ep_incoming_slots
        .iter()
        .zip(&ep_incoming_histories)
        .map(
            |(slot, history)| OfflineCashRecursiveIncomingParityWitnessV1 {
                instances: slot.instances,
                proof: slot.proof,
                history,
                history_fold_proof: slot.history_fold_proof.as_bytes(),
                merge_fold_proof: slot.merge_fold_proof.as_bytes(),
            },
        )
        .collect::<Vec<_>>();
    let (mut eq_builder, eq_sha, eq_output) = build_scalar_half::<EqAffine>(
        witness.state.clone(),
        witness.guard_relation.clone(),
        &eq_svk,
        OfflineCashPastaParityV1::Eq,
        OfflineCashRecursiveParityWitnessV1 {
            suite_upgrade_bridge: witness.suite_upgrade_bridge,
            parent_protocol: witness.eq_parent_protocol,
            parent_instances: witness.eq_parent_instances,
            parent_proof: witness.eq_parent_proof,
            predecessor_history: &eq_history,
            parent_fold_proof: witness.eq_parent_fold_proof.as_bytes(),
            successor_history: witness.eq_successor_history.as_bytes(),
            incoming_protocol: witness.eq_incoming_protocol,
            incoming_eq_protocol_digest: eq_incoming_protocol_digest,
            incoming_ep_protocol_digest: ep_incoming_protocol_digest,
            incoming_slots: &eq_incoming_slots,
            guard_protocol: witness.eq_guard_protocol,
            guard_proof: witness.eq_guard_proof,
            guard_history: &eq_guard_history,
            guard_history_bytes: witness.eq_guard_history.as_bytes(),
            guard_history_fold_proof: witness.eq_guard_history_fold_proof.as_bytes(),
            guard_merge_fold_proof: witness.eq_guard_merge_fold_proof.as_bytes(),
            mint_protocol: witness.eq_mint_protocol,
            mint_instances: witness.eq_mint_instances,
            mint_proof: witness.eq_mint_proof,
            mint_history: &eq_mint_history,
            mint_history_fold_proof: witness.eq_mint_history_fold_proof.as_bytes(),
            mint_merge_fold_proof: witness.eq_mint_merge_fold_proof.as_bytes(),
        },
    )?;
    let (mut ep_builder, ep_sha, ep_output) = build_scalar_half::<EpAffine>(
        witness.state,
        witness.guard_relation,
        &ep_svk,
        OfflineCashPastaParityV1::Ep,
        OfflineCashRecursiveParityWitnessV1 {
            suite_upgrade_bridge: witness.suite_upgrade_bridge,
            parent_protocol: witness.ep_parent_protocol,
            parent_instances: witness.ep_parent_instances,
            parent_proof: witness.ep_parent_proof,
            predecessor_history: &ep_history,
            parent_fold_proof: witness.ep_parent_fold_proof.as_bytes(),
            successor_history: witness.ep_successor_history.as_bytes(),
            incoming_protocol: witness.ep_incoming_protocol,
            incoming_eq_protocol_digest: eq_incoming_protocol_digest,
            incoming_ep_protocol_digest: ep_incoming_protocol_digest,
            incoming_slots: &ep_incoming_slots,
            guard_protocol: witness.ep_guard_protocol,
            guard_proof: witness.ep_guard_proof,
            guard_history: &ep_guard_history,
            guard_history_bytes: witness.ep_guard_history.as_bytes(),
            guard_history_fold_proof: witness.ep_guard_history_fold_proof.as_bytes(),
            guard_merge_fold_proof: witness.ep_guard_merge_fold_proof.as_bytes(),
            mint_protocol: witness.ep_mint_protocol,
            mint_instances: witness.ep_mint_instances,
            mint_proof: witness.ep_mint_proof,
            mint_history: &ep_mint_history,
            mint_history_fold_proof: witness.ep_mint_history_fold_proof.as_bytes(),
            mint_merge_fold_proof: witness.ep_mint_merge_fold_proof.as_bytes(),
        },
    )?;

    let mut eq_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EpAffine>(
        &mut eq_builder,
        OfflineCashPastaParityV1::Ep,
        &ep_output,
        &mut eq_dense,
    )?;
    let mut ep_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EqAffine>(
        &mut ep_builder,
        OfflineCashPastaParityV1::Eq,
        &eq_output,
        &mut ep_dense,
    )?;
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << 16) - MINIMUM_UNUSABLE_ROWS;
    eq_sha.validate_capacity(usable_rows)?;
    ep_sha.validate_capacity(usable_rows)?;
    eq_dense.validate_capacity(usable_rows)?;
    ep_dense.validate_capacity(usable_rows)?;
    let eq_audit = assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    Ok((
        OfflineCashRecursiveStateEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_sha,
            dense_jobs: eq_dense,
        },
        OfflineCashRecursiveStateEpCircuitV1 {
            builder: ep_builder,
            sha_jobs: ep_sha,
            dense_jobs: ep_dense,
        },
        eq_audit,
        ep_audit,
    ))
}

pub(super) fn assigned_digest_bytes<F: halo2_base::utils::ScalarField>(
    limbs: &[AssignedValue<F>; 2],
) -> Result<[u8; 32], String> {
    let mut digest = [0_u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        let bytes = fe_to_biguint(limb.value()).to_bytes_le();
        if bytes.len() > 16 {
            return Err("recursive-state audit limb exceeds its canonical u128 range".to_owned());
        }
        let offset = index * 16;
        digest[offset..offset + bytes.len()].copy_from_slice(&bytes);
    }
    if digest == [0; 32] {
        return Err("recursive-state deferred audit is zero".to_owned());
    }
    Ok(digest)
}

fn build_scalar_half<C>(
    state: OfflineCashStateRelationWitnessV1,
    guard_relation: OfflineCashGuardBundleRelationWitnessV1,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: OfflineCashPastaParityV1,
    witness: OfflineCashRecursiveParityWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        PastaSha256JobsV1<C::ScalarExt>,
        OfflineCashDeferredParentOutputV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    let parent_enabled = state.operation != OfflineCashOperationV1::Bootstrap;
    let mint_enabled = state.operation == OfflineCashOperationV1::MintFold;
    let (mut builder, assigned_state) =
        state_relation::relation_builder_with_bindings::<C::ScalarExt>(Some(&state))?;
    let mut sha_jobs = PastaSha256JobsV1::default();
    let assigned_guard =
        constrain_guard_bundle_semantics_v1(&mut builder, &mut sha_jobs, &guard_relation)?;
    constrain_state_guard_binding_v1(&mut builder, &assigned_state, &assigned_guard)?;
    let public = builder
        .assigned_instances
        .first()
        .cloned()
        .ok_or_else(|| "Offline Cash recursive state public column is absent".to_owned())?;
    let current_expected_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            OfflineCashPastaParityV1::Eq => {
                public_instance::EQ_PROTOCOL_LO..public_instance::EQ_PROTOCOL_LO + 2
            }
            OfflineCashPastaParityV1::Ep => {
                public_instance::EP_PROTOCOL_LO..public_instance::EP_PROTOCOL_LO + 2
            }
        })
        .ok_or_else(|| "Offline Cash recursive protocol public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash recursive protocol public limbs have wrong shape".to_owned())?;
    let expected_predecessor_state = public[public_instance::PREDECESSOR_STATE];
    let expected_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            OfflineCashPastaParityV1::Eq => {
                public_instance::EQ_DEFERRED_AUDIT_LO..public_instance::EQ_DEFERRED_AUDIT_LO + 2
            }
            OfflineCashPastaParityV1::Ep => {
                public_instance::EP_DEFERRED_AUDIT_LO..public_instance::EP_DEFERRED_AUDIT_LO + 2
            }
        })
        .ok_or_else(|| "Offline Cash recursive audit public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash recursive audit public limbs have wrong shape".to_owned())?;
    let expected_guard_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            OfflineCashPastaParityV1::Eq => {
                public_instance::GUARD_EQ_PROTOCOL_LO..public_instance::GUARD_EQ_PROTOCOL_LO + 2
            }
            OfflineCashPastaParityV1::Ep => {
                public_instance::GUARD_EP_PROTOCOL_LO..public_instance::GUARD_EP_PROTOCOL_LO + 2
            }
        })
        .ok_or_else(|| "Offline Cash GuardBundle protocol public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| {
            "Offline Cash GuardBundle protocol public limbs have wrong shape".to_owned()
        })?;
    let expected_mint_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            OfflineCashPastaParityV1::Eq => {
                public_instance::MINT_EQ_PROTOCOL_LO..public_instance::MINT_EQ_PROTOCOL_LO + 2
            }
            OfflineCashPastaParityV1::Ep => {
                public_instance::MINT_EP_PROTOCOL_LO..public_instance::MINT_EP_PROTOCOL_LO + 2
            }
        })
        .ok_or_else(|| "Offline Cash mint protocol public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash mint protocol public limbs have wrong shape".to_owned())?;
    let guard_digest: [AssignedValue<C::ScalarExt>; 2] = public
        .get(public_instance::GUARD_LO..public_instance::GUARD_HI + 1)
        .ok_or_else(|| "Offline Cash GuardBundle public digest is absent".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash GuardBundle public digest has wrong shape".to_owned())?;
    let guard_eq_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(
            public_instance::GUARD_EQ_CREDENTIAL_AUDIT_LO
                ..public_instance::GUARD_EQ_CREDENTIAL_AUDIT_LO + 2,
        )
        .ok_or_else(|| "Offline Cash Eq credential audit public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash Eq credential audit public limbs have wrong shape".to_owned())?;
    let guard_ep_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(
            public_instance::GUARD_EP_CREDENTIAL_AUDIT_LO
                ..public_instance::GUARD_EP_CREDENTIAL_AUDIT_LO + 2,
        )
        .ok_or_else(|| "Offline Cash Ep credential audit public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Offline Cash Ep credential audit public limbs have wrong shape".to_owned())?;

    let range = builder.range_chip();
    let operation = builder.assigned_instances[0][public_instance::OPERATION];
    let bootstrap = range.gate().is_zero(builder.main(0), operation);
    let mint = range.gate().is_equal(
        builder.main(0),
        operation,
        halo2_base::QuantumCell::Constant(C::ScalarExt::ONE),
    );
    let suite_upgrade = range.gate().is_equal(
        builder.main(0),
        operation,
        halo2_base::QuantumCell::Constant(C::ScalarExt::from(5)),
    );
    let non_bootstrap = range.gate().not(builder.main(0), bootstrap);
    let native_parent_protocol_digest =
        native_parent_protocol_digest_v1(witness.parent_protocol, parity)?;
    if state.operation == OfflineCashOperationV1::SuiteUpgrade {
        let authorized = match parity {
            OfflineCashPastaParityV1::Eq => witness.suite_upgrade_bridge.old_eq_protocol_digest,
            OfflineCashPastaParityV1::Ep => witness.suite_upgrade_bridge.old_ep_protocol_digest,
        };
        if authorized != native_parent_protocol_digest {
            return Err(
                "suite-upgrade parent protocol is not the authorized old protocol".to_owned(),
            );
        }
    }
    let expected_protocol = constrain_suite_upgrade_bridge_v1(
        &mut builder,
        &mut sha_jobs,
        &assigned_state,
        witness.suite_upgrade_bridge,
        parity,
        current_expected_protocol,
        suite_upgrade,
    )?;
    let predecessor_components = state
        .predecessor
        .as_ref()
        .map_or(OfflineCashPastaStateCommitmentV1::ZERO, |state| {
            state.state_commitment_components
        });
    let active_successor = builder.main(0).load_constant(C::ScalarExt::ONE);
    constrain_outer_state_head_v1(
        &mut builder,
        &mut sha_jobs,
        predecessor_components,
        assigned_state.predecessor_eq_components,
        assigned_state.predecessor_ep_components,
        assigned_state.predecessor_outer,
        non_bootstrap,
    )?;
    constrain_outer_state_head_v1(
        &mut builder,
        &mut sha_jobs,
        state.successor.state_commitment_components,
        assigned_state.successor_eq_components,
        assigned_state.successor_ep_components,
        assigned_state.successor_outer,
        active_successor,
    )?;
    let history_limbs = assign_history_limbs(&mut builder, &range, witness.successor_history)?;
    builder.assigned_instances[0].extend(history_limbs.iter().copied());

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let structure = offline_cash_protocol_structure_digest_v1(witness.parent_protocol, parity)?;
    let parent_protocol = load_and_constrain_parent_protocol_if_v1(
        &loader,
        witness.parent_protocol,
        parity,
        structure,
        &expected_protocol,
        Some(non_bootstrap),
    )
    .map_err(|error| format!("failed to bind predecessor protocol: {error:?}"))?;
    let base_successor_history = constrain_parent_and_history_into_loader_v1(
        succinct_vk,
        &parent_protocol.protocol,
        OfflineCashDeferredParentWitnessV1 {
            instances: witness.parent_instances,
            proof_bytes: witness.parent_proof,
            predecessor_history: witness.predecessor_history,
            fold_proof_bytes: witness.parent_fold_proof,
        },
        expected_predecessor_state,
        assigned_state.predecessor_outer,
        non_bootstrap,
        &loader,
    )
    .map_err(|error| format!("failed to verify/fold predecessor proof: {error:?}"))?;
    let parent_end = loader.ecc_chip().equation_count();
    if parent_end == 0 {
        return Err("Offline Cash predecessor verifier emitted no equations".to_owned());
    }

    if witness.incoming_slots.len() != state_relation::OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1 {
        return Err("Offline Cash incoming sender batch has wrong fixed width".to_owned());
    }
    // Incoming monetary authority is the release-pinned terminal CommitWrapper verifier. Its
    // complete protocol is embedded as circuit constants, just like the terminal wrapper's own
    // nested protocols; it is deliberately independent of the predecessor state protocol.
    let incoming_protocol = witness.incoming_protocol.loaded(&loader);
    let mut state_history = base_successor_history;
    let mut incoming_equation_spans = Vec::with_capacity(witness.incoming_slots.len());
    let mut previous_incoming_end = parent_end;
    for (index, (slot, assigned_slot)) in witness
        .incoming_slots
        .iter()
        .zip(&assigned_state.receive_slots)
        .enumerate()
    {
        validate_incoming_commit_wrapper_shape_v1(
            &witness.incoming_protocol.num_instance,
            slot.instances.iter().map(Vec::len),
        )
        .map_err(|error| format!("Offline Cash incoming sender proof slot {index}: {error}"))?;
        let incoming_instances = slot
            .instances
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| loader.assign_scalar(*value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let incoming_current = verify_ordinary_proof_v1(
            &loader,
            succinct_vk,
            &incoming_protocol,
            &incoming_instances,
            slot.proof,
        )
        .map_err(|error| {
            format!("failed to verify incoming sender proof slot {index}: {error:?}")
        })?;
        let incoming_column = incoming_instances.first().ok_or_else(|| {
            format!("Offline Cash incoming sender public column {index} is absent")
        })?;
        let incoming_history = load_native_accumulator(&loader, slot.history).map_err(|error| {
            format!("failed to load incoming sender history slot {index}: {error:?}")
        })?;
        let incoming_history_limbs = incoming_column
            .get(COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1..)
            .ok_or_else(|| format!("Offline Cash incoming sender history {index} is absent"))?
            .iter()
            .map(|value| *value.assigned())
            .collect::<Vec<_>>();
        bind_accumulator_limbs(&loader, &incoming_history, &incoming_history_limbs).map_err(
            |error| format!("failed to bind incoming sender history slot {index}: {error:?}"),
        )?;

        constrain_incoming_scalar_if_v1(
            &loader,
            &mut sha_jobs,
            incoming_column,
            &assigned_state,
            assigned_slot,
            witness.incoming_eq_protocol_digest,
            witness.incoming_ep_protocol_digest,
        )?;
        constrain_incoming_common_binding_v1(
            &loader,
            &mut sha_jobs,
            incoming_column,
            assigned_slot.incoming_proof_binding_digest,
            assigned_slot.active,
        )?;
        let incoming_complete = verify_fold(
            &loader,
            succinct_vk,
            &[incoming_current, incoming_history],
            slot.history_fold_proof,
        )
        .map_err(|error| {
            format!("failed to fold incoming sender history slot {index}: {error:?}")
        })?;
        let merged_history = verify_fold(
            &loader,
            succinct_vk,
            &[state_history.clone(), incoming_complete],
            slot.merge_fold_proof,
        )
        .map_err(|error| {
            format!("failed to merge incoming sender history slot {index}: {error:?}")
        })?;
        state_history = select_accumulator_v1(
            &loader,
            &merged_history,
            &state_history,
            assigned_slot.active,
        )
        .map_err(|error| {
            format!("failed to select ReceiveFoldBatch history slot {index}: {error:?}")
        })?;
        let slot_end = loader.ecc_chip().equation_count();
        if slot_end <= previous_incoming_end {
            return Err(format!(
                "Offline Cash incoming sender verifier slot {index} emitted no equations"
            ));
        }
        incoming_equation_spans.push((
            slot_end - previous_incoming_end,
            assigned_slot.active,
            state.receive_slots.get(index).is_some_and(Option::is_some),
        ));
        previous_incoming_end = slot_end;
    }
    let incoming_end = previous_incoming_end;
    if incoming_end <= parent_end {
        return Err("Offline Cash incoming sender verifier emitted no equations".to_owned());
    }
    constrain_receive_batch_binding_v1(&loader, &mut sha_jobs, &assigned_state)?;

    if witness.guard_protocol.num_instance != [GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1] {
        return Err("Offline Cash GuardBundle proof has wrong public shape".to_owned());
    }
    let guard_structure =
        offline_cash_protocol_structure_digest_v1(witness.guard_protocol, parity)?;
    let loaded_guard = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.guard_protocol,
        parity,
        guard_structure,
        &expected_guard_protocol,
    )
    .map_err(|error| format!("failed to bind GuardBundle protocol: {error:?}"))?;
    let guard_history_cells =
        assign_history_limbs(&mut builder, &range, witness.guard_history_bytes)?;
    let guard_column = guard_digest
        .into_iter()
        .chain(guard_eq_audit)
        .chain(guard_ep_audit)
        .chain(guard_history_cells.iter().copied())
        .map(|cell| loader.scalar_from_assigned(cell))
        .collect::<Vec<_>>();
    let guard_current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &loaded_guard.protocol,
        &[guard_column],
        witness.guard_proof,
    )
    .map_err(|error| format!("failed to verify GuardBundle proof: {error:?}"))?;
    let guard_history = load_native_accumulator(&loader, witness.guard_history)
        .map_err(|error| format!("failed to load GuardBundle history: {error:?}"))?;
    bind_accumulator_limbs(&loader, &guard_history, &guard_history_cells)
        .map_err(|error| format!("failed to bind GuardBundle history: {error:?}"))?;
    let complete_guard = verify_fold(
        &loader,
        succinct_vk,
        &[guard_current, guard_history],
        witness.guard_history_fold_proof,
    )
    .map_err(|error| format!("failed to fold GuardBundle history: {error:?}"))?;
    let history_with_guard = verify_fold(
        &loader,
        succinct_vk,
        &[state_history, complete_guard],
        witness.guard_merge_fold_proof,
    )
    .map_err(|error| format!("failed to merge GuardBundle history: {error:?}"))?;
    let guard_end = loader.ecc_chip().equation_count();
    if guard_end <= incoming_end {
        return Err("Offline Cash GuardBundle verifier emitted no equations".to_owned());
    }

    if witness.mint_protocol.num_instance != [OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        || witness.mint_instances.len() != 1
        || witness.mint_instances[0].len() != OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("Offline Cash finalized-mint proof has wrong public shape".to_owned());
    }
    let mint_structure = offline_cash_protocol_structure_digest_v1(witness.mint_protocol, parity)?;
    let loaded_mint = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.mint_protocol,
        parity,
        mint_structure,
        &expected_mint_protocol,
    )
    .map_err(|error| format!("failed to bind finalized-mint protocol: {error:?}"))?;
    let mint_instances = witness
        .mint_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mint_column = mint_instances
        .first()
        .ok_or_else(|| "Offline Cash mint public column is absent".to_owned())?;
    constrain_mint_authority_binding_v1(&loader, mint_column, &public, mint)?;
    let mint_current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &loaded_mint.protocol,
        &mint_instances,
        witness.mint_proof,
    )
    .map_err(|error| format!("failed to verify finalized-mint proof: {error:?}"))?;
    let mint_history = load_native_accumulator(&loader, witness.mint_history)
        .map_err(|error| format!("failed to load finalized-mint history: {error:?}"))?;
    let mint_history_cells = mint_column
        .get(mint_public_instance::HISTORY_START..)
        .ok_or_else(|| "Offline Cash finalized-mint history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &mint_history, &mint_history_cells)
        .map_err(|error| format!("failed to bind finalized-mint history: {error:?}"))?;
    let complete_mint = verify_fold(
        &loader,
        succinct_vk,
        &[mint_current, mint_history],
        witness.mint_history_fold_proof,
    )
    .map_err(|error| format!("failed to fold finalized-mint history: {error:?}"))?;
    let history_with_mint = verify_fold(
        &loader,
        succinct_vk,
        &[history_with_guard.clone(), complete_mint],
        witness.mint_merge_fold_proof,
    )
    .map_err(|error| format!("failed to merge finalized-mint history: {error:?}"))?;
    let successor_history =
        select_accumulator_v1(&loader, &history_with_mint, &history_with_guard, mint)
            .map_err(|error| format!("failed to select finalized-mint history: {error:?}"))?;
    bind_accumulator_limbs(&loader, &successor_history, &history_limbs)
        .map_err(|error| format!("failed to bind successor history: {error:?}"))?;
    let mint_end = loader.ecc_chip().equation_count();
    if mint_end <= guard_end {
        return Err("Offline Cash finalized-mint verifier emitted no equations".to_owned());
    }

    let mut equation_tags = Vec::with_capacity(mint_end);
    equation_tags.extend(std::iter::repeat_n(PARENT_EQUATION_TAG, parent_end));
    equation_tags.extend(std::iter::repeat_n(
        INCOMING_CREDIT_EQUATION_TAG,
        incoming_end - parent_end,
    ));
    equation_tags.extend(std::iter::repeat_n(
        GUARD_BUNDLE_EQUATION_TAG,
        guard_end - incoming_end,
    ));
    equation_tags.extend(std::iter::repeat_n(
        MINT_FINALITY_EQUATION_TAG,
        mint_end - guard_end,
    ));
    let mut assigned_selectors = Vec::with_capacity(mint_end);
    assigned_selectors.extend(std::iter::repeat_n(non_bootstrap, parent_end));
    for (equation_count, assigned, _) in &incoming_equation_spans {
        assigned_selectors.extend(std::iter::repeat_n(*assigned, *equation_count));
    }
    let guard_enabled = loader.ctx_mut().main().load_constant(C::ScalarExt::ONE);
    assigned_selectors.extend(std::iter::repeat_n(guard_enabled, guard_end - incoming_end));
    assigned_selectors.extend(std::iter::repeat_n(mint, mint_end - guard_end));
    let mut equation_selectors = vec![parent_enabled; parent_end];
    for (equation_count, _, enabled) in &incoming_equation_spans {
        equation_selectors.extend(std::iter::repeat_n(*enabled, *equation_count));
    }
    equation_selectors.extend(std::iter::repeat_n(true, guard_end - incoming_end));
    equation_selectors.extend(std::iter::repeat_n(mint_enabled, mint_end - guard_end));
    let output = finalize_deferred_audit_plan_v1(
        &mut builder,
        loader,
        equation_tags,
        assigned_selectors,
        equation_selectors,
    )
    .map_err(|error| format!("failed to finalize deferred audit: {error:?}"))?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected_audit) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok((builder, sha_jobs, output))
}

fn assign_history_limbs<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &halo2_base::gates::RangeChip<F>,
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<AssignedValue<F>>, String> {
    let limbs = history
        .chunks_exact(16)
        .map(|chunk| {
            let value = F::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history chunk has sixteen bytes"),
            ));
            let assigned = builder.main(0).load_witness(value);
            range.range_check(builder.main(0), assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    if limbs.len() != accumulator_limb_count() {
        return Err("Offline Cash history limb count is not fixed".to_owned());
    }
    Ok(limbs)
}

fn constrain_suite_upgrade_bridge_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &state_relation::OfflineCashAssignedStateRelationV1<F>,
    bridge: OfflineCashSuiteUpgradeBridgeWitnessV1,
    parity: OfflineCashPastaParityV1,
    current_expected_protocol: [AssignedValue<F>; 2],
    suite_upgrade: AssignedValue<F>,
) -> Result<[AssignedValue<F>; 2], String> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let old_eq: [PastaSha256ByteV1<F>; 32] =
        assign_bytes(ctx, &range, &bridge.old_eq_protocol_digest)
            .try_into()
            .expect("suite-upgrade Eq protocol digest width");
    let old_ep: [PastaSha256ByteV1<F>; 32] =
        assign_bytes(ctx, &range, &bridge.old_ep_protocol_digest)
            .try_into()
            .expect("suite-upgrade Ep protocol digest width");
    let old_suite: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &bridge.old_suite_id)
        .try_into()
        .expect("suite-upgrade old suite digest width");
    let old_vk: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &bridge.old_vk_digest)
        .try_into()
        .expect("suite-upgrade old VK digest width");
    let new_suite: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &bridge.new_suite_id)
        .try_into()
        .expect("suite-upgrade new suite digest width");
    let new_vk: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &bridge.new_vk_digest)
        .try_into()
        .expect("suite-upgrade new VK digest width");
    let governance: [PastaSha256ByteV1<F>; 32] =
        assign_bytes(ctx, &range, &bridge.governance_authorization_proof_digest)
            .try_into()
            .expect("suite-upgrade governance digest width");

    let old_eq_limbs = digest_limbs_assigned(ctx, &old_eq);
    let old_ep_limbs = digest_limbs_assigned(ctx, &old_ep);
    let old_suite_limbs = digest_limbs_assigned(ctx, &old_suite);
    let old_vk_limbs = digest_limbs_assigned(ctx, &old_vk);
    let new_suite_limbs = digest_limbs_assigned(ctx, &new_suite);
    let new_vk_limbs = digest_limbs_assigned(ctx, &new_vk);
    let governance_limbs = digest_limbs_assigned(ctx, &governance);
    let inactive = gate.not(ctx, suite_upgrade);
    for limb in old_eq_limbs
        .into_iter()
        .chain(old_ep_limbs)
        .chain(old_suite_limbs)
        .chain(old_vk_limbs)
        .chain(new_suite_limbs)
        .chain(new_vk_limbs)
        .chain(governance_limbs)
    {
        let selected = gate.mul(ctx, limb, inactive);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }

    let old_eq_limbs = digest_limbs_assigned(ctx, &old_eq);
    let old_ep_limbs = digest_limbs_assigned(ctx, &old_ep);
    let old_suite_limbs = digest_limbs_assigned(ctx, &old_suite);
    let old_vk_limbs = digest_limbs_assigned(ctx, &old_vk);
    let new_suite_limbs = digest_limbs_assigned(ctx, &new_suite);
    let new_vk_limbs = digest_limbs_assigned(ctx, &new_vk);
    let governance_limbs = digest_limbs_assigned(ctx, &governance);
    for limbs in [
        old_eq_limbs,
        old_ep_limbs,
        old_suite_limbs,
        old_vk_limbs,
        new_suite_limbs,
        new_vk_limbs,
        governance_limbs,
    ] {
        let low_zero = gate.is_zero(ctx, limbs[0]);
        let high_zero = gate.is_zero(ctx, limbs[1]);
        let both_zero = gate.and(ctx, low_zero, high_zero);
        let invalid = gate.mul(ctx, suite_upgrade, both_zero);
        gate.assert_is_const(ctx, &invalid, &F::ZERO);
    }
    for (actual, expected) in old_suite_limbs
        .into_iter()
        .zip(state.predecessor.suite_id)
        .chain(old_vk_limbs.into_iter().zip(state.predecessor.vk_digest))
        .chain(new_suite_limbs.into_iter().zip(state.successor.suite_id))
        .chain(new_vk_limbs.into_iter().zip(state.successor.vk_digest))
    {
        let difference = gate.sub(ctx, actual, expected);
        let selected = gate.mul(ctx, difference, suite_upgrade);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let eq_low_equal = gate.is_equal(ctx, old_eq_limbs[0], old_ep_limbs[0]);
    let eq_high_equal = gate.is_equal(ctx, old_eq_limbs[1], old_ep_limbs[1]);
    let protocols_equal = gate.and(ctx, eq_low_equal, eq_high_equal);
    let invalid_equal_protocol = gate.mul(ctx, suite_upgrade, protocols_equal);
    gate.assert_is_const(ctx, &invalid_equal_protocol, &F::ZERO);

    let bridge_digest = hash(
        ctx,
        jobs,
        [
            constant_bytes(SUITE_UPGRADE_BRIDGE_DOMAIN_V1),
            old_eq.to_vec(),
            old_ep.to_vec(),
            old_suite.to_vec(),
            old_vk.to_vec(),
            new_suite.to_vec(),
            new_vk.to_vec(),
            governance.to_vec(),
        ]
        .concat(),
    )?;
    for (actual, expected) in digest_limbs_assigned(ctx, &bridge_digest)
        .into_iter()
        .zip(state.suite_upgrade_authorization_digest)
    {
        let difference = gate.sub(ctx, actual, expected);
        let selected = gate.mul(ctx, difference, suite_upgrade);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }

    let old_protocol = match parity {
        OfflineCashPastaParityV1::Eq => old_eq_limbs,
        OfflineCashPastaParityV1::Ep => old_ep_limbs,
    };
    Ok(core::array::from_fn(|index| {
        gate.select(
            ctx,
            old_protocol[index],
            current_expected_protocol[index],
            suite_upgrade,
        )
    }))
}

fn constrain_incoming_scalar_if_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
    incoming: &[DeferredScalar<'chip, C>],
    state: &state_relation::OfflineCashAssignedStateRelationV1<C::ScalarExt>,
    slot: &state_relation::OfflineCashAssignedReceiveFoldSlotV1<C::ScalarExt>,
    incoming_eq_protocol_digest: DigestV1,
    incoming_ep_protocol_digest: DigestV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    if incoming.len() != COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("Offline Cash incoming CommitWrapper public instance is truncated".to_owned());
    }
    let send_tag = loader.ctx_mut().main().load_constant(C::ScalarExt::from(2));
    constrain_loader_equal_if_v1(
        loader,
        *incoming[wrapper_public_instance::OPERATION].assigned(),
        send_tag,
        slot.active,
    );
    constrain_loader_equal_if_v1(
        loader,
        *incoming[wrapper_public_instance::AMOUNT].assigned(),
        slot.amount,
        slot.active,
    );
    constrain_loader_equal_if_v1(
        loader,
        *incoming[wrapper_public_instance::PROTOCOL_VERSION].assigned(),
        state.successor.protocol_version,
        slot.active,
    );
    constrain_loader_equal_if_v1(
        loader,
        *incoming[wrapper_public_instance::ASSET_SCALE].assigned(),
        state.successor.scale,
        slot.active,
    );
    for (offset, expected) in [
        (wrapper_public_instance::SUITE_LO, state.successor.suite_id),
        (wrapper_public_instance::VK_LO, state.successor.vk_digest),
        (
            wrapper_public_instance::RELEASE_LO,
            state.successor.release_id,
        ),
        (
            wrapper_public_instance::NETWORK_LO,
            state.successor.network_id,
        ),
        (wrapper_public_instance::ASSET_LO, state.successor.asset_id),
        (
            wrapper_public_instance::ASSET_INCARNATION_LO,
            state.successor.asset_incarnation,
        ),
        (
            wrapper_public_instance::LIABILITY_POOL_LO,
            state.successor.liability_pool_id,
        ),
    ] {
        for (index, expected) in (offset..offset + 2).zip(expected) {
            constrain_loader_equal_if_v1(
                loader,
                *incoming[index].assigned(),
                expected,
                slot.active,
            );
        }
    }

    for (offset, digest) in [
        (
            wrapper_public_instance::EQ_PROTOCOL_LO,
            incoming_eq_protocol_digest,
        ),
        (
            wrapper_public_instance::EP_PROTOCOL_LO,
            incoming_ep_protocol_digest,
        ),
    ] {
        let expected = crate::zk::offline_cash_v1_poseidon::digest_limbs::<C::ScalarExt>(digest);
        for (index, expected) in (offset..offset + 2).zip(expected) {
            let expected = loader.ctx_mut().main().load_constant(expected);
            constrain_loader_equal_if_v1(
                loader,
                *incoming[index].assigned(),
                expected,
                slot.active,
            );
        }
    }

    let gate = halo2_base::gates::GateChip::default();
    let certificate_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        *incoming[wrapper_public_instance::COMMIT_CERTIFICATE_LO].assigned(),
    );
    let certificate_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        *incoming[wrapper_public_instance::COMMIT_CERTIFICATE_LO + 1].assigned(),
    );
    let certificate_zero = gate.and(
        loader.ctx_mut().main(),
        certificate_low_zero,
        certificate_high_zero,
    );
    let active_authorization = gate.mul(loader.ctx_mut().main(), certificate_zero, slot.active);
    gate.assert_is_const(
        loader.ctx_mut().main(),
        &active_authorization,
        &C::ScalarExt::ZERO,
    );

    let mut message = constant_bytes(TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1);
    message.extend(constant_bytes(&1_u16.to_le_bytes()));
    let mut ctx = loader.ctx_mut();
    for value in slot.credit_id.into_iter().chain(slot.recipient_lane_id) {
        let bits = PastaSha256BitV1::decompose(ctx.main(), &gate, value, 128);
        for byte_bits in bits.chunks_exact(8) {
            message.push(PastaSha256ByteV1::from_bits_le(
                ctx.main(),
                &gate,
                byte_bits,
            ));
        }
    }
    for range in [
        wrapper_public_instance::REQUEST_LO..wrapper_public_instance::REQUEST_LO + 2,
        wrapper_public_instance::ACCEPTANCE_TICKET_LO
            ..wrapper_public_instance::ACCEPTANCE_TICKET_LO + 2,
        wrapper_public_instance::CIPHERTEXT_LO..wrapper_public_instance::CIPHERTEXT_LO + 2,
    ] {
        for index in range {
            let bits =
                PastaSha256BitV1::decompose(ctx.main(), &gate, *incoming[index].assigned(), 128);
            for byte_bits in bits.chunks_exact(8) {
                message.push(PastaSha256ByteV1::from_bits_le(
                    ctx.main(),
                    &gate,
                    byte_bits,
                ));
            }
        }
    }
    let amount_bits = PastaSha256BitV1::decompose(ctx.main(), &gate, slot.amount, 128);
    for byte_bits in amount_bits.chunks_exact(8) {
        message.push(PastaSha256ByteV1::from_bits_le(
            ctx.main(),
            &gate,
            byte_bits,
        ));
    }
    let output_binding = hash(ctx.main(), jobs, message)?;
    let output_binding = digest_limbs_assigned(ctx.main(), &output_binding);
    drop(ctx);
    for (actual, index) in output_binding.into_iter().zip(
        wrapper_public_instance::OUTPUT_BINDING_LO..wrapper_public_instance::OUTPUT_BINDING_LO + 2,
    ) {
        constrain_loader_equal_if_v1(loader, actual, *incoming[index].assigned(), slot.active);
    }
    Ok(())
}

fn constrain_mint_authority_binding_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    mint: &[DeferredScalar<'chip, C>],
    state_public: &[AssignedValue<C::ScalarExt>],
    enabled: AssignedValue<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    if mint.len() != OFFLINE_CASH_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        || state_public.len() < state_relation::PUBLIC_INSTANCE_COUNT
    {
        return Err("Offline Cash finalized-mint binding input is truncated".to_owned());
    }
    let finalized_mint = loader.ctx_mut().main().load_constant(C::ScalarExt::from(2));
    constrain_loader_equal_if_v1(
        loader,
        *mint[mint_public_instance::STEP].assigned(),
        finalized_mint,
        enabled,
    );
    let bindings = [
        (
            mint_public_instance::SEMANTIC_LO,
            public_instance::MINT_SEMANTIC_LO,
        ),
        (
            mint_public_instance::SEMANTIC_HI,
            public_instance::MINT_SEMANTIC_HI,
        ),
        (mint_public_instance::AMOUNT, public_instance::AMOUNT),
        (
            mint_public_instance::RELEASE_LO,
            public_instance::RELEASE_LO,
        ),
        (
            mint_public_instance::RELEASE_HI,
            public_instance::RELEASE_HI,
        ),
        (
            mint_public_instance::EQ_PROTOCOL_LO,
            public_instance::MINT_EQ_PROTOCOL_LO,
        ),
        (
            mint_public_instance::EQ_PROTOCOL_HI,
            public_instance::MINT_EQ_PROTOCOL_HI,
        ),
        (
            mint_public_instance::EP_PROTOCOL_LO,
            public_instance::MINT_EP_PROTOCOL_LO,
        ),
        (
            mint_public_instance::EP_PROTOCOL_HI,
            public_instance::MINT_EP_PROTOCOL_HI,
        ),
        (
            mint_public_instance::PAIR_BINDING_LO,
            public_instance::MINT_PROOF_BINDING_LO,
        ),
        (
            mint_public_instance::PAIR_BINDING_HI,
            public_instance::MINT_PROOF_BINDING_HI,
        ),
    ];
    for (mint_index, state_index) in bindings {
        constrain_loader_equal_if_v1(
            loader,
            *mint[mint_index].assigned(),
            state_public[state_index],
            enabled,
        );
    }
    Ok(())
}

fn constrain_loader_equal_if_v1<C>(
    loader: &DeferredLoader<'_, C>,
    left: AssignedValue<C::ScalarExt>,
    right: AssignedValue<C::ScalarExt>,
    enabled: AssignedValue<C::ScalarExt>,
) where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    let chip = loader.ecc_chip();
    let mut ctx = loader.ctx_mut();
    let difference = chip.range().gate().sub(ctx.main(), left, right);
    let selected = chip.range().gate().mul(ctx.main(), difference, enabled);
    chip.range()
        .gate()
        .assert_is_const(ctx.main(), &selected, &C::ScalarExt::ZERO);
}

fn constrain_incoming_common_binding_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
    incoming: &[DeferredScalar<'chip, C>],
    expected: [AssignedValue<C::ScalarExt>; 2],
    enabled: AssignedValue<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    if incoming.len() < COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1 {
        return Err("Offline Cash incoming CommitWrapper binding prefix is truncated".to_owned());
    }
    let mut message = constant_bytes(super::INCOMING_PROOF_BINDING_DOMAIN_V1);
    message.push(PastaSha256ByteV1::constant(0));
    let indices = [
        wrapper_public_instance::LIFECYCLE_LO..wrapper_public_instance::LIFECYCLE_LO + 2,
        wrapper_public_instance::SEMANTIC_LO..wrapper_public_instance::SEMANTIC_LO + 2,
        wrapper_public_instance::CANDIDATE_LO..wrapper_public_instance::CANDIDATE_LO + 2,
        wrapper_public_instance::COMMIT_CERTIFICATE_LO
            ..wrapper_public_instance::COMMIT_CERTIFICATE_LO + 2,
        wrapper_public_instance::TRANSITION_NULLIFIER_LO
            ..wrapper_public_instance::TRANSITION_NULLIFIER_LO + 2,
        wrapper_public_instance::REQUEST_LO..wrapper_public_instance::REQUEST_LO + 2,
        wrapper_public_instance::ACCEPTANCE_TICKET_LO
            ..wrapper_public_instance::ACCEPTANCE_TICKET_LO + 2,
        wrapper_public_instance::CIPHERTEXT_LO..wrapper_public_instance::CIPHERTEXT_LO + 2,
        wrapper_public_instance::OUTPUT_BINDING_LO..wrapper_public_instance::OUTPUT_BINDING_LO + 2,
        wrapper_public_instance::EQ_PROTOCOL_LO..wrapper_public_instance::EQ_PROTOCOL_LO + 2,
        wrapper_public_instance::EP_PROTOCOL_LO..wrapper_public_instance::EP_PROTOCOL_LO + 2,
        wrapper_public_instance::EQ_DEFERRED_AUDIT_LO
            ..wrapper_public_instance::EQ_DEFERRED_AUDIT_LO + 2,
        wrapper_public_instance::EP_DEFERRED_AUDIT_LO
            ..wrapper_public_instance::EP_DEFERRED_AUDIT_LO + 2,
    ];
    let gate = halo2_base::gates::GateChip::default();
    let mut ctx = loader.ctx_mut();
    for index in indices.into_iter().flatten() {
        let value = incoming
            .get(index)
            .ok_or_else(|| "Offline Cash incoming proof binding input is absent".to_owned())?;
        let bits = PastaSha256BitV1::decompose(ctx.main(), &gate, *value.assigned(), 128);
        for byte_bits in bits.chunks_exact(8) {
            message.push(PastaSha256ByteV1::from_bits_le(
                ctx.main(),
                &gate,
                byte_bits,
            ));
        }
    }
    let amount = incoming
        .get(wrapper_public_instance::AMOUNT)
        .ok_or_else(|| "Offline Cash incoming wrapper amount is absent".to_owned())?;
    let amount_bits = PastaSha256BitV1::decompose(ctx.main(), &gate, *amount.assigned(), 128);
    for byte_bits in amount_bits.chunks_exact(8) {
        message.push(PastaSha256ByteV1::from_bits_le(
            ctx.main(),
            &gate,
            byte_bits,
        ));
    }
    let digest = hash(ctx.main(), jobs, message)?;
    let actual = digest_limbs_assigned(ctx.main(), &digest);
    drop(ctx);
    for (actual, expected) in actual.into_iter().zip(expected) {
        constrain_loader_equal_if_v1(loader, actual, expected, enabled);
    }
    Ok(())
}

fn constrain_receive_batch_binding_v1<C>(
    loader: &DeferredLoader<'_, C>,
    jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
    state: &state_relation::OfflineCashAssignedStateRelationV1<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    let gate = halo2_base::gates::GateChip::default();
    let mut ctx = loader.ctx_mut();
    let mut message = constant_bytes(RECEIVE_BATCH_BINDING_DOMAIN_V1);
    macro_rules! append_le_bytes {
        ($value:expr, $bits:expr) => {{
            let bits = PastaSha256BitV1::decompose(ctx.main(), &gate, $value, $bits);
            for byte_bits in bits.chunks_exact(8) {
                message.push(PastaSha256ByteV1::from_bits_le(
                    ctx.main(),
                    &gate,
                    byte_bits,
                ));
            }
        }};
    }
    append_le_bytes!(state.receive_active_count, 8);
    for slot in &state.receive_slots {
        append_le_bytes!(slot.amount, 128);
        for digest in [
            slot.credit_id,
            slot.recipient_lane_id,
            slot.incoming_proof_binding_digest,
            slot.envelope_digest,
        ] {
            for limb in digest {
                append_le_bytes!(limb, 128);
            }
        }
    }
    let digest = hash(ctx.main(), jobs, message)?;
    let actual = digest_limbs_assigned(ctx.main(), &digest);
    let count_is_zero = gate.is_zero(ctx.main(), state.receive_active_count);
    let enabled = gate.not(ctx.main(), count_is_zero);
    drop(ctx);
    for (actual, expected) in actual.into_iter().zip(state.receive_batch_binding_digest) {
        constrain_loader_equal_if_v1(loader, actual, expected, enabled);
    }
    Ok(())
}

fn constrain_state_guard_binding_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    state: &state_relation::OfflineCashAssignedStateRelationV1<F>,
    guard: &OfflineCashAssignedGuardBundleV1<F>,
) -> Result<(), String> {
    let guard_digest = digest_limbs_assigned(builder.main(0), &guard.guard_digest);
    let scalar_pairs = [
        (state.successor.protocol_version, guard.protocol_version),
        (state.operation, guard.operation),
        (state.amount, guard.amount),
        (state.successor.scale, guard.asset_scale),
        (state.successor.policy_epoch, guard.policy_epoch),
        (state.receive_active_count, guard.receive_active_count),
        (state.predecessor.sequence, guard.predecessor_sequence),
        (state.successor.sequence, guard.successor_sequence),
        (
            state.predecessor.epoch_generation,
            guard.predecessor_generation,
        ),
        (state.successor.epoch_generation, guard.successor_generation),
        (state.journal_revision_before, guard.journal_before),
        (state.journal_revision_after, guard.journal_after),
    ];
    for (state_cell, guard_cell) in scalar_pairs {
        builder.main(0).constrain_equal(&state_cell, &guard_cell);
    }
    let digest_pairs = [
        (state.guard_digest, guard_digest),
        (state.predecessor.suite_id, guard.predecessor_suite_id),
        (state.predecessor.vk_digest, guard.predecessor_vk_digest),
        (state.successor.suite_id, guard.successor_suite_id),
        (state.successor.vk_digest, guard.successor_vk_digest),
        (state.peer_credit_id, guard.peer_credit_id),
        (state.peer_recipient_lane_id, guard.peer_recipient_lane_id),
        (
            state.mint_finality_proof_binding_digest,
            guard.mint_finality_proof_binding_digest,
        ),
        (state.successor.release_id, guard.release_id),
        (state.successor.network_id, guard.network_id),
        (state.successor.asset_id, guard.asset_id),
        (state.successor.asset_incarnation, guard.asset_incarnation),
        (state.successor.liability_pool_id, guard.liability_pool_id),
        (
            state.successor.hardware_profile_id,
            guard.hardware_profile_id,
        ),
        (state.successor.lane_id, guard.lane_id),
        (state.predecessor_outer, guard.predecessor_state),
        (state.successor_outer, guard.successor_state),
        (state.predecessor.nonce, guard.predecessor_nonce),
        (state.successor.nonce, guard.successor_nonce),
        (state.predecessor.epoch_id, guard.predecessor_epoch),
        (state.successor.epoch_id, guard.successor_epoch),
        (state.predecessor.key_reference, guard.predecessor_key),
        (state.successor.key_reference, guard.successor_key),
        (state.predecessor.policy_id, guard.predecessor_policy),
        (state.successor.policy_id, guard.successor_policy),
        (
            state.lifecycle_binding_digest,
            guard.lifecycle_binding_digest,
        ),
        (
            state.precommit_binding_digest,
            guard.precommit_binding_digest,
        ),
        (
            state.suite_upgrade_authorization_digest,
            guard.suite_upgrade_authorization_digest,
        ),
        (
            state.receive_batch_binding_digest,
            guard.receive_batch_binding_digest,
        ),
        (state.transition_effect_digest, guard.transition_effect),
    ];
    for (state_digest, guard_digest) in digest_pairs {
        for (state_cell, guard_cell) in state_digest.into_iter().zip(guard_digest) {
            builder.main(0).constrain_equal(&state_cell, &guard_cell);
        }
    }
    Ok(())
}

fn constrain_outer_state_head_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    components: OfflineCashPastaStateCommitmentV1,
    expected_eq_components: [AssignedValue<F>; 2],
    expected_ep_components: [AssignedValue<F>; 2],
    expected_outer: [AssignedValue<F>; 2],
    enabled: AssignedValue<F>,
) -> Result<(), String> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let eq: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &components.eq)
        .try_into()
        .expect("state Eq component width");
    let ep: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &components.ep)
        .try_into()
        .expect("state Ep component width");
    for (actual, expected) in digest_limbs_assigned(ctx, &eq)
        .into_iter()
        .zip(expected_eq_components)
        .chain(
            digest_limbs_assigned(ctx, &ep)
                .into_iter()
                .zip(expected_ep_components),
        )
    {
        ctx.constrain_equal(&actual, &expected);
    }
    let digest = hash(
        ctx,
        jobs,
        [
            constant_bytes(OFFLINE_CASH_PASTA_STATE_COMMITMENT_DOMAIN_V1),
            vec![PastaSha256ByteV1::constant(0)],
            eq.to_vec(),
            ep.to_vec(),
        ]
        .concat(),
    )?;
    for (actual, expected) in digest_limbs_assigned(ctx, &digest)
        .into_iter()
        .zip(expected_outer)
    {
        let difference = range.gate().sub(ctx, actual, expected);
        let selected = range.gate().mul(ctx, difference, enabled);
        range.gate().assert_is_const(ctx, &selected, &F::ZERO);
    }
    Ok(())
}

pub(super) fn eq_succinct_vk(params: &ParamsIPA<EqAffine>) -> IpaSuccinctVerifyingKey<EqAffine> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

pub(super) fn ep_succinct_vk(params: &ParamsIPA<EpAffine>) -> IpaSuccinctVerifyingKey<EpAffine> {
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_only_precommit_send_split_cannot_enter_receive_fold() {
        let state_precommit_instances =
            state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count();
        assert_eq!(state_precommit_instances, 115);
        assert_eq!(COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1, 81);

        assert!(
            validate_incoming_commit_wrapper_shape_v1(
                &[state_precommit_instances],
                [state_precommit_instances],
            )
            .is_err()
        );
        assert!(
            validate_incoming_commit_wrapper_shape_v1(
                &[COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1],
                [state_precommit_instances],
            )
            .is_err()
        );
        assert!(
            validate_incoming_commit_wrapper_shape_v1(
                &[COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1],
                [COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1],
            )
            .is_ok()
        );
        assert_ne!(
            state_relation::PUBLIC_INSTANCE_COUNT,
            COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1,
            "incoming history must start at the terminal wrapper prefix, never the state prefix",
        );
    }
}
