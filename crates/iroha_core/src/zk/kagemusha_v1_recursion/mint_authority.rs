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
use sha2::{Digest as _, Sha256};
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
        bind_accumulator_limbs, constrain_reciprocal_audit_plan_v1, deferred_field_chips_v1,
        deferred_loader_v1, finalize_deferred_audit_plan_v1,
        load_and_constrain_parent_protocol_if_v1, load_native_accumulator,
        kagemusha_protocol_structure_digest_v1, select_accumulator_v1, verify_fold,
        verify_ordinary_proof_v1,
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
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const MINT_PARENT_EQUATION_TAG_V1: u32 = 5;
const MINT_PAIR_BINDING_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-authority-pair";

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
    /// Canonical binding of both proof transcripts, audits, and histories.
    pub proof_binding_digest: DigestV1,
    /// Constant-size paired recursive proof.
    pub proof: KagemushaPairedProofV1,
}

impl KagemushaMintAuthorityCheckpointV1 {
    /// Validate all non-cryptographic public bindings.
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
        let eq_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] = self
            .proof
            .eq_history
            .as_slice()
            .try_into()
            .map_err(|_| "mint-authority Eq checkpoint history has wrong size".to_owned())?;
        let ep_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] = self
            .proof
            .ep_history
            .as_slice()
            .try_into()
            .map_err(|_| "mint-authority Ep checkpoint history has wrong size".to_owned())?;
        let expected = KagemushaMintAuthorityPairBindingV1 {
            step: self.step,
            semantic_digest,
            amount: self.statement.amount,
            certificate_binding: self.certificate_binding,
            authority_head: self.authority_head,
            release_id: self.release_id,
            genesis_roster_id: self.genesis_roster_id,
            eq_protocol_digest: self.proof.eq_protocol_digest,
            ep_protocol_digest: self.proof.ep_protocol_digest,
            eq_deferred_audit: self.proof.eq_deferred_audit,
            ep_deferred_audit: self.proof.ep_deferred_audit,
            eq_history,
            ep_history,
        }
        .canonical_digest();
        if expected != self.proof_binding_digest {
            return Err("mint-authority checkpoint paired binding differs".to_owned());
        }
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
    /// Low limb of the Eq helper protocol identity.
    pub const EQ_PROTOCOL_LO: usize = 12;
    /// High limb of the Eq helper protocol identity.
    pub const EQ_PROTOCOL_HI: usize = 13;
    /// Low limb of the Ep helper protocol identity.
    pub const EP_PROTOCOL_LO: usize = 14;
    /// High limb of the Ep helper protocol identity.
    pub const EP_PROTOCOL_HI: usize = 15;
    /// Low limb of the Eq scalar-verifier equation audit.
    pub const EQ_AUDIT_LO: usize = 16;
    /// High limb of the Eq scalar-verifier equation audit.
    pub const EQ_AUDIT_HI: usize = 17;
    /// Low limb of the Ep scalar-verifier equation audit.
    pub const EP_AUDIT_LO: usize = 18;
    /// High limb of the Ep scalar-verifier equation audit.
    pub const EP_AUDIT_HI: usize = 19;
    /// Low limb of the canonical binding of both complete helper parities.
    pub const PAIR_BINDING_LO: usize = 20;
    /// High limb of the canonical binding of both complete helper parities.
    pub const PAIR_BINDING_HI: usize = 21;
    /// First limb of the complete carried IPA history.
    pub const HISTORY_START: usize = 22;
}

/// Fixed public cell count, including all 34 injective `u128` history limbs.
pub(super) const KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1: usize =
    public_instance::HISTORY_START + accumulator_limb_count();

/// Canonical public material shared by both halves of one mint-authority proof.
///
/// The digest includes both complete history accumulators. A state proof therefore cannot splice
/// an Eq helper from one certificate/authority chain with an Ep helper from another.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaMintAuthorityPairBindingV1<'a> {
    /// Explicit helper branch.
    pub step: KagemushaMintAuthorityStepV1,
    /// Common semantic statement digest.
    pub semantic_digest: DigestV1,
    /// Exact semantic amount.
    pub amount: u128,
    /// Exact paired certificate binding.
    pub certificate_binding: DigestV1,
    /// Recursively authenticated current roster.
    pub authority_head: DigestV1,
    /// Authenticated proof release.
    pub release_id: DigestV1,
    /// Release-pinned genesis roster.
    pub genesis_roster_id: DigestV1,
    /// Eq helper protocol identity.
    pub eq_protocol_digest: DigestV1,
    /// Ep helper protocol identity.
    pub ep_protocol_digest: DigestV1,
    /// Eq deferred-equation audit.
    pub eq_deferred_audit: DigestV1,
    /// Ep deferred-equation audit.
    pub ep_deferred_audit: DigestV1,
    /// Complete Eq helper history.
    pub eq_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    /// Complete Ep helper history.
    pub ep_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

impl KagemushaMintAuthorityPairBindingV1<'_> {
    /// Hash the exact fixed-width public pair transcript constrained by both circuits.
    #[must_use]
    pub fn canonical_digest(self) -> DigestV1 {
        let mut hasher = Sha256::new();
        hasher.update(MINT_PAIR_BINDING_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(u128::from(self.step as u64).to_le_bytes());
        hasher.update(self.semantic_digest);
        hasher.update(self.amount.to_le_bytes());
        for value in [
            self.certificate_binding,
            self.authority_head,
            self.release_id,
            self.genesis_roster_id,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.eq_deferred_audit,
            self.ep_deferred_audit,
        ] {
            hasher.update(value);
        }
        hasher.update(self.eq_history);
        hasher.update(self.ep_history);
        hasher.finalize().into()
    }
}

/// Same-parity predecessor material consumed by one mint-authority half.
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
}

/// Complete shared witness used to build the mutually audited helper pair.
pub(super) struct KagemushaMintAuthorityPairWitnessV1<'a> {
    pub(super) step: KagemushaMintAuthorityStepV1,
    pub(super) release_id: DigestV1,
    pub(super) genesis_roster_id: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
    pub(super) eq_deferred_audit: DigestV1,
    pub(super) ep_deferred_audit: DigestV1,
    pub(super) certificate: KagemushaMintCertificateWitnessV1,
    pub(super) eq: KagemushaMintAuthorityParityWitnessV1<'a, EqAffine>,
    pub(super) ep: KagemushaMintAuthorityParityWitnessV1<'a, EpAffine>,
}

/// Base/Table16/dense-MSM configuration shared by both authority parities.
#[derive(Clone, Debug)]
pub(super) struct KagemushaMintAuthorityCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp stable mint-authority half.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorityEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq stable mint-authority half.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorityEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
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
                KagemushaMintAuthorityCircuitConfigV1 {
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

/// Build the stable mutually audited authority pair.
pub(super) fn build_kagemusha_mint_authority_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintAuthorityPairWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintAuthorityEqCircuitV1,
        KagemushaMintAuthorityEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    if witness.release_id == [0; 32]
        || witness.genesis_roster_id == [0; 32]
        || witness.eq_protocol_digest == [0; 32]
        || witness.ep_protocol_digest == [0; 32]
        || witness.eq_protocol_digest == witness.ep_protocol_digest
        || witness.eq_deferred_audit == [0; 32]
        || witness.ep_deferred_audit == [0; 32]
    {
        return Err("mint-authority public binding is absent or aliased".to_owned());
    }
    witness.certificate.validate_for_step(witness.step)?;
    let eq_svk = eq_succinct_vk(eq_params);
    let ep_svk = ep_succinct_vk(ep_params);
    let eq_successor_history = witness.eq.successor_history;
    let ep_successor_history = witness.ep.successor_history;
    let (mut eq_builder, eq_jobs, eq_output) = build_scalar_half::<EqAffine, EpAffine>(
        &eq_svk,
        KagemushaPastaParityV1::Eq,
        witness.step,
        witness.release_id,
        witness.genesis_roster_id,
        witness.eq_protocol_digest,
        witness.ep_protocol_digest,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        &witness.certificate,
        eq_successor_history,
        ep_successor_history,
        witness.eq,
    )?;
    let (mut ep_builder, ep_jobs, ep_output) = build_scalar_half::<EpAffine, EqAffine>(
        &ep_svk,
        KagemushaPastaParityV1::Ep,
        witness.step,
        witness.release_id,
        witness.genesis_roster_id,
        witness.eq_protocol_digest,
        witness.ep_protocol_digest,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        &witness.certificate,
        eq_successor_history,
        ep_successor_history,
        witness.ep,
    )?;

    let mut eq_dense = eq_jobs.dense;
    let eq_expected_ep_audit = public_digest_cells(
        &eq_builder,
        public_instance::EP_AUDIT_LO,
        "Eq helper Ep audit",
    )?;
    constrain_reciprocal_audit_plan_v1::<EpAffine>(
        &mut eq_builder,
        &ep_output.audit,
        &ep_output.equation_tags,
        &ep_output.equation_selectors,
        &eq_expected_ep_audit,
        &mut eq_dense,
    )?;
    let mut ep_dense = ep_jobs.dense;
    let ep_expected_eq_audit = public_digest_cells(
        &ep_builder,
        public_instance::EQ_AUDIT_LO,
        "Ep helper Eq audit",
    )?;
    constrain_reciprocal_audit_plan_v1::<EqAffine>(
        &mut ep_builder,
        &eq_output.audit,
        &eq_output.equation_tags,
        &eq_output.equation_selectors,
        &ep_expected_eq_audit,
        &mut ep_dense,
    )?;

    // These are witness values, not host-authorized assertions.  The returned bytes let the
    // production prover rebuild the same fixed circuit with the exact public audit cells; both
    // circuits still recompute and constrain them, including the reciprocal point equations.
    let eq_audit = assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = assigned_digest_bytes(&ep_output.audit_digest_limbs)?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << 16) - MINIMUM_UNUSABLE_ROWS;
    eq_jobs.sha.validate_capacity(usable_rows)?;
    ep_jobs.sha.validate_capacity(usable_rows)?;
    eq_dense.validate_capacity(usable_rows)?;
    ep_dense.validate_capacity(usable_rows)?;
    Ok((
        KagemushaMintAuthorityEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_jobs.sha,
            dense_jobs: eq_dense,
        },
        KagemushaMintAuthorityEpCircuitV1 {
            builder: ep_builder,
            sha_jobs: ep_jobs.sha,
            dense_jobs: ep_dense,
        },
        eq_audit,
        ep_audit,
    ))
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
    let range = builder.range_chip();
    let gate = range.gate();
    let ctx = builder.main(0);
    let release = assign_digest(ctx, &range, release_id);
    let genesis = assign_digest(ctx, &range, genesis_roster_id);
    let eq_protocol = assign_digest(ctx, &range, eq_protocol_digest);
    let ep_protocol = assign_digest(ctx, &range, ep_protocol_digest);
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
    let mut pair_preimage = super::mint_helper::constant_bytes(MINT_PAIR_BINDING_DOMAIN_V1);
    pair_preimage.push(PastaSha256ByteV1::constant(0));
    for value in [assigned.step]
        .into_iter()
        .chain(assigned.mint_instances)
        .chain(assigned.certificate_binding_digest)
        .chain(authority)
        .chain(release)
        .chain(genesis)
        .chain(eq_protocol)
        .chain(ep_protocol)
        .chain(eq_audit)
        .chain(ep_audit)
        .chain(eq_history.iter().copied())
        .chain(ep_history.iter().copied())
    {
        pair_preimage.extend(assigned_u128_bytes(ctx, gate, value));
    }
    let pair_binding_bytes = super::mint_helper::sha_digest(ctx, &mut jobs.sha, pair_preimage)?;
    let pair_binding = super::mint_helper::sha_digest_limbs(ctx, gate, &pair_binding_bytes);
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
            .chain(pair_binding)
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
    bind_accumulator_limbs(&loader, &successor, history)
        .map_err(|error| format!("failed to bind mint-authority successor history: {error:?}"))?;

    let equation_count = loader.ecc_chip().equation_count();
    if equation_count == 0 {
        return Err("mint-authority predecessor verifier emitted no equations".to_owned());
    }
    let output = finalize_deferred_audit_plan_v1(
        &mut builder,
        loader,
        vec![MINT_PARENT_EQUATION_TAG_V1; equation_count],
        vec![parent_enabled; equation_count],
        vec![step != KagemushaMintAuthorityStepV1::Bootstrap; equation_count],
    )
    .map_err(|error| format!("failed to finalize mint-authority audit: {error:?}"))?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected_audit) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok((builder, jobs, output))
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

fn assigned_u128_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    value: AssignedValue<F>,
) -> Vec<PastaSha256ByteV1<F>> {
    PastaSha256BitV1::decompose(ctx, gate, value, 128)
        .chunks_exact(8)
        .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, gate, bits))
        .collect()
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
};

#[cfg(test)]
mod tests {
    use super::*;

    fn binding<'a>(
        eq_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: &'a [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    ) -> KagemushaMintAuthorityPairBindingV1<'a> {
        KagemushaMintAuthorityPairBindingV1 {
            step: KagemushaMintAuthorityStepV1::FinalizedMint,
            semantic_digest: [1; 32],
            amount: u128::MAX,
            certificate_binding: [2; 32],
            authority_head: [3; 32],
            release_id: [4; 32],
            genesis_roster_id: [5; 32],
            eq_protocol_digest: [6; 32],
            ep_protocol_digest: [7; 32],
            eq_deferred_audit: [8; 32],
            ep_deferred_audit: [9; 32],
            eq_history,
            ep_history,
        }
    }

    #[test]
    fn pair_binding_rejects_cross_parity_history_and_audit_splicing() {
        let eq_history = [0x11; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
        let ep_history = [0x22; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
        let expected = binding(&eq_history, &ep_history).canonical_digest();

        let mut substituted_eq = eq_history;
        substituted_eq[0] ^= 1;
        assert_ne!(
            expected,
            binding(&substituted_eq, &ep_history).canonical_digest()
        );

        let mut substituted = binding(&eq_history, &ep_history);
        substituted.ep_deferred_audit[31] ^= 0x80;
        assert_ne!(expected, substituted.canonical_digest());
        substituted = binding(&eq_history, &ep_history);
        substituted.amount = u128::MAX - 1;
        assert_ne!(expected, substituted.canonical_digest());
    }

    #[test]
    fn carrier_public_shape_has_explicit_u128_cells() {
        assert_eq!(public_instance::STEP, 0);
        assert_eq!(public_instance::AMOUNT, 3);
        assert_eq!(public_instance::PAIR_BINDING_LO, 20);
        assert_eq!(public_instance::HISTORY_START, 22);
        assert_eq!(KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1, 56);
    }
}
