//! Recursive GuardBundle wrapper for Offline Cash V1.
//!
//! Each wrapper scalar-verifies the six fixed children on its own Pasta
//! parity, folds their opening accumulators to one carried lineage, and binds
//! the exact 36-cell clean-V1 projection.  Its reciprocal sibling supplies the
//! other parity's opaque scalar audit; this circuit enforces every equation in
//! that audit through the reviewed serial Base-graph point machine. Consequently neither a
//! native pre-verification result nor an unclosed deferred equation can grant
//! proof authority.

use std::mem;

use ff::{Field as _, WithSmallOrderMulGroup};
use halo2_base::{
    AssignedValue, QuantumCell,
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt, ScalarField},
};
use halo2_ecc::fields::fp::FpChip;
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
    OFFLINE_CASH_IPA_LINEAGE_VERSION_V1, OfflineCashIpaLineageV1,
    OfflineCashRecursivePairBindingV1, OfflineCashRecursivePairTopologyV1,
};
use snark_verifier::{pcs::ipa::IpaSuccinctVerifyingKey, verifier::plonk::PlonkProtocol};
use zeroize::Zeroizing;

use crate::zk::{
    kagemusha_accumulation::kagemusha_ipa_accumulation_proof_bytes_v4,
    kagemusha_cycle_loader::{DeferredScalarEccChip, LIMB_BITS, LIMBS},
    kagemusha_recursion_adapter::{
        constrain_poseidon_reciprocal_audit_serial_v1,
        scalar_lineage_v1::{
            DeferredLoader, PoseidonDeferredEquationStageV1, PoseidonRecursiveAuditBindingCellsV1,
            PoseidonRecursiveScalarAuditV1, capture_poseidon_recursive_scalar_audit_v1,
            constrain_poseidon_child_fold_v1, constrain_poseidon_child_proof_v1,
            constrain_poseidon_folded_accumulator_instance_v1,
            constrain_poseidon_recursive_scalar_audit_v1,
        },
    },
    kagemusha_sha256_v4::KagemushaConstrainedSha256V1,
};

use super::{
    OfflineCashHalo2ParityV1,
    helper_abi::{
        HELPER_ABI_WORDS, HELPER_ANDROID_PRESENT_WORD, HELPER_INSTANCE_CELLS,
        HELPER_WORDS_PER_INSTANCE, OfflineCashHelperAbiErrorV1, OfflineCashHelperPublicInstancesV1,
        fixed_helper_word_v1, pack_words_as_field,
    },
    packed_base::{
        OfflineCashPackedBaseConfigV1, OfflineCashPackedBaseTraceV1, OfflineCashPackedSha256JobsV1,
    },
    protocol::{
        OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1,
        OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1,
        OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1, OfflineCashHalo2CircuitRoleV1,
        offline_cash_internal_child_proof_max_bytes_v1,
    },
};

const PAIR_EQ_DIGEST_WORD_START: usize = 32;
const PAIR_EP_DIGEST_WORD_START: usize = 40;
const PAIR_DIGEST_WORDS: usize = 8;
const PAIR_CHILD_BINDING_DIGEST_WORD_START: usize = 48;
const PAIR_RESERVED_WORD_START: usize = 56;
const P256_INSTANCE_CELLS: usize = 396;
const P256_STATEMENT_PREFIX_CELLS: usize = 65 + 32;
const GUARD_BUNDLE_CHILDREN: usize = 6;
const GUARD_BUNDLE_STAGE_FOLD: u32 = 7;

// GuardBundle is an internal proof, so its role-specific 64-KiB slot—not the
// 3,200-byte final-State wire cap—governs this shape.  These fixed columns are
// part of the authenticated wrapper protocol identity.
const GUARD_BUNDLE_ADVICE_COLUMNS: usize = 96;
const GUARD_BUNDLE_LOOKUP_COLUMNS: usize = 12;
const GUARD_BUNDLE_FIXED_COLUMNS: usize = 1;
const GUARD_BUNDLE_INSTANCE_COLUMNS: usize = 3;

const _: () = assert!(OFFLINE_CASH_IPA_LINEAGE_VERSION_V1 == 1);
const _: () = assert!(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1 == 16);
const _: () = assert!(OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 == 36);
const _: () = assert!(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 == 20);

/// Fixed child slot; order is authenticated by the wrapper VK and audit tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashGuardBundleChildSlotV1 {
    GuardUse = 1,
    PlatformBind = 2,
    AndroidKeyCert = 3,
    GuardBundleLeaf = 4,
    PlatformP256 = 5,
    AndroidP256 = 6,
}

impl OfflineCashGuardBundleChildSlotV1 {
    const ALL: [Self; GUARD_BUNDLE_CHILDREN] = [
        Self::GuardUse,
        Self::PlatformBind,
        Self::AndroidKeyCert,
        Self::GuardBundleLeaf,
        Self::PlatformP256,
        Self::AndroidP256,
    ];

    const fn role(self) -> OfflineCashHalo2CircuitRoleV1 {
        match self {
            Self::GuardUse => OfflineCashHalo2CircuitRoleV1::GuardUse,
            Self::PlatformBind => OfflineCashHalo2CircuitRoleV1::PlatformBind,
            Self::AndroidKeyCert => OfflineCashHalo2CircuitRoleV1::AndroidKeyCert,
            Self::GuardBundleLeaf => OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf,
            Self::PlatformP256 | Self::AndroidP256 => OfflineCashHalo2CircuitRoleV1::P256V3,
        }
    }

    const fn instance_column_lengths(self) -> &'static [usize] {
        match self {
            Self::GuardUse | Self::GuardBundleLeaf => &[HELPER_INSTANCE_CELLS],
            Self::PlatformBind | Self::AndroidKeyCert => &[
                HELPER_INSTANCE_CELLS,
                OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1 as usize,
            ],
            Self::PlatformP256 | Self::AndroidP256 => &[P256_INSTANCE_CELLS],
        }
    }
}

/// Owned, length-bounded ordinary child proof and its authenticated protocol.
#[derive(Clone)]
pub(super) struct OfflineCashGuardBundleChildProofV1<C>
where
    C: CurveAffineExt,
{
    slot: OfflineCashGuardBundleChildSlotV1,
    protocol: PlonkProtocol<C>,
    instances: Vec<Vec<C::ScalarExt>>,
    proof_bytes: Zeroizing<Vec<u8>>,
}

impl<C> core::fmt::Debug for OfflineCashGuardBundleChildProofV1<C>
where
    C: CurveAffineExt,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashGuardBundleChildProofV1")
            .field("slot", &self.slot)
            .field("instance_columns", &self.instances.len())
            .field("proof_bytes", &self.proof_bytes.len())
            .finish_non_exhaustive()
    }
}

impl<C> OfflineCashGuardBundleChildProofV1<C>
where
    C: CurveAffineExt,
{
    /// Close one slot over owned proof bytes. Protocol/VK selection happens at
    /// the authenticated artifact loader before this constructor is called;
    /// the wrapper VK then commits the complete protocol constants in this
    /// fixed slot.
    pub(super) fn new(
        slot: OfflineCashGuardBundleChildSlotV1,
        protocol: PlonkProtocol<C>,
        instances: Vec<Vec<C::ScalarExt>>,
        proof_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        let expected = slot.instance_column_lengths();
        let maximum = usize::try_from(
            offline_cash_internal_child_proof_max_bytes_v1(slot.role())
                .ok_or_else(|| "GuardBundle child has no governed proof slot".to_owned())?,
        )
        .map_err(|_| "GuardBundle child proof bound does not fit usize".to_owned())?;
        if proof_bytes.is_empty()
            || proof_bytes.len() > maximum
            || protocol.domain.k != OFFLINE_CASH_HALO2_K_V1 as usize
            || protocol.domain.n != 1_usize << OFFLINE_CASH_HALO2_K_V1
            || protocol.num_instance.as_slice() != expected
            || instances.len() != expected.len()
            || instances
                .iter()
                .zip(expected)
                .any(|(column, expected)| column.len() != *expected)
        {
            return Err("GuardBundle child proof/protocol/instance shape mismatch".to_owned());
        }
        Ok(Self {
            slot,
            protocol,
            instances,
            proof_bytes: Zeroizing::new(proof_bytes),
        })
    }
}

/// Complete same-parity recursive witness for one GuardBundle wrapper.
#[derive(Clone)]
pub(super) struct OfflineCashGuardBundleParityRecursionV1<C>
where
    C: CurveAffineExt,
{
    succinct_vk: IpaSuccinctVerifyingKey<C>,
    children: [OfflineCashGuardBundleChildProofV1<C>; GUARD_BUNDLE_CHILDREN],
    fold_proof_bytes: Zeroizing<Vec<u8>>,
}

impl<C> OfflineCashGuardBundleParityRecursionV1<C>
where
    C: CurveAffineExt,
{
    pub(super) fn new(
        succinct_vk: IpaSuccinctVerifyingKey<C>,
        children: [OfflineCashGuardBundleChildProofV1<C>; GUARD_BUNDLE_CHILDREN],
        fold_proof_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        if succinct_vk.domain.k != OFFLINE_CASH_HALO2_K_V1 as usize
            || succinct_vk.domain.n != 1_usize << OFFLINE_CASH_HALO2_K_V1
        {
            return Err("GuardBundle succinct verifier is not on the common k16 domain".to_owned());
        }
        for (expected, child) in OfflineCashGuardBundleChildSlotV1::ALL
            .into_iter()
            .zip(&children)
        {
            if child.slot != expected {
                return Err("GuardBundle child role/order mismatch".to_owned());
            }
        }
        let expected_fold =
            kagemusha_ipa_accumulation_proof_bytes_v4(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)?;
        if fold_proof_bytes.len() != expected_fold {
            return Err("GuardBundle fold proof has the wrong exact length".to_owned());
        }
        Ok(Self {
            succinct_vk,
            children,
            fold_proof_bytes: Zeroizing::new(fold_proof_bytes),
        })
    }
}

/// Exact public columns of one recursive GuardBundle parity.
#[derive(Clone, Debug)]
pub(super) struct OfflineCashGuardBundleRecursivePublicV1 {
    common: OfflineCashHelperPublicInstancesV1,
    pair_binding: OfflineCashRecursivePairBindingV1,
    carried_lineage: OfflineCashIpaLineageV1,
}

impl OfflineCashGuardBundleRecursivePublicV1 {
    pub(super) fn new(
        common: OfflineCashHelperPublicInstancesV1,
        pair_binding: OfflineCashRecursivePairBindingV1,
        carried_lineage: OfflineCashIpaLineageV1,
    ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
        if common.role() != OfflineCashHalo2CircuitRoleV1::GuardBundle
            || pair_binding.topology().ok() != Some(OfflineCashRecursivePairTopologyV1::GuardBundle)
            || carried_lineage.validate().is_err()
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }
        Ok(Self {
            common,
            pair_binding,
            carried_lineage,
        })
    }

    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV1 {
        self.common.parity()
    }

    pub(super) fn instance_columns<F>(&self) -> Result<Vec<Vec<F>>, OfflineCashHelperAbiErrorV1>
    where
        F: ff::PrimeField,
    {
        let common = self.common.field_instances::<F>().to_vec();
        let pair_words = self
            .pair_binding
            .canonical_words()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidLayout)?;
        let pair = pair_words
            .chunks(HELPER_WORDS_PER_INSTANCE)
            .map(pack_words_as_field::<F>)
            .collect::<Vec<_>>();
        let lineage = self
            .carried_lineage
            .instance_limbs()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidLayout)?
            .into_iter()
            .map(F::from_u128)
            .collect::<Vec<_>>();
        if common.len() != HELPER_INSTANCE_CELLS
            || pair.len()
                != usize::try_from(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1)
                    .expect("fixed pair-binding cell count fits usize")
            || lineage.len()
                != usize::try_from(OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1)
                    .expect("fixed lineage cell count fits usize")
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidLayout);
        }
        Ok(vec![common, pair, lineage])
    }
}

fn guard_bundle_base_params_v1() -> BaseCircuitParams {
    BaseCircuitParams {
        k: OFFLINE_CASH_HALO2_K_V1 as usize,
        num_advice_per_phase: vec![GUARD_BUNDLE_ADVICE_COLUMNS],
        num_fixed: GUARD_BUNDLE_FIXED_COLUMNS,
        num_lookup_advice_per_phase: vec![GUARD_BUNDLE_LOOKUP_COLUMNS, 0, 0],
        lookup_bits: Some(OFFLINE_CASH_HALO2_K_V1 as usize - 1),
        num_instance_columns: GUARD_BUNDLE_INSTANCE_COLUMNS,
    }
}

fn pack_assigned_words<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    words: impl IntoIterator<Item = QuantumCell<F>>,
) -> AssignedValue<F>
where
    F: BigPrimeField,
{
    let words = words.into_iter().collect::<Vec<_>>();
    let radix = F::from(1_u64 << 32);
    let mut weight = F::ONE;
    let weights = (0..words.len())
        .map(|_| {
            let current = QuantumCell::Constant(weight);
            weight *= radix;
            current
        })
        .collect::<Vec<_>>();
    range.gate().inner_product(ctx, words, weights)
}

fn assign_guard_bundle_public_v1<F>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &OfflineCashGuardBundleRecursivePublicV1,
) -> Result<
    (
        Vec<AssignedValue<F>>,
        Vec<AssignedValue<F>>,
        Vec<AssignedValue<F>>,
    ),
    String,
>
where
    F: BigPrimeField + ScalarField,
{
    let columns = public
        .instance_columns::<F>()
        .map_err(|error| format!("invalid GuardBundle public columns: {error}"))?;
    if columns.iter().map(Vec::len).collect::<Vec<_>>()
        != vec![
            HELPER_INSTANCE_CELLS,
            OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 as usize,
            OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize,
        ]
    {
        return Err("GuardBundle public column geometry mismatch".to_owned());
    }
    let assigned = columns
        .into_iter()
        .map(|values| builder.main(0).assign_witnesses(values))
        .collect::<Vec<_>>();
    builder.assigned_instances = assigned.clone();

    let common_values = public
        .common
        .words()
        .iter()
        .map(|word| F::from(u64::from(*word)))
        .collect::<Vec<_>>();
    let pair_values = public
        .pair_binding
        .canonical_words()
        .map_err(|_| "invalid GuardBundle pair binding".to_owned())?
        .into_iter()
        .map(|word| F::from(u64::from(word)))
        .collect::<Vec<_>>();
    let common_words = builder.main(0).assign_witnesses(common_values);
    let pair_words = builder.main(0).assign_witnesses(pair_values);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for word in common_words.iter().chain(&pair_words) {
        range.range_check(ctx, *word, 32);
    }
    for (cell, chunk) in assigned[0]
        .iter()
        .zip(common_words.chunks(HELPER_WORDS_PER_INSTANCE))
    {
        let packed = pack_assigned_words(
            ctx,
            &range,
            chunk.iter().copied().map(QuantumCell::Existing),
        );
        ctx.constrain_equal(&packed, cell);
    }
    for (cell, chunk) in assigned[1]
        .iter()
        .zip(pair_words.chunks(HELPER_WORDS_PER_INSTANCE))
    {
        let packed = pack_assigned_words(
            ctx,
            &range,
            chunk.iter().copied().map(QuantumCell::Existing),
        );
        ctx.constrain_equal(&packed, cell);
    }

    for (index, word) in common_words.iter().enumerate() {
        if let Some(expected) = fixed_helper_word_v1(
            public.parity(),
            OfflineCashHalo2CircuitRoleV1::GuardBundle,
            index,
        ) {
            range
                .gate()
                .assert_is_const(ctx, word, &F::from(u64::from(expected)));
        }
    }
    // Use dummy distinct digests only to derive the topology's fixed header and
    // mandatory zero tail. Digest words 32..47 remain witness/public values.
    let template = OfflineCashRecursivePairBindingV1::new_guard_bundle([1; 32], [2; 32])
        .and_then(|binding| binding.canonical_words())
        .map_err(|_| "failed to derive fixed GuardBundle pair-binding template".to_owned())?;
    for (index, word) in pair_words.iter().enumerate() {
        if !(PAIR_EQ_DIGEST_WORD_START..PAIR_CHILD_BINDING_DIGEST_WORD_START).contains(&index) {
            range
                .gate()
                .assert_is_const(ctx, word, &F::from(u64::from(template[index])));
        }
    }
    let mut all_digest_words_equal = ctx.load_constant(F::ONE);
    for (eq, ep) in pair_words
        [PAIR_EQ_DIGEST_WORD_START..PAIR_EQ_DIGEST_WORD_START + PAIR_DIGEST_WORDS]
        .iter()
        .zip(&pair_words[PAIR_EP_DIGEST_WORD_START..PAIR_EP_DIGEST_WORD_START + PAIR_DIGEST_WORDS])
    {
        let equal = range.gate().is_equal(ctx, *eq, *ep);
        all_digest_words_equal = range.gate().mul(
            ctx,
            QuantumCell::Existing(all_digest_words_equal),
            QuantumCell::Existing(equal),
        );
    }
    range
        .gate()
        .assert_is_const(ctx, &all_digest_words_equal, &F::ZERO);
    for digest in [
        &pair_words[PAIR_EQ_DIGEST_WORD_START..PAIR_EQ_DIGEST_WORD_START + PAIR_DIGEST_WORDS],
        &pair_words[PAIR_EP_DIGEST_WORD_START..PAIR_EP_DIGEST_WORD_START + PAIR_DIGEST_WORDS],
    ] {
        let mut all_zero = ctx.load_constant(F::ONE);
        for word in digest {
            let zero = range.gate().is_zero(ctx, *word);
            all_zero = range.gate().mul(
                ctx,
                QuantumCell::Existing(all_zero),
                QuantumCell::Existing(zero),
            );
        }
        range.gate().assert_is_const(ctx, &all_zero, &F::ZERO);
    }
    Ok((common_words, pair_words, assigned[2].clone()))
}

fn expected_helper_child_primary_v1<C>(
    loader: &DeferredLoader<'_, C>,
    parent_words: &[AssignedValue<C::ScalarExt>],
    parity: OfflineCashHalo2ParityV1,
    role: OfflineCashHalo2CircuitRoleV1,
) -> Result<Vec<AssignedValue<C::ScalarExt>>, snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if parent_words.len() != HELPER_ABI_WORDS {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    Ok(parent_words
        .chunks(HELPER_WORDS_PER_INSTANCE)
        .enumerate()
        .map(|(cell, chunk)| {
            let start = cell * HELPER_WORDS_PER_INSTANCE;
            pack_assigned_words(
                ctx.main(),
                range,
                chunk.iter().enumerate().map(|(lane, assigned)| {
                    let index = start + lane;
                    fixed_helper_word_v1(parity, role, index)
                        .map_or(QuantumCell::Existing(*assigned), |word| {
                            QuantumCell::Constant(C::ScalarExt::from(u64::from(word)))
                        })
                }),
            )
        })
        .collect())
}

fn constrain_equal_when_v1<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    selector: AssignedValue<F>,
    lhs: AssignedValue<F>,
    rhs: AssignedValue<F>,
) where
    F: BigPrimeField,
{
    let difference = range
        .gate()
        .sub(ctx, QuantumCell::Existing(lhs), QuantumCell::Existing(rhs));
    let selected = range.gate().mul(
        ctx,
        QuantumCell::Existing(selector),
        QuantumCell::Existing(difference),
    );
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn constrain_guard_bundle_child_equality_v1<C>(
    loader: &DeferredLoader<'_, C>,
    parity: OfflineCashHalo2ParityV1,
    parent_words: &[AssignedValue<C::ScalarExt>],
    children: &[crate::zk::kagemusha_recursion_adapter::scalar_lineage_v1::ConstrainedPoseidonChildProofV1<'_, C>],
) -> Result<(), snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if children.len() != GUARD_BUNDLE_CHILDREN {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    for (slot, child) in OfflineCashGuardBundleChildSlotV1::ALL
        .into_iter()
        .zip(children)
        .take(4)
    {
        let expected = expected_helper_child_primary_v1(loader, parent_words, parity, slot.role())?;
        if child.instances.first().map(Vec::len) != Some(expected.len()) {
            return Err(snark_verifier::Error::InvalidInstances);
        }
        let mut ctx = loader.ctx_mut();
        for (actual, expected) in child.instances[0].iter().zip(expected) {
            ctx.main().constrain_equal(actual, &expected);
        }
    }

    let platform_aux = children[OfflineCashGuardBundleChildSlotV1::PlatformBind as usize - 1]
        .instances
        .get(1)
        .ok_or(snark_verifier::Error::InvalidInstances)?;
    let android_aux = children[OfflineCashGuardBundleChildSlotV1::AndroidKeyCert as usize - 1]
        .instances
        .get(1)
        .ok_or(snark_verifier::Error::InvalidInstances)?;
    let platform_p256 =
        &children[OfflineCashGuardBundleChildSlotV1::PlatformP256 as usize - 1].instances[0];
    let android_p256 =
        &children[OfflineCashGuardBundleChildSlotV1::AndroidP256 as usize - 1].instances[0];
    if platform_aux.len() != P256_STATEMENT_PREFIX_CELLS
        || android_aux.len() != P256_STATEMENT_PREFIX_CELLS
        || platform_p256.len() != P256_INSTANCE_CELLS
        || android_p256.len() != P256_INSTANCE_CELLS
    {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let android_present = parent_words[HELPER_ANDROID_PRESENT_WORD];
    range.gate().assert_bit(ctx.main(), android_present);
    let android_absent = range.gate().not(ctx.main(), android_present);
    let zero = ctx.main().load_constant(C::ScalarExt::ZERO);
    for (aux, p256) in platform_aux.iter().zip(platform_p256) {
        ctx.main().constrain_equal(aux, p256);
    }
    for (aux, p256) in android_aux.iter().zip(android_p256) {
        constrain_equal_when_v1(ctx.main(), range, android_present, *aux, *p256);
        constrain_equal_when_v1(ctx.main(), range, android_absent, *aux, zero);
    }
    // The absent Android slot is never skipped. It is the exact authenticated
    // platform P256 proof/statement duplicated into the Android P256 slot.
    for (android, platform) in android_p256.iter().zip(platform_p256) {
        constrain_equal_when_v1(ctx.main(), range, android_absent, *android, *platform);
    }
    Ok(())
}

fn constrain_guard_bundle_scalar_v1<C, S>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    sha_jobs: &mut S,
    public: &OfflineCashGuardBundleRecursivePublicV1,
    recursion: &OfflineCashGuardBundleParityRecursionV1<C>,
    bind_own_audit: bool,
) -> Result<PoseidonRecursiveScalarAuditV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
    S: KagemushaConstrainedSha256V1<C::ScalarExt>,
{
    let expected_parity = public.parity();
    let (parent_words, pair_words, lineage_cells) = assign_guard_bundle_public_v1(builder, public)?;
    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let chip = DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer);
    let loader = snark_verifier::loader::halo2::Halo2Loader::new(chip, mem::take(builder.pool(0)));
    let maximums = recursion
        .children
        .iter()
        .map(|child| {
            offline_cash_internal_child_proof_max_bytes_v1(child.slot.role())
                .and_then(|maximum| usize::try_from(maximum).ok())
                .ok_or_else(|| "GuardBundle child proof cap is invalid".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut constrained = Vec::with_capacity(GUARD_BUNDLE_CHILDREN);
    let mut stages = Vec::with_capacity(GUARD_BUNDLE_CHILDREN + 1);
    for (index, (child, maximum)) in recursion.children.iter().zip(maximums).enumerate() {
        let output = constrain_poseidon_child_proof_v1(
            &loader,
            &recursion.succinct_vk,
            &child.protocol,
            &child.instances,
            &child.proof_bytes,
            maximum,
        )
        .map_err(|error| {
            format!(
                "failed to constrain GuardBundle {:?} child: {error:?}",
                child.slot
            )
        })?;
        stages.push(
            PoseidonDeferredEquationStageV1::new(
                output.deferred_equations.clone(),
                u32::try_from(index + 1)
                    .map_err(|_| "GuardBundle audit tag overflow".to_owned())?,
            )
            .map_err(|error| format!("invalid GuardBundle child audit stage: {error:?}"))?,
        );
        constrained.push(output);
    }
    constrain_guard_bundle_child_equality_v1(&loader, expected_parity, &parent_words, &constrained)
        .map_err(|error| format!("failed GuardBundle child public equality: {error:?}"))?;

    let accumulators = constrained
        .iter()
        .map(|child| child.accumulator.clone())
        .collect::<Vec<_>>();
    let expected_fold_bytes =
        kagemusha_ipa_accumulation_proof_bytes_v4(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)?;
    let (folded, fold_range) = constrain_poseidon_child_fold_v1(
        &loader,
        &recursion.succinct_vk,
        &accumulators,
        &recursion.fold_proof_bytes,
        expected_fold_bytes,
    )
    .map_err(|error| format!("failed to constrain GuardBundle child fold: {error:?}"))?;
    stages.push(
        PoseidonDeferredEquationStageV1::new(fold_range, GUARD_BUNDLE_STAGE_FOLD)
            .map_err(|error| format!("invalid GuardBundle fold audit stage: {error:?}"))?,
    );
    constrain_poseidon_folded_accumulator_instance_v1(
        &loader,
        OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
        OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
        &folded,
        &lineage_cells,
    )
    .map_err(|error| format!("failed to bind GuardBundle carried lineage: {error:?}"))?;
    let own_digest_start = match expected_parity {
        OfflineCashHalo2ParityV1::Eq => PAIR_EQ_DIGEST_WORD_START,
        OfflineCashHalo2ParityV1::Ep => PAIR_EP_DIGEST_WORD_START,
    };
    let audit = if bind_own_audit {
        constrain_poseidon_recursive_scalar_audit_v1(
            &loader,
            sha_jobs,
            &stages,
            PoseidonRecursiveAuditBindingCellsV1 {
                audit_digest_words: &pair_words
                    [own_digest_start..own_digest_start + PAIR_DIGEST_WORDS],
            },
        )
        .map_err(|error| format!("failed to bind GuardBundle scalar audit: {error:?}"))?
    } else {
        capture_poseidon_recursive_scalar_audit_v1(&loader, &stages)
            .map_err(|error| format!("failed to capture GuardBundle scalar audit: {error:?}"))?
    };
    *builder.pool(0) = loader.take_ctx();
    Ok(audit)
}

fn constrain_guard_bundle_reciprocal_v1<C, S>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    sha_jobs: &mut S,
    public: &OfflineCashGuardBundleRecursivePublicV1,
    reciprocal: &PoseidonRecursiveScalarAuditV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ScalarField + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3>,
    S: KagemushaConstrainedSha256V1<C::Base>,
{
    // Public columns and raw words were already assigned by this circuit's
    // same-parity scalar half. Recover the exact pair words as fresh witnesses
    // and copy-pack them into the same second public column, so both halves bind
    // one canonical value without relying on host identity.
    let pair_values = public
        .pair_binding
        .canonical_words()
        .map_err(|_| "invalid reciprocal GuardBundle pair binding".to_owned())?
        .into_iter()
        .map(|word| C::Base::from(u64::from(word)))
        .collect::<Vec<_>>();
    let pair_words = builder.main(0).assign_witnesses(pair_values);
    if builder.assigned_instances.len() != GUARD_BUNDLE_INSTANCE_COLUMNS
        || builder.assigned_instances[1].len()
            != OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 as usize
    {
        return Err("GuardBundle reciprocal public geometry mismatch".to_owned());
    }
    let pair_public_cells = builder.assigned_instances[1].clone();
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for word in &pair_words {
        range.range_check(ctx, *word, 32);
    }
    for (cell, chunk) in pair_public_cells
        .iter()
        .zip(pair_words.chunks(HELPER_WORDS_PER_INSTANCE))
    {
        let packed = pack_assigned_words(
            ctx,
            &range,
            chunk.iter().copied().map(QuantumCell::Existing),
        );
        ctx.constrain_equal(&packed, cell);
    }
    let reciprocal_digest_start = match public.parity() {
        // Eq wrapper enforces Ep's audit; Ep wrapper enforces Eq's audit.
        OfflineCashHalo2ParityV1::Eq => PAIR_EP_DIGEST_WORD_START,
        OfflineCashHalo2ParityV1::Ep => PAIR_EQ_DIGEST_WORD_START,
    };
    constrain_poseidon_reciprocal_audit_serial_v1(
        builder,
        sha_jobs,
        reciprocal,
        PoseidonRecursiveAuditBindingCellsV1 {
            audit_digest_words: &pair_words
                [reciprocal_digest_start..reciprocal_digest_start + PAIR_DIGEST_WORDS],
        },
    )
}

#[derive(Clone, Debug)]
pub(super) struct OfflineCashGuardBundleCompositeConfigV1 {
    packed: OfflineCashPackedBaseConfigV1<GUARD_BUNDLE_INSTANCE_COLUMNS>,
}

fn configure_guard_bundle_eq_v1(
    meta: &mut ConstraintSystem<Fp>,
) -> OfflineCashGuardBundleCompositeConfigV1 {
    OfflineCashGuardBundleCompositeConfigV1 {
        packed: OfflineCashPackedBaseConfigV1::<GUARD_BUNDLE_INSTANCE_COLUMNS>::configure(meta),
    }
}

fn configure_guard_bundle_ep_v1(
    meta: &mut ConstraintSystem<Fq>,
) -> OfflineCashGuardBundleCompositeConfigV1 {
    OfflineCashGuardBundleCompositeConfigV1 {
        packed: OfflineCashPackedBaseConfigV1::<GUARD_BUNDLE_INSTANCE_COLUMNS>::configure(meta),
    }
}

/// Eq/Fp recursive GuardBundle wrapper. The serial point half enforces the Ep
/// scalar audit supplied during pair construction.
#[derive(Clone)]
pub(super) struct OfflineCashEqGuardBundleCircuitV1 {
    trace: OfflineCashPackedBaseTraceV1<Fp, GUARD_BUNDLE_INSTANCE_COLUMNS>,
    break_points: Vec<Vec<usize>>,
    audit_inventory: [usize; 4],
}

impl core::fmt::Debug for OfflineCashEqGuardBundleCircuitV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEqGuardBundleCircuitV1")
            .field("instance_columns", &GUARD_BUNDLE_INSTANCE_COLUMNS)
            .field("carried_lineage_cells", &36)
            .field("packed_advice", &8)
            .field("packed_fixed", &3)
            .field("assigned_rows", &self.trace.assigned_rows())
            .field("sha_inventory[jobs,blocks]", &self.trace.sha_inventory())
            .field(
                "audit_inventory[own_sources,own_equations,reciprocal_sources,reciprocal_equations]",
                &self.audit_inventory,
            )
            .field("reciprocal_point_audit", &"Ep/serial-base")
            .finish_non_exhaustive()
    }
}

impl Circuit<Fp> for OfflineCashEqGuardBundleCircuitV1 {
    type Config = OfflineCashGuardBundleCompositeConfigV1;
    type FloorPlanner = V1;
    type Params = ();

    fn params(&self) -> Self::Params {
        ()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            trace: self.trace.without_witnesses(),
            break_points: self.break_points.clone(),
            audit_inventory: self.audit_inventory,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        configure_guard_bundle_eq_v1(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        self.trace.synthesize(&config.packed, &mut layouter)
    }
}

/// Ep/Fq recursive GuardBundle wrapper. The serial point half enforces the Eq
/// scalar audit supplied during pair construction.
#[derive(Clone)]
pub(super) struct OfflineCashEpGuardBundleCircuitV1 {
    trace: OfflineCashPackedBaseTraceV1<Fq, GUARD_BUNDLE_INSTANCE_COLUMNS>,
    break_points: Vec<Vec<usize>>,
    audit_inventory: [usize; 4],
}

impl core::fmt::Debug for OfflineCashEpGuardBundleCircuitV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEpGuardBundleCircuitV1")
            .field("instance_columns", &GUARD_BUNDLE_INSTANCE_COLUMNS)
            .field("carried_lineage_cells", &36)
            .field("packed_advice", &8)
            .field("packed_fixed", &3)
            .field("assigned_rows", &self.trace.assigned_rows())
            .field("sha_inventory[jobs,blocks]", &self.trace.sha_inventory())
            .field(
                "audit_inventory[own_sources,own_equations,reciprocal_sources,reciprocal_equations]",
                &self.audit_inventory,
            )
            .field("reciprocal_point_audit", &"Eq/serial-base")
            .finish_non_exhaustive()
    }
}

impl Circuit<Fq> for OfflineCashEpGuardBundleCircuitV1 {
    type Config = OfflineCashGuardBundleCompositeConfigV1;
    type FloorPlanner = V1;
    type Params = ();

    fn params(&self) -> Self::Params {
        ()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            trace: self.trace.without_witnesses(),
            break_points: self.break_points.clone(),
            audit_inventory: self.audit_inventory,
        }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
        configure_guard_bundle_ep_v1(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fq>,
    ) -> Result<(), Error> {
        self.trace.synthesize(&config.packed, &mut layouter)
    }
}

#[derive(Clone, Copy)]
enum GuardBundleBuilderStageV1<'a> {
    Keygen,
    Prover(&'a [Vec<usize>]),
}

fn guard_bundle_builder_v1<F>(stage: GuardBundleBuilderStageV1<'_>) -> BaseCircuitBuilder<F>
where
    F: ScalarField,
{
    let _ = stage;
    // The packed compiler owns the physical schedule. Both keygen and proving
    // collect the same constraint-bearing virtual graph; halo2-base
    // witness-only breakpoints are metadata and never proof authority.
    BaseCircuitBuilder::new(false).use_params(guard_bundle_base_params_v1())
}

fn compile_guard_bundle_trace_v1<F>(
    builder: &BaseCircuitBuilder<F>,
    sha_jobs: &OfflineCashPackedSha256JobsV1<F>,
) -> Result<OfflineCashPackedBaseTraceV1<F, GUARD_BUNDLE_INSTANCE_COLUMNS>, String>
where
    F: BigPrimeField + ScalarField,
{
    OfflineCashPackedBaseTraceV1::from_builder(builder, sha_jobs)
}

fn collect_guard_bundle_scalar_audit_v1<C>(
    public: &OfflineCashGuardBundleRecursivePublicV1,
    recursion: &OfflineCashGuardBundleParityRecursionV1<C>,
) -> Result<PoseidonRecursiveScalarAuditV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ff::PrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
{
    let mut builder = guard_bundle_builder_v1::<C::ScalarExt>(GuardBundleBuilderStageV1::Keygen);
    let mut sha_jobs = OfflineCashPackedSha256JobsV1::default();
    constrain_guard_bundle_scalar_v1(&mut builder, &mut sha_jobs, public, recursion, false)
}

/// Derive the exact reciprocal audit digests needed to construct the governed
/// GuardBundle pair binding during dev-only artifact generation. This exposes
/// no proof-verification or activation capability; the returned digests are
/// accepted only after the final packed circuits, keys, and signed release
/// artifacts are generated and authenticated.
#[cfg(feature = "dev-tools")]
pub(super) fn offline_cash_guard_bundle_keygen_audit_digests_v1(
    eq_public: &OfflineCashGuardBundleRecursivePublicV1,
    ep_public: &OfflineCashGuardBundleRecursivePublicV1,
    eq_recursion: &OfflineCashGuardBundleParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashGuardBundleParityRecursionV1<EpAffine>,
) -> Result<([u8; 32], [u8; 32]), String> {
    if eq_public.parity() != OfflineCashHalo2ParityV1::Eq
        || ep_public.parity() != OfflineCashHalo2ParityV1::Ep
        || eq_public.pair_binding != ep_public.pair_binding
        || !guard_bundle_common_semantics_match_v1(eq_public, ep_public)
    {
        return Err("GuardBundle keygen audit prepass parity/semantics mismatch".to_owned());
    }
    let eq = collect_guard_bundle_scalar_audit_v1(eq_public, eq_recursion)?
        .audit_sha256()
        .map_err(|error| format!("invalid Eq GuardBundle keygen audit: {error:?}"))?;
    let ep = collect_guard_bundle_scalar_audit_v1(ep_public, ep_recursion)?
        .audit_sha256()
        .map_err(|error| format!("invalid Ep GuardBundle keygen audit: {error:?}"))?;
    if eq == ep {
        return Err("GuardBundle keygen audit parities unexpectedly alias".to_owned());
    }
    Ok((eq, ep))
}

fn guard_bundle_common_semantics_match_v1(
    eq: &OfflineCashGuardBundleRecursivePublicV1,
    ep: &OfflineCashGuardBundleRecursivePublicV1,
) -> bool {
    use super::helper_abi::{HELPER_PARITY_WORD, HELPER_PROTOCOL_WORD_START, RELEASE_WORD_START};
    eq.common
        .words()
        .iter()
        .zip(ep.common.words())
        .enumerate()
        .all(|(index, (eq, ep))| {
            index == HELPER_PARITY_WORD
                || (HELPER_PROTOCOL_WORD_START..RELEASE_WORD_START).contains(&index)
                || eq == ep
        })
}

fn build_guard_bundle_eq_circuit_v1(
    public: &OfflineCashGuardBundleRecursivePublicV1,
    recursion: &OfflineCashGuardBundleParityRecursionV1<EqAffine>,
    reciprocal: &PoseidonRecursiveScalarAuditV1<EpAffine>,
    stage: GuardBundleBuilderStageV1<'_>,
) -> Result<OfflineCashEqGuardBundleCircuitV1, String> {
    if public.parity() != OfflineCashHalo2ParityV1::Eq {
        return Err("Eq GuardBundle circuit received the wrong public parity".to_owned());
    }
    let mut builder = guard_bundle_builder_v1::<Fp>(stage);
    let mut sha_jobs = OfflineCashPackedSha256JobsV1::default();
    let own_audit =
        constrain_guard_bundle_scalar_v1(&mut builder, &mut sha_jobs, public, recursion, true)?;
    constrain_guard_bundle_reciprocal_v1::<EpAffine, _>(
        &mut builder,
        &mut sha_jobs,
        public,
        reciprocal,
    )?;
    let trace = compile_guard_bundle_trace_v1(&builder, &sha_jobs)?;
    let break_points = match stage {
        GuardBundleBuilderStageV1::Keygen => Vec::new(),
        GuardBundleBuilderStageV1::Prover(value) => value.to_vec(),
    };
    Ok(OfflineCashEqGuardBundleCircuitV1 {
        trace,
        break_points,
        audit_inventory: [
            own_audit.source_count(),
            own_audit.equation_count(),
            reciprocal.source_count(),
            reciprocal.equation_count(),
        ],
    })
}

fn build_guard_bundle_ep_circuit_v1(
    public: &OfflineCashGuardBundleRecursivePublicV1,
    recursion: &OfflineCashGuardBundleParityRecursionV1<EpAffine>,
    reciprocal: &PoseidonRecursiveScalarAuditV1<EqAffine>,
    stage: GuardBundleBuilderStageV1<'_>,
) -> Result<OfflineCashEpGuardBundleCircuitV1, String> {
    if public.parity() != OfflineCashHalo2ParityV1::Ep {
        return Err("Ep GuardBundle circuit received the wrong public parity".to_owned());
    }
    let mut builder = guard_bundle_builder_v1::<Fq>(stage);
    let mut sha_jobs = OfflineCashPackedSha256JobsV1::default();
    let own_audit =
        constrain_guard_bundle_scalar_v1(&mut builder, &mut sha_jobs, public, recursion, true)?;
    constrain_guard_bundle_reciprocal_v1::<EqAffine, _>(
        &mut builder,
        &mut sha_jobs,
        public,
        reciprocal,
    )?;
    let trace = compile_guard_bundle_trace_v1(&builder, &sha_jobs)?;
    let break_points = match stage {
        GuardBundleBuilderStageV1::Keygen => Vec::new(),
        GuardBundleBuilderStageV1::Prover(value) => value.to_vec(),
    };
    Ok(OfflineCashEpGuardBundleCircuitV1 {
        trace,
        break_points,
        audit_inventory: [
            own_audit.source_count(),
            own_audit.equation_count(),
            reciprocal.source_count(),
            reciprocal.equation_count(),
        ],
    })
}

/// Construct the reciprocal Eq/Ep keygen circuits from one exact shared
/// GuardBundle pair binding. The two scalar prepasses are witness-only and
/// grant no authority; their audit digests must equal the public binding before
/// either final circuit is returned.
pub(super) fn build_guard_bundle_keygen_pair_v1(
    eq_public: &OfflineCashGuardBundleRecursivePublicV1,
    ep_public: &OfflineCashGuardBundleRecursivePublicV1,
    eq_recursion: &OfflineCashGuardBundleParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashGuardBundleParityRecursionV1<EpAffine>,
) -> Result<
    (
        OfflineCashEqGuardBundleCircuitV1,
        OfflineCashEpGuardBundleCircuitV1,
    ),
    String,
> {
    build_guard_bundle_pair_v1(
        eq_public,
        ep_public,
        eq_recursion,
        ep_recursion,
        GuardBundleBuilderStageV1::Keygen,
        GuardBundleBuilderStageV1::Keygen,
    )
}

/// Construct the reciprocal Eq/Ep prover circuits with authenticated keygen
/// breakpoints. A role/parity/protocol swap changes either the scalar audit or
/// the wrapper graph and is rejected by the shared binding/release VK.
pub(super) fn build_guard_bundle_prover_pair_v1(
    eq_public: &OfflineCashGuardBundleRecursivePublicV1,
    ep_public: &OfflineCashGuardBundleRecursivePublicV1,
    eq_recursion: &OfflineCashGuardBundleParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashGuardBundleParityRecursionV1<EpAffine>,
    eq_break_points: &[Vec<usize>],
    ep_break_points: &[Vec<usize>],
) -> Result<
    (
        OfflineCashEqGuardBundleCircuitV1,
        OfflineCashEpGuardBundleCircuitV1,
    ),
    String,
> {
    build_guard_bundle_pair_v1(
        eq_public,
        ep_public,
        eq_recursion,
        ep_recursion,
        GuardBundleBuilderStageV1::Prover(eq_break_points),
        GuardBundleBuilderStageV1::Prover(ep_break_points),
    )
}

fn build_guard_bundle_pair_v1(
    eq_public: &OfflineCashGuardBundleRecursivePublicV1,
    ep_public: &OfflineCashGuardBundleRecursivePublicV1,
    eq_recursion: &OfflineCashGuardBundleParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashGuardBundleParityRecursionV1<EpAffine>,
    eq_stage: GuardBundleBuilderStageV1<'_>,
    ep_stage: GuardBundleBuilderStageV1<'_>,
) -> Result<
    (
        OfflineCashEqGuardBundleCircuitV1,
        OfflineCashEpGuardBundleCircuitV1,
    ),
    String,
> {
    if eq_public.parity() != OfflineCashHalo2ParityV1::Eq
        || ep_public.parity() != OfflineCashHalo2ParityV1::Ep
        || eq_public.pair_binding != ep_public.pair_binding
        || !guard_bundle_common_semantics_match_v1(eq_public, ep_public)
    {
        return Err("GuardBundle reciprocal public pair mismatch".to_owned());
    }
    let eq_audit = collect_guard_bundle_scalar_audit_v1(eq_public, eq_recursion)?;
    let ep_audit = collect_guard_bundle_scalar_audit_v1(ep_public, ep_recursion)?;
    let eq_digest = eq_audit
        .audit_sha256()
        .map_err(|error| format!("invalid Eq GuardBundle scalar audit: {error:?}"))?;
    let ep_digest = ep_audit
        .audit_sha256()
        .map_err(|error| format!("invalid Ep GuardBundle scalar audit: {error:?}"))?;
    if eq_public.pair_binding.eq_audit_digest != eq_digest
        || eq_public.pair_binding.ep_audit_digest != ep_digest
        || eq_digest == ep_digest
    {
        return Err("GuardBundle public audit digests do not match scalar prepasses".to_owned());
    }
    let eq = build_guard_bundle_eq_circuit_v1(eq_public, eq_recursion, &ep_audit, eq_stage)?;
    let ep = build_guard_bundle_ep_circuit_v1(ep_public, ep_recursion, &eq_audit, ep_stage)?;
    Ok((eq, ep))
}

impl OfflineCashEqGuardBundleCircuitV1 {
    pub(super) fn break_points(&self) -> Vec<Vec<usize>> {
        self.break_points.clone()
    }
}

impl OfflineCashEpGuardBundleCircuitV1 {
    pub(super) fn break_points(&self) -> Vec<Vec<usize>> {
        self.break_points.clone()
    }
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{dev::MockProver, halo2curves::group::prime::PrimeCurveAffine as _};
    use snark_verifier::{loader::native::NativeLoader, pcs::ipa::IpaAccumulator};

    use super::*;
    use crate::zk::offline_cash_v1::helper_recursion::{
        offline_cash_ep_lineage_instance_column_v1, offline_cash_eq_lineage_instance_column_v1,
        offline_cash_lineage_from_ep_v1, offline_cash_lineage_from_eq_v1,
        offline_cash_lineage_to_ep_v1, offline_cash_lineage_to_eq_v1,
    };
    use crate::zk::pasta_ipa_recursion::{
        PastaIpaInstanceQueryV1, pasta_ipa_augmented_proof_shape_v1,
    };

    fn eq_accumulator() -> IpaAccumulator<EqAffine, NativeLoader> {
        IpaAccumulator::new(
            (1..=OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)
                .map(|value| Fp::from(u64::from(value)))
                .collect(),
            EqAffine::generator(),
        )
    }

    fn ep_accumulator() -> IpaAccumulator<EpAffine, NativeLoader> {
        IpaAccumulator::new(
            (1..=OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)
                .map(|value| Fq::from(u64::from(value)))
                .collect(),
            EpAffine::generator(),
        )
    }

    #[test]
    fn guard_bundle_configured_shape_is_exact_and_internally_bounded() {
        for shape in [
            {
                let mut constraints = ConstraintSystem::<Fp>::default();
                let _ = configure_guard_bundle_eq_v1(&mut constraints);
                pasta_ipa_augmented_proof_shape_v1(
                    &constraints,
                    OFFLINE_CASH_HALO2_K_V1,
                    PastaIpaInstanceQueryV1::Direct,
                )
                .expect("Eq GuardBundle shape")
            },
            {
                let mut constraints = ConstraintSystem::<Fq>::default();
                let _ = configure_guard_bundle_ep_v1(&mut constraints);
                pasta_ipa_augmented_proof_shape_v1(
                    &constraints,
                    OFFLINE_CASH_HALO2_K_V1,
                    PastaIpaInstanceQueryV1::Direct,
                )
                .expect("Ep GuardBundle shape")
            },
        ] {
            eprintln!(
                "GuardBundle shape: degree={} advice={}/queries={} instance={}/queries={} fixed_queries={} selectors={} lookups={} permutation={}/chunks={} point_sets={} commitments={} evaluations={} ordinary={}",
                shape.degree(),
                shape.advice_columns(),
                shape.advice_queries(),
                shape.instance_columns(),
                shape.instance_queries(),
                shape.fixed_queries(),
                shape.selectors(),
                shape.lookups(),
                shape.permutation_columns(),
                shape.permutation_chunks(),
                shape.point_sets(),
                shape.commitments(),
                shape.evaluations(),
                shape.ordinary_proof_bytes(),
            );
            assert_eq!(shape.degree(), 10);
            assert_eq!(shape.advice_columns(), 8);
            assert_eq!(shape.advice_queries(), 8);
            assert_eq!(shape.instance_columns(), 3);
            assert_eq!(shape.instance_queries(), 2);
            assert_eq!(shape.fixed_queries(), 3);
            assert_eq!(shape.selectors(), 0);
            assert_eq!(shape.lookups(), 2);
            assert_eq!(shape.permutation_columns(), 9);
            assert_eq!(shape.permutation_chunks(), 2);
            assert_eq!(shape.point_sets(), 4);
            assert_eq!(shape.commitments(), 60);
            assert_eq!(shape.evaluations(), 42);
            assert_eq!(shape.ordinary_proof_bytes(), 3_264);
        }
    }

    fn eq_lineage_binding_builder(tamper: Option<usize>) -> (BaseCircuitBuilder<Fp>, Vec<Fp>) {
        let native = eq_accumulator();
        let lineage = offline_cash_lineage_from_eq_v1(&native).expect("encode Eq lineage");
        let mut instances = offline_cash_eq_lineage_instance_column_v1(&lineage)
            .expect("Eq lineage cells")
            .to_vec();
        if let Some(index) = tamper {
            instances[index] += Fp::ONE;
        }
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(14)
            .use_lookup_bits(13);
        let public = builder.main(0).assign_witnesses(instances.clone());
        builder.assigned_instances = vec![public.clone()];
        let range = builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
        let chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let loader =
            snark_verifier::loader::halo2::Halo2Loader::new(chip, mem::take(builder.pool(0)));
        let folded =
            crate::zk::kagemusha_recursion_adapter::scalar_lineage_v1::load_native_accumulator(
                &loader, &native,
            );
        constrain_poseidon_folded_accumulator_instance_v1(
            &loader,
            OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
            OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
            &folded,
            &public,
        )
        .expect("constrain clean-V1 Eq lineage");
        *builder.pool(0) = loader.take_ctx();
        builder.calculate_params(Some(9));
        (builder, instances)
    }

    #[test]
    fn clean_v1_lineage_roundtrips_both_parities_and_projects_all_36_cells() {
        let eq = eq_accumulator();
        let eq_wire = offline_cash_lineage_from_eq_v1(&eq).expect("Eq lineage wire");
        let eq_cells = offline_cash_eq_lineage_instance_column_v1(&eq_wire).expect("Eq cells");
        assert_eq!(eq_cells.len(), 36);
        assert_eq!(eq_cells[0], Fp::ONE);
        assert_eq!(eq_cells[1], Fp::from(16));
        let parsed_eq = offline_cash_lineage_to_eq_v1(&eq_wire).expect("parse Eq lineage");
        assert_eq!(parsed_eq.xi, eq.xi);
        assert_eq!(parsed_eq.u, eq.u);

        let ep = ep_accumulator();
        let ep_wire = offline_cash_lineage_from_ep_v1(&ep).expect("Ep lineage wire");
        let ep_cells = offline_cash_ep_lineage_instance_column_v1(&ep_wire).expect("Ep cells");
        assert_eq!(ep_cells.len(), 36);
        assert_eq!(ep_cells[0], Fq::ONE);
        assert_eq!(ep_cells[1], Fq::from(16));
        let parsed_ep = offline_cash_lineage_to_ep_v1(&ep_wire).expect("parse Ep lineage");
        assert_eq!(parsed_ep.xi, ep.xi);
        assert_eq!(parsed_ep.u, ep.u);
        assert_ne!(eq_wire, ep_wire, "opposite parity lineages must not alias");
    }

    #[test]
    fn clean_v1_lineage_circuit_binds_metadata_challenges_and_compressed_point() {
        let (valid, instances) = eq_lineage_binding_builder(None);
        MockProver::run(valid.config_params.k as u32, &valid, vec![instances])
            .expect("valid clean-V1 lineage prover")
            .assert_satisfied();
        for index in 0..OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize {
            let (tampered, instances) = eq_lineage_binding_builder(Some(index));
            assert!(
                MockProver::run(tampered.config_params.k as u32, &tampered, vec![instances])
                    .expect("tampered clean-V1 lineage prover")
                    .verify()
                    .is_err(),
                "lineage instance cell {index} must be equality-bound"
            );
        }
    }
}
