//! Stable recursive authority carrier for finalized Kagemusha mint credits.
//!
//! A helper proof is either the release-pinned genesis roster, a quorum-authorized roster
//! rotation, or one finalized reserve receipt.  Rotation and mint branches recursively verify an
//! authority-only predecessor under the same helper protocol and fold both the predecessor's
//! current IPA opening claim and its complete carried history.  The reciprocal Pasta proof checks
//! every deferred curve equation.  Consequently the public helper key is independent of the
//! current validator roster and no host-side certificate or roster selection grants value.

use ff::Field as _;
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
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
use iroha_data_model::kagemusha::{KagemushaMintCreditStatementV1, KagemushaPairedProofV1};
use norito::codec::{Decode, Encode};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    util::arithmetic::{Domain, root_of_unity},
    verifier::plonk::PlonkProtocol,
};

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaPastaParityV1,
    deferred_parent::{
        DeferredLoader, DeferredScalar, KagemushaDeferredParentOutputV1, accumulator_limb_count,
        bind_accumulator_limbs, constrain_reciprocal_output_with_u128_binding_v1,
        deferred_field_chips_v1, deferred_loader_v1,
        finalize_deferred_audit_plan_with_u128_binding_v1, kagemusha_protocol_structure_digest_v1,
        load_and_constrain_parent_protocol_if_v1, load_and_constrain_parent_protocol_v1,
        load_native_accumulator, select_accumulator_v1, verify_fold, verify_ordinary_proof_v1,
        verify_two_carrier_hybrid_ordinary_proof_and_stream_v1,
    },
    mint_hash_claim_fold::{
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
        canonical_claim_carrier_binding_tail_v1, constrain_complete_claim_against_sha_jobs_v1,
        public_instance as hash_claim_public,
    },
    mint_helper::{
        KagemushaMintAuthorityStepV1, KagemushaMintCertificateJobsV1,
        KagemushaMintCertificateWitnessV1, ReciprocalAffine,
        constrain_kagemusha_mint_certificate_v1,
    },
};
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs},
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const MINT_PARENT_EQUATION_TAG_V1: u32 = 5;
const MINT_HASH_CLAIM_EQUATION_TAG_V1: u32 = 13;
const MINT_PAIR_BASE_BOUND_U128_COUNT_V1: usize = 92;
const MINT_HASH_CLAIM_BOUND_TAIL_U128_COUNT_V1: usize =
    KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        - KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1;
const MINT_PAIR_BOUND_U128_COUNT_V1: usize =
    MINT_PAIR_BASE_BOUND_U128_COUNT_V1 + MINT_HASH_CLAIM_BOUND_TAIL_U128_COUNT_V1;
const MINT_EQ_AUDIT_BOUND_U128_COUNT_V1: usize = MINT_PAIR_BOUND_U128_COUNT_V1 + 2;

/// Release-authenticated bootstrap or epoch-rotation proof used as a finalized-mint parent.
///
/// Later rotation checkpoints are persisted in Kura. Every use re-verifies the paired proof and
/// both carried histories, so a process-local cache can never replace durable authority.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaMintAuthorityCheckpointV1 {
    /// Bootstrap or rotation branch proved by this checkpoint.
    pub step: KagemushaMintAuthorityStepV1,
    /// Canonical statement retained as fixed-shape input for no-top-up rotations.
    pub statement: KagemushaMintCreditStatementV1,
    /// Exact paired certificate binding.
    pub certificate_binding: DigestV1,
    /// Current recursively authenticated roster identifier.
    pub authority_head: DigestV1,
    /// Authenticated proof-release identifier.
    pub release_id: DigestV1,
    /// Release-pinned genesis roster identifier.
    pub genesis_roster_id: DigestV1,
    /// Inner Eq deferred-audit commitment to the paired authority metadata, proved in outer
    /// cells 20..21.
    ///
    /// Its inner audit/history inputs differ from the transported outer metadata. Shape
    /// validation cannot recompute this commitment; both outer proofs must authenticate it.
    pub proof_binding_digest: DigestV1,
    /// Constant-size paired recursive proof.
    pub proof: KagemushaPairedProofV1,
}

impl KagemushaMintAuthorityCheckpointV1 {
    /// Validate non-cryptographic public bindings and canonical outer histories.
    ///
    /// The nonzero inner pair commitment is not authority by itself. The native verifier must
    /// verify both compact outer proofs and terminally decide their complete histories.
    pub fn validate_shape(&self) -> Result<(), String> {
        if !matches!(
            self.step,
            KagemushaMintAuthorityStepV1::Bootstrap | KagemushaMintAuthorityStepV1::Rotate
        ) {
            return Err("mint-authority checkpoint must be bootstrap or rotation".to_owned());
        }
        self.statement
            .validate_shape()
            .map_err(|error| format!("invalid checkpoint statement: {error}"))?;
        let semantic_digest = self
            .statement
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        self.proof
            .validate_shape_for_semantic_digest(semantic_digest)
            .map_err(|error| error.to_string())?;
        if self.certificate_binding == [0; 32]
            || self.authority_head == [0; 32]
            || self.release_id == [0; 32]
            || self.genesis_roster_id == [0; 32]
            || self.proof_binding_digest == [0; 32]
            || self.statement.lifecycle.release_id != self.release_id
            || self.proof.guard_eq_credential_audit != self.certificate_binding
            || self.proof.guard_ep_credential_audit != self.authority_head
        {
            return Err("mint-authority checkpoint public binding is invalid".to_owned());
        }
        super::KagemushaEqAccumulatorV1::try_from_bytes(&self.proof.eq_history)
            .map_err(|error| format!("invalid mint-authority Eq checkpoint history: {error}"))?;
        super::KagemushaEpAccumulatorV1::try_from_bytes(&self.proof.ep_history)
            .map_err(|error| format!("invalid mint-authority Ep checkpoint history: {error}"))?;
        Ok(())
    }
}

/// Public instance layout of both stable mint-authority parities.
pub(super) mod public_instance {
    /// Explicit branch selector: bootstrap, rotation, or finalized mint.
    pub const STEP: usize = 0;
    /// Low `u128` limb of the final mint statement digest.
    pub const SEMANTIC_LO: usize = 1;
    /// High `u128` limb of the final mint statement digest.
    pub const SEMANTIC_HI: usize = 2;
    /// Exact range-constrained mint amount.
    pub const AMOUNT: usize = 3;
    /// Low limb of the exact paired finality-certificate binding.
    pub const CERTIFICATE_LO: usize = 4;
    /// High limb of the exact paired finality-certificate binding.
    pub const CERTIFICATE_HI: usize = 5;
    /// Low limb of the current recursively authenticated roster identifier.
    pub const AUTHORITY_LO: usize = 6;
    /// High limb of the current recursively authenticated roster identifier.
    pub const AUTHORITY_HI: usize = 7;
    /// Low limb of the authenticated Kagemusha release identifier.
    pub const RELEASE_LO: usize = 8;
    /// High limb of the authenticated Kagemusha release identifier.
    pub const RELEASE_HI: usize = 9;
    /// Low limb of the release-pinned genesis roster identifier.
    pub const GENESIS_LO: usize = 10;
    /// High limb of the release-pinned genesis roster identifier.
    pub const GENESIS_HI: usize = 11;
    /// Low limb of the Eq compact outer checkpoint protocol identity.
    pub const EQ_PROTOCOL_LO: usize = 12;
    /// High limb of the Eq compact outer checkpoint protocol identity.
    pub const EQ_PROTOCOL_HI: usize = 13;
    /// Low limb of the Ep compact outer checkpoint protocol identity.
    pub const EP_PROTOCOL_LO: usize = 14;
    /// High limb of the Ep compact outer checkpoint protocol identity.
    pub const EP_PROTOCOL_HI: usize = 15;
    /// Low limb of the Eq scalar-verifier equation audit.
    pub const EQ_AUDIT_LO: usize = 16;
    /// High limb of the Eq scalar-verifier equation audit.
    pub const EQ_AUDIT_HI: usize = 17;
    /// Low limb of the Ep scalar-verifier equation audit.
    pub const EP_AUDIT_LO: usize = 18;
    /// High limb of the Ep scalar-verifier equation audit.
    pub const EP_AUDIT_HI: usize = 19;
    /// Low limb of the proven inner paired authority commitment.
    pub const PAIR_BINDING_LO: usize = 20;
    /// High limb of the proven inner paired authority commitment.
    pub const PAIR_BINDING_HI: usize = 21;
    /// First limb of the complete carried IPA history.
    pub const HISTORY_START: usize = 22;
}

/// Fixed public cell count, including all 34 injective `u128` history limbs.
pub(super) const KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1: usize =
    public_instance::HISTORY_START + accumulator_limb_count();

/// Same-parity predecessor material consumed by one mint-authority half.
#[derive(Clone, Copy)]
pub(super) struct KagemushaMintAuthorityParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(super) parent_protocol: &'a PlonkProtocol<C>,
    pub(super) parent_instances: &'a [Vec<C::ScalarExt>],
    pub(super) parent_proof: &'a [u8],
    pub(super) parent_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) parent_fold_proof: &'a [u8],
    pub(super) successor_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    pub(super) hash_claim_protocol: &'a PlonkProtocol<C>,
    pub(super) hash_claim_instances: &'a [Vec<C::ScalarExt>],
    pub(super) hash_claim_proof: &'a [u8],
    pub(super) hash_claim_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(super) hash_claim_history_fold_proof: &'a [u8],
    pub(super) hash_claim_merge_fold_proof: &'a [u8],
}

/// Complete shared witness used to build the mutually audited helper pair.
pub(super) struct KagemushaMintAuthorityPairWitnessV1<'a> {
    pub(super) step: KagemushaMintAuthorityStepV1,
    pub(super) release_id: DigestV1,
    pub(super) genesis_roster_id: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
    pub(super) eq_hash_claim_protocol_digest: DigestV1,
    pub(super) ep_hash_claim_protocol_digest: DigestV1,
    pub(super) eq_hash_shard_protocol_digest: DigestV1,
    pub(super) ep_hash_shard_protocol_digest: DigestV1,
    pub(super) eq_deferred_audit: DigestV1,
    pub(super) ep_deferred_audit: DigestV1,
    pub(super) certificate: KagemushaMintCertificateWitnessV1,
    pub(super) eq: KagemushaMintAuthorityParityWitnessV1<'a, EqAffine>,
    pub(super) ep: KagemushaMintAuthorityParityWitnessV1<'a, EpAffine>,
}

/// Detached reciprocal plans and canonical audits discovered without retaining either Base graph.
///
/// The two deferred outputs own only native points, coefficients, selectors, and transcript
/// values. Their `AssignedValue` digest handles are fixed-size value/location records; they do not
/// retain the builders from which the values were read.
pub(super) struct KagemushaMintAuthorityAuditDiscoveryV1 {
    eq_output: KagemushaDeferredParentOutputV1<EqAffine>,
    ep_output: KagemushaDeferredParentOutputV1<EpAffine>,
    pub(super) eq_deferred_audit: DigestV1,
    pub(super) ep_deferred_audit: DigestV1,
}

/// Base/dense-MSM configuration shared by both authority parities.
#[derive(Clone, Debug)]
pub(super) struct KagemushaMintAuthorityCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp stable mint-authority half.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorityEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq stable mint-authority half.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorityEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_mint_authority_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaMintAuthorityCircuitConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
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
                KagemushaMintAuthorityCircuitConfigV1 {
                    base,
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
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

impl_mint_authority_circuit!(
    KagemushaMintAuthorityEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq mint authority"
);
impl_mint_authority_circuit!(
    KagemushaMintAuthorityEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep mint authority"
);

fn validate_pair_witness_v1(
    witness: &KagemushaMintAuthorityPairWitnessV1<'_>,
) -> Result<(), String> {
    if witness.release_id == [0; 32]
        || witness.genesis_roster_id == [0; 32]
        || witness.eq_protocol_digest == [0; 32]
        || witness.ep_protocol_digest == [0; 32]
        || witness.eq_protocol_digest == witness.ep_protocol_digest
        || witness.eq_hash_claim_protocol_digest == [0; 32]
        || witness.ep_hash_claim_protocol_digest == [0; 32]
        || witness.eq_hash_shard_protocol_digest == [0; 32]
        || witness.ep_hash_shard_protocol_digest == [0; 32]
        || witness.eq_hash_claim_protocol_digest == witness.ep_hash_claim_protocol_digest
        || witness.eq_hash_shard_protocol_digest == witness.ep_hash_shard_protocol_digest
        || witness.eq_deferred_audit == [0; 32]
        || witness.ep_deferred_audit == [0; 32]
    {
        return Err("mint-authority public binding is absent or aliased".to_owned());
    }
    let eq_claim_tail = canonical_claim_carrier_binding_tail_v1(witness.eq.hash_claim_instances)?;
    let ep_claim_tail = canonical_claim_carrier_binding_tail_v1(witness.ep.hash_claim_instances)?;
    if eq_claim_tail != ep_claim_tail {
        return Err(
            "mint-authority paired claim carrier-binding tails are not canonical and identical"
                .to_owned(),
        );
    }
    witness.certificate.validate_for_step(witness.step)
}

/// Discover the stable audit pair while keeping at most one scalar-half Base graph alive.
///
/// Ep commits only the common transcript, so it is discovered first. Eq is then discovered with
/// that exact Ep audit in its bound tail. Each builder and its certificate jobs are dropped before
/// the other parity is constructed; only the compact native reciprocal plans survive.
pub(super) fn discover_kagemusha_mint_authority_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaMintAuthorityPairWitnessV1<'_>,
) -> Result<KagemushaMintAuthorityAuditDiscoveryV1, String> {
    validate_pair_witness_v1(witness)?;
    let ep_svk = ep_succinct_vk(ep_params);
    let eq_successor_history = witness.eq.successor_history;
    let ep_successor_history = witness.ep.successor_history;
    let (ep_builder, ep_jobs, ep_output, ep_pair_binding) = build_scalar_half::<EpAffine, EqAffine>(
        &ep_svk,
        KagemushaPastaParityV1::Ep,
        witness.step,
        witness.release_id,
        witness.genesis_roster_id,
        witness.eq_protocol_digest,
        witness.ep_protocol_digest,
        witness.eq_hash_claim_protocol_digest,
        witness.ep_hash_claim_protocol_digest,
        witness.eq_hash_shard_protocol_digest,
        witness.ep_hash_shard_protocol_digest,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        &witness.certificate,
        eq_successor_history,
        ep_successor_history,
        witness.ep,
    )?;
    let ep_deferred_audit = assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    drop(ep_builder);
    drop(ep_jobs);
    drop(ep_pair_binding);
    halo2_proofs::release_allocator_slack();

    let eq_svk = eq_succinct_vk(eq_params);
    let (eq_builder, eq_jobs, eq_output, eq_pair_binding) = build_scalar_half::<EqAffine, EpAffine>(
        &eq_svk,
        KagemushaPastaParityV1::Eq,
        witness.step,
        witness.release_id,
        witness.genesis_roster_id,
        witness.eq_protocol_digest,
        witness.ep_protocol_digest,
        witness.eq_hash_claim_protocol_digest,
        witness.ep_hash_claim_protocol_digest,
        witness.eq_hash_shard_protocol_digest,
        witness.ep_hash_shard_protocol_digest,
        witness.eq_deferred_audit,
        ep_deferred_audit,
        &witness.certificate,
        eq_successor_history,
        ep_successor_history,
        witness.eq,
    )?;
    let eq_deferred_audit = assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    drop(eq_builder);
    drop(eq_jobs);
    drop(eq_pair_binding);
    halo2_proofs::release_allocator_slack();

    Ok(KagemushaMintAuthorityAuditDiscoveryV1 {
        eq_output,
        ep_output,
        eq_deferred_audit,
        ep_deferred_audit,
    })
}

/// Build the exact Eq half from the detached Ep reciprocal plan.
pub(super) fn build_kagemusha_mint_authority_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &KagemushaMintAuthorityPairWitnessV1<'_>,
    discovery: &KagemushaMintAuthorityAuditDiscoveryV1,
) -> Result<KagemushaMintAuthorityEqCircuitV1, String> {
    validate_pair_witness_v1(witness)?;
    let eq_svk = eq_succinct_vk(eq_params);
    let (mut eq_builder, eq_jobs, eq_output, eq_pair_binding) =
        build_scalar_half::<EqAffine, EpAffine>(
            &eq_svk,
            KagemushaPastaParityV1::Eq,
            witness.step,
            witness.release_id,
            witness.genesis_roster_id,
            witness.eq_protocol_digest,
            witness.ep_protocol_digest,
            witness.eq_hash_claim_protocol_digest,
            witness.ep_hash_claim_protocol_digest,
            witness.eq_hash_shard_protocol_digest,
            witness.ep_hash_shard_protocol_digest,
            discovery.eq_deferred_audit,
            discovery.ep_deferred_audit,
            &witness.certificate,
            witness.eq.successor_history,
            witness.ep.successor_history,
            witness.eq,
        )?;
    if assigned_digest_bytes(&eq_output.audit_digest_limbs)? != discovery.eq_deferred_audit {
        return Err("mint-authority Eq audit changed after compact discovery".to_owned());
    }
    drop(eq_output);
    let mut eq_dense = eq_jobs.dense;
    let eq_expected_ep_audit = public_digest_cells(
        &eq_builder,
        public_instance::EP_AUDIT_LO,
        "Eq helper Ep audit",
    )?;
    constrain_reciprocal_output_with_u128_binding_v1::<EpAffine>(
        &mut eq_builder,
        &discovery.ep_output,
        &eq_expected_ep_audit,
        &eq_pair_binding,
        &mut eq_dense,
    )?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << 16) - MINIMUM_UNUSABLE_ROWS;
    eq_dense.validate_capacity(usable_rows)?;
    Ok(KagemushaMintAuthorityEqCircuitV1 {
        builder: eq_builder,
        dense_jobs: eq_dense,
    })
}

/// Build the exact Ep half from the detached Eq reciprocal plan.
pub(super) fn build_kagemusha_mint_authority_ep_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaMintAuthorityPairWitnessV1<'_>,
    discovery: &KagemushaMintAuthorityAuditDiscoveryV1,
) -> Result<KagemushaMintAuthorityEpCircuitV1, String> {
    validate_pair_witness_v1(witness)?;
    let ep_svk = ep_succinct_vk(ep_params);
    let (mut ep_builder, ep_jobs, ep_output, ep_pair_binding) =
        build_scalar_half::<EpAffine, EqAffine>(
            &ep_svk,
            KagemushaPastaParityV1::Ep,
            witness.step,
            witness.release_id,
            witness.genesis_roster_id,
            witness.eq_protocol_digest,
            witness.ep_protocol_digest,
            witness.eq_hash_claim_protocol_digest,
            witness.ep_hash_claim_protocol_digest,
            witness.eq_hash_shard_protocol_digest,
            witness.ep_hash_shard_protocol_digest,
            discovery.eq_deferred_audit,
            discovery.ep_deferred_audit,
            &witness.certificate,
            witness.eq.successor_history,
            witness.ep.successor_history,
            witness.ep,
        )?;
    if assigned_digest_bytes(&ep_output.audit_digest_limbs)? != discovery.ep_deferred_audit {
        return Err("mint-authority Ep audit changed after compact discovery".to_owned());
    }
    drop(ep_output);
    let mut ep_dense = ep_jobs.dense;
    let ep_expected_eq_audit = public_digest_cells(
        &ep_builder,
        public_instance::EQ_AUDIT_LO,
        "Ep helper Eq audit",
    )?;
    let mut ep_eq_pair_binding = ep_pair_binding;
    ep_eq_pair_binding.extend(public_digest_cells(
        &ep_builder,
        public_instance::EP_AUDIT_LO,
        "Ep helper own audit",
    )?);
    if ep_eq_pair_binding.len() != MINT_EQ_AUDIT_BOUND_U128_COUNT_V1 {
        return Err("mint-authority Eq audit binding shape drifted".to_owned());
    }
    constrain_reciprocal_output_with_u128_binding_v1::<EqAffine>(
        &mut ep_builder,
        &discovery.eq_output,
        &ep_expected_eq_audit,
        &ep_eq_pair_binding,
        &mut ep_dense,
    )?;

    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << 16) - MINIMUM_UNUSABLE_ROWS;
    ep_dense.validate_capacity(usable_rows)?;
    Ok(KagemushaMintAuthorityEpCircuitV1 {
        builder: ep_builder,
        dense_jobs: ep_dense,
    })
}

fn assigned_digest_bytes<F: halo2_base::utils::ScalarField>(
    limbs: &[AssignedValue<F>; 2],
) -> Result<DigestV1, String> {
    let mut digest = [0_u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        let bytes = fe_to_biguint(limb.value()).to_bytes_le();
        if bytes.len() > 16 {
            return Err("mint-authority audit limb exceeds its canonical u128 range".to_owned());
        }
        let offset = index * 16;
        digest[offset..offset + bytes.len()].copy_from_slice(&bytes);
    }
    if digest == [0; 32] {
        return Err("mint-authority deferred audit is zero".to_owned());
    }
    Ok(digest)
}

#[allow(clippy::too_many_arguments)]
fn build_scalar_half<C, S>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    step: KagemushaMintAuthorityStepV1,
    release_id: DigestV1,
    genesis_roster_id: DigestV1,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
    eq_hash_claim_protocol_digest: DigestV1,
    ep_hash_claim_protocol_digest: DigestV1,
    eq_hash_shard_protocol_digest: DigestV1,
    ep_hash_shard_protocol_digest: DigestV1,
    eq_deferred_audit: DigestV1,
    ep_deferred_audit: DigestV1,
    certificate: &KagemushaMintCertificateWitnessV1,
    eq_successor_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    ep_successor_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    witness: KagemushaMintAuthorityParityWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        KagemushaMintCertificateJobsV1<S>,
        KagemushaDeferredParentOutputV1<C>,
        Vec<AssignedValue<C::ScalarExt>>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
    S: CurveAffineExt<Base = C::ScalarExt, ScalarExt = C::Base>,
    S::ScalarExt: ReciprocalAffine,
{
    let mut builder = authority_builder::<C::ScalarExt>();
    let (assigned, mut jobs) =
        constrain_kagemusha_mint_certificate_v1::<S>(&mut builder, certificate, parity, step)?;
    // The fixed certificate transcript is authorized by the recursively verified ordered claim
    // below. The remaining pair transcript is absorbed into both deferred-audit sponges instead
    // of instantiating a five-lane Table8 gadget for one local digest.
    let claimed_sha = core::mem::take(&mut jobs.sha);
    let range = builder.range_chip();
    let gate = range.gate();
    let ctx = builder.main(0);
    let release = assign_digest(ctx, &range, release_id);
    let genesis = assign_digest(ctx, &range, genesis_roster_id);
    let eq_protocol = assign_digest(ctx, &range, eq_protocol_digest);
    let ep_protocol = assign_digest(ctx, &range, ep_protocol_digest);
    // These internal protocol identities have no direct state-public cells.  Put them in fixed
    // columns so the release-authenticated MintAuthority VK, rather than a prover-selected
    // witness or host preflight, owns the exact claim/shard verifier suite.  The recursively
    // verified claim still exposes the same four digests and is equality-bound to these cells.
    let eq_hash_claim_protocol = constant_digest(ctx, eq_hash_claim_protocol_digest);
    let ep_hash_claim_protocol = constant_digest(ctx, ep_hash_claim_protocol_digest);
    let eq_hash_shard_protocol = constant_digest(ctx, eq_hash_shard_protocol_digest);
    let ep_hash_shard_protocol = constant_digest(ctx, ep_hash_shard_protocol_digest);
    let eq_audit = assign_digest(ctx, &range, eq_deferred_audit);
    let ep_audit = assign_digest(ctx, &range, ep_deferred_audit);

    for (roster, expected) in assigned.roster_state_digest.iter().zip(genesis) {
        constrain_equal_if(ctx, gate, *roster, expected, assigned.bootstrap);
    }
    let authority: [AssignedValue<C::ScalarExt>; 2] = std::array::from_fn(|index| {
        gate.select(
            ctx,
            Existing(assigned.next_epoch_id_digest[index]),
            Existing(assigned.roster_state_digest[index]),
            Existing(assigned.rotate),
        )
    });
    let eq_history = assign_history_limbs(ctx, &range, eq_successor_history)?;
    let ep_history = assign_history_limbs(ctx, &range, ep_successor_history)?;
    let history = match parity {
        KagemushaPastaParityV1::Eq => &eq_history,
        KagemushaPastaParityV1::Ep => &ep_history,
    };
    let mut pair_binding_values = [assigned.step]
        .into_iter()
        .chain(assigned.mint_instances)
        .chain(assigned.certificate_binding_digest)
        .chain(authority)
        .chain(release)
        .chain(genesis)
        .chain(eq_protocol)
        .chain(ep_protocol)
        .chain(eq_hash_claim_protocol)
        .chain(ep_hash_claim_protocol)
        .chain(eq_hash_shard_protocol)
        .chain(ep_hash_shard_protocol)
        .chain(eq_history.iter().copied())
        .chain(ep_history.iter().copied())
        .collect::<Vec<_>>();
    if pair_binding_values.len() != MINT_PAIR_BASE_BOUND_U128_COUNT_V1 {
        return Err("mint-authority base paired transcript shape drifted".to_owned());
    }
    if jobs.sha.compression_blocks()? != 0 {
        return Err(
            "mint-authority local SHA queue must remain empty after claim offload".to_owned(),
        );
    }
    builder.assigned_instances = vec![
        [assigned.step]
            .into_iter()
            .chain(assigned.mint_instances[..2].iter().copied())
            .chain([assigned.mint_instances[2]])
            .chain(assigned.certificate_binding_digest)
            .chain(authority)
            .chain(release)
            .chain(genesis)
            .chain(eq_protocol)
            .chain(ep_protocol)
            .chain(eq_audit)
            .chain(ep_audit)
            // The Eq audit is a constrained 255-bit Poseidon commitment to the complete paired
            // transcript above. Reusing its canonical limbs avoids an unconstrained host digest
            // and lets native/state consumers retain the existing two-limb binding ABI.
            .chain(eq_audit)
            .chain(history.iter().copied())
            .collect(),
    ];
    if builder.assigned_instances[0].len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint-authority public instance shape drifted".to_owned());
    }

    let parent_enabled = gate.add(
        builder.main(0),
        Existing(assigned.rotate),
        Existing(assigned.finalized_mint),
    );
    let expected_protocol = match parity {
        KagemushaPastaParityV1::Eq => eq_protocol,
        KagemushaPastaParityV1::Ep => ep_protocol,
    };
    let expected_audit = match parity {
        KagemushaPastaParityV1::Eq => eq_audit,
        KagemushaPastaParityV1::Ep => ep_audit,
    };
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let structure = kagemusha_protocol_structure_digest_v1(witness.parent_protocol, parity)?;
    let loaded = load_and_constrain_parent_protocol_if_v1(
        &loader,
        witness.parent_protocol,
        parity,
        structure,
        &expected_protocol,
        Some(parent_enabled),
    )
    .map_err(|error| format!("failed to bind mint-authority parent protocol: {error:?}"))?;
    if loaded.protocol.num_instance != [KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        || witness.parent_instances.len() != 1
        || witness.parent_instances[0].len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("mint-authority parent public shape is not fixed".to_owned());
    }
    let parent_instances = witness
        .parent_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &loaded.protocol,
        &parent_instances,
        witness.parent_proof,
    )
    .map_err(|error| format!("failed to verify mint-authority predecessor: {error:?}"))?;
    let parent_column = parent_instances
        .first()
        .ok_or_else(|| "mint-authority parent public column is absent".to_owned())?;
    constrain_authority_parent(
        &loader,
        parent_column,
        &assigned.roster_state_digest,
        &release,
        &genesis,
        &eq_protocol,
        &ep_protocol,
        parent_enabled,
    )?;
    let parent_history = load_native_accumulator(&loader, witness.parent_history)
        .map_err(|error| format!("failed to load mint-authority history: {error:?}"))?;
    let parent_history_cells = parent_column
        .get(public_instance::HISTORY_START..)
        .ok_or_else(|| "mint-authority predecessor history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &parent_history, &parent_history_cells)
        .map_err(|error| format!("failed to bind mint-authority predecessor history: {error:?}"))?;
    let folded = verify_fold(
        &loader,
        succinct_vk,
        &[current, parent_history.clone()],
        witness.parent_fold_proof,
    )
    .map_err(|error| format!("failed to fold mint-authority predecessor: {error:?}"))?;
    let successor = select_accumulator_v1(&loader, &folded, &parent_history, parent_enabled)
        .map_err(|error| format!("failed to select mint-authority successor history: {error:?}"))?;
    let parent_equation_count = loader.ecc_chip().equation_count();
    if parent_equation_count == 0 {
        return Err("mint-authority predecessor verifier emitted no equations".to_owned());
    }

    let expected_hash_claim_protocol = match parity {
        KagemushaPastaParityV1::Eq => eq_hash_claim_protocol,
        KagemushaPastaParityV1::Ep => ep_hash_claim_protocol,
    };
    let claim_structure =
        kagemusha_protocol_structure_digest_v1(witness.hash_claim_protocol, parity)?;
    let loaded_claim = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.hash_claim_protocol,
        parity,
        claim_structure,
        &expected_hash_claim_protocol,
    )
    .map_err(|error| format!("failed to bind mint-authority hash-claim protocol: {error:?}"))?;
    if loaded_claim.protocol.num_instance
        != [
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]
        || witness.hash_claim_instances.len() != 3
        || witness.hash_claim_instances[0].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.hash_claim_instances[1].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.hash_claim_instances[2].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err("mint-authority terminal hash claim public shape is not fixed".to_owned());
    }
    let claim_semantic = witness.hash_claim_instances[0]
        .iter()
        .map(|value| loader.assign_scalar(*value))
        .collect::<Vec<_>>();
    pair_binding_values.extend(
        claim_semantic[KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
            ..KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1]
            .iter()
            .map(|value| *value.assigned()),
    );
    if pair_binding_values.len() != MINT_PAIR_BOUND_U128_COUNT_V1 {
        return Err("mint-authority claim-bound paired transcript shape drifted".to_owned());
    }
    let claim_current = verify_two_carrier_hybrid_ordinary_proof_and_stream_v1(
        &loader,
        succinct_vk,
        &loaded_claim.protocol,
        &claim_semantic,
        match parity {
            KagemushaPastaParityV1::Eq => [
                [
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 1,
                ],
                [
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 2,
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 3,
                ],
            ],
            KagemushaPastaParityV1::Ep => [
                [
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 4,
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 5,
                ],
                [
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 6,
                    KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 + 7,
                ],
            ],
        },
        witness.hash_claim_proof,
    )
    .map_err(|error| format!("failed to verify terminal mint hash claim: {error:?}"))?;
    let claim_column = &claim_semantic[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1];
    {
        let chip = loader.ecc_chip();
        let mut loader_ctx = loader.ctx_mut();
        let assigned_claim = claim_column
            .iter()
            .map(|value| *value.assigned())
            .collect::<Vec<_>>();
        constrain_complete_claim_against_sha_jobs_v1(
            loader_ctx.main(),
            chip.range(),
            &claimed_sha,
            &assigned_claim,
            parity,
            release,
            eq_hash_claim_protocol,
            ep_hash_claim_protocol,
            eq_hash_shard_protocol,
            ep_hash_shard_protocol,
        )?;
    }
    let claim_history = load_native_accumulator(&loader, witness.hash_claim_history)
        .map_err(|error| format!("failed to load terminal mint hash claim history: {error:?}"))?;
    let claim_history_cells = claim_column
        .get(hash_claim_public::HISTORY_START..)
        .ok_or_else(|| "terminal mint hash claim history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &claim_history, &claim_history_cells)
        .map_err(|error| format!("failed to bind terminal mint hash claim history: {error:?}"))?;
    let complete_claim = verify_fold(
        &loader,
        succinct_vk,
        &[claim_current.accumulator, claim_history],
        witness.hash_claim_history_fold_proof,
    )
    .map_err(|error| format!("failed to fold terminal mint hash claim history: {error:?}"))?;
    let successor = verify_fold(
        &loader,
        succinct_vk,
        &[successor, complete_claim],
        witness.hash_claim_merge_fold_proof,
    )
    .map_err(|error| format!("failed to merge terminal mint hash claim authority: {error:?}"))?;
    bind_accumulator_limbs(&loader, &successor, history)
        .map_err(|error| format!("failed to bind mint-authority successor history: {error:?}"))?;

    let equation_count = loader.ecc_chip().equation_count();
    if equation_count <= parent_equation_count {
        return Err("mint-authority hash-claim verifier emitted no equations".to_owned());
    }
    let mut equation_tags = vec![MINT_PARENT_EQUATION_TAG_V1; parent_equation_count];
    equation_tags.resize(equation_count, MINT_HASH_CLAIM_EQUATION_TAG_V1);
    let mut assigned_selectors = vec![parent_enabled; parent_equation_count];
    assigned_selectors.extend(
        (parent_equation_count..equation_count)
            .map(|_| loader.ctx_mut().main().load_constant(C::ScalarExt::ONE)),
    );
    let mut selectors =
        vec![step != KagemushaMintAuthorityStepV1::Bootstrap; parent_equation_count];
    selectors.resize(equation_count, true);
    // Acyclic exact-pair binding: Ep commits the shared transcript first; Eq additionally
    // absorbs the Ep audit. The canonical Eq audit therefore commits both equation transcripts
    // without asking either circuit to solve a self-referential hash fixed point.
    let mut audit_bound_values = pair_binding_values.clone();
    if parity == KagemushaPastaParityV1::Eq {
        audit_bound_values.extend(ep_audit);
    }
    let expected_bound_values = match parity {
        KagemushaPastaParityV1::Eq => MINT_EQ_AUDIT_BOUND_U128_COUNT_V1,
        KagemushaPastaParityV1::Ep => MINT_PAIR_BOUND_U128_COUNT_V1,
    };
    if audit_bound_values.len() != expected_bound_values {
        return Err("mint-authority deferred-audit binding shape drifted".to_owned());
    }
    let output = finalize_deferred_audit_plan_with_u128_binding_v1(
        &mut builder,
        loader,
        equation_tags,
        assigned_selectors,
        selectors,
        &audit_bound_values,
    )
    .map_err(|error| format!("failed to finalize mint-authority audit: {error:?}"))?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected_audit) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok((builder, jobs, output, pair_binding_values))
}

fn constrain_authority_parent<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    parent: &[DeferredScalar<'chip, C>],
    signing_roster: &[AssignedValue<C::ScalarExt>; 2],
    release: &[AssignedValue<C::ScalarExt>; 2],
    genesis: &[AssignedValue<C::ScalarExt>; 2],
    eq_protocol: &[AssignedValue<C::ScalarExt>; 2],
    ep_protocol: &[AssignedValue<C::ScalarExt>; 2],
    enabled: AssignedValue<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if parent.len() != KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint-authority predecessor public shape is truncated".to_owned());
    }
    let chip = loader.ecc_chip();
    let mut ctx = loader.ctx_mut();
    let parent_step = *parent[public_instance::STEP].assigned();
    let is_bootstrap = chip.range().gate().is_zero(ctx.main(), parent_step);
    let is_rotate =
        chip.range()
            .gate()
            .is_equal(ctx.main(), parent_step, Constant(C::ScalarExt::ONE));
    let authority_step =
        chip.range()
            .gate()
            .add(ctx.main(), Existing(is_bootstrap), Existing(is_rotate));
    constrain_equal_if(
        ctx.main(),
        chip.range().gate(),
        authority_step,
        enabled,
        enabled,
    );
    for (offset, expected) in [
        (public_instance::AUTHORITY_LO, signing_roster.as_slice()),
        (public_instance::RELEASE_LO, release.as_slice()),
        (public_instance::GENESIS_LO, genesis.as_slice()),
        (public_instance::EQ_PROTOCOL_LO, eq_protocol.as_slice()),
        (public_instance::EP_PROTOCOL_LO, ep_protocol.as_slice()),
    ] {
        for (actual, expected) in parent[offset..offset + 2].iter().zip(expected) {
            constrain_equal_if(
                ctx.main(),
                chip.range().gate(),
                *actual.assigned(),
                *expected,
                enabled,
            );
        }
    }
    Ok(())
}

fn authority_builder<F: KagemushaPoseidonFieldV1>() -> BaseCircuitBuilder<F> {
    BaseCircuitBuilder::new(false)
        .use_k(16)
        .use_lookup_bits(15)
        .use_instance_columns(1)
}

fn assign_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    digest_limbs::<F>(digest).map(|limb| {
        let assigned = ctx.load_witness(limb);
        range.range_check(ctx, assigned, 128);
        assigned
    })
}

fn constant_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    digest_limbs::<F>(digest).map(|limb| ctx.load_constant(limb))
}

fn assign_history_limbs<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<AssignedValue<F>>, String> {
    let limbs = history
        .chunks_exact(16)
        .map(|chunk| {
            let assigned = ctx.load_witness(F::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history limb is sixteen bytes"),
            )));
            range.range_check(ctx, assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    if limbs.len() != accumulator_limb_count() {
        return Err("mint-authority history limb count is not fixed".to_owned());
    }
    Ok(limbs)
}

fn public_digest_cells<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
    label: &str,
) -> Result<[AssignedValue<F>; 2], String> {
    builder
        .assigned_instances
        .first()
        .and_then(|column| column.get(offset..offset + 2))
        .ok_or_else(|| format!("{label} public limbs are absent"))?
        .try_into()
        .map_err(|_| format!("{label} public limbs have wrong shape"))
}

fn constrain_equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
    enabled: AssignedValue<F>,
) {
    let difference = gate.sub(ctx, Existing(left), Existing(right));
    let selected = gate.mul(ctx, Existing(difference), Existing(enabled));
    gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn eq_succinct_vk(params: &ParamsIPA<EqAffine>) -> IpaSuccinctVerifyingKey<EqAffine> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

fn ep_succinct_vk(params: &ParamsIPA<EpAffine>) -> IpaSuccinctVerifyingKey<EpAffine> {
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    IpaSuccinctVerifyingKey::new(
        Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

const _: () = {
    assert!(KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1 == 56);
    assert!(MINT_PAIR_BASE_BOUND_U128_COUNT_V1 == 92);
    assert!(MINT_PAIR_BOUND_U128_COUNT_V1 == 106);
    assert!(MINT_EQ_AUDIT_BOUND_U128_COUNT_V1 == 108);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_audit_binding_covers_the_complete_fixed_u128_transcript() {
        let common_scalars = 1; // step
        let semantic_and_amount = 3;
        let six_public_digests = 6 * 2;
        let four_release_fixed_hash_protocol_digests = 4 * 2;
        let paired_histories = 2 * accumulator_limb_count();
        assert_eq!(
            MINT_PAIR_BASE_BOUND_U128_COUNT_V1,
            common_scalars
                + semantic_and_amount
                + six_public_digests
                + four_release_fixed_hash_protocol_digests
                + paired_histories
        );
        let claim_carrier_binding_tail = KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
            - KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1;
        assert_eq!(claim_carrier_binding_tail, 14);
        assert_eq!(
            MINT_HASH_CLAIM_BOUND_TAIL_U128_COUNT_V1,
            claim_carrier_binding_tail
        );
        assert_eq!(MINT_PAIR_BASE_BOUND_U128_COUNT_V1, 92);
        assert_eq!(MINT_PAIR_BOUND_U128_COUNT_V1, 106);
        assert_eq!(
            MINT_PAIR_BOUND_U128_COUNT_V1,
            MINT_PAIR_BASE_BOUND_U128_COUNT_V1 + claim_carrier_binding_tail
        );
        assert_eq!(
            MINT_EQ_AUDIT_BOUND_U128_COUNT_V1,
            MINT_PAIR_BOUND_U128_COUNT_V1 + 2
        );
        assert_eq!(MINT_EQ_AUDIT_BOUND_U128_COUNT_V1, 108);
    }

    #[test]
    fn carrier_public_shape_has_explicit_u128_cells() {
        assert_eq!(public_instance::STEP, 0);
        assert_eq!(public_instance::AMOUNT, 3);
        assert_eq!(public_instance::PAIR_BINDING_LO, 20);
        assert_eq!(public_instance::HISTORY_START, 22);
        assert_eq!(KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, 56);
    }

    #[test]
    fn compact_checkpoint_shape_preserves_inner_commitment_without_authorizing_it() {
        let credit = super::super::tests::compact_mint_credit_fixture();
        let mut checkpoint = KagemushaMintAuthorityCheckpointV1 {
            step: KagemushaMintAuthorityStepV1::Bootstrap,
            release_id: credit.statement.lifecycle.release_id,
            statement: credit.statement,
            certificate_binding: credit.finality_certificate_binding,
            authority_head: credit.finality_authority_head,
            genesis_roster_id: credit.finality_genesis_roster_id,
            proof_binding_digest: credit.finality_proof_binding_digest,
            proof: credit.proof,
        };
        // These are deliberately opaque mock proof bytes. Passing shape does not verify them.
        checkpoint
            .validate_shape()
            .expect("compact bootstrap framing");
        checkpoint.step = KagemushaMintAuthorityStepV1::Rotate;
        checkpoint
            .validate_shape()
            .expect("the constrained Eq audit remains the pair binding");
        for index in 0..10 {
            let mut changed = checkpoint.clone();
            match index {
                0 => changed.step = KagemushaMintAuthorityStepV1::FinalizedMint,
                1 => changed.proof_binding_digest = [0; 32],
                2 => changed.certificate_binding = [0xB2; 32],
                3 => changed.authority_head = [0xB3; 32],
                4 => changed.release_id = [0xB4; 32],
                5 => changed.genesis_roster_id = [0; 32],
                6 => changed.proof.eq_history[0..32].fill(0xFF),
                7 => changed.proof.ep_history[0..32].fill(0xFF),
                8 => {
                    changed.proof.eq_history.pop();
                }
                _ => {
                    changed.proof.ep_history.push(0);
                }
            }
            assert!(
                changed.validate_shape().is_err(),
                "invalid checkpoint case {index}"
            );
        }
    }
}
