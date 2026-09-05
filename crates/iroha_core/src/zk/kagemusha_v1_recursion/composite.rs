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
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    util::arithmetic::{Domain, root_of_unity},
    verifier::plonk::PlonkProtocol,
};

use super::{
    KagemushaEpAccumulatorV1, KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1,
    KagemushaEqFoldProofV1, KagemushaGuardBundleRelationWitnessV1, KagemushaOperationV1,
    KagemushaPastaParityV1, KagemushaStateRelationWitnessV1,
    canonical_preimage::{
        assemble_bounded_canonical_frame_v1, stream::KagemushaBoundedByteStreamV1,
    },
    deferred_parent::{
        DeferredLoader, DeferredScalar, KagemushaDeferredParentOutputV1,
        KagemushaDeferredParentWitnessV1, accumulator_limb_count, bind_accumulator_limbs,
        constrain_parent_and_history_into_loader_v1, constrain_reciprocal_parent_pass_v1,
        deferred_field_chips_v1, deferred_loader_v1, finalize_deferred_audit_plan_v1,
        kagemusha_protocol_structure_digest_v1, load_and_constrain_parent_protocol_if_v1,
        load_and_constrain_parent_protocol_v1, load_native_accumulator,
        native_parent_protocol_digest_v1, select_accumulator_v1, verify_fold,
        verify_ordinary_proof_v1,
    },
    guard_bundle::{
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, KagemushaAssignedGuardBundleV1,
        assert_bytes_nonzero, assign_bytes, constant_bytes, constrain_guard_bundle_semantics_v1,
        digest_limbs_assigned, hash,
    },
    mint_authority::{
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, public_instance as mint_public_instance,
    },
    state_relation::{self, public_instance},
    terminal_authorization::{
        TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
        TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1, hash_terminal_send_output_binding_v1,
        public_instance as incoming_public_instance,
    },
};

const INCOMING_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1: usize =
    TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1;
const INCOMING_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1: usize =
    TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1;
use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    kagemusha_v1_state::{
        KAGEMUSHA_RECEIVE_FOLD_DOMAIN_V1, KagemushaMintFoldOpeningWitnessV1,
        mint_envelope_digest_v1,
    },
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_PASTA_STATE_COMMITMENT_DOMAIN_V1, KAGEMUSHA_PEER_CREDIT_OPENING_COMMITMENT_DOMAIN_V1,
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaLifecycleBindingV1, KagemushaOperationKindV1,
    KagemushaPastaStateCommitmentV1, kagemusha_canonical_mint_frame_prefix_v1,
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const PARENT_EQUATION_TAG: u32 = 1;
const INCOMING_CREDIT_EQUATION_TAG: u32 = 2;
const GUARD_BUNDLE_EQUATION_TAG: u32 = 3;
const MINT_FINALITY_EQUATION_TAG: u32 = 4;
// These private data-model domains are repeated next to the circuit relation deliberately. Native
// parity tests below pin them to the model-owned canonical digest APIs.
const MINT_FOLD_ASSET_IDENTITY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:asset-identity";
const MINT_FOLD_LIFECYCLE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:lifecycle-binding";
const MINT_FOLD_ASSET_PAYLOAD_BYTES_V1: usize = 32;
const MINT_FOLD_ASSET_FRAME_BYTES_V1: usize = 72;
const MINT_FOLD_LIFECYCLE_PAYLOAD_BYTES_V1: usize = 422;
const MINT_FOLD_LIFECYCLE_FRAME_BYTES_V1: usize = 462;

fn mint_fold_padding_lifecycle_v1(
    state: &KagemushaStateRelationWitnessV1,
) -> KagemushaLifecycleBindingV1 {
    let successor = &state.successor;
    KagemushaLifecycleBindingV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        network_id: successor.lane.network_id,
        protocol_version: successor.protocol_version,
        suite_id: successor.suite_id,
        vk_digest: successor.vk_digest,
        release_id: successor.release_id,
        asset: successor.lane.asset.clone(),
        asset_incarnation: successor.asset_incarnation,
        scale: successor.lane.scale,
        liability_pool_id: successor.liability_pool_id,
        hardware_profile_id: successor.hardware_profile_id,
        policy_epoch: successor.policy_epoch,
        operation_kind: KagemushaOperationKindV1::MintFold,
        request_id: [0; 32],
        receiver_lane_commitment: [0; 32],
        credit_id: successor.release_id,
        ciphertext_digest: successor.vk_digest,
    }
}

fn mint_fold_lifecycle_witness_v1(
    state: &KagemushaStateRelationWitnessV1,
    opening: Option<KagemushaMintFoldOpeningWitnessV1<'_>>,
) -> KagemushaLifecycleBindingV1 {
    opening.map_or_else(
        || mint_fold_padding_lifecycle_v1(state),
        |opening| opening.credit().statement.lifecycle.clone(),
    )
}

fn validate_mint_fold_opening_against_state_v1(
    state: &KagemushaStateRelationWitnessV1,
    opening: Option<KagemushaMintFoldOpeningWitnessV1<'_>>,
) -> Result<(), String> {
    let is_mint = state.operation == KagemushaOperationV1::MintFold;
    let Some(opening) = opening else {
        if is_mint {
            return Err("MintFold requires its checked-preview private opening".to_owned());
        }
        return mint_fold_padding_lifecycle_v1(state)
            .validate()
            .map_err(|error| format!("invalid MintFold lifecycle padding: {error}"));
    };
    if !is_mint {
        return Err("MintFold opening must be absent outside MintFold".to_owned());
    }

    let credit = opening.credit();
    credit
        .validate_shape_against_authorization(opening.authorization())
        .map_err(|error| format!("invalid checked MintFold opening: {error}"))?;
    let lifecycle = &credit.statement.lifecycle;
    let successor = &state.successor;
    let replay = state
        .replay_insert
        .as_ref()
        .ok_or_else(|| "MintFold replay insertion is absent".to_owned())?;
    let lifecycle_digest = lifecycle
        .canonical_digest()
        .map_err(|error| format!("invalid MintFold lifecycle: {error}"))?;
    let statement_digest = credit
        .statement
        .canonical_digest()
        .map_err(|error| format!("invalid MintFold credit statement: {error}"))?;
    let envelope_digest = mint_envelope_digest_v1(credit)
        .map_err(|error| format!("invalid MintFold credit envelope: {error}"))?;

    if lifecycle.version != KAGEMUSHA_WIRE_VERSION_V1
        || lifecycle.network_id != successor.lane.network_id
        || lifecycle.protocol_version != successor.protocol_version
        || lifecycle.suite_id != successor.suite_id
        || lifecycle.vk_digest != successor.vk_digest
        || lifecycle.release_id != successor.release_id
        || lifecycle.asset != successor.lane.asset
        || lifecycle.asset_incarnation != successor.asset_incarnation
        || lifecycle.scale != successor.lane.scale
        || lifecycle.liability_pool_id != successor.liability_pool_id
        || lifecycle.hardware_profile_id != successor.hardware_profile_id
        || lifecycle.policy_epoch != successor.policy_epoch
        || lifecycle.operation_kind != KagemushaOperationKindV1::MintFold
        || lifecycle.request_id != [0; 32]
        || lifecycle.receiver_lane_commitment != [0; 32]
        || lifecycle.credit_id != replay.credit_id
        || envelope_digest != replay.envelope_digest
        || credit.statement.amount != state.amount
        || lifecycle_digest != state.lifecycle_binding_digest
        || statement_digest != state.mint_finality_semantic_digest
        || credit.finality_proof_binding_digest != state.mint_finality_proof_binding_digest
    {
        return Err("checked MintFold opening does not match the aggregate transition".to_owned());
    }
    Ok(())
}

/// One Eq/Fp incoming sender proof slot consumed by `ReceiveFold`.
///
/// Inactive positions carry the release-pinned valid padding proof and history; only their
/// semantic slot data are canonical zero. This keeps proof verification shape fixed.
pub(super) struct KagemushaRecursiveIncomingEqWitnessV1<'a> {
    pub(super) instances: &'a [Vec<Fp>],
    pub(super) proof: &'a [u8],
    pub(super) history: &'a KagemushaEqAccumulatorV1,
    pub(super) history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(super) merge_fold_proof: &'a KagemushaEqFoldProofV1,
}

/// One Ep/Fq incoming sender proof slot consumed by `ReceiveFold`.
pub(super) struct KagemushaRecursiveIncomingEpWitnessV1<'a> {
    pub(super) instances: &'a [Vec<Fq>],
    pub(super) proof: &'a [u8],
    pub(super) history: &'a KagemushaEpAccumulatorV1,
    pub(super) history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(super) merge_fold_proof: &'a KagemushaEpFoldProofV1,
}

struct KagemushaRecursiveIncomingParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    instances: &'a [Vec<C::ScalarExt>],
    proof: &'a [u8],
    history: &'a IpaAccumulator<C, NativeLoader>,
    history_fold_proof: &'a [u8],
    merge_fold_proof: &'a [u8],
}

fn validate_incoming_authorization_proof_shape_v1(
    protocol_num_instance: &[usize],
    slot_instance_lengths: impl IntoIterator<Item = usize>,
) -> Result<(), String> {
    if protocol_num_instance != [INCOMING_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1] {
        return Err(
            "KAGEMUSHA incoming post-commit payment protocol has wrong public shape".to_owned(),
        );
    }
    if !slot_instance_lengths
        .into_iter()
        .eq(protocol_num_instance.iter().copied())
    {
        return Err(
            "KAGEMUSHA incoming post-commit payment proof has wrong public shape".to_owned(),
        );
    }
    Ok(())
}

/// One parity's predecessor and GuardBundle proof material consumed by the aggregate circuit.
pub(super) struct KagemushaRecursiveParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(super) mint_fold_opening: Option<KagemushaMintFoldOpeningWitnessV1<'a>>,
    pub(super) parent_protocol: &'a PlonkProtocol<C>,
    pub(super) parent_instances: &'a [Vec<C::ScalarExt>],
    pub(super) parent_proof: &'a [u8],
    pub(super) predecessor_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) parent_fold_proof: &'a [u8],
    pub(super) successor_history: &'a [u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    pub(super) incoming_protocol: &'a PlonkProtocol<C>,
    incoming_slots: &'a [KagemushaRecursiveIncomingParityWitnessV1<'a, C>],
    pub(super) guard_protocol: &'a PlonkProtocol<C>,
    pub(super) guard_proof: &'a [u8],
    pub(super) guard_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) guard_history_bytes: &'a [u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
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
pub(super) struct KagemushaRecursiveStateWitnessV1<'a> {
    pub(super) state: KagemushaStateRelationWitnessV1,
    pub(super) mint_fold_opening: Option<KagemushaMintFoldOpeningWitnessV1<'a>>,
    pub(super) guard_relation: KagemushaGuardBundleRelationWitnessV1,
    pub(super) eq_parent_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_parent_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_parent_instances: &'a [Vec<Fp>],
    pub(super) ep_parent_instances: &'a [Vec<Fq>],
    pub(super) eq_parent_proof: &'a [u8],
    pub(super) ep_parent_proof: &'a [u8],
    pub(super) eq_predecessor_history: &'a KagemushaEqAccumulatorV1,
    pub(super) ep_predecessor_history: &'a KagemushaEpAccumulatorV1,
    pub(super) eq_parent_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(super) ep_parent_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(super) eq_incoming_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_incoming_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_incoming_slots: &'a [KagemushaRecursiveIncomingEqWitnessV1<'a>;
            state_relation::KAGEMUSHA_RECEIVE_FOLD_ARITY_V1],
    pub(super) ep_incoming_slots: &'a [KagemushaRecursiveIncomingEpWitnessV1<'a>;
            state_relation::KAGEMUSHA_RECEIVE_FOLD_ARITY_V1],
    pub(super) eq_successor_history: &'a KagemushaEqAccumulatorV1,
    pub(super) ep_successor_history: &'a KagemushaEpAccumulatorV1,
    pub(super) eq_guard_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_guard_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_guard_proof: &'a [u8],
    pub(super) ep_guard_proof: &'a [u8],
    pub(super) eq_guard_history: &'a KagemushaEqAccumulatorV1,
    pub(super) ep_guard_history: &'a KagemushaEpAccumulatorV1,
    pub(super) eq_guard_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(super) ep_guard_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(super) eq_guard_merge_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(super) ep_guard_merge_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(super) eq_mint_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_mint_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) eq_mint_instances: &'a [Vec<Fp>],
    pub(super) ep_mint_instances: &'a [Vec<Fq>],
    pub(super) eq_mint_proof: &'a [u8],
    pub(super) ep_mint_proof: &'a [u8],
    pub(super) eq_mint_history: &'a KagemushaEqAccumulatorV1,
    pub(super) ep_mint_history: &'a KagemushaEpAccumulatorV1,
    pub(super) eq_mint_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(super) ep_mint_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(super) eq_mint_merge_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(super) ep_mint_merge_fold_proof: &'a KagemushaEpFoldProofV1,
}

/// Shared Base plus reciprocal dense-MSM configuration.
#[derive(Clone, Debug)]
pub(super) struct KagemushaRecursiveStateConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp half of one production recursive aggregate-state proof.
#[derive(Clone)]
pub(super) struct KagemushaRecursiveStateEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq half of one production recursive aggregate-state proof.
#[derive(Clone)]
pub(super) struct KagemushaRecursiveStateEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_recursive_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaRecursiveStateConfigV1<$field>;
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
                KagemushaRecursiveStateConfigV1 {
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
    KagemushaRecursiveStateEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq recursive state"
);
impl_recursive_circuit!(
    KagemushaRecursiveStateEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep recursive state"
);

/// Build both mutually-audited recursive circuits from one exact state transition.
pub(super) fn build_kagemusha_recursive_state_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaRecursiveStateWitnessV1<'_>,
) -> Result<
    (
        KagemushaRecursiveStateEqCircuitV1,
        KagemushaRecursiveStateEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    witness.state.validate()?;
    validate_mint_fold_opening_against_state_v1(&witness.state, witness.mint_fold_opening)?;
    witness.guard_relation.validate()?;
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
    let eq_incoming_protocol_digest =
        native_parent_protocol_digest_v1(witness.eq_incoming_protocol, KagemushaPastaParityV1::Eq)?;
    let ep_incoming_protocol_digest =
        native_parent_protocol_digest_v1(witness.ep_incoming_protocol, KagemushaPastaParityV1::Ep)?;
    if eq_incoming_protocol_digest != witness.state.commit_wrapper_eq_protocol_digest
        || ep_incoming_protocol_digest != witness.state.commit_wrapper_ep_protocol_digest
    {
        return Err(
            "Kagemusha incoming post-commit payment protocol differs from the state public identity"
                .to_owned(),
        );
    }
    let eq_incoming_slots = witness
        .eq_incoming_slots
        .iter()
        .zip(&eq_incoming_histories)
        .map(
            |(slot, history)| KagemushaRecursiveIncomingParityWitnessV1 {
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
            |(slot, history)| KagemushaRecursiveIncomingParityWitnessV1 {
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
        KagemushaPastaParityV1::Eq,
        KagemushaRecursiveParityWitnessV1 {
            mint_fold_opening: witness.mint_fold_opening,
            parent_protocol: witness.eq_parent_protocol,
            parent_instances: witness.eq_parent_instances,
            parent_proof: witness.eq_parent_proof,
            predecessor_history: &eq_history,
            parent_fold_proof: witness.eq_parent_fold_proof.as_bytes(),
            successor_history: witness.eq_successor_history.as_bytes(),
            incoming_protocol: witness.eq_incoming_protocol,
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
        KagemushaPastaParityV1::Ep,
        KagemushaRecursiveParityWitnessV1 {
            mint_fold_opening: witness.mint_fold_opening,
            parent_protocol: witness.ep_parent_protocol,
            parent_instances: witness.ep_parent_instances,
            parent_proof: witness.ep_parent_proof,
            predecessor_history: &ep_history,
            parent_fold_proof: witness.ep_parent_fold_proof.as_bytes(),
            successor_history: witness.ep_successor_history.as_bytes(),
            incoming_protocol: witness.ep_incoming_protocol,
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
        KagemushaPastaParityV1::Ep,
        &ep_output,
        &mut eq_dense,
    )?;
    let mut ep_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EqAffine>(
        &mut ep_builder,
        KagemushaPastaParityV1::Eq,
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
        KagemushaRecursiveStateEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_sha,
            dense_jobs: eq_dense,
        },
        KagemushaRecursiveStateEpCircuitV1 {
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
    state: KagemushaStateRelationWitnessV1,
    guard_relation: KagemushaGuardBundleRelationWitnessV1,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    witness: KagemushaRecursiveParityWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        PastaSha256JobsV1<C::ScalarExt>,
        KagemushaDeferredParentOutputV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let parent_enabled = state.operation != KagemushaOperationV1::Bootstrap;
    let mint_enabled = state.operation == KagemushaOperationV1::MintFold;
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
        .ok_or_else(|| "Kagemusha recursive state public column is absent".to_owned())?;
    let current_expected_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            KagemushaPastaParityV1::Eq => {
                public_instance::EQ_PROTOCOL_LO..public_instance::EQ_PROTOCOL_LO + 2
            }
            KagemushaPastaParityV1::Ep => {
                public_instance::EP_PROTOCOL_LO..public_instance::EP_PROTOCOL_LO + 2
            }
        })
        .ok_or_else(|| "Kagemusha recursive protocol public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha recursive protocol public limbs have wrong shape".to_owned())?;
    let expected_predecessor_state = public[public_instance::PREDECESSOR_STATE];
    let expected_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            KagemushaPastaParityV1::Eq => {
                public_instance::EQ_DEFERRED_AUDIT_LO..public_instance::EQ_DEFERRED_AUDIT_LO + 2
            }
            KagemushaPastaParityV1::Ep => {
                public_instance::EP_DEFERRED_AUDIT_LO..public_instance::EP_DEFERRED_AUDIT_LO + 2
            }
        })
        .ok_or_else(|| "Kagemusha recursive audit public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha recursive audit public limbs have wrong shape".to_owned())?;
    let expected_guard_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            KagemushaPastaParityV1::Eq => {
                public_instance::GUARD_EQ_PROTOCOL_LO..public_instance::GUARD_EQ_PROTOCOL_LO + 2
            }
            KagemushaPastaParityV1::Ep => {
                public_instance::GUARD_EP_PROTOCOL_LO..public_instance::GUARD_EP_PROTOCOL_LO + 2
            }
        })
        .ok_or_else(|| "Kagemusha GuardBundle protocol public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha GuardBundle protocol public limbs have wrong shape".to_owned())?;
    let expected_mint_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(match parity {
            KagemushaPastaParityV1::Eq => {
                public_instance::MINT_EQ_PROTOCOL_LO..public_instance::MINT_EQ_PROTOCOL_LO + 2
            }
            KagemushaPastaParityV1::Ep => {
                public_instance::MINT_EP_PROTOCOL_LO..public_instance::MINT_EP_PROTOCOL_LO + 2
            }
        })
        .ok_or_else(|| "Kagemusha mint protocol public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha mint protocol public limbs have wrong shape".to_owned())?;
    let expected_incoming_eq_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(
            public_instance::COMMIT_WRAPPER_EQ_PROTOCOL_LO
                ..public_instance::COMMIT_WRAPPER_EQ_PROTOCOL_LO + 2,
        )
        .ok_or_else(|| "Kagemusha Eq commit-wrapper protocol limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha Eq commit-wrapper protocol limbs have wrong shape".to_owned())?;
    let expected_incoming_ep_protocol: [AssignedValue<C::ScalarExt>; 2] = public
        .get(
            public_instance::COMMIT_WRAPPER_EP_PROTOCOL_LO
                ..public_instance::COMMIT_WRAPPER_EP_PROTOCOL_LO + 2,
        )
        .ok_or_else(|| "Kagemusha Ep commit-wrapper protocol limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha Ep commit-wrapper protocol limbs have wrong shape".to_owned())?;
    let guard_digest: [AssignedValue<C::ScalarExt>; 2] = public
        .get(public_instance::GUARD_LO..public_instance::GUARD_HI + 1)
        .ok_or_else(|| "Kagemusha GuardBundle public digest is absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha GuardBundle public digest has wrong shape".to_owned())?;
    let guard_eq_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(
            public_instance::GUARD_EQ_CREDENTIAL_AUDIT_LO
                ..public_instance::GUARD_EQ_CREDENTIAL_AUDIT_LO + 2,
        )
        .ok_or_else(|| "Kagemusha Eq credential audit public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha Eq credential audit public limbs have wrong shape".to_owned())?;
    let guard_ep_audit: [AssignedValue<C::ScalarExt>; 2] = public
        .get(
            public_instance::GUARD_EP_CREDENTIAL_AUDIT_LO
                ..public_instance::GUARD_EP_CREDENTIAL_AUDIT_LO + 2,
        )
        .ok_or_else(|| "Kagemusha Ep credential audit public limbs are absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha Ep credential audit public limbs have wrong shape".to_owned())?;

    let range = builder.range_chip();
    let operation = builder.assigned_instances[0][public_instance::OPERATION];
    let bootstrap = range.gate().is_zero(builder.main(0), operation);
    let mint = range.gate().is_equal(
        builder.main(0),
        operation,
        halo2_base::QuantumCell::Constant(C::ScalarExt::ONE),
    );
    let non_bootstrap = range.gate().not(builder.main(0), bootstrap);
    constrain_mint_fold_opening_v1(
        &mut builder,
        &mut sha_jobs,
        &assigned_state,
        &state,
        witness.mint_fold_opening,
        mint,
    )?;
    let expected_protocol = current_expected_protocol;
    let predecessor_components = state
        .predecessor
        .as_ref()
        .map_or(KagemushaPastaStateCommitmentV1::ZERO, |state| {
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
    let structure = kagemusha_protocol_structure_digest_v1(witness.parent_protocol, parity)?;
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
        KagemushaDeferredParentWitnessV1 {
            instances: witness.parent_instances,
            proof_bytes: witness.parent_proof,
            predecessor_history: witness.predecessor_history,
            fold_proof_bytes: witness.parent_fold_proof,
        },
        expected_predecessor_state,
        assigned_state.predecessor_outer,
        [expected_incoming_eq_protocol, expected_incoming_ep_protocol],
        non_bootstrap,
        &loader,
    )
    .map_err(|error| format!("failed to verify/fold predecessor proof: {error:?}"))?;
    let parent_end = loader.ecc_chip().equation_count();
    if parent_end == 0 {
        return Err("Kagemusha predecessor verifier emitted no equations".to_owned());
    }

    if witness.incoming_slots.len() != state_relation::KAGEMUSHA_RECEIVE_FOLD_ARITY_V1 {
        return Err("Kagemusha incoming sender proof has wrong fixed arity".to_owned());
    }
    // Incoming monetary authority is a release-pinned commit-wrapper verifier.
    // Its self-referential preprocessed points are witnesses so the production key graph remains
    // acyclic. The value-free structure stays fixed in the State key, while the complete native
    // protocol identity is recomputed in-circuit and constrained to the authenticated public ABI.
    let incoming_structure =
        kagemusha_protocol_structure_digest_v1(witness.incoming_protocol, parity)?;
    let expected_incoming_protocol = match parity {
        KagemushaPastaParityV1::Eq => expected_incoming_eq_protocol,
        KagemushaPastaParityV1::Ep => expected_incoming_ep_protocol,
    };
    let incoming_protocol = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.incoming_protocol,
        parity,
        incoming_structure,
        &expected_incoming_protocol,
    )
    .map_err(|error| format!("failed to bind incoming post-commit payment protocol: {error:?}"))?
    .protocol;
    let mut state_history = base_successor_history;
    let mut incoming_equation_spans = Vec::with_capacity(witness.incoming_slots.len());
    let mut previous_incoming_end = parent_end;
    for (index, (slot, assigned_slot)) in witness
        .incoming_slots
        .iter()
        .zip(core::iter::once(&assigned_state.receive_credit))
        .enumerate()
    {
        validate_incoming_authorization_proof_shape_v1(
            &witness.incoming_protocol.num_instance,
            slot.instances.iter().map(Vec::len),
        )
        .map_err(|error| format!("Kagemusha incoming sender proof slot {index}: {error}"))?;
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
        let incoming_column = incoming_instances
            .first()
            .ok_or_else(|| format!("Kagemusha incoming sender public column {index} is absent"))?;
        let incoming_history = load_native_accumulator(&loader, slot.history).map_err(|error| {
            format!("failed to load incoming sender history slot {index}: {error:?}")
        })?;
        let incoming_history_limbs = incoming_column
            .get(INCOMING_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1..)
            .ok_or_else(|| format!("Kagemusha incoming sender history {index} is absent"))?
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
            assigned_slot.active,
            expected_incoming_eq_protocol,
            expected_incoming_ep_protocol,
        )?;
        constrain_incoming_common_binding_v1(
            &loader,
            &mut sha_jobs,
            incoming_column,
            assigned_slot,
            assigned_slot.active,
            expected_incoming_eq_protocol,
            expected_incoming_ep_protocol,
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
        .map_err(|error| format!("failed to select ReceiveFold history slot {index}: {error:?}"))?;
        let slot_end = loader.ecc_chip().equation_count();
        if slot_end <= previous_incoming_end {
            return Err(format!(
                "Kagemusha incoming sender verifier slot {index} emitted no equations"
            ));
        }
        incoming_equation_spans.push((
            slot_end - previous_incoming_end,
            assigned_slot.active,
            state.receive_credit.is_some(),
        ));
        previous_incoming_end = slot_end;
    }
    let incoming_end = previous_incoming_end;
    if incoming_end <= parent_end {
        return Err("Kagemusha incoming sender verifier emitted no equations".to_owned());
    }
    constrain_receive_credit_binding_v1(&loader, &mut sha_jobs, &assigned_state)?;

    if witness.guard_protocol.num_instance != [GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1] {
        return Err("Kagemusha GuardBundle proof has wrong public shape".to_owned());
    }
    let guard_structure = kagemusha_protocol_structure_digest_v1(witness.guard_protocol, parity)?;
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
        return Err("Kagemusha GuardBundle verifier emitted no equations".to_owned());
    }

    if witness.mint_protocol.num_instance != [KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        || witness.mint_instances.len() != 1
        || witness.mint_instances[0].len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("Kagemusha finalized-mint proof has wrong public shape".to_owned());
    }
    let mint_structure = kagemusha_protocol_structure_digest_v1(witness.mint_protocol, parity)?;
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
        .ok_or_else(|| "Kagemusha mint public column is absent".to_owned())?;
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
        .ok_or_else(|| "Kagemusha finalized-mint history is absent".to_owned())?
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
        return Err("Kagemusha finalized-mint verifier emitted no equations".to_owned());
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

fn assign_history_limbs<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &halo2_base::gates::RangeChip<F>,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
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
        return Err("Kagemusha history limb count is not fixed".to_owned());
    }
    Ok(limbs)
}

fn constrain_byte_bit_pattern_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    byte: PastaSha256ByteV1<F>,
    expected_bits: &[(usize, bool)],
    enabled: AssignedValue<F>,
) -> Result<(), String> {
    let assigned = byte
        .assigned()
        .ok_or_else(|| "canonical typed identity byte is not witness-backed".to_owned())?;
    let gate = range.gate();
    let bits = gate.num_to_bits(ctx, assigned, 8);
    for &(index, expected) in expected_bits {
        let expected = halo2_base::QuantumCell::Constant(if expected { F::ONE } else { F::ZERO });
        let difference = gate.sub(ctx, bits[index], expected);
        let selected = gate.mul(ctx, difference, enabled);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    Ok(())
}

fn mint_fold_asset_payload_v1<F: KagemushaPoseidonFieldV1>(
    asset_bytes: &[PastaSha256ByteV1<F>],
) -> Result<Vec<PastaSha256ByteV1<F>>, String> {
    if asset_bytes.len() != 16 {
        return Err("canonical AssetDefinitionId UUID width changed".to_owned());
    }
    // AssetDefinitionId delegates to Norito's generic `[u8; 16]` serializer. Each byte therefore
    // has its own compact one-byte length prefix in the root payload and in the lifecycle field.
    let mut payload = Vec::with_capacity(MINT_FOLD_ASSET_PAYLOAD_BYTES_V1);
    for byte in asset_bytes.iter().copied() {
        payload.push(PastaSha256ByteV1::constant(1));
        payload.push(byte);
    }
    if payload.len() != MINT_FOLD_ASSET_PAYLOAD_BYTES_V1 {
        return Err("canonical AssetDefinitionId payload width changed".to_owned());
    }
    Ok(payload)
}

struct MintFoldLifecyclePayloadBytesV1<'a, F: KagemushaPoseidonFieldV1> {
    network_id: &'a [PastaSha256ByteV1<F>],
    protocol_version: &'a [PastaSha256ByteV1<F>],
    suite_id: &'a [PastaSha256ByteV1<F>],
    vk_digest: &'a [PastaSha256ByteV1<F>],
    release_id: &'a [PastaSha256ByteV1<F>],
    asset: &'a [PastaSha256ByteV1<F>],
    asset_incarnation: &'a [PastaSha256ByteV1<F>],
    scale: &'a [PastaSha256ByteV1<F>],
    liability_pool_id: &'a [PastaSha256ByteV1<F>],
    hardware_profile_id: &'a [PastaSha256ByteV1<F>],
    policy_epoch: &'a [PastaSha256ByteV1<F>],
    credit_id: &'a [PastaSha256ByteV1<F>],
    ciphertext_digest: &'a [PastaSha256ByteV1<F>],
}

fn mint_fold_lifecycle_payload_v1<F: KagemushaPoseidonFieldV1>(
    fields: MintFoldLifecyclePayloadBytesV1<'_, F>,
) -> Result<Vec<PastaSha256ByteV1<F>>, String> {
    if fields.network_id.len() != 32
        || fields.protocol_version.len() != 2
        || fields.suite_id.len() != 32
        || fields.vk_digest.len() != 32
        || fields.release_id.len() != 32
        || fields.asset.len() != MINT_FOLD_ASSET_PAYLOAD_BYTES_V1
        || fields.asset_incarnation.len() != 32
        || fields.scale.len() != 4
        || fields.liability_pool_id.len() != 32
        || fields.hardware_profile_id.len() != 32
        || fields.policy_epoch.len() != 8
        || fields.credit_id.len() != 32
        || fields.ciphertext_digest.len() != 32
    {
        return Err("canonical MintFold lifecycle field width changed".to_owned());
    }

    let mut payload = Vec::with_capacity(MINT_FOLD_LIFECYCLE_PAYLOAD_BYTES_V1);
    payload.push(PastaSha256ByteV1::constant(2));
    payload.extend(constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()));
    payload.push(PastaSha256ByteV1::constant(32));
    payload.extend_from_slice(fields.network_id);
    payload.push(PastaSha256ByteV1::constant(2));
    payload.extend_from_slice(fields.protocol_version);
    for digest in [fields.suite_id, fields.vk_digest, fields.release_id] {
        payload.push(PastaSha256ByteV1::constant(32));
        payload.extend_from_slice(digest);
    }
    payload.push(PastaSha256ByteV1::constant(32));
    payload.extend_from_slice(fields.asset);
    payload.extend(constant_bytes(&[33, 32]));
    payload.extend_from_slice(fields.asset_incarnation);
    payload.push(PastaSha256ByteV1::constant(4));
    payload.extend_from_slice(fields.scale);
    for digest in [fields.liability_pool_id, fields.hardware_profile_id] {
        payload.push(PastaSha256ByteV1::constant(32));
        payload.extend_from_slice(digest);
    }
    payload.push(PastaSha256ByteV1::constant(8));
    payload.extend_from_slice(fields.policy_epoch);
    payload.push(PastaSha256ByteV1::constant(4));
    payload.extend(constant_bytes(&1_u32.to_le_bytes()));
    for _ in 0..2 {
        payload.push(PastaSha256ByteV1::constant(32));
        payload.extend(constant_bytes(&[0; 32]));
    }
    payload.push(PastaSha256ByteV1::constant(32));
    payload.extend_from_slice(fields.credit_id);
    payload.push(PastaSha256ByteV1::constant(32));
    payload.extend_from_slice(fields.ciphertext_digest);
    if payload.len() != MINT_FOLD_LIFECYCLE_PAYLOAD_BYTES_V1 {
        return Err("canonical MintFold lifecycle payload width changed".to_owned());
    }
    Ok(payload)
}

fn constrain_mint_fold_opening_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &state_relation::KagemushaAssignedStateRelationV1<F>,
    native_state: &KagemushaStateRelationWitnessV1,
    opening: Option<KagemushaMintFoldOpeningWitnessV1<'_>>,
    mint: AssignedValue<F>,
) -> Result<(), String> {
    let lifecycle = mint_fold_lifecycle_witness_v1(native_state, opening);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();

    let network_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.network_id);
    let suite_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.suite_id);
    let vk_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.vk_digest);
    let release_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.release_id);
    let incarnation_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.asset_incarnation);
    let pool_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.liability_pool_id);
    let profile_bytes = assigned_digest_bytes_v1(ctx, gate, state.successor.hardware_profile_id);
    let replay_credit_id_bytes = assigned_digest_bytes_v1(ctx, gate, state.replay_credit_id);
    let protocol_bytes = assigned_uint_bytes_v1(ctx, gate, state.successor.protocol_version, 16);
    let scale_bytes = assigned_uint_bytes_v1(ctx, gate, state.successor.scale, 32);
    let policy_epoch_bytes = assigned_uint_bytes_v1(ctx, gate, state.successor.policy_epoch, 64);
    let asset_bytes = assign_bytes(ctx, &range, &lifecycle.asset.aid_bytes());
    let ciphertext_bytes = assign_bytes(ctx, &range, &lifecycle.ciphertext_digest);
    let credit_id_bytes = replay_credit_id_bytes
        .iter()
        .copied()
        .zip(release_bytes.iter().copied())
        .map(|(replay, padding)| {
            let selected = gate.select(ctx, replay.quantum_cell(), padding.quantum_cell(), mint);
            PastaSha256ByteV1::range_checked(ctx, &range, selected)
        })
        .collect::<Vec<_>>();
    assert_bytes_nonzero(ctx, &range, &credit_id_bytes);
    assert_bytes_nonzero(ctx, &range, &ciphertext_bytes);

    // Network IDs and asset incarnations wrap canonical Iroha hashes, whose final byte has its
    // least-significant marker bit set. Asset IDs carry UUIDv4 version and RFC4122 variant bits.
    constrain_byte_bit_pattern_if_v1(ctx, &range, network_bytes[31], &[(0, true)], mint)?;
    constrain_byte_bit_pattern_if_v1(ctx, &range, incarnation_bytes[31], &[(0, true)], mint)?;
    constrain_byte_bit_pattern_if_v1(
        ctx,
        &range,
        asset_bytes[6],
        &[(4, false), (5, false), (6, true), (7, false)],
        mint,
    )?;
    constrain_byte_bit_pattern_if_v1(ctx, &range, asset_bytes[8], &[(6, false), (7, true)], mint)?;

    let asset_payload = mint_fold_asset_payload_v1(&asset_bytes)?;
    let asset_payload_len = ctx.load_constant(F::from(
        u64::try_from(MINT_FOLD_ASSET_PAYLOAD_BYTES_V1).expect("asset payload width fits u64"),
    ));
    let asset_payload_stream = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        asset_payload.clone(),
        asset_payload_len,
    )?;
    let asset_prefix = kagemusha_canonical_mint_frame_prefix_v1(&lifecycle.asset)
        .map_err(|error| format!("invalid canonical AssetDefinitionId framing: {error}"))?;
    if asset_prefix.payload_offset() != 40 {
        return Err("canonical AssetDefinitionId payload offset changed".to_owned());
    }
    let asset_frame =
        assemble_bounded_canonical_frame_v1(ctx, &range, &asset_prefix, &asset_payload_stream)?;
    if asset_frame.bytes().len() != MINT_FOLD_ASSET_FRAME_BYTES_V1 {
        return Err("canonical AssetDefinitionId frame width changed".to_owned());
    }
    let mut asset_digest_message = constant_bytes(MINT_FOLD_ASSET_IDENTITY_DIGEST_DOMAIN_V1);
    asset_digest_message.push(PastaSha256ByteV1::constant(0));
    asset_digest_message.extend(constant_bytes(
        &u64::try_from(asset_frame.bytes().len())
            .expect("asset frame width fits u64")
            .to_le_bytes(),
    ));
    asset_digest_message.extend_from_slice(asset_frame.bytes());
    let asset_digest = hash(ctx, jobs, asset_digest_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &asset_digest)
        .into_iter()
        .zip(state.successor.asset_id)
    {
        let difference = gate.sub(ctx, actual, expected);
        let selected = gate.mul(ctx, difference, mint);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }

    let payload = mint_fold_lifecycle_payload_v1(MintFoldLifecyclePayloadBytesV1 {
        network_id: &network_bytes,
        protocol_version: &protocol_bytes,
        suite_id: &suite_bytes,
        vk_digest: &vk_bytes,
        release_id: &release_bytes,
        asset: &asset_payload,
        asset_incarnation: &incarnation_bytes,
        scale: &scale_bytes,
        liability_pool_id: &pool_bytes,
        hardware_profile_id: &profile_bytes,
        policy_epoch: &policy_epoch_bytes,
        credit_id: &credit_id_bytes,
        ciphertext_digest: &ciphertext_bytes,
    })?;

    let lifecycle_payload_len = ctx.load_constant(F::from(
        u64::try_from(MINT_FOLD_LIFECYCLE_PAYLOAD_BYTES_V1)
            .expect("lifecycle payload width fits u64"),
    ));
    let lifecycle_payload =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, payload, lifecycle_payload_len)?;
    let lifecycle_prefix = kagemusha_canonical_mint_frame_prefix_v1(&lifecycle)
        .map_err(|error| format!("invalid canonical MintFold lifecycle framing: {error}"))?;
    if lifecycle_prefix.payload_offset() != 40 {
        return Err("canonical MintFold lifecycle payload offset changed".to_owned());
    }
    let lifecycle_frame =
        assemble_bounded_canonical_frame_v1(ctx, &range, &lifecycle_prefix, &lifecycle_payload)?;
    if lifecycle_frame.bytes().len() != MINT_FOLD_LIFECYCLE_FRAME_BYTES_V1 {
        return Err("canonical MintFold lifecycle frame width changed".to_owned());
    }
    let mut lifecycle_digest_message = constant_bytes(MINT_FOLD_LIFECYCLE_DIGEST_DOMAIN_V1);
    lifecycle_digest_message.push(PastaSha256ByteV1::constant(0));
    lifecycle_digest_message.extend(constant_bytes(
        &u64::try_from(lifecycle_frame.bytes().len())
            .expect("lifecycle frame width fits u64")
            .to_le_bytes(),
    ));
    lifecycle_digest_message.extend_from_slice(lifecycle_frame.bytes());
    let lifecycle_digest = hash(ctx, jobs, lifecycle_digest_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &lifecycle_digest)
        .into_iter()
        .zip(state.lifecycle_binding_digest)
    {
        let difference = gate.sub(ctx, actual, expected);
        let selected = gate.mul(ctx, difference, mint);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }

    // TODO: bind `replay_envelope_digest` to the exact variable-size canonical credit frame when
    // the bounded proof-byte frame relation is integrated. This tranche binds its exact replay key.
    Ok(())
}

fn constrain_incoming_scalar_if_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
    incoming: &[DeferredScalar<'chip, C>],
    state: &state_relation::KagemushaAssignedStateRelationV1<C::ScalarExt>,
    credit: &state_relation::KagemushaAssignedReceiveFoldCreditV1<C::ScalarExt>,
    enabled: AssignedValue<C::ScalarExt>,
    incoming_eq_protocol: [AssignedValue<C::ScalarExt>; 2],
    incoming_ep_protocol: [AssignedValue<C::ScalarExt>; 2],
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if incoming.len() != INCOMING_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err(
            "KAGEMUSHA incoming post-commit payment public instance is truncated".to_owned(),
        );
    }
    let send_tag = loader.ctx_mut().main().load_constant(C::ScalarExt::from(2));
    constrain_loader_equal_if_v1(
        loader,
        *incoming[incoming_public_instance::OPERATION].assigned(),
        send_tag,
        enabled,
    );
    constrain_loader_equal_if_v1(
        loader,
        *incoming[incoming_public_instance::AMOUNT].assigned(),
        credit.amount,
        enabled,
    );
    constrain_loader_equal_if_v1(
        loader,
        *incoming[incoming_public_instance::PROTOCOL_VERSION].assigned(),
        state.successor.protocol_version,
        enabled,
    );
    constrain_loader_equal_if_v1(
        loader,
        *incoming[incoming_public_instance::ASSET_SCALE].assigned(),
        state.successor.scale,
        enabled,
    );
    for (offset, expected) in [
        (incoming_public_instance::SUITE_LO, state.successor.suite_id),
        (incoming_public_instance::VK_LO, state.successor.vk_digest),
        (
            incoming_public_instance::RELEASE_LO,
            state.successor.release_id,
        ),
        (
            incoming_public_instance::NETWORK_LO,
            state.successor.network_id,
        ),
        (incoming_public_instance::ASSET_LO, state.successor.asset_id),
        (
            incoming_public_instance::ASSET_INCARNATION_LO,
            state.successor.asset_incarnation,
        ),
        (
            incoming_public_instance::LIABILITY_POOL_LO,
            state.successor.liability_pool_id,
        ),
    ] {
        for (index, expected) in (offset..offset + 2).zip(expected) {
            constrain_loader_equal_if_v1(loader, *incoming[index].assigned(), expected, enabled);
        }
    }

    // Every slot, including inactive padding, is a real release-pinned final payment proof. Bind its
    // protocol pair unconditionally so padding cannot smuggle an alternate verifier identity.
    for (offset, expected) in [
        (
            incoming_public_instance::EQ_PROTOCOL_LO,
            incoming_eq_protocol,
        ),
        (
            incoming_public_instance::EP_PROTOCOL_LO,
            incoming_ep_protocol,
        ),
    ] {
        for (index, expected) in (offset..offset + 2).zip(expected) {
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&incoming[index].assigned(), &expected);
        }
    }

    for (offset, expected) in [
        (incoming_public_instance::REQUEST_LO, credit.request_digest),
        (
            incoming_public_instance::TRANSITION_NULLIFIER_LO,
            credit.transition_nullifier,
        ),
        (
            incoming_public_instance::RECEIVER_BINDING_LO,
            credit.receiver_binding_digest,
        ),
        (
            incoming_public_instance::CIPHERTEXT_LO,
            credit.ciphertext_commitment,
        ),
    ] {
        for (index, expected) in (offset..offset + 2).zip(expected) {
            constrain_loader_equal_if_v1(loader, *incoming[index].assigned(), expected, enabled);
        }
    }
    // The final proof independently authenticates an enabled sender hardware profile and policy.
    // They need not equal the receiving wallet's profile or policy epoch.
    let _ = jobs;
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
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if mint.len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        || state_public.len() < state_relation::PUBLIC_INSTANCE_COUNT
    {
        return Err("Kagemusha finalized-mint binding input is truncated".to_owned());
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
    credit: &state_relation::KagemushaAssignedReceiveFoldCreditV1<C::ScalarExt>,
    enabled: AssignedValue<C::ScalarExt>,
    _incoming_eq_protocol: [AssignedValue<C::ScalarExt>; 2],
    _incoming_ep_protocol: [AssignedValue<C::ScalarExt>; 2],
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if incoming.len() < INCOMING_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1 {
        return Err("KAGEMUSHA incoming post-commit proof prefix is truncated".to_owned());
    }
    let gate = halo2_base::gates::GateChip::default();
    let mut ctx = loader.ctx_mut();
    let request_bytes = assigned_digest_bytes_v1(ctx.main(), &gate, credit.request_digest);
    let amount_bytes = assigned_uint_bytes_v1(ctx.main(), &gate, credit.amount, 128);
    let recipient_key_bytes =
        assigned_digest_bytes_v1(ctx.main(), &gate, credit.recipient_encryption_key);
    let recipient_lane_bytes =
        assigned_digest_bytes_v1(ctx.main(), &gate, credit.recipient_lane_id);
    let credit_commitment_opening_bytes =
        assigned_digest_bytes_v1(ctx.main(), &gate, credit.credit_commitment_opening);
    let recipient_binding_opening_bytes =
        assigned_digest_bytes_v1(ctx.main(), &gate, credit.recipient_binding_opening);
    let recovery_nonce_bytes = assigned_digest_bytes_v1(ctx.main(), &gate, credit.recovery_nonce);
    let output_bytes = assigned_digest_bytes_v1(ctx.main(), &gate, credit.payment_output_digest);
    let credit_id_bytes = assigned_digest_bytes_v1(ctx.main(), &gate, credit.credit_id);
    let prepared_bytes =
        assigned_digest_bytes_v1(ctx.main(), &gate, credit.prepared_transfer_digest);
    let incoming_claims_bytes =
        assigned_digest_bytes_v1(ctx.main(), &gate, credit.incoming_proof_binding_digest);
    let opening_commitment = hash_receiver_plaintext_opening_v1(
        ctx.main(),
        jobs,
        [
            &request_bytes,
            &recipient_key_bytes,
            &credit_commitment_opening_bytes,
            &recipient_binding_opening_bytes,
            &recovery_nonce_bytes,
        ],
        &amount_bytes,
    )?;
    // TerminalAuthorization derives intent, prepared transfer, body, credit ID, and the exact
    // candidate/certificate claims once. Its verified CommitWrapper exposes their shared binding.
    // Keep a separate plaintext-opening hash here: the sender's proof cannot establish that this
    // recipient knows the recovered credit's secrets.
    let output_binding = hash_terminal_send_output_binding_v1(
        ctx.main(),
        jobs,
        [
            &credit_id_bytes,
            &recipient_key_bytes,
            &recipient_lane_bytes,
            &prepared_bytes,
            &output_bytes,
            &incoming_claims_bytes,
        ],
    )?;

    let output_binding_limbs = digest_limbs_assigned(ctx.main(), &output_binding);
    let opening_limbs = digest_limbs_assigned(ctx.main(), &opening_commitment);
    drop(ctx);
    for (index, actual) in output_binding_limbs.into_iter().enumerate() {
        constrain_loader_equal_if_v1(
            loader,
            actual,
            *incoming[incoming_public_instance::OUTPUT_BINDING_LO + index].assigned(),
            enabled,
        );
    }
    for (actual, expected) in opening_limbs.into_iter().zip(credit.ciphertext_commitment) {
        constrain_loader_equal_if_v1(loader, actual, expected, enabled);
    }
    Ok(())
}

/// Keep recipient plaintext knowledge separate from the sender-authenticated output claims.
fn hash_receiver_plaintext_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    digests: [&[PastaSha256ByteV1<F>]; 5],
    amount: &[PastaSha256ByteV1<F>],
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if digests.iter().any(|digest| digest.len() != 32) || amount.len() != 16 {
        return Err("receiver plaintext-opening transcript width changed".to_owned());
    }
    let mut message = constant_bytes(KAGEMUSHA_PEER_CREDIT_OPENING_COMMITMENT_DOMAIN_V1);
    message.extend(constant_bytes(&[0]));
    message.extend(constant_bytes(&1_u16.to_le_bytes()));
    // Request, recipient key, amount, and the three receiver-only opening secrets.
    for digest in &digests[..2] {
        message.extend_from_slice(digest);
    }
    message.extend_from_slice(amount);
    for digest in &digests[2..] {
        message.extend_from_slice(digest);
    }
    hash(ctx, jobs, message)
}

fn assigned_uint_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    value: AssignedValue<F>,
    bit_len: usize,
) -> Vec<PastaSha256ByteV1<F>> {
    PastaSha256BitV1::decompose(ctx, gate, value, bit_len)
        .chunks_exact(8)
        .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, gate, bits))
        .collect()
}

fn assigned_digest_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    digest: [AssignedValue<F>; 2],
) -> Vec<PastaSha256ByteV1<F>> {
    digest
        .into_iter()
        .flat_map(|limb| assigned_uint_bytes_v1(ctx, gate, limb, 128))
        .collect()
}

fn constrain_receive_credit_binding_v1<C>(
    loader: &DeferredLoader<'_, C>,
    jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
    state: &state_relation::KagemushaAssignedStateRelationV1<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let gate = halo2_base::gates::GateChip::default();
    let mut ctx = loader.ctx_mut();
    let mut message = constant_bytes(KAGEMUSHA_RECEIVE_FOLD_DOMAIN_V1);
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
    let credit = &state.receive_credit;
    append_le_bytes!(credit.amount, 128);
    for digest in [
        credit.credit_id,
        credit.recipient_lane_id,
        credit.incoming_proof_binding_digest,
        credit.receiver_binding_digest,
        credit.payment_output_digest,
        credit.envelope_digest,
    ] {
        for limb in digest {
            append_le_bytes!(limb, 128);
        }
    }
    let digest = hash(ctx.main(), jobs, message)?;
    let actual = digest_limbs_assigned(ctx.main(), &digest);
    let enabled = credit.active;
    drop(ctx);
    for (actual, expected) in actual.into_iter().zip(state.receive_credit_binding_digest) {
        constrain_loader_equal_if_v1(loader, actual, expected, enabled);
    }
    Ok(())
}

fn constrain_state_guard_binding_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    state: &state_relation::KagemushaAssignedStateRelationV1<F>,
    guard: &KagemushaAssignedGuardBundleV1<F>,
) -> Result<(), String> {
    let guard_digest = digest_limbs_assigned(builder.main(0), &guard.guard_digest);
    let scalar_pairs = [
        (state.successor.protocol_version, guard.protocol_version),
        (state.operation, guard.operation),
        (state.amount, guard.amount),
        (state.successor.scale, guard.asset_scale),
        (state.successor.policy_epoch, guard.policy_epoch),
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
        (state.predecessor.release_id, guard.predecessor_release_id),
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
            state.receive_credit_binding_digest,
            guard.receive_credit_binding_digest,
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

fn constrain_outer_state_head_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    components: KagemushaPastaStateCommitmentV1,
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
            constant_bytes(KAGEMUSHA_PASTA_STATE_COMMITMENT_DOMAIN_V1),
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
    use super::super::DigestV1;
    use super::super::terminal_authorization::{
        constrain_receiver_credential_lane_v1, hash_incoming_payment_claims_binding_v1,
        hash_terminal_prepared_transfer_v1,
    };
    use super::*;
    use halo2_base::gates::RangeChip;
    use halo2_proofs::dev::MockProver;
    use sha2::{Digest as _, Sha256};

    const RECEIVER_LANE_TEST_K: u32 = 17;

    #[derive(Clone, Debug)]
    struct ReceiverLaneTestConfig<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct ReceiverLaneTestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
        instances: Vec<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for ReceiverLaneTestCircuit<F> {
        type Config = ReceiverLaneTestConfig<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }

        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
                instances: self.instances.clone(),
            }
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            Self::Config {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("receiver-lane regression uses fixed Base parameters")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "receiver lane Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << RECEIVER_LANE_TEST_K) - MINIMUM_UNUSABLE_ROWS,
            )
        }
    }

    fn receiver_lane_test_circuit<F: KagemushaPoseidonFieldV1>(
        opening: Option<&[u8]>,
        credential_id: DigestV1,
        recipient_lane: DigestV1,
    ) -> Result<ReceiverLaneTestCircuit<F>, String> {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(RECEIVER_LANE_TEST_K as usize)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let active = F::from(u64::from(opening.is_some()));
        let instances = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(credential_id)
            .into_iter()
            .chain(crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(
                recipient_lane,
            ))
            .chain([active])
            .collect::<Vec<_>>();
        let cells = builder.main(0).assign_witnesses(instances.iter().copied());
        range.gate().assert_bit(builder.main(0), cells[4]);
        let mut jobs = PastaSha256JobsV1::default();
        let requested_credential =
            assigned_digest_bytes_v1(builder.main(0), range.gate(), [cells[0], cells[1]]);
        // Exercise the production terminal gadget directly, without host ID/lane validation.
        let lane = constrain_receiver_credential_lane_v1(
            builder.main(0),
            &range,
            &mut jobs,
            opening,
            &requested_credential,
            cells[4],
        )?;
        for (actual, expected) in digest_limbs_assigned(builder.main(0), &lane)
            .into_iter()
            .zip([cells[2], cells[3]])
        {
            builder.main(0).constrain_equal(&actual, &expected);
        }
        assert_eq!(
            jobs.compression_blocks().expect("credential hash geometry"),
            7
        );
        builder.assigned_instances = vec![cells];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        Ok(ReceiverLaneTestCircuit {
            builder,
            jobs,
            instances,
        })
    }

    fn assert_receiver_lane_constraint<F: KagemushaPoseidonFieldV1>(
        opening: Option<&[u8]>,
        credential_id: DigestV1,
        recipient_lane: DigestV1,
        expected: bool,
    ) {
        let circuit = receiver_lane_test_circuit::<F>(opening, credential_id, recipient_lane)
            .expect("fixed receiver-lane witness shape");
        let result = MockProver::run(
            RECEIVER_LANE_TEST_K,
            &circuit,
            vec![circuit.instances.clone()],
        )
        .expect("receiver-lane constraint synthesis")
        .verify();
        assert_eq!(
            result.is_ok(),
            expected,
            "receiver-lane constraint result: {result:?}"
        );
    }

    fn canonical_test_digest(domain: &[u8], bytes: &[u8]) -> DigestV1 {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
        hasher.finalize().into()
    }

    fn canonical_mint_lifecycle_test_circuit<F: KagemushaPoseidonFieldV1>(
        lifecycle: &KagemushaLifecycleBindingV1,
    ) -> ReceiverLaneTestCircuit<F> {
        let expected_asset_payload = norito::codec::encode_adaptive(&lifecycle.asset);
        let asset_canonical =
            norito::encode_canonical(&lifecycle.asset).expect("canonical asset frame");
        assert_eq!(
            expected_asset_payload.len(),
            MINT_FOLD_ASSET_PAYLOAD_BYTES_V1
        );
        assert_eq!(asset_canonical.len(), MINT_FOLD_ASSET_FRAME_BYTES_V1);
        let expected_lifecycle_payload = norito::codec::encode_adaptive(lifecycle);
        let lifecycle_canonical = norito::encode_canonical(lifecycle).expect("canonical lifecycle");
        assert_eq!(
            expected_lifecycle_payload.len(),
            MINT_FOLD_LIFECYCLE_PAYLOAD_BYTES_V1
        );
        assert_eq!(
            lifecycle_canonical.len(),
            MINT_FOLD_LIFECYCLE_FRAME_BYTES_V1
        );

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(RECEIVER_LANE_TEST_K as usize)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let mut jobs = PastaSha256JobsV1::default();

        let asset_source = assign_bytes(builder.main(0), &range, &lifecycle.asset.aid_bytes());
        let asset_payload =
            mint_fold_asset_payload_v1(&asset_source).expect("production asset payload");
        assert_eq!(
            asset_payload
                .iter()
                .copied()
                .map(PastaSha256ByteV1::test_value)
                .collect::<Vec<_>>(),
            expected_asset_payload
        );

        let network_id = assign_bytes(builder.main(0), &range, lifecycle.network_id.as_bytes());
        let protocol_version = assign_bytes(
            builder.main(0),
            &range,
            &lifecycle.protocol_version.to_le_bytes(),
        );
        let suite_id = assign_bytes(builder.main(0), &range, &lifecycle.suite_id);
        let vk_digest = assign_bytes(builder.main(0), &range, &lifecycle.vk_digest);
        let release_id = assign_bytes(builder.main(0), &range, &lifecycle.release_id);
        let asset_incarnation = assign_bytes(
            builder.main(0),
            &range,
            lifecycle.asset_incarnation.as_bytes(),
        );
        let scale = assign_bytes(builder.main(0), &range, &lifecycle.scale.to_le_bytes());
        let liability_pool_id = assign_bytes(builder.main(0), &range, &lifecycle.liability_pool_id);
        let hardware_profile_id =
            assign_bytes(builder.main(0), &range, &lifecycle.hardware_profile_id);
        let policy_epoch = assign_bytes(
            builder.main(0),
            &range,
            &lifecycle.policy_epoch.to_le_bytes(),
        );
        let credit_id = assign_bytes(builder.main(0), &range, &lifecycle.credit_id);
        let ciphertext_digest = assign_bytes(builder.main(0), &range, &lifecycle.ciphertext_digest);
        let lifecycle_payload = mint_fold_lifecycle_payload_v1(MintFoldLifecyclePayloadBytesV1 {
            network_id: &network_id,
            protocol_version: &protocol_version,
            suite_id: &suite_id,
            vk_digest: &vk_digest,
            release_id: &release_id,
            asset: &asset_payload,
            asset_incarnation: &asset_incarnation,
            scale: &scale,
            liability_pool_id: &liability_pool_id,
            hardware_profile_id: &hardware_profile_id,
            policy_epoch: &policy_epoch,
            credit_id: &credit_id,
            ciphertext_digest: &ciphertext_digest,
        })
        .expect("production lifecycle payload");
        assert_eq!(
            lifecycle_payload
                .iter()
                .copied()
                .map(PastaSha256ByteV1::test_value)
                .collect::<Vec<_>>(),
            expected_lifecycle_payload
        );

        let asset_len = builder.main(0).load_constant(F::from(
            u64::try_from(asset_payload.len()).expect("asset payload length fits u64"),
        ));
        let asset_stream = KagemushaBoundedByteStreamV1::constrain(
            builder.main(0),
            &range,
            asset_payload,
            asset_len,
        )
        .expect("bounded asset payload");
        let asset_prefix = kagemusha_canonical_mint_frame_prefix_v1(&lifecycle.asset)
            .expect("model-owned asset prefix");
        let asset_frame = assemble_bounded_canonical_frame_v1(
            builder.main(0),
            &range,
            &asset_prefix,
            &asset_stream,
        )
        .expect("constrained asset frame");
        assert_eq!(
            asset_frame
                .bytes()
                .iter()
                .copied()
                .map(PastaSha256ByteV1::test_value)
                .collect::<Vec<_>>(),
            asset_canonical
        );
        let mut asset_message = constant_bytes(MINT_FOLD_ASSET_IDENTITY_DIGEST_DOMAIN_V1);
        asset_message.push(PastaSha256ByteV1::constant(0));
        asset_message.extend(constant_bytes(
            &u64::try_from(asset_frame.bytes().len())
                .expect("asset frame length fits u64")
                .to_le_bytes(),
        ));
        asset_message.extend_from_slice(asset_frame.bytes());
        let asset_digest =
            hash(builder.main(0), &mut jobs, asset_message).expect("constrained asset digest");

        let lifecycle_len = builder.main(0).load_constant(F::from(
            u64::try_from(lifecycle_payload.len()).expect("lifecycle payload length fits u64"),
        ));
        let lifecycle_stream = KagemushaBoundedByteStreamV1::constrain(
            builder.main(0),
            &range,
            lifecycle_payload,
            lifecycle_len,
        )
        .expect("bounded lifecycle payload");
        let lifecycle_prefix = kagemusha_canonical_mint_frame_prefix_v1(lifecycle)
            .expect("model-owned lifecycle prefix");
        let lifecycle_frame = assemble_bounded_canonical_frame_v1(
            builder.main(0),
            &range,
            &lifecycle_prefix,
            &lifecycle_stream,
        )
        .expect("constrained lifecycle frame");
        assert_eq!(
            lifecycle_frame
                .bytes()
                .iter()
                .copied()
                .map(PastaSha256ByteV1::test_value)
                .collect::<Vec<_>>(),
            lifecycle_canonical
        );
        let mut lifecycle_message = constant_bytes(MINT_FOLD_LIFECYCLE_DIGEST_DOMAIN_V1);
        lifecycle_message.push(PastaSha256ByteV1::constant(0));
        lifecycle_message.extend(constant_bytes(
            &u64::try_from(lifecycle_frame.bytes().len())
                .expect("lifecycle frame length fits u64")
                .to_le_bytes(),
        ));
        lifecycle_message.extend_from_slice(lifecycle_frame.bytes());
        let lifecycle_digest = hash(builder.main(0), &mut jobs, lifecycle_message)
            .expect("constrained lifecycle digest");

        let expected_asset =
            iroha_data_model::kagemusha::kagemusha_asset_identity_digest_v1(&lifecycle.asset)
                .expect("native asset digest");
        let expected_lifecycle = lifecycle
            .canonical_digest()
            .expect("native lifecycle digest");
        let instances = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(expected_asset)
            .into_iter()
            .chain(crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(
                expected_lifecycle,
            ))
            .collect::<Vec<_>>();
        let public = builder.main(0).assign_witnesses(instances.iter().copied());
        for (actual, expected) in digest_limbs_assigned(builder.main(0), &asset_digest)
            .into_iter()
            .chain(digest_limbs_assigned(builder.main(0), &lifecycle_digest))
            .zip(public.iter())
        {
            builder.main(0).constrain_equal(&actual, expected);
        }
        assert_eq!(jobs.compression_blocks().expect("canonical frame jobs"), 11);
        builder.assigned_instances = vec![public];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        ReceiverLaneTestCircuit {
            builder,
            jobs,
            instances,
        }
    }

    fn assert_canonical_mint_lifecycle_digest<F: KagemushaPoseidonFieldV1>(
        lifecycle: &KagemushaLifecycleBindingV1,
    ) {
        let circuit = canonical_mint_lifecycle_test_circuit::<F>(lifecycle);
        MockProver::run(
            RECEIVER_LANE_TEST_K,
            &circuit,
            vec![circuit.instances.clone()],
        )
        .expect("canonical lifecycle synthesis")
        .assert_satisfied();
    }

    fn hash_binding_test_circuit<F: KagemushaPoseidonFieldV1, const N: usize>(
        values: [DigestV1; N],
        expected: DigestV1,
        expected_blocks: usize,
        hash_binding: impl FnOnce(
            &mut halo2_base::Context<F>,
            &RangeChip<F>,
            &mut PastaSha256JobsV1<F>,
            &[Vec<PastaSha256ByteV1<F>>; N],
        ) -> Result<[PastaSha256ByteV1<F>; 32], String>,
    ) -> ReceiverLaneTestCircuit<F> {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(RECEIVER_LANE_TEST_K as usize)
            .use_lookup_bits(16)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let bytes = values.map(|value| assign_bytes(builder.main(0), &range, &value));
        let mut jobs = PastaSha256JobsV1::default();
        let actual = hash_binding(builder.main(0), &range, &mut jobs, &bytes)
            .expect("fixed binding transcript");
        let instances = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(expected).to_vec();
        let public = builder.main(0).assign_witnesses(instances.iter().copied());
        for (actual, expected) in digest_limbs_assigned(builder.main(0), &actual)
            .into_iter()
            .zip(&public)
        {
            builder.main(0).constrain_equal(&actual, expected);
        }
        let (job_count, blocks, _) = jobs.capacity_profile().expect("binding geometry");
        assert_eq!((job_count, blocks), (1, expected_blocks));
        builder.assigned_instances = vec![public];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        ReceiverLaneTestCircuit {
            builder,
            jobs,
            instances,
        }
    }

    fn assert_binding_circuit<F: KagemushaPoseidonFieldV1>(
        circuit: ReceiverLaneTestCircuit<F>,
        valid: bool,
    ) {
        let result = MockProver::run(
            RECEIVER_LANE_TEST_K,
            &circuit,
            vec![circuit.instances.clone()],
        )
        .expect("output-binding constraint synthesis")
        .verify();
        assert_eq!(
            result.is_ok(),
            valid,
            "output-binding constraint result: {result:?}"
        );
    }

    fn output_binding_values(
        fixture: &super::super::tests::IncomingPaymentFixtureV1,
    ) -> [DigestV1; 6] {
        use iroha_data_model::kagemusha::kagemusha_prepared_transfer_digest_v1;
        [
            fixture.payment.output.credit_id,
            fixture.request.recipient_encryption_key,
            fixture.request.hardware_credential.lane_commitment,
            kagemusha_prepared_transfer_digest_v1(
                &fixture.request,
                fixture.payment.output.sender_before_commitment,
                fixture.payment.output.sender_after_commitment,
                fixture.payment.output.transition_nullifier,
                fixture.payment.output.ciphertext_commitment,
            )
            .expect("prepared transfer"),
            fixture
                .payment
                .output
                .canonical_digest_against(&fixture.request)
                .expect("output digest"),
            super::super::kagemusha_incoming_proof_binding_digest_v1(
                &fixture.request,
                &fixture.payment,
            )
            .expect("incoming claims"),
        ]
    }

    fn assert_output_binding<F: KagemushaPoseidonFieldV1>(
        values: [DigestV1; 6],
        expected: DigestV1,
        valid: bool,
    ) {
        let circuit =
            hash_binding_test_circuit::<F, 6>(values, expected, 4, |ctx, _range, jobs, bytes| {
                hash_terminal_send_output_binding_v1(ctx, jobs, bytes.each_ref().map(Vec::as_slice))
            });
        assert_binding_circuit(circuit, valid);
    }

    #[test]
    fn mint_fold_canonical_lifecycle_frames_match_model_in_both_fields() {
        let credit = super::super::tests::compact_mint_credit_fixture();
        let lifecycle = &credit.statement.lifecycle;
        assert_canonical_mint_lifecycle_digest::<Fp>(lifecycle);
        assert_canonical_mint_lifecycle_digest::<Fq>(lifecycle);
    }

    #[test]
    fn incoming_output_binding_constraint_rejects_cross_lane_replay_and_recipient_key_change() {
        // Signed structural fixture, not an actual recursive proof. Only the production binding
        // gadget is qualified here; actual final proofs and history decisions are separate gates.
        let fixture = super::super::tests::incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
        let original_payment = fixture.payment.clone();
        let values = output_binding_values(&fixture);
        let public = super::super::native_backend::payment_terminal_public_inputs_v1(
            &fixture.request,
            &fixture.payment,
            [0xA1; 32],
        )
        .expect("native terminal public projection");
        let binding = public.terminal_output_binding;
        assert_output_binding::<Fp>(values, binding, true);
        assert_output_binding::<Fq>(values, binding, true);
        // Credit ID, request key, local lane, prepared transfer, output, and exact incoming claims
        // must all be authenticated by the unchanged final proof's public output binding.
        for index in 0..values.len() {
            let mut changed = values;
            changed[index][0] ^= 1;
            assert_output_binding::<Fp>(changed, binding, false);
            if index == 2 {
                assert_output_binding::<Fq>(changed, binding, false);
            }
        }
        assert_eq!(
            fixture.payment, original_payment,
            "cross-lane/key attacks keep the final payment and opening commitments unchanged"
        );
    }

    #[test]
    fn terminal_incoming_claims_constraint_binds_every_staged_component() {
        let values = std::array::from_fn::<_, 7, _>(|index| [index as u8 + 1; 32]);
        let expected = super::super::canonical_incoming_payment_claims_binding_v1(values);
        let circuit = |values| {
            hash_binding_test_circuit::<Fp, 7>(values, expected, 5, |ctx, _range, jobs, bytes| {
                hash_incoming_payment_claims_binding_v1(
                    ctx,
                    jobs,
                    bytes.each_ref().map(Vec::as_slice),
                )
            })
        };
        assert_binding_circuit(circuit(values), true);
        assert_binding_circuit(
            hash_binding_test_circuit::<Fq, 7>(values, expected, 5, |ctx, _range, jobs, bytes| {
                hash_incoming_payment_claims_binding_v1(
                    ctx,
                    jobs,
                    bytes.each_ref().map(Vec::as_slice),
                )
            }),
            true,
        );
        // Request, receiver, sender-state pair, output, ciphertext, candidate, certificate.
        // These are only digest-gadget witnesses; no synthetic proof is treated as authority.
        for index in 0..values.len() {
            let mut changed = values;
            changed[index][0] ^= 1;
            assert_binding_circuit(circuit(changed), false);
        }
    }

    #[test]
    fn terminal_prepared_transfer_constraint_matches_model_and_binds_every_component() {
        let fixture = super::super::tests::incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
        let values = [
            fixture.request.canonical_digest().expect("request"),
            fixture.payment.output.sender_before_commitment,
            fixture.payment.output.sender_after_commitment,
            fixture.payment.output.transition_nullifier,
            fixture.request.recipient_encryption_key,
            fixture.payment.output.ciphertext_commitment,
        ];
        let amount = fixture.request.amount;
        let expected = output_binding_values(&fixture)[3];
        let circuit = |values, amount: u128| {
            hash_binding_test_circuit::<Fp, 6>(values, expected, 5, |ctx, range, jobs, bytes| {
                let amount = assign_bytes(ctx, range, &amount.to_le_bytes());
                hash_terminal_prepared_transfer_v1(
                    ctx,
                    jobs,
                    bytes.each_ref().map(Vec::as_slice),
                    &amount,
                )
            })
        };
        assert_binding_circuit(circuit(values, amount), true);
        assert_binding_circuit(
            hash_binding_test_circuit::<Fq, 6>(values, expected, 5, |ctx, range, jobs, bytes| {
                let amount = assign_bytes(ctx, range, &amount.to_le_bytes());
                hash_terminal_prepared_transfer_v1(
                    ctx,
                    jobs,
                    bytes.each_ref().map(Vec::as_slice),
                    &amount,
                )
            }),
            true,
        );
        for index in 0..values.len() {
            let mut changed = values;
            changed[index][0] ^= 1;
            assert_binding_circuit(circuit(changed, amount), false);
        }
        assert_binding_circuit(circuit(values, amount + 1), false);
    }

    #[test]
    fn receiver_plaintext_opening_constraint_retains_secret_knowledge() {
        use iroha_data_model::kagemusha::kagemusha_peer_credit_opening_commitment_v1;
        let values = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        let amount = 7_u128;
        let expected = kagemusha_peer_credit_opening_commitment_v1(
            values[0], values[1], amount, values[2], values[3], values[4],
        )
        .expect("native opening commitment");
        let circuit = |values, amount: u128| {
            hash_binding_test_circuit::<Fp, 5>(values, expected, 4, |ctx, range, jobs, bytes| {
                let amount = assign_bytes(ctx, range, &amount.to_le_bytes());
                hash_receiver_plaintext_opening_v1(
                    ctx,
                    jobs,
                    bytes.each_ref().map(Vec::as_slice),
                    &amount,
                )
            })
        };
        assert_binding_circuit(circuit(values, amount), true);
        assert_binding_circuit(
            hash_binding_test_circuit::<Fq, 5>(values, expected, 4, |ctx, range, jobs, bytes| {
                let amount = assign_bytes(ctx, range, &amount.to_le_bytes());
                hash_receiver_plaintext_opening_v1(
                    ctx,
                    jobs,
                    bytes.each_ref().map(Vec::as_slice),
                    &amount,
                )
            }),
            true,
        );
        for index in 0..values.len() {
            let mut changed = values;
            changed[index][0] ^= 1;
            assert_binding_circuit(circuit(changed, amount), false);
        }
        assert_binding_circuit(circuit(values, amount + 1), false);
    }

    #[test]
    fn receive_binding_sha_inventory_is_fixed_for_one_credit() {
        // Structural queue inventory, not a proof or k=16 placement claim. The sole inactive
        // proof still instantiates both production hashes over canonical zero fields, so changing
        // operation kind cannot change the keygen/prover shape.
        for active in [false, true] {
            let mut builder = BaseCircuitBuilder::<Fp>::new(false).use_lookup_bits(16);
            let range = builder.range_chip();
            let mut jobs = PastaSha256JobsV1::default();
            let byte = u8::from(active);
            let digest = assign_bytes(builder.main(0), &range, &[byte; 32]);
            let amount = assign_bytes(builder.main(0), &range, &u128::from(byte).to_le_bytes());
            hash_receiver_plaintext_opening_v1(builder.main(0), &mut jobs, [&digest; 5], &amount)
                .expect("fixed plaintext-opening hash");
            hash_terminal_send_output_binding_v1(builder.main(0), &mut jobs, [&digest; 6])
                .expect("fixed output-binding hash");
            let (count, blocks, _) = jobs.capacity_profile().expect("single-credit geometry");
            assert_eq!((count, blocks), (2, 8));
            assert_eq!(jobs.capacity_profile(), jobs.unknown().capacity_profile());
        }
    }

    #[test]
    fn receive_binding_helpers_reject_noncanonical_widths() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false).use_lookup_bits(16);
        let range = builder.range_chip();
        let digest = assign_bytes(builder.main(0), &range, &[1; 32]);
        let short = &digest[..31];
        let amount = assign_bytes(builder.main(0), &range, &1_u128.to_le_bytes());
        let mut jobs = PastaSha256JobsV1::default();
        assert!(
            hash_terminal_send_output_binding_v1(builder.main(0), &mut jobs, [short; 6]).is_err()
        );
        assert!(
            hash_incoming_payment_claims_binding_v1(builder.main(0), &mut jobs, [short; 7])
                .is_err()
        );
        assert!(
            hash_terminal_prepared_transfer_v1(builder.main(0), &mut jobs, [short; 6], &amount)
                .is_err()
        );
        assert!(
            hash_terminal_prepared_transfer_v1(
                builder.main(0),
                &mut jobs,
                [&digest; 6],
                &amount[..15]
            )
            .is_err()
        );
        assert!(
            hash_receiver_plaintext_opening_v1(builder.main(0), &mut jobs, [short; 5], &amount)
                .is_err()
        );
        assert!(
            hash_receiver_plaintext_opening_v1(
                builder.main(0),
                &mut jobs,
                [&digest; 5],
                &amount[..15]
            )
            .is_err()
        );
        assert_eq!(jobs.compression_blocks().expect("empty queue"), 0);
    }

    #[test]
    fn terminal_receiver_credential_constraint_rejects_cross_lane_opening() {
        // This signed, shape-valid payment fixture has synthetic proof bytes. It supplies no proof
        // authority: this regression isolates the production terminal lane gadget; the unchanged final proof,
        // ciphertext, and opening commitments are authenticated separately by the recursive verifier.
        let fixture = super::super::tests::incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
        fixture
            .payment
            .validate_shape_against(&fixture.request)
            .expect("signed payment context");
        let original_payment = fixture.payment.clone();
        let credential = fixture
            .request
            .hardware_credential
            .canonical_id_preimage_bytes()
            .expect("canonical credential ID preimage");
        let digest = fixture.request.hardware_credential.credential_id;
        let lane_a = fixture.request.hardware_credential.lane_commitment;
        let lane_b = [0xB2; 32];
        assert_ne!(lane_a, lane_b);
        let opening = Some(credential.as_slice());
        assert_receiver_lane_constraint::<Fp>(opening, digest, lane_a, true);
        assert_receiver_lane_constraint::<Fq>(opening, digest, lane_a, true);
        assert_receiver_lane_constraint::<Fp>(opening, digest, lane_b, false);
        assert_receiver_lane_constraint::<Fq>(opening, digest, lane_b, false);
        assert_eq!(
            fixture.payment, original_payment,
            "cross-lane attempt preserves the final payment"
        );
    }

    #[test]
    fn terminal_receiver_credential_constraint_pins_framing_with_recomputed_digest() {
        let fixture = super::super::tests::incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
        let credential = fixture
            .request
            .hardware_credential
            .canonical_id_preimage_bytes()
            .expect("canonical credential ID preimage");
        // Recompute the claimed hash so failures must come from the framing constraints, not
        // merely a stale digest. No signature/proof authority is claimed for these malformed inputs.
        for offset in [6, 39, 184] {
            let mut malformed = credential.clone();
            malformed[offset] ^= 1;
            let digest =
                canonical_test_digest(b"iroha:kagemusha:v1:hardware-credential-id", &malformed);
            assert_receiver_lane_constraint::<Fp>(
                Some(&malformed),
                digest,
                fixture.request.hardware_credential.lane_commitment,
                false,
            );
        }
    }

    #[test]
    fn terminal_receiver_credential_constraint_has_fixed_padding_and_rejects_wrong_widths() {
        assert_receiver_lane_constraint::<Fp>(None, [0; 32], [0; 32], true);
        assert!(receiver_lane_test_circuit::<Fp>(Some(&[0; 375]), [0; 32], [0; 32]).is_err());
        assert!(receiver_lane_test_circuit::<Fp>(Some(&[0; 377]), [0; 32], [0; 32]).is_err());
    }

    #[test]
    fn acceptance_authorization_has_the_receive_fold_input_shape() {
        let authorization_proof_instances = TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1;
        assert_eq!(
            authorization_proof_instances,
            INCOMING_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
        );
        assert_eq!(authorization_proof_instances, 81);

        assert!(
            validate_incoming_authorization_proof_shape_v1(
                &[authorization_proof_instances],
                [authorization_proof_instances],
            )
            .is_ok()
        );
        assert!(
            validate_incoming_authorization_proof_shape_v1(&[119], [authorization_proof_instances])
                .is_err()
        );
        assert!(
            validate_incoming_authorization_proof_shape_v1(&[authorization_proof_instances], [119])
                .is_err()
        );
        assert_eq!(
            incoming_public_instance::HISTORY_START,
            INCOMING_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1,
            "incoming history starts immediately after the terminal-authorization public prefix",
        );
    }
}
