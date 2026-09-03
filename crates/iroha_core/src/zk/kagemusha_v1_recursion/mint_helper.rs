//! Circuit components for reserve-backed mint credits and recursive roster authority.
//!
//! The mint certificate relation is deliberately split from the final recursive relation.  This
//! module proves the expensive, reusable part of the relation: an exact quorum of the dynamic
//! epoch roster signed the block-local paired top-up root, and the credited statement/amount is a
//! member of that root.  It also derives a circuit-authenticated roster-state digest.  The final
//! helper must recursively bind that digest to the genesis-pinned roster/rotation chain; treating
//! it as a host assertion would let an attacker choose a fresh roster and mint arbitrary value.

use ff::{Field as _, PrimeField, WithSmallOrderMulGroup};
use halo2_base::{
    AssignedValue, Context,
    QuantumCell::{Constant, Existing},
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_ecc::{
    bigint::ProperCrtUint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::{
        CurveAffine,
        group::{GroupEncoding, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
};
use iroha_data_model::{
    block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT,
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1,
        KagemushaMintFinalityEpochRosterV1, KagemushaMintFinalitySealBundleV1,
        KagemushaPastaSchnorrSignatureV1, KagemushaTopUpMembershipWitnessV1,
        kagemusha_mint_finality_peer_id_digest_v1, kagemusha_mint_finality_root_v1,
    },
    kagemusha::KagemushaMintCreditStatementV1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use super::{
    DigestV1, KagemushaPastaParityV1,
    mint_finality::{MINT_LEAF_DOMAIN_V1, MINT_NODE_DOMAIN_V1},
};
use crate::zk::{
    kagemusha_v1_poseidon::{
        KagemushaPoseidonChipV1, KagemushaPoseidonFieldV1, decode, digest_limbs, from_u128,
    },
    pasta_cycle_loader::{compressed_point_bytes, proper_uint_le_bytes},
    pasta_dense_msm::PastaDenseMsmConfigV1,
    pasta_dense_msm::{PastaDenseMsmJobsV1, PastaDenseMsmSourceV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const MINT_ROOT_BRIDGE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality-root";
const MINT_SEAL_MESSAGE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality-seal-message";
const MINT_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-finality:challenge";
const MINT_FINALITY_EPOCH_ROSTER_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:mint-finality-epoch-roster";
const MINT_CERTIFICATE_BINDING_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-certificate-binding";
const EQ_PARITY_TAG: u8 = 0;
const EP_PARITY_TAG: u8 = 1;

/// Fixed branches of the stable mint-authority recursion circuit.
///
/// The branch is an explicit range-constrained public cell. It is never hidden in unused bits of
/// the semantic amount, so a finalized mint proof cannot be substituted for a roster carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[repr(u64)]
pub enum KagemushaMintAuthorityStepV1 {
    /// Pin the release-authenticated genesis roster without accepting a quorum assertion.
    Bootstrap = 0,
    /// Let the current roster's exact quorum authorize the next roster identifier.
    Rotate = 1,
    /// Prove one reserve receipt under the recursively authenticated current roster.
    FinalizedMint = 2,
}

/// Public cells of a finalized certificate before the recursive authority carrier is attached:
/// statement digest limbs, amount, and the exact paired certificate-binding digest limbs.
pub(super) const KAGEMUSHA_MINT_CERTIFICATE_PUBLIC_INSTANCE_COUNT_V1: usize = 5;

/// Private inputs consumed by the fixed-shape mint-certificate component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaMintCertificateWitnessV1 {
    /// Exact public mint statement whose digest and amount are exposed by the final helper.
    pub statement: KagemushaMintCreditStatementV1,
    /// Sparse paired-Poseidon membership path for the finalized reserve receipt.
    pub membership: KagemushaTopUpMembershipWitnessV1,
    /// Exact current-epoch `2f + 1` paired Pasta seal bundle.
    pub seal_bundle: KagemushaMintFinalitySealBundleV1,
    /// Complete dynamic epoch roster.  The recursive relation must authenticate its derived state
    /// digest; it is not trusted merely because it is present here.
    pub epoch_roster: KagemushaMintFinalityEpochRosterV1,
}

impl KagemushaMintCertificateWitnessV1 {
    /// Validate all non-authoritative shape and semantic bindings before circuit construction.
    ///
    /// Signature equations and roster authority are intentionally not reduced to this native
    /// preflight: the circuit proves the former and returns the digest which recursion must prove
    /// for the latter.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.validate_for_step(KagemushaMintAuthorityStepV1::FinalizedMint)
    }

    /// Derive the exact cross-parity certificate binding exposed by the recursive helper.
    ///
    /// This native derivation is only an instance constructor. Both parity circuits recompute the
    /// same digest from canonical paired roots, the signed message, and the complete roster.
    pub fn certificate_binding_digest(
        &self,
        step: KagemushaMintAuthorityStepV1,
    ) -> Result<[u8; 32], String> {
        self.validate_for_step(step)?;
        let semantic = self
            .statement
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let signing = self
            .seal_bundle
            .message
            .signing_digest()
            .map_err(|error| error.to_string())?;
        let roster = self
            .epoch_roster
            .finality_epoch_id()
            .map_err(|error| error.to_string())?;
        let mut hasher = Sha256::new();
        hasher.update(MINT_CERTIFICATE_BINDING_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(semantic);
        hasher.update(self.membership.root.eq);
        hasher.update(self.membership.root.ep);
        hasher.update(signing);
        hasher.update(roster);
        Ok(hasher.finalize().into())
    }

    pub(super) fn validate_for_step(
        &self,
        step: KagemushaMintAuthorityStepV1,
    ) -> Result<(), String> {
        self.statement
            .validate_shape()
            .map_err(|error| format!("invalid mint statement: {error}"))?;
        self.epoch_roster
            .validate()
            .map_err(|error| format!("invalid mint-finality roster: {error}"))?;
        self.seal_bundle
            .message
            .validate()
            .map_err(|error| format!("invalid mint-finality seal message: {error}"))?;
        self.membership
            .leaf
            .validate()
            .map_err(|error| format!("invalid top-up leaf: {error}"))?;
        if self.membership.siblings.len() != KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1
            || kagemusha_mint_finality_root_v1(self.membership.root)
                != self.seal_bundle.message.kagemusha_top_up_root
        {
            return Err("mint-finality paired root or path shape differs from seal message".into());
        }
        if step != KagemushaMintAuthorityStepV1::Bootstrap {
            self.seal_bundle
                .validate()
                .map_err(|error| format!("invalid mint-finality seal bundle: {error}"))?;
        } else if !self.seal_bundle.seals.is_empty() {
            return Err("mint-authority bootstrap must not carry validator seals".into());
        }
        if step == KagemushaMintAuthorityStepV1::FinalizedMint {
            self.membership
                .validate_against(&self.seal_bundle.message)
                .map_err(|error| format!("invalid top-up membership witness: {error}"))?;
            super::mint_finality::verify_kagemusha_top_up_membership_v1(
                &self.membership,
                self.seal_bundle.message.kagemusha_top_up_count,
            )
            .map_err(|error| error.to_string())?;
        } else if step == KagemushaMintAuthorityStepV1::Rotate
            && self.seal_bundle.message.next_finality_epoch_id.is_none()
        {
            return Err("mint-authority rotation lacks the quorum-signed next roster".into());
        }
        let statement_digest = self
            .statement
            .canonical_digest()
            .map_err(|error| format!("failed to digest mint statement: {error}"))?;
        let expected_epoch_id = self
            .epoch_roster
            .finality_epoch_id()
            .map_err(|error| format!("failed to digest mint-finality roster: {error}"))?;
        let message = &self.seal_bundle.message;
        if (step == KagemushaMintAuthorityStepV1::FinalizedMint
            && (self.membership.leaf.statement_digest != statement_digest
                || self.membership.leaf.amount != self.statement.amount))
            || self.statement.lifecycle.network_id != self.epoch_roster.network_id
            || message.network_id != self.epoch_roster.network_id
            || message.finality_epoch_id != expected_epoch_id
            || usize::try_from(message.validator_count).ok()
                != Some(self.epoch_roster.validators.len())
        {
            return Err(
                "mint statement, receipt leaf, seal message, and epoch roster differ".into(),
            );
        }
        Ok(())
    }
}

/// Cells produced by the reusable mint-certificate relation.
///
/// `roster_state_digest` and `epoch` must be consumed by the stable recursive authority carrier.
/// Exposing only `mint_instances` without that recursive check is not monetary authority.
pub(super) struct KagemushaAssignedMintCertificateV1<F: KagemushaPoseidonFieldV1> {
    pub(super) step: AssignedValue<F>,
    pub(super) bootstrap: AssignedValue<F>,
    pub(super) rotate: AssignedValue<F>,
    pub(super) finalized_mint: AssignedValue<F>,
    pub(super) mint_instances: [AssignedValue<F>; 3],
    pub(super) roster_state_digest: [AssignedValue<F>; 2],
    pub(super) certificate_binding_digest: [AssignedValue<F>; 2],
    pub(super) epoch: AssignedValue<F>,
    pub(super) next_epoch_present: AssignedValue<F>,
    pub(super) next_epoch_id_digest: [AssignedValue<F>; 2],
}

/// Circuit-side jobs emitted by one parity's mint-certificate component.
pub(super) struct KagemushaMintCertificateJobsV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
{
    pub(super) sha: PastaSha256JobsV1<C::Base>,
    pub(super) dense: PastaDenseMsmJobsV1<C>,
}

/// Fixed Base/Table16/dense-MSM configuration shared by both mint-certificate parities.
#[derive(Clone, Debug)]
pub(super) struct KagemushaMintCertificateCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp mint-certificate half. Pallas signatures are checked natively in the Fp circuit.
#[derive(Clone)]
pub(super) struct KagemushaMintCertificateEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq mint-certificate half. Vesta signatures are checked natively in the Fq circuit.
#[derive(Clone)]
pub(super) struct KagemushaMintCertificateEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_mint_certificate_circuit {
    ($circuit:ty, $field:ty, $signature_curve:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaMintCertificateCircuitConfigV1<$field>;
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
                KagemushaMintCertificateCircuitConfigV1 {
                    base,
                    sha: PastaSha256ConfigV1::configure(meta),
                    dense: PastaDenseMsmConfigV1::configure::<$signature_curve>(meta),
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

impl_mint_certificate_circuit!(
    KagemushaMintCertificateEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq mint certificate"
);
impl_mint_certificate_circuit!(
    KagemushaMintCertificateEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep mint certificate"
);

/// Build both fixed-shape certificate halves from the same exact finalized top-up evidence.
pub(super) fn build_kagemusha_mint_certificate_pair_v1(
    witness: KagemushaMintCertificateWitnessV1,
) -> Result<
    (
        KagemushaMintCertificateEqCircuitV1,
        KagemushaMintCertificateEpCircuitV1,
    ),
    String,
> {
    witness.validate_shape()?;
    let mut eq_builder = mint_certificate_builder::<Fp>();
    let (eq_assigned, eq_jobs) = constrain_kagemusha_mint_certificate_v1::<EpAffine>(
        &mut eq_builder,
        &witness,
        KagemushaPastaParityV1::Eq,
        KagemushaMintAuthorityStepV1::FinalizedMint,
    )?;
    eq_builder.assigned_instances = vec![
        eq_assigned
            .mint_instances
            .into_iter()
            .chain(eq_assigned.certificate_binding_digest)
            .collect(),
    ];
    let mut ep_builder = mint_certificate_builder::<Fq>();
    let (ep_assigned, ep_jobs) = constrain_kagemusha_mint_certificate_v1::<EqAffine>(
        &mut ep_builder,
        &witness,
        KagemushaPastaParityV1::Ep,
        KagemushaMintAuthorityStepV1::FinalizedMint,
    )?;
    ep_builder.assigned_instances = vec![
        ep_assigned
            .mint_instances
            .into_iter()
            .chain(ep_assigned.certificate_binding_digest)
            .collect(),
    ];
    if eq_builder.assigned_instances[0].len() != KAGEMUSHA_MINT_CERTIFICATE_PUBLIC_INSTANCE_COUNT_V1
        || ep_builder.assigned_instances[0].len()
            != KAGEMUSHA_MINT_CERTIFICATE_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("Kagemusha mint-certificate public shape drifted".to_owned());
    }
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << 16) - MINIMUM_UNUSABLE_ROWS;
    eq_jobs.sha.validate_capacity(usable_rows)?;
    eq_jobs.dense.validate_capacity(usable_rows)?;
    ep_jobs.sha.validate_capacity(usable_rows)?;
    ep_jobs.dense.validate_capacity(usable_rows)?;
    Ok((
        KagemushaMintCertificateEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_jobs.sha,
            dense_jobs: eq_jobs.dense,
        },
        KagemushaMintCertificateEpCircuitV1 {
            builder: ep_builder,
            sha_jobs: ep_jobs.sha,
            dense_jobs: ep_jobs.dense,
        },
    ))
}

fn mint_certificate_builder<F: KagemushaPoseidonFieldV1>() -> BaseCircuitBuilder<F> {
    BaseCircuitBuilder::new(false)
        .use_k(16)
        .use_lookup_bits(15)
        .use_instance_columns(1)
}

/// Constrain one parity of the complete dynamic-roster mint certificate.
///
/// The function always allocates `MAX_VALIDATORS_PER_HEIGHT` signature slots.  Roster activity
/// is a constrained prefix, signer activity is a constrained subset with exact `2f + 1` size,
/// and disabled equations have all three scalar coefficients forced to zero.  Therefore roster
/// size, quorum subset, and history do not change the circuit shape.
pub(super) fn constrain_kagemusha_mint_certificate_v1<C>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    witness: &KagemushaMintCertificateWitnessV1,
    parity: KagemushaPastaParityV1,
    step: KagemushaMintAuthorityStepV1,
) -> Result<
    (
        KagemushaAssignedMintCertificateV1<C::Base>,
        KagemushaMintCertificateJobsV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3> + ReciprocalAffine,
{
    witness.validate_for_step(step)?;
    let expected_eq = matches!(parity, KagemushaPastaParityV1::Eq);
    if C::Base::IS_EQ_PARITY != expected_eq {
        return Err("mint-certificate curve/parity mismatch".to_owned());
    }

    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let poseidon = KagemushaPoseidonChipV1::new(ctx, &range);
    let coordinate_chip = FpChip::<C::Base, C::Base>::new(&range, 86, 3);
    let scalar_chip = FpChip::<C::Base, C::ScalarExt>::new(&range, 86, 3);
    let mut sha = PastaSha256JobsV1::default();
    let mut dense = PastaDenseMsmJobsV1::default();

    let step = assign_uint(ctx, &range, u128::from(step as u64), 2);
    let bootstrap = gate.is_zero(ctx, step);
    let rotate = gate.is_equal(ctx, step, Constant(C::Base::ONE));
    let finalized_mint = gate.is_equal(ctx, step, Constant(C::Base::from(2)));
    let non_bootstrap_steps = gate.add(ctx, Existing(rotate), Existing(finalized_mint));
    let selected_step_count = gate.add(ctx, Existing(bootstrap), Existing(non_bootstrap_steps));
    gate.assert_is_const(ctx, &selected_step_count, &C::Base::ONE);
    let seal_enabled = non_bootstrap_steps;

    let semantic_digest = witness
        .statement
        .canonical_digest()
        .map_err(|error| error.to_string())?;
    let semantic_bytes: [PastaSha256ByteV1<C::Base>; 32] =
        assign_bytes(ctx, &range, &semantic_digest)
            .try_into()
            .expect("mint statement digest width");
    let semantic_limbs = sha_digest_limbs(ctx, gate, &semantic_bytes);
    let amount = assign_u128(ctx, &range, witness.statement.amount);
    let mint_instances = [semantic_limbs[0], semantic_limbs[1], amount];

    let leaf = &witness.membership.leaf;
    let operation = assign_digest_limbs(ctx, &range, leaf.operation_id);
    let receipt = assign_digest_limbs(ctx, &range, leaf.reserve_receipt_digest);
    let leaf_statement = assign_digest_limbs(ctx, &range, leaf.statement_digest);
    constrain_equal_if(
        ctx,
        gate,
        leaf_statement[0],
        semantic_limbs[0],
        finalized_mint,
    );
    constrain_equal_if(
        ctx,
        gate,
        leaf_statement[1],
        semantic_limbs[1],
        finalized_mint,
    );
    let leaf_amount = assign_u128(ctx, &range, leaf.amount);
    constrain_equal_if(ctx, gate, leaf_amount, amount, finalized_mint);
    let mut current = poseidon.hash(
        ctx,
        &range,
        MINT_LEAF_DOMAIN_V1,
        &[
            operation[0],
            operation[1],
            receipt[0],
            receipt[1],
            leaf_statement[0],
            leaf_statement[1],
            leaf_amount,
        ],
    );
    let leaf_index = assign_uint(ctx, &range, u128::from(witness.membership.leaf_index), 32);
    let index_bits = gate.num_to_bits(ctx, leaf_index, 32);
    for (level, sibling) in witness.membership.siblings.iter().copied().enumerate() {
        let component = C::Base::select_component(sibling);
        let sibling = decode::<C::Base>(component)
            .ok_or_else(|| format!("noncanonical mint sibling at depth {level}"))?;
        let sibling = coordinate_chip.load_private_reduced(ctx, sibling);
        let sibling = *sibling.inner().native();
        let direction = index_bits[level];
        let left = gate.select(
            ctx,
            Existing(sibling),
            Existing(current),
            Existing(direction),
        );
        let right = gate.select(
            ctx,
            Existing(current),
            Existing(sibling),
            Existing(direction),
        );
        current = poseidon.hash(ctx, &range, MINT_NODE_DOMAIN_V1, &[left, right]);
    }

    let root_component = C::Base::select_component(witness.membership.root);
    let root_value = decode::<C::Base>(root_component)
        .ok_or_else(|| "noncanonical parity-native mint root".to_owned())?;
    let root_value = coordinate_chip.load_private_reduced(ctx, root_value);
    constrain_equal_if(
        ctx,
        gate,
        current,
        *root_value.inner().native(),
        finalized_mint,
    );
    let root_native_bytes = proper_uint_le_bytes(ctx, &range, root_value.inner());
    let other_root_component = if C::Base::IS_EQ_PARITY {
        witness.membership.root.ep
    } else {
        witness.membership.root.eq
    };
    let other_root_value = decode::<C::ScalarExt>(other_root_component)
        .ok_or_else(|| "noncanonical reciprocal mint root".to_owned())?;
    let other_root_value = scalar_chip.load_private_reduced(ctx, other_root_value);
    let other_root_bytes = proper_uint_le_bytes(ctx, &range, other_root_value.inner());
    let (eq_root_bytes, ep_root_bytes) = if C::Base::IS_EQ_PARITY {
        (root_native_bytes, other_root_bytes)
    } else {
        (other_root_bytes, root_native_bytes)
    };
    let bridge = sha_digest(
        ctx,
        &mut sha,
        [
            constant_bytes(MINT_ROOT_BRIDGE_DOMAIN_V1),
            vec![PastaSha256ByteV1::constant(0)],
            eq_root_bytes.to_vec(),
            ep_root_bytes.to_vec(),
        ]
        .concat(),
    )?;
    let marked_root = mark_iroha_hash(ctx, gate, bridge);

    let message = &witness.seal_bundle.message;
    let top_up_count = assign_uint(ctx, &range, u128::from(message.kagemusha_top_up_count), 32);
    let top_up_count_is_zero = gate.is_zero(ctx, top_up_count);
    let enabled_zero_count = gate.mul(
        ctx,
        Existing(finalized_mint),
        Existing(top_up_count_is_zero),
    );
    gate.assert_is_const(ctx, &enabled_zero_count, &C::Base::ZERO);
    let index_lt_count = range.is_less_than(ctx, leaf_index, top_up_count, 32);
    let one = ctx.load_constant(C::Base::ONE);
    constrain_equal_if(ctx, gate, index_lt_count, one, finalized_mint);

    let validator_count_value = witness.epoch_roster.validators.len();
    let validator_count_u32 = u32::try_from(validator_count_value)
        .map_err(|_| "mint roster length does not fit u32".to_owned())?;
    let validator_count = assign_uint(ctx, &range, u128::from(validator_count_u32), 32);
    let fault_count_value = (validator_count_value - 1) / 3;
    let fault_count = assign_uint(
        ctx,
        &range,
        u128::try_from(fault_count_value).map_err(|_| "fault count overflow".to_owned())?,
        4,
    );
    let three_f_plus_one = gate.mul_add(
        ctx,
        Existing(fault_count),
        Constant(C::Base::from(3)),
        Constant(C::Base::ONE),
    );
    ctx.constrain_equal(&three_f_plus_one, &validator_count);

    let finality_epoch_id = assign_bytes(ctx, &range, &message.finality_epoch_id);
    let network_id = assign_bytes(ctx, &range, witness.epoch_roster.network_id.as_bytes());
    let epoch = assign_uint(ctx, &range, u128::from(witness.epoch_roster.epoch), 64);
    let block_height = assign_uint(ctx, &range, u128::from(message.block_height), 64);
    assert_nonzero(ctx, gate, block_height);
    let context_id = assign_bytes(ctx, &range, message.height_context_id.0.as_ref());
    let subject_digest = assign_bytes(ctx, &range, &message.subject_digest);
    let execution_digest = assign_bytes(ctx, &range, &message.execution_commitment_digest);
    let (next_epoch_present_value, next_epoch_id_value) = match message.next_finality_epoch_id {
        Some(next_epoch_id) => (true, next_epoch_id),
        None => (false, [0; 32]),
    };
    let next_epoch_present = ctx.load_witness(C::Base::from(u64::from(next_epoch_present_value)));
    gate.assert_bit(ctx, next_epoch_present);
    let next_epoch_present_byte = PastaSha256ByteV1::range_checked(ctx, &range, next_epoch_present);
    let next_epoch_id = assign_bytes(ctx, &range, &next_epoch_id_value);
    let next_epoch_absent = gate.not(ctx, next_epoch_present);
    for byte in &next_epoch_id {
        let absent_byte = gate.mul(ctx, byte.quantum_cell(), Existing(next_epoch_absent));
        gate.assert_is_const(ctx, &absent_byte, &C::Base::ZERO);
    }
    let next_epoch_sum = next_epoch_id
        .iter()
        .copied()
        .fold(ctx.load_zero(), |sum, byte| {
            gate.add(ctx, Existing(sum), byte.quantum_cell())
        });
    let next_epoch_is_zero = gate.is_zero(ctx, next_epoch_sum);
    let present_zero = gate.mul(
        ctx,
        Existing(next_epoch_present),
        Existing(next_epoch_is_zero),
    );
    gate.assert_is_const(ctx, &present_zero, &C::Base::ZERO);
    constrain_equal_if(ctx, gate, next_epoch_present, one, rotate);
    let next_epoch_id_digest = sha_digest_limbs(
        ctx,
        gate,
        &next_epoch_id
            .clone()
            .try_into()
            .expect("next mint-finality epoch ID width"),
    );
    for bytes in [
        &finality_epoch_id,
        &network_id,
        &context_id,
        &subject_digest,
        &execution_digest,
    ] {
        assert_bytes_nonzero(ctx, gate, bytes);
    }
    let validator_count_bytes = uint_bytes_le(ctx, gate, validator_count, 32);
    let block_height_bytes = uint_bytes_le(ctx, gate, block_height, 64);
    let top_up_count_bytes = uint_bytes_le(ctx, gate, top_up_count, 32);
    let signing_digest = sha_digest(
        ctx,
        &mut sha,
        [
            constant_bytes(MINT_SEAL_MESSAGE_DOMAIN_V1),
            vec![PastaSha256ByteV1::constant(0)],
            constant_bytes(&KAGEMUSHA_CHAIN_VERSION_V1.to_le_bytes()),
            finality_epoch_id.clone(),
            validator_count_bytes,
            network_id.clone(),
            block_height_bytes,
            context_id.clone(),
            subject_digest.clone(),
            execution_digest.clone(),
            marked_root.to_vec(),
            top_up_count_bytes,
            vec![next_epoch_present_byte],
            next_epoch_id,
        ]
        .concat(),
    )?;

    let mut roster_preimage = [
        constant_bytes(MINT_FINALITY_EPOCH_ROSTER_DOMAIN_V1),
        vec![PastaSha256ByteV1::constant(0)],
        constant_bytes(&KAGEMUSHA_CHAIN_VERSION_V1.to_le_bytes()),
        network_id,
        uint_bytes_le(ctx, gate, epoch, 64),
        uint_bytes_le(ctx, gate, validator_count, 32),
    ]
    .concat();

    let generator = C::generator();
    let reciprocal_generator = reciprocal_generator::<C>();
    let seals_by_index = witness
        .seal_bundle
        .seals
        .iter()
        .map(|seal| {
            (
                usize::try_from(seal.validator_index).expect("u32 fits usize"),
                seal,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut active_sum = ctx.load_zero();
    let mut signer_sum = ctx.load_zero();
    let mut previous_active = ctx.load_constant(C::Base::ONE);
    for slot in 0..MAX_VALIDATORS_PER_HEIGHT {
        let active_value = slot < validator_count_value;
        let signer = seals_by_index.get(&slot).copied();
        let signer_value = signer.is_some();
        let active = ctx.load_witness(C::Base::from(u64::from(active_value)));
        let signed = ctx.load_witness(C::Base::from(u64::from(signer_value)));
        gate.assert_bit(ctx, active);
        gate.assert_bit(ctx, signed);
        let prior_inactive = gate.not(ctx, previous_active);
        let inactive_after_active = gate.mul(ctx, Existing(active), Existing(prior_inactive));
        gate.assert_is_const(ctx, &inactive_after_active, &C::Base::ZERO);
        let inactive = gate.not(ctx, active);
        let signed_when_inactive = gate.mul(ctx, Existing(signed), Existing(inactive));
        gate.assert_is_const(ctx, &signed_when_inactive, &C::Base::ZERO);
        let seal_disabled = gate.not(ctx, seal_enabled);
        let signed_when_disabled = gate.mul(ctx, Existing(signed), Existing(seal_disabled));
        gate.assert_is_const(ctx, &signed_when_disabled, &C::Base::ZERO);
        active_sum = gate.add(ctx, Existing(active_sum), Existing(active));
        signer_sum = gate.add(ctx, Existing(signer_sum), Existing(signed));
        previous_active = active;

        let (peer_digest, current_key_bytes, reciprocal_key_bytes) = if active_value {
            let keys = &witness.epoch_roster.validators[slot];
            let peer_digest = kagemusha_mint_finality_peer_id_digest_v1(&keys.validator)
                .map_err(|error| error.to_string())?;
            if C::Base::IS_EQ_PARITY {
                (
                    peer_digest,
                    keys.eq_proof_public_key,
                    keys.ep_proof_public_key,
                )
            } else {
                (
                    peer_digest,
                    keys.ep_proof_public_key,
                    keys.eq_proof_public_key,
                )
            }
        } else {
            (
                [0; 32],
                encode_point(generator),
                encode_point(reciprocal_generator),
            )
        };
        let public = decode_point::<C>(current_key_bytes)
            .ok_or_else(|| format!("invalid current-parity roster key at slot {slot}"))?;
        let reciprocal = decode_reciprocal_point::<C>(reciprocal_key_bytes)
            .ok_or_else(|| format!("invalid reciprocal roster key at slot {slot}"))?;
        // Roster values are private witnesses even when active.  Loading them as constants would
        // silently make the helper VK epoch-specific and strand balances at validator rotation.
        let public_assigned = assign_point(ctx, &coordinate_chip, public);
        let reciprocal_assigned = assign_scalar_base_point::<C>(ctx, &scalar_chip, reciprocal);
        let public_bytes =
            compressed_point_bytes(ctx, &range, &public_assigned.x, &public_assigned.y);
        let reciprocal_bytes =
            compressed_point_bytes(ctx, &range, &reciprocal_assigned.x, &reciprocal_assigned.y);
        let (eq_key_bytes, ep_key_bytes) = if C::Base::IS_EQ_PARITY {
            (public_bytes, reciprocal_bytes)
        } else {
            (reciprocal_bytes, public_bytes)
        };
        roster_preimage.push(PastaSha256ByteV1::range_checked(ctx, &range, active));
        let peer_digest = assign_bytes(ctx, &range, &peer_digest);
        roster_preimage.extend(mask_sha_bytes(ctx, &range, &peer_digest, active));
        roster_preimage.extend(mask_sha_bytes(ctx, &range, &eq_key_bytes, active));
        roster_preimage.extend(mask_sha_bytes(ctx, &range, &ep_key_bytes, active));

        let signature = signer.map_or_else(dummy_signature::<C>, |seal| {
            if C::Base::IS_EQ_PARITY {
                seal.eq_proof_signature
            } else {
                seal.ep_proof_signature
            }
        });
        let nonce = decode_point::<C>(signature.nonce_commitment)
            .ok_or_else(|| format!("invalid nonce point at mint signer slot {slot}"))?;
        let nonce_assigned = assign_point(ctx, &coordinate_chip, nonce);
        let nonce_bytes = compressed_point_bytes(ctx, &range, &nonce_assigned.x, &nonce_assigned.y);
        let response_value = decode_scalar::<C::ScalarExt>(signature.response)
            .filter(|value| !bool::from(value.is_zero()))
            .ok_or_else(|| format!("invalid response scalar at mint signer slot {slot}"))?;
        let response = scalar_chip.load_private_reduced(ctx, response_value);
        let response: ProperCrtUint<C::Base> = response.into();

        let challenge_digest = sha_digest(
            ctx,
            &mut sha,
            [
                constant_bytes(MINT_CHALLENGE_DOMAIN_V1),
                vec![PastaSha256ByteV1::constant(0)],
                vec![PastaSha256ByteV1::constant(if C::Base::IS_EQ_PARITY {
                    EQ_PARITY_TAG
                } else {
                    EP_PARITY_TAG
                })],
                constant_bytes(
                    &u32::try_from(slot)
                        .expect("bounded validator slot fits u32")
                        .to_le_bytes(),
                ),
                signing_digest.to_vec(),
                nonce_bytes.to_vec(),
                public_bytes.to_vec(),
            ]
            .concat(),
        )?;
        let challenge_host = from_u128::<C::ScalarExt>(u128::from_le_bytes(
            Sha256::digest(challenge_preimage_host::<C>(
                slot,
                message,
                witness.membership.root,
                signature.nonce_commitment,
                current_key_bytes,
            )?)[..16]
                .try_into()
                .expect("challenge half"),
        ));
        let challenge = scalar_chip.load_private_reduced(ctx, challenge_host);
        bind_challenge_scalar(ctx, gate, &range, challenge.inner(), &challenge_digest);
        let challenge: ProperCrtUint<C::Base> = challenge.into();

        let signer_scalar = scalar_from_bit(ctx, &scalar_chip, signed);
        let response_enabled = scalar_mul(&scalar_chip, ctx, response, signer_scalar.clone());
        let negative_one = scalar_chip.load_constant(ctx, -C::ScalarExt::ONE);
        let nonce_coefficient = scalar_mul(&scalar_chip, ctx, negative_one, signer_scalar.clone());
        let negative_challenge = scalar_chip.negate(ctx, challenge);
        let public_coefficient = scalar_mul(&scalar_chip, ctx, negative_challenge, signer_scalar);
        let (generator_x, generator_y) = generator.into_coordinates();
        let sources = [
            PastaDenseMsmSourceV1 {
                point: generator,
                x: ctx.load_constant(generator_x),
                y: ctx.load_constant(generator_y),
                coefficient: response_enabled,
            },
            PastaDenseMsmSourceV1 {
                point: nonce,
                x: *nonce_assigned.x.as_ref().native(),
                y: *nonce_assigned.y.as_ref().native(),
                coefficient: nonce_coefficient,
            },
            PastaDenseMsmSourceV1 {
                point: public,
                x: *public_assigned.x.as_ref().native(),
                y: *public_assigned.y.as_ref().native(),
                coefficient: public_coefficient,
            },
        ];
        dense.queue_constrained(ctx, &scalar_chip, &sources)?;
    }
    ctx.constrain_equal(&active_sum, &validator_count);
    let expected_quorum = gate.mul_add(
        ctx,
        Existing(fault_count),
        Constant(C::Base::from(2)),
        Constant(C::Base::ONE),
    );
    constrain_equal_if(ctx, gate, signer_sum, expected_quorum, seal_enabled);
    let roster_digest = sha_digest(ctx, &mut sha, roster_preimage)?;
    for (actual, expected) in roster_digest.iter().zip(&finality_epoch_id) {
        ctx.constrain_equal(
            &actual.assigned().expect("roster digest byte is assigned"),
            &expected.assigned().expect("epoch ID byte is assigned"),
        );
    }
    let roster_state_digest = sha_digest_limbs(ctx, gate, &roster_digest);
    let certificate_binding = sha_digest(
        ctx,
        &mut sha,
        [
            constant_bytes(MINT_CERTIFICATE_BINDING_DOMAIN_V1),
            vec![PastaSha256ByteV1::constant(0)],
            semantic_bytes.to_vec(),
            eq_root_bytes.to_vec(),
            ep_root_bytes.to_vec(),
            signing_digest.to_vec(),
            roster_digest.to_vec(),
        ]
        .concat(),
    )?;
    let certificate_binding_digest = sha_digest_limbs(ctx, gate, &certificate_binding);

    Ok((
        KagemushaAssignedMintCertificateV1 {
            step,
            bootstrap,
            rotate,
            finalized_mint,
            mint_instances,
            roster_state_digest,
            certificate_binding_digest,
            epoch,
            next_epoch_present,
            next_epoch_id_digest,
        },
        KagemushaMintCertificateJobsV1 { sha, dense },
    ))
}

struct AssignedPoint<F: BigPrimeField> {
    x: ProperCrtUint<F>,
    y: ProperCrtUint<F>,
}

fn assign_point<C>(
    ctx: &mut Context<C::Base>,
    chip: &FpChip<'_, C::Base, C::Base>,
    point: C,
) -> AssignedPoint<C::Base>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
{
    let (x, y) = point.into_coordinates();
    let x = chip.load_private(ctx, x);
    let y = chip.load_private(ctx, y);
    AssignedPoint {
        x: chip.enforce_less_than(ctx, x).into(),
        y: chip.enforce_less_than(ctx, y).into(),
    }
}

fn assign_scalar_base_point<C>(
    ctx: &mut Context<C::Base>,
    chip: &FpChip<'_, C::Base, C::ScalarExt>,
    point: ReciprocalCurve<C>,
) -> AssignedPoint<C::Base>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ReciprocalAffine,
{
    let (x, y) = <ReciprocalCurve<C> as CurveAffineExt>::into_coordinates(point);
    let x = chip.load_private(ctx, x);
    let y = chip.load_private(ctx, y);
    AssignedPoint {
        x: chip.enforce_less_than(ctx, x).into(),
        y: chip.enforce_less_than(ctx, y).into(),
    }
}

type ReciprocalCurve<C> = <<C as CurveAffine>::ScalarExt as ReciprocalAffine>::Affine;

pub(super) trait ReciprocalAffine: PrimeField {
    type Affine: CurveAffine<Base = Self> + CurveAffineExt;
    fn generator() -> Self::Affine;
    fn decode(bytes: [u8; 32]) -> Option<Self::Affine>;
}

impl ReciprocalAffine for halo2_proofs::halo2curves::pasta::Fp {
    type Affine = halo2_proofs::halo2curves::pasta::EpAffine;
    fn generator() -> Self::Affine {
        Self::Affine::generator()
    }
    fn decode(bytes: [u8; 32]) -> Option<Self::Affine> {
        decode_point(bytes)
    }
}

impl ReciprocalAffine for halo2_proofs::halo2curves::pasta::Fq {
    type Affine = halo2_proofs::halo2curves::pasta::EqAffine;
    fn generator() -> Self::Affine {
        Self::Affine::generator()
    }
    fn decode(bytes: [u8; 32]) -> Option<Self::Affine> {
        decode_point(bytes)
    }
}

fn reciprocal_generator<C>() -> ReciprocalCurve<C>
where
    C: CurveAffine,
    C::ScalarExt: ReciprocalAffine,
{
    C::ScalarExt::generator()
}

fn decode_reciprocal_point<C>(bytes: [u8; 32]) -> Option<ReciprocalCurve<C>>
where
    C: CurveAffine,
    C::ScalarExt: ReciprocalAffine,
{
    C::ScalarExt::decode(bytes)
}

fn scalar_mul<F, S>(
    chip: &FpChip<'_, F, S>,
    ctx: &mut Context<F>,
    left: ProperCrtUint<F>,
    right: ProperCrtUint<F>,
) -> ProperCrtUint<F>
where
    F: BigPrimeField,
    S: BigPrimeField,
{
    let product = chip.mul_no_carry(ctx, left, right);
    chip.carry_mod(ctx, product)
}

fn scalar_from_bit<F, S>(
    ctx: &mut Context<F>,
    chip: &FpChip<'_, F, S>,
    bit: AssignedValue<F>,
) -> ProperCrtUint<F>
where
    F: BigPrimeField,
    S: BigPrimeField,
{
    let value = if bool::from(bit.value().is_zero()) {
        S::ZERO
    } else {
        S::ONE
    };
    let scalar = chip.load_private_reduced(ctx, value);
    ctx.constrain_equal(&scalar.inner().limbs()[0], &bit);
    for limb in &scalar.inner().limbs()[1..] {
        chip.gate().assert_is_const(ctx, limb, &F::ZERO);
    }
    scalar.into()
}

fn bind_challenge_scalar<F>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    range: &RangeChip<F>,
    scalar: &ProperCrtUint<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
) where
    F: BigPrimeField,
{
    let bytes = proper_uint_le_bytes(ctx, range, scalar);
    for (actual, expected) in bytes[..16].iter().zip(&digest[..16]) {
        ctx.constrain_equal(
            &actual.assigned().expect("proper scalar byte is assigned"),
            &expected.assigned().expect("SHA challenge byte is assigned"),
        );
    }
    for byte in &bytes[16..] {
        gate.assert_is_const(
            ctx,
            &byte.assigned().expect("proper scalar byte is assigned"),
            &F::ZERO,
        );
    }
}

fn assign_digest_limbs<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    digest_limbs::<F>(digest).map(|value| {
        let assigned = ctx.load_witness(value);
        range.range_check(ctx, assigned, 128);
        assigned
    })
}

fn assign_u128<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u128,
) -> AssignedValue<F> {
    let assigned = ctx.load_witness(from_u128(value));
    range.range_check(ctx, assigned, 128);
    assigned
}

fn assign_uint<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u128,
    bits: usize,
) -> AssignedValue<F> {
    let assigned = ctx.load_witness(from_u128(value));
    range.range_check(ctx, assigned, bits);
    assigned
}

fn uint_bytes_le<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    value: AssignedValue<F>,
    bits: usize,
) -> Vec<PastaSha256ByteV1<F>> {
    PastaSha256BitV1::decompose(ctx, gate, value, bits)
        .chunks_exact(8)
        .map(|chunk| PastaSha256ByteV1::from_bits_le(ctx, gate, chunk))
        .collect()
}

fn assign_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
) -> Vec<PastaSha256ByteV1<F>> {
    bytes
        .iter()
        .copied()
        .map(|byte| {
            let value = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, range, value)
        })
        .collect()
}

pub(super) fn constant_bytes<F: KagemushaPoseidonFieldV1>(
    bytes: &[u8],
) -> Vec<PastaSha256ByteV1<F>> {
    bytes
        .iter()
        .copied()
        .map(PastaSha256ByteV1::constant)
        .collect()
}

fn mask_sha_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
    enabled: AssignedValue<F>,
) -> Vec<PastaSha256ByteV1<F>> {
    bytes
        .iter()
        .copied()
        .map(|byte| {
            let selected = range
                .gate()
                .mul(ctx, byte.quantum_cell(), Existing(enabled));
            PastaSha256ByteV1::range_checked(ctx, range, selected)
        })
        .collect()
}

pub(super) fn sha_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    message: Vec<PastaSha256ByteV1<F>>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let words = jobs.digest_constrained(ctx, &message)?;
    let gate = halo2_base::gates::GateChip::default();
    let mut bytes = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, &gate, word, 32);
        for offset in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                &gate,
                &bits[offset..offset + 8],
            ));
        }
    }
    Ok(bytes.try_into().expect("SHA-256 digest width"))
}

pub(super) fn sha_digest_limbs<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
) -> [AssignedValue<F>; 2] {
    std::array::from_fn(|half| {
        gate.inner_product(
            ctx,
            digest[half * 16..half * 16 + 16]
                .iter()
                .copied()
                .map(PastaSha256ByteV1::quantum_cell),
            (0..16).map(|index| Constant(F::from_u128(1_u128 << (8 * index)))),
        )
    })
}

fn mark_iroha_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    mut digest: [PastaSha256ByteV1<F>; 32],
) -> [PastaSha256ByteV1<F>; 32] {
    let mut bits = digest[31].decompose_bits_le(ctx, gate);
    let one = ctx.load_constant(F::ONE);
    bits[0] = PastaSha256BitV1::decompose(ctx, gate, one, 1)[0];
    digest[31] = PastaSha256ByteV1::from_bits_le(ctx, gate, &bits);
    digest
}

fn constrain_equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
    enabled: AssignedValue<F>,
) {
    let difference = gate.sub(ctx, Existing(left), Existing(right));
    let selected = gate.mul(ctx, Existing(difference), Existing(enabled));
    gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn assert_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    value: AssignedValue<F>,
) {
    let is_zero = gate.is_zero(ctx, value);
    gate.assert_is_const(ctx, &is_zero, &F::ZERO);
}

fn assert_bytes_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) {
    let sum = bytes.iter().copied().fold(ctx.load_zero(), |sum, byte| {
        gate.add(ctx, Existing(sum), byte.quantum_cell())
    });
    assert_nonzero(ctx, gate, sum);
}

fn encode_point<C: CurveAffine>(point: C) -> [u8; 32] {
    point
        .to_bytes()
        .as_ref()
        .try_into()
        .expect("Pasta point width")
}

fn decode_point<C: CurveAffine>(bytes: [u8; 32]) -> Option<C> {
    let mut repr = <C as GroupEncoding>::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::<C>::from(C::from_bytes(&repr)).filter(|point| !bool::from(point.is_identity()))
}

fn decode_scalar<F: PrimeField>(bytes: [u8; 32]) -> Option<F> {
    let mut repr = F::Repr::default();
    repr.as_mut().copy_from_slice(&bytes);
    Option::from(F::from_repr(repr))
}

fn dummy_signature<C>() -> KagemushaPastaSchnorrSignatureV1
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    KagemushaPastaSchnorrSignatureV1 {
        nonce_commitment: encode_point(C::generator()),
        response: C::ScalarExt::ONE
            .to_repr()
            .as_ref()
            .try_into()
            .expect("Pasta scalar width"),
    }
}

fn challenge_preimage_host<C>(
    slot: usize,
    message: &iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalitySealMessageV1,
    root: iroha_data_model::kagemusha::KagemushaPastaStateCommitmentV1,
    nonce: [u8; 32],
    public: [u8; 32],
) -> Result<Vec<u8>, String>
where
    C: CurveAffine,
    C::Base: KagemushaPoseidonFieldV1,
{
    let mut signing = Vec::new();
    signing.extend_from_slice(MINT_SEAL_MESSAGE_DOMAIN_V1);
    signing.push(0);
    signing.extend_from_slice(&KAGEMUSHA_CHAIN_VERSION_V1.to_le_bytes());
    signing.extend_from_slice(&message.finality_epoch_id);
    signing.extend_from_slice(&message.validator_count.to_le_bytes());
    signing.extend_from_slice(message.network_id.as_bytes());
    signing.extend_from_slice(&message.block_height.to_le_bytes());
    signing.extend_from_slice(message.height_context_id.0.as_ref());
    signing.extend_from_slice(&message.subject_digest);
    signing.extend_from_slice(&message.execution_commitment_digest);
    let mut bridge = Sha256::new();
    bridge.update(MINT_ROOT_BRIDGE_DOMAIN_V1);
    bridge.update([0]);
    bridge.update(root.eq);
    bridge.update(root.ep);
    let mut bridge: [u8; 32] = bridge.finalize().into();
    bridge[31] |= 1;
    signing.extend_from_slice(&bridge);
    signing.extend_from_slice(&message.kagemusha_top_up_count.to_le_bytes());
    match message.next_finality_epoch_id {
        Some(next_epoch_id) => {
            signing.push(1);
            signing.extend_from_slice(&next_epoch_id);
        }
        None => {
            signing.push(0);
            signing.extend_from_slice(&[0; 32]);
        }
    }
    let signing: [u8; 32] = Sha256::digest(signing).into();
    let mut challenge = Vec::new();
    challenge.extend_from_slice(MINT_CHALLENGE_DOMAIN_V1);
    challenge.push(0);
    challenge.push(if C::Base::IS_EQ_PARITY {
        EQ_PARITY_TAG
    } else {
        EP_PARITY_TAG
    });
    challenge.extend_from_slice(
        &u32::try_from(slot)
            .map_err(|_| "validator slot overflow".to_owned())?
            .to_le_bytes(),
    );
    challenge.extend_from_slice(&signing);
    challenge.extend_from_slice(&nonce);
    challenge.extend_from_slice(&public);
    Ok(challenge)
}

const _: () = {
    assert!(KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1 == 32);
    assert!(MAX_VALIDATORS_PER_HEIGHT == 31);
    assert!(MINIMUM_UNUSABLE_ROWS == 9);
};
