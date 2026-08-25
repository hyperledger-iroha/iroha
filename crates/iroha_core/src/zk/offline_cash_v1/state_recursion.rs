//! Final reciprocal State wrappers for Offline Cash V1.
//!
//! Each parity verifies the same-parity `StateLeaf` and completed
//! `GuardBundle` proofs with the ordinary Poseidon transcript. The wrapper
//! folds both outer proof accumulators together with the GuardBundle's
//! circuit-bound carried lineage, exposes one clean-V1 36-cell lineage, and
//! reciprocally enforces every deferred equation emitted by the other Pasta
//! parity. The exact GuardBundle pair binding is reconstructed from the child
//! proof, hashed with the source-authoritative SHA-256 framing, and joined by
//! the shared final-State pair binding. No native-only child acceptance or
//! deferred equation remains.

use std::mem;

use ff::{PrimeField as _, WithSmallOrderMulGroup};
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
    OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1, OFFLINE_CASH_HALO2_K_V1,
    OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1, OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
    OfflineCashIpaLineageV1, OfflineCashRecursivePairBindingV1, OfflineCashRecursivePairTopologyV1,
    offline_cash_guard_bundle_pair_binding_digest_message_v1,
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};
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
            constrain_poseidon_recursive_scalar_audit_v1, constrain_sha256_digest_words_le_v1,
            load_native_accumulator,
        },
    },
    kagemusha_sha256_v4::{
        KagemushaConstrainedSha256V1, KagemushaSha256BitV4, KagemushaSha256ByteV4,
    },
};

use super::{
    OfflineCashHalo2ParityV1,
    helper_abi::{
        CONTEXT_WORD_START as HELPER_CONTEXT_WORD_START,
        CURRENT_HEAD_WORD_START as HELPER_CURRENT_HEAD_WORD_START, HELPER_ABI_WORDS,
        HELPER_INSTANCE_CELLS, HELPER_OPERATION_WORD, HELPER_PROTOCOL_WORD_START,
        HELPER_WORDS_PER_INSTANCE, OfflineCashHelperPublicInstancesV1,
        RELEASE_WORD_START as HELPER_RELEASE_WORD_START,
        TRANSITION_WORD_START as HELPER_TRANSITION_WORD_START, pack_words_as_field,
    },
    packed_base::{
        OfflineCashPackedBaseConfigV1, OfflineCashPackedBaseTraceV1, OfflineCashPackedSha256JobsV1,
    },
    protocol::{
        OFFLINE_CASH_GUARD_BUNDLE_PROOF_MAX_BYTES_V1,
        OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1,
        OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1,
        OFFLINE_CASH_STATE_LEAF_PROOF_MAX_BYTES_V1, OfflineCashHalo2CircuitRoleV1,
    },
    state_abi::{
        CONTEXT_WORD_START as STATE_CONTEXT_WORD_START, OfflineCashStateAbiErrorV1,
        OfflineCashStatePublicInstancesV1, PARENT_0_WORD_START, RECURSIVE_PAIR_BINDING_WORD_START,
        RELEASE_WORD_START as STATE_RELEASE_WORD_START, STATE_ABI_WORDS, STATE_INSTANCE_CELLS,
        STATE_LEAF_ABI_WORDS, STATE_LEAF_INSTANCE_CELLS, STATE_OPERATION_WORD, STATE_PARITY_WORD,
        STATE_PROTOCOL_WORD_START, STATE_WORDS_PER_INSTANCE,
        TRANSITION_WORD_START as STATE_TRANSITION_WORD_START, fixed_state_word_v1,
    },
};

const STATE_CHILDREN: usize = 2;
const STATE_STAGE_FOLD: u32 = 3;
const STATE_RECURSIVE_INSTANCE_COLUMNS: usize = 2;
const PAIR_EQ_DIGEST_WORD_START: usize = 32;
const PAIR_EP_DIGEST_WORD_START: usize = 40;
const PAIR_CHILD_BINDING_DIGEST_WORD_START: usize = 48;
const PAIR_RESERVED_WORD_START: usize = 56;
const PAIR_DIGEST_WORDS: usize = 8;

const _: () = assert!(OFFLINE_CASH_IPA_LINEAGE_VERSION_V1 == 1);
const _: () = assert!(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1 == 16);
const _: () = assert!(OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 == 36);
const _: () = assert!(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 == 20);
const _: () = assert!(RECURSIVE_PAIR_BINDING_WORD_START == STATE_LEAF_ABI_WORDS);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OfflineCashStateChildSlotV1 {
    StateLeaf = 1,
    GuardBundle = 2,
}

impl OfflineCashStateChildSlotV1 {
    const ALL: [Self; STATE_CHILDREN] = [Self::StateLeaf, Self::GuardBundle];

    const fn role(self) -> OfflineCashHalo2CircuitRoleV1 {
        match self {
            Self::StateLeaf => OfflineCashHalo2CircuitRoleV1::StateLeaf,
            Self::GuardBundle => OfflineCashHalo2CircuitRoleV1::GuardBundle,
        }
    }

    const fn instance_column_lengths(self) -> &'static [usize] {
        match self {
            Self::StateLeaf => &[STATE_LEAF_INSTANCE_CELLS],
            Self::GuardBundle => &[
                HELPER_INSTANCE_CELLS,
                OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 as usize,
                OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize,
            ],
        }
    }

    const fn proof_max(self) -> usize {
        match self {
            Self::StateLeaf => OFFLINE_CASH_STATE_LEAF_PROOF_MAX_BYTES_V1 as usize,
            Self::GuardBundle => OFFLINE_CASH_GUARD_BUNDLE_PROOF_MAX_BYTES_V1 as usize,
        }
    }
}

/// Owned, zeroizing ordinary proof for one authenticated final-State child.
#[derive(Clone)]
pub(super) struct OfflineCashStateChildProofV1<C>
where
    C: CurveAffineExt,
{
    slot: OfflineCashStateChildSlotV1,
    protocol: PlonkProtocol<C>,
    instances: Vec<Vec<C::ScalarExt>>,
    proof_bytes: Zeroizing<Vec<u8>>,
}

impl<C> core::fmt::Debug for OfflineCashStateChildProofV1<C>
where
    C: CurveAffineExt,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashStateChildProofV1")
            .field("slot", &self.slot)
            .field("instance_columns", &self.instances.len())
            .field("proof_bytes", &self.proof_bytes.len())
            .finish_non_exhaustive()
    }
}

impl<C> OfflineCashStateChildProofV1<C>
where
    C: CurveAffineExt,
{
    pub(super) fn new(
        slot: OfflineCashStateChildSlotV1,
        protocol: PlonkProtocol<C>,
        instances: Vec<Vec<C::ScalarExt>>,
        proof_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        let expected = slot.instance_column_lengths();
        if proof_bytes.is_empty()
            || proof_bytes.len() > slot.proof_max()
            || protocol.domain.k != OFFLINE_CASH_HALO2_K_V1 as usize
            || protocol.domain.n != 1_usize << OFFLINE_CASH_HALO2_K_V1
            || protocol.num_instance.as_slice() != expected
            || instances.len() != expected.len()
            || instances
                .iter()
                .zip(expected)
                .any(|(column, expected)| column.len() != *expected)
        {
            return Err("final-State child proof/protocol/instance shape mismatch".to_owned());
        }
        Ok(Self {
            slot,
            protocol,
            instances,
            proof_bytes: Zeroizing::new(proof_bytes),
        })
    }
}

/// Complete same-parity recursive witness for one final State wrapper.
#[derive(Clone)]
pub(super) struct OfflineCashStateParityRecursionV1<C>
where
    C: CurveAffineExt,
{
    succinct_vk: IpaSuccinctVerifyingKey<C>,
    children: [OfflineCashStateChildProofV1<C>; STATE_CHILDREN],
    guard_bundle_common: OfflineCashHelperPublicInstancesV1,
    guard_bundle_pair_binding: OfflineCashRecursivePairBindingV1,
    guard_bundle_carried: IpaAccumulator<C, NativeLoader>,
    fold_proof_bytes: Zeroizing<Vec<u8>>,
}

impl<C> OfflineCashStateParityRecursionV1<C>
where
    C: CurveAffineExt,
    C::ScalarExt: ff::PrimeField,
    <C::ScalarExt as ff::PrimeField>::Repr: AsRef<[u8]>,
    C::Repr: AsRef<[u8]>,
{
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        parity: OfflineCashHalo2ParityV1,
        succinct_vk: IpaSuccinctVerifyingKey<C>,
        children: [OfflineCashStateChildProofV1<C>; STATE_CHILDREN],
        guard_bundle_common: OfflineCashHelperPublicInstancesV1,
        guard_bundle_pair_binding: OfflineCashRecursivePairBindingV1,
        guard_bundle_carried: IpaAccumulator<C, NativeLoader>,
        fold_proof_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        if succinct_vk.domain.k != OFFLINE_CASH_HALO2_K_V1 as usize
            || succinct_vk.domain.n != 1_usize << OFFLINE_CASH_HALO2_K_V1
        {
            return Err("final-State succinct verifier is not on the common k16 domain".to_owned());
        }
        for (expected, child) in OfflineCashStateChildSlotV1::ALL.into_iter().zip(&children) {
            if child.slot != expected {
                return Err("final-State child role/order mismatch".to_owned());
            }
        }
        if guard_bundle_common.role() != OfflineCashHalo2CircuitRoleV1::GuardBundle
            || guard_bundle_common.parity() != parity
            || guard_bundle_pair_binding.topology().ok()
                != Some(OfflineCashRecursivePairTopologyV1::GuardBundle)
            || guard_bundle_carried.xi.len() != OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1 as usize
            || bool::from(guard_bundle_carried.u.is_identity())
        {
            return Err("final-State GuardBundle witness shape mismatch".to_owned());
        }
        let guard_child = &children[OfflineCashStateChildSlotV1::GuardBundle as usize - 1];
        let expected_common = guard_bundle_common.field_instances::<C::ScalarExt>();
        let pair_words = guard_bundle_pair_binding
            .canonical_words()
            .map_err(|_| "invalid final-State GuardBundle pair binding".to_owned())?;
        let expected_pair = pair_words
            .chunks(HELPER_WORDS_PER_INSTANCE)
            .map(pack_words_as_field::<C::ScalarExt>)
            .collect::<Vec<_>>();
        let expected_carried = native_accumulator_instance_cells_v1(&guard_bundle_carried)?;
        if !guard_child_instance_columns_match_v1(
            &guard_child.instances,
            &expected_common,
            &expected_pair,
            &expected_carried,
        ) {
            return Err("final-State typed GuardBundle child instances mismatch".to_owned());
        }
        let expected_fold =
            kagemusha_ipa_accumulation_proof_bytes_v4(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)?;
        if fold_proof_bytes.len() != expected_fold {
            return Err("final-State fold proof has the wrong exact length".to_owned());
        }
        Ok(Self {
            succinct_vk,
            children,
            guard_bundle_common,
            guard_bundle_pair_binding,
            guard_bundle_carried,
            fold_proof_bytes: Zeroizing::new(fold_proof_bytes),
        })
    }
}

fn guard_child_instance_columns_match_v1<F: PartialEq>(
    child_instances: &[Vec<F>],
    expected_common: &[F],
    expected_pair: &[F],
    expected_carried: &[F],
) -> bool {
    child_instances.len() == 3
        && child_instances[0].as_slice() == expected_common
        && child_instances[1].as_slice() == expected_pair
        && child_instances[2].as_slice() == expected_carried
}

fn native_accumulator_instance_cells_v1<C>(
    accumulator: &IpaAccumulator<C, NativeLoader>,
) -> Result<Vec<C::ScalarExt>, String>
where
    C: CurveAffineExt,
    C::ScalarExt: ff::PrimeField,
    <C::ScalarExt as ff::PrimeField>::Repr: AsRef<[u8]>,
    C::Repr: AsRef<[u8]>,
{
    if accumulator.xi.len() != OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1 as usize
        || bool::from(accumulator.u.is_identity())
    {
        return Err("final-State GuardBundle carried accumulator is malformed".to_owned());
    }
    let mut cells = Vec::with_capacity(OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize);
    cells.push(C::ScalarExt::from(u64::from(
        OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
    )));
    cells.push(C::ScalarExt::from(u64::from(
        OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
    )));
    for bytes in accumulator
        .xi
        .iter()
        .map(ff::PrimeField::to_repr)
        .map(|repr| repr.as_ref().to_vec())
        .chain(core::iter::once(accumulator.u.to_bytes().as_ref().to_vec()))
    {
        if bytes.len() != 32 {
            return Err("final-State GuardBundle carried encoding width mismatch".to_owned());
        }
        for chunk in bytes.chunks_exact(16) {
            cells.push(C::ScalarExt::from_u128(u128::from_le_bytes(
                chunk
                    .try_into()
                    .expect("fixed sixteen-byte carried-lineage limb"),
            )));
        }
    }
    if cells.len() != OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize {
        return Err("final-State GuardBundle carried projection mismatch".to_owned());
    }
    Ok(cells)
}

/// Exact two-column public value of one final State parity.
#[derive(Clone, Debug)]
pub(super) struct OfflineCashStateRecursivePublicV1 {
    state: OfflineCashStatePublicInstancesV1,
    carried_lineage: OfflineCashIpaLineageV1,
}

impl OfflineCashStateRecursivePublicV1 {
    pub(super) fn new(
        state: OfflineCashStatePublicInstancesV1,
        carried_lineage: OfflineCashIpaLineageV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        if state
            .recursive_pair_binding()?
            .topology()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidRecursivePairBinding)?
            != OfflineCashRecursivePairTopologyV1::State
            || carried_lineage.validate().is_err()
        {
            return Err(OfflineCashStateAbiErrorV1::InvalidLayout);
        }
        Ok(Self {
            state,
            carried_lineage,
        })
    }

    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV1 {
        self.state.parity()
    }

    pub(super) fn instance_columns<F>(&self) -> Result<Vec<Vec<F>>, OfflineCashStateAbiErrorV1>
    where
        F: ff::PrimeField,
    {
        let state = self.state.field_instances::<F>().to_vec();
        let lineage = self
            .carried_lineage
            .instance_limbs()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidLayout)?
            .into_iter()
            .map(F::from_u128)
            .collect::<Vec<_>>();
        if state.len() != STATE_INSTANCE_CELLS
            || lineage.len() != OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize
        {
            return Err(OfflineCashStateAbiErrorV1::InvalidLayout);
        }
        Ok(vec![state, lineage])
    }
}

fn state_base_params_v1() -> BaseCircuitParams {
    // Virtual collection geometry only. The authenticated physical proof
    // shape is `OfflineCashPackedBaseConfigV1` (8 advice/3 fixed/2 instance).
    BaseCircuitParams {
        k: OFFLINE_CASH_HALO2_K_V1 as usize,
        num_advice_per_phase: vec![12],
        num_fixed: 1,
        num_lookup_advice_per_phase: vec![4, 0, 0],
        lookup_bits: Some(OFFLINE_CASH_HALO2_K_V1 as usize - 1),
        num_instance_columns: STATE_RECURSIVE_INSTANCE_COLUMNS,
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

fn constrain_nonzero_digest_words_v1<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    words: &[AssignedValue<F>],
) where
    F: BigPrimeField,
{
    let mut all_zero = ctx.load_constant(F::ONE);
    for word in words {
        let zero = range.gate().is_zero(ctx, *word);
        all_zero = range.gate().mul(
            ctx,
            QuantumCell::Existing(all_zero),
            QuantumCell::Existing(zero),
        );
    }
    range.gate().assert_is_const(ctx, &all_zero, &F::ZERO);
}

fn pack_raw_words_to_cells_v1<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    raw_words: &[AssignedValue<F>],
    words_per_cell: usize,
) -> Vec<AssignedValue<F>>
where
    F: BigPrimeField,
{
    raw_words
        .chunks(words_per_cell)
        .map(|chunk| {
            pack_assigned_words(ctx, range, chunk.iter().copied().map(QuantumCell::Existing))
        })
        .collect()
}

fn assign_state_public_v1<F>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &OfflineCashStateRecursivePublicV1,
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
        .map_err(|error| format!("invalid final-State public columns: {error}"))?;
    if columns.iter().map(Vec::len).collect::<Vec<_>>()
        != vec![
            STATE_INSTANCE_CELLS,
            OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize,
        ]
    {
        return Err("final-State public column geometry mismatch".to_owned());
    }
    let assigned = columns
        .into_iter()
        .map(|values| builder.main(0).assign_witnesses(values))
        .collect::<Vec<_>>();
    builder.assigned_instances = assigned.clone();

    let raw_words = builder.main(0).assign_witnesses(
        public
            .state
            .words()
            .iter()
            .map(|word| F::from(u64::from(*word))),
    );
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for word in &raw_words {
        range.range_check(ctx, *word, 32);
    }
    let packed = pack_raw_words_to_cells_v1(ctx, &range, &raw_words, STATE_WORDS_PER_INSTANCE);
    if packed.len() != assigned[0].len() {
        return Err("final-State packed primary length mismatch".to_owned());
    }
    for (actual, expected) in packed.iter().zip(&assigned[0]) {
        ctx.constrain_equal(actual, expected);
    }
    for (index, word) in raw_words[..STATE_LEAF_ABI_WORDS].iter().enumerate() {
        if let Some(expected) = fixed_state_word_v1(public.parity(), index) {
            range
                .gate()
                .assert_is_const(ctx, word, &F::from(u64::from(expected)));
        }
    }

    let pair_words = raw_words[RECURSIVE_PAIR_BINDING_WORD_START..].to_vec();
    let dummy_guard = OfflineCashRecursivePairBindingV1::new_guard_bundle([3; 32], [4; 32])
        .map_err(|_| "failed to derive final-State pair template child".to_owned())?;
    let template = OfflineCashRecursivePairBindingV1::new_state([1; 32], [2; 32], &dummy_guard)
        .and_then(|binding| binding.canonical_words())
        .map_err(|_| "failed to derive fixed final-State pair template".to_owned())?;
    for (index, word) in pair_words.iter().enumerate() {
        if !(PAIR_EQ_DIGEST_WORD_START..PAIR_RESERVED_WORD_START).contains(&index) {
            range
                .gate()
                .assert_is_const(ctx, word, &F::from(u64::from(template[index])));
        }
    }
    for digest in [
        &pair_words[PAIR_EQ_DIGEST_WORD_START..PAIR_EQ_DIGEST_WORD_START + PAIR_DIGEST_WORDS],
        &pair_words[PAIR_EP_DIGEST_WORD_START..PAIR_EP_DIGEST_WORD_START + PAIR_DIGEST_WORDS],
        &pair_words[PAIR_CHILD_BINDING_DIGEST_WORD_START
            ..PAIR_CHILD_BINDING_DIGEST_WORD_START + PAIR_DIGEST_WORDS],
    ] {
        constrain_nonzero_digest_words_v1(ctx, &range, digest);
    }
    let mut all_audit_words_equal = ctx.load_constant(F::ONE);
    for (eq, ep) in pair_words
        [PAIR_EQ_DIGEST_WORD_START..PAIR_EQ_DIGEST_WORD_START + PAIR_DIGEST_WORDS]
        .iter()
        .zip(&pair_words[PAIR_EP_DIGEST_WORD_START..PAIR_EP_DIGEST_WORD_START + PAIR_DIGEST_WORDS])
    {
        let equal = range.gate().is_equal(ctx, *eq, *ep);
        all_audit_words_equal = range.gate().mul(
            ctx,
            QuantumCell::Existing(all_audit_words_equal),
            QuantumCell::Existing(equal),
        );
    }
    range
        .gate()
        .assert_is_const(ctx, &all_audit_words_equal, &F::ZERO);
    Ok((raw_words, pair_words, assigned[1].clone()))
}

fn assign_helper_words_and_bind_child_v1<C>(
    loader: &DeferredLoader<'_, C>,
    recursion: &OfflineCashStateParityRecursionV1<C>,
    child: &crate::zk::kagemusha_recursion_adapter::scalar_lineage_v1::ConstrainedPoseidonChildProofV1<'_, C>,
) -> Result<Vec<AssignedValue<C::ScalarExt>>, snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if child.instances.len() != 3
        || child.instances[0].len() != HELPER_INSTANCE_CELLS
        || child.instances[1].len()
            != OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 as usize
        || child.instances[2].len() != OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize
    {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let helper_words = ctx.main().assign_witnesses(
        recursion
            .guard_bundle_common
            .words()
            .iter()
            .map(|word| C::ScalarExt::from(u64::from(*word))),
    );
    for word in &helper_words {
        range.range_check(ctx.main(), *word, 32);
    }
    let packed =
        pack_raw_words_to_cells_v1(ctx.main(), range, &helper_words, HELPER_WORDS_PER_INSTANCE);
    for (actual, expected) in packed.iter().zip(&child.instances[0]) {
        ctx.main().constrain_equal(actual, expected);
    }
    Ok(helper_words)
}

fn assign_guard_pair_words_and_bind_child_v1<C>(
    loader: &DeferredLoader<'_, C>,
    recursion: &OfflineCashStateParityRecursionV1<C>,
    child: &crate::zk::kagemusha_recursion_adapter::scalar_lineage_v1::ConstrainedPoseidonChildProofV1<'_, C>,
) -> Result<Vec<AssignedValue<C::ScalarExt>>, snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    let words = recursion
        .guard_bundle_pair_binding
        .canonical_words()
        .map_err(|_| snark_verifier::Error::InvalidInstances)?;
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let assigned = ctx.main().assign_witnesses(
        words
            .into_iter()
            .map(|word| C::ScalarExt::from(u64::from(word))),
    );
    for word in &assigned {
        range.range_check(ctx.main(), *word, 32);
    }
    let packed =
        pack_raw_words_to_cells_v1(ctx.main(), range, &assigned, HELPER_WORDS_PER_INSTANCE);
    if packed.len() != child.instances[1].len() {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    for (actual, expected) in packed.iter().zip(&child.instances[1]) {
        ctx.main().constrain_equal(actual, expected);
    }
    Ok(assigned)
}

fn constrain_state_child_semantics_v1<C>(
    loader: &DeferredLoader<'_, C>,
    state_words: &[AssignedValue<C::ScalarExt>],
    helper_words: &[AssignedValue<C::ScalarExt>],
) -> Result<(), snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if state_words.len() != STATE_ABI_WORDS || helper_words.len() != HELPER_ABI_WORDS {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let mut ctx = loader.ctx_mut();
    for (state, helper) in [
        (STATE_OPERATION_WORD, HELPER_OPERATION_WORD),
        (STATE_RELEASE_WORD_START, HELPER_RELEASE_WORD_START),
        (STATE_CONTEXT_WORD_START, HELPER_CONTEXT_WORD_START),
        (PARENT_0_WORD_START, HELPER_CURRENT_HEAD_WORD_START),
        (STATE_TRANSITION_WORD_START, HELPER_TRANSITION_WORD_START),
    ] {
        let width = if state == STATE_OPERATION_WORD { 1 } else { 8 };
        for offset in 0..width {
            ctx.main()
                .constrain_equal(&state_words[state + offset], &helper_words[helper + offset]);
        }
    }
    Ok(())
}

fn constrain_state_leaf_child_v1<C>(
    loader: &DeferredLoader<'_, C>,
    state_words: &[AssignedValue<C::ScalarExt>],
    child: &crate::zk::kagemusha_recursion_adapter::scalar_lineage_v1::ConstrainedPoseidonChildProofV1<'_, C>,
) -> Result<(), snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
{
    if child.instances.len() != 1 || child.instances[0].len() != STATE_LEAF_INSTANCE_CELLS {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let packed = pack_raw_words_to_cells_v1(
        ctx.main(),
        range,
        &state_words[..STATE_LEAF_ABI_WORDS],
        STATE_WORDS_PER_INSTANCE,
    );
    for (actual, expected) in packed.iter().zip(&child.instances[0]) {
        ctx.main().constrain_equal(actual, expected);
    }
    Ok(())
}

fn assigned_u32_bytes_le_v1<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    word: AssignedValue<F>,
) -> [KagemushaSha256ByteV4<F>; 4]
where
    F: BigPrimeField,
{
    let bits = KagemushaSha256BitV4::decompose(ctx, range.gate(), word, 32);
    std::array::from_fn(|byte| {
        KagemushaSha256ByteV4::from_bits_le(ctx, range.gate(), &bits[byte * 8..byte * 8 + 8])
    })
}

fn constrain_guard_bundle_pair_digest_join_v1<C, S>(
    loader: &DeferredLoader<'_, C>,
    sha_jobs: &mut S,
    state_pair_words: &[AssignedValue<C::ScalarExt>],
    guard_pair_words: &[AssignedValue<C::ScalarExt>],
    guard_pair_binding: &OfflineCashRecursivePairBindingV1,
) -> Result<(), snark_verifier::Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField,
    S: KagemushaConstrainedSha256V1<C::ScalarExt>,
{
    if state_pair_words.len() < PAIR_RESERVED_WORD_START
        || guard_pair_words.len() < PAIR_CHILD_BINDING_DIGEST_WORD_START
    {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let canonical = guard_pair_binding
        .guard_bundle_canonical_bytes68()
        .map_err(|_| snark_verifier::Error::InvalidInstances)?;
    let message = offline_cash_guard_bundle_pair_binding_digest_message_v1(guard_pair_binding)
        .map_err(|_| snark_verifier::Error::InvalidInstances)?;
    if message.len() < canonical.len()
        || message[message.len() - canonical.len()..] != canonical
        || canonical.len() != OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1
    {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let prefix_len = message.len() - canonical.len();
    let chip = loader.ecc_chip();
    let range = chip.range();
    let mut ctx = loader.ctx_mut();
    let mut circuit_message = message[..prefix_len]
        .iter()
        .copied()
        .map(KagemushaSha256ByteV4::constant)
        .collect::<Vec<_>>();
    for word in core::iter::once(&guard_pair_words[1])
        .chain(&guard_pair_words[PAIR_EQ_DIGEST_WORD_START..PAIR_CHILD_BINDING_DIGEST_WORD_START])
    {
        circuit_message.extend(assigned_u32_bytes_le_v1(ctx.main(), range, *word));
    }
    if circuit_message.len() != message.len() {
        return Err(snark_verifier::Error::InvalidInstances);
    }
    let digest = sha_jobs
        .digest_constrained_v1(ctx.main(), &circuit_message)
        .map_err(|error| {
            snark_verifier::Error::Transcript(std::io::ErrorKind::InvalidData, error)
        })?;
    constrain_sha256_digest_words_le_v1(
        ctx.main(),
        range,
        &digest,
        &state_pair_words[PAIR_CHILD_BINDING_DIGEST_WORD_START
            ..PAIR_CHILD_BINDING_DIGEST_WORD_START + PAIR_DIGEST_WORDS],
    )
}

fn constrain_state_scalar_v1<C, S>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    sha_jobs: &mut S,
    public: &OfflineCashStateRecursivePublicV1,
    recursion: &OfflineCashStateParityRecursionV1<C>,
    bind_own_audit: bool,
) -> Result<PoseidonRecursiveScalarAuditV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
    S: KagemushaConstrainedSha256V1<C::ScalarExt>,
{
    let parity = public.parity();
    if recursion.guard_bundle_common.parity() != parity {
        return Err("final-State and GuardBundle parity mismatch".to_owned());
    }
    let (state_words, state_pair_words, final_lineage_cells) =
        assign_state_public_v1(builder, public)?;
    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let chip = DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer);
    let loader = snark_verifier::loader::halo2::Halo2Loader::new(chip, mem::take(builder.pool(0)));

    let mut constrained = Vec::with_capacity(STATE_CHILDREN);
    let mut stages = Vec::with_capacity(STATE_CHILDREN + 1);
    for (index, child) in recursion.children.iter().enumerate() {
        let output = constrain_poseidon_child_proof_v1(
            &loader,
            &recursion.succinct_vk,
            &child.protocol,
            &child.instances,
            &child.proof_bytes,
            child.slot.proof_max(),
        )
        .map_err(|error| {
            format!(
                "failed to constrain final-State {:?} child: {error:?}",
                child.slot
            )
        })?;
        stages.push(
            PoseidonDeferredEquationStageV1::new(
                output.deferred_equations.clone(),
                u32::try_from(index + 1)
                    .map_err(|_| "final-State audit tag overflow".to_owned())?,
            )
            .map_err(|error| format!("invalid final-State child audit stage: {error:?}"))?,
        );
        constrained.push(output);
    }
    constrain_state_leaf_child_v1(&loader, &state_words, &constrained[0])
        .map_err(|error| format!("failed final-State leaf equality: {error:?}"))?;
    let helper_words =
        assign_helper_words_and_bind_child_v1(&loader, recursion, &constrained[1])
            .map_err(|error| format!("failed final-State GuardBundle equality: {error:?}"))?;
    constrain_state_child_semantics_v1(&loader, &state_words, &helper_words)
        .map_err(|error| format!("failed final-State/helper semantic join: {error:?}"))?;
    let guard_pair_words =
        assign_guard_pair_words_and_bind_child_v1(&loader, recursion, &constrained[1])
            .map_err(|error| format!("failed final-State GuardBundle pair equality: {error:?}"))?;
    constrain_guard_bundle_pair_digest_join_v1(
        &loader,
        sha_jobs,
        &state_pair_words,
        &guard_pair_words,
        &recursion.guard_bundle_pair_binding,
    )
    .map_err(|error| format!("failed final-State GuardBundle digest join: {error:?}"))?;

    // The final fold includes the completed GuardBundle proof's own carried
    // lineage. Merely folding the two outer proof accumulators would leave all
    // helper/P256 child accumulators undecided at terminal verification.
    let guard_carried = load_native_accumulator(&loader, &recursion.guard_bundle_carried);
    constrain_poseidon_folded_accumulator_instance_v1(
        &loader,
        OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
        OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
        &guard_carried,
        &constrained[1].instances[2],
    )
    .map_err(|error| format!("failed to bind GuardBundle carried lineage: {error:?}"))?;
    let accumulators = vec![
        constrained[0].accumulator.clone(),
        constrained[1].accumulator.clone(),
        guard_carried,
    ];
    let expected_fold =
        kagemusha_ipa_accumulation_proof_bytes_v4(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)?;
    let (folded, fold_range) = constrain_poseidon_child_fold_v1(
        &loader,
        &recursion.succinct_vk,
        &accumulators,
        &recursion.fold_proof_bytes,
        expected_fold,
    )
    .map_err(|error| format!("failed to constrain final-State child fold: {error:?}"))?;
    stages.push(
        PoseidonDeferredEquationStageV1::new(fold_range, STATE_STAGE_FOLD)
            .map_err(|error| format!("invalid final-State fold audit stage: {error:?}"))?,
    );
    constrain_poseidon_folded_accumulator_instance_v1(
        &loader,
        OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
        OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
        &folded,
        &final_lineage_cells,
    )
    .map_err(|error| format!("failed to bind final-State carried lineage: {error:?}"))?;

    let own_start = match parity {
        OfflineCashHalo2ParityV1::Eq => PAIR_EQ_DIGEST_WORD_START,
        OfflineCashHalo2ParityV1::Ep => PAIR_EP_DIGEST_WORD_START,
    };
    let audit = if bind_own_audit {
        constrain_poseidon_recursive_scalar_audit_v1(
            &loader,
            sha_jobs,
            &stages,
            PoseidonRecursiveAuditBindingCellsV1 {
                audit_digest_words: &state_pair_words[own_start..own_start + PAIR_DIGEST_WORDS],
            },
        )
        .map_err(|error| format!("failed to bind final-State scalar audit: {error:?}"))?
    } else {
        capture_poseidon_recursive_scalar_audit_v1(&loader, &stages)
            .map_err(|error| format!("failed to capture final-State scalar audit: {error:?}"))?
    };
    *builder.pool(0) = loader.take_ctx();
    Ok(audit)
}

fn constrain_state_reciprocal_v1<C, S>(
    builder: &mut BaseCircuitBuilder<C::Base>,
    sha_jobs: &mut S,
    public: &OfflineCashStateRecursivePublicV1,
    reciprocal: &PoseidonRecursiveScalarAuditV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + ScalarField + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3>,
    S: KagemushaConstrainedSha256V1<C::Base>,
{
    if builder.assigned_instances.len() != STATE_RECURSIVE_INSTANCE_COLUMNS
        || builder.assigned_instances[0].len() != STATE_INSTANCE_CELLS
    {
        return Err("final-State reciprocal public geometry mismatch".to_owned());
    }
    let raw = builder.main(0).assign_witnesses(
        public
            .state
            .words()
            .iter()
            .map(|word| C::Base::from(u64::from(*word))),
    );
    let public_primary = builder.assigned_instances[0].clone();
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for word in &raw {
        range.range_check(ctx, *word, 32);
    }
    let packed = pack_raw_words_to_cells_v1(ctx, &range, &raw, STATE_WORDS_PER_INSTANCE);
    for (actual, expected) in packed.iter().zip(&public_primary) {
        ctx.constrain_equal(actual, expected);
    }
    let pair = &raw[RECURSIVE_PAIR_BINDING_WORD_START..];
    let reciprocal_start = match public.parity() {
        OfflineCashHalo2ParityV1::Eq => PAIR_EP_DIGEST_WORD_START,
        OfflineCashHalo2ParityV1::Ep => PAIR_EQ_DIGEST_WORD_START,
    };
    constrain_poseidon_reciprocal_audit_serial_v1(
        builder,
        sha_jobs,
        reciprocal,
        PoseidonRecursiveAuditBindingCellsV1 {
            audit_digest_words: &pair[reciprocal_start..reciprocal_start + PAIR_DIGEST_WORDS],
        },
    )
}

#[derive(Clone, Debug)]
pub(super) struct OfflineCashStateRecursiveConfigV1 {
    packed: OfflineCashPackedBaseConfigV1,
}

fn configure_state_eq_v1(meta: &mut ConstraintSystem<Fp>) -> OfflineCashStateRecursiveConfigV1 {
    OfflineCashStateRecursiveConfigV1 {
        packed: OfflineCashPackedBaseConfigV1::configure(meta),
    }
}

fn configure_state_ep_v1(meta: &mut ConstraintSystem<Fq>) -> OfflineCashStateRecursiveConfigV1 {
    OfflineCashStateRecursiveConfigV1 {
        packed: OfflineCashPackedBaseConfigV1::configure(meta),
    }
}

/// Eq/Fp final State wrapper. Its point half enforces the Ep audit.
#[derive(Clone)]
pub(super) struct OfflineCashEqStateCircuitV1 {
    trace: OfflineCashPackedBaseTraceV1<Fp>,
    break_points: Vec<Vec<usize>>,
    audit_inventory: [usize; 4],
}

impl core::fmt::Debug for OfflineCashEqStateCircuitV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEqStateCircuitV1")
            .field("instance_columns", &STATE_RECURSIVE_INSTANCE_COLUMNS)
            .field("state_cells", &STATE_INSTANCE_CELLS)
            .field("carried_lineage_cells", &36)
            .field("packed_advice", &8)
            .field("packed_fixed", &3)
            .field("typed_lookup_lanes", &2)
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

impl Circuit<Fp> for OfflineCashEqStateCircuitV1 {
    type Config = OfflineCashStateRecursiveConfigV1;
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
        configure_state_eq_v1(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        self.trace.synthesize(&config.packed, &mut layouter)
    }
}

/// Ep/Fq final State wrapper. Its point half enforces the Eq audit.
#[derive(Clone)]
pub(super) struct OfflineCashEpStateCircuitV1 {
    trace: OfflineCashPackedBaseTraceV1<Fq>,
    break_points: Vec<Vec<usize>>,
    audit_inventory: [usize; 4],
}

impl core::fmt::Debug for OfflineCashEpStateCircuitV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEpStateCircuitV1")
            .field("instance_columns", &STATE_RECURSIVE_INSTANCE_COLUMNS)
            .field("state_cells", &STATE_INSTANCE_CELLS)
            .field("carried_lineage_cells", &36)
            .field("packed_advice", &8)
            .field("packed_fixed", &3)
            .field("typed_lookup_lanes", &2)
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

impl Circuit<Fq> for OfflineCashEpStateCircuitV1 {
    type Config = OfflineCashStateRecursiveConfigV1;
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
        configure_state_ep_v1(meta)
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
enum StateBuilderStageV1<'a> {
    Keygen,
    Prover(&'a [Vec<usize>]),
}

fn state_builder_v1<F>(stage: StateBuilderStageV1<'_>) -> BaseCircuitBuilder<F>
where
    F: ScalarField,
{
    let _ = stage;
    // The packed compiler owns the physical schedule. Both keygen and proving
    // therefore collect the exact same constraint-bearing virtual graph;
    // halo2-base witness-only breakpoints are intentionally not authoritative.
    BaseCircuitBuilder::new(false).use_params(state_base_params_v1())
}

fn compile_state_trace_v1<F>(
    builder: &BaseCircuitBuilder<F>,
    sha_jobs: &OfflineCashPackedSha256JobsV1<F>,
) -> Result<OfflineCashPackedBaseTraceV1<F>, String>
where
    F: BigPrimeField + ScalarField,
{
    OfflineCashPackedBaseTraceV1::from_builder(builder, sha_jobs)
}

fn collect_state_scalar_audit_v1<C>(
    public: &OfflineCashStateRecursivePublicV1,
    recursion: &OfflineCashStateParityRecursionV1<C>,
) -> Result<PoseidonRecursiveScalarAuditV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
{
    let mut builder = state_builder_v1::<C::ScalarExt>(StateBuilderStageV1::Keygen);
    let mut sha_jobs = OfflineCashPackedSha256JobsV1::default();
    constrain_state_scalar_v1(&mut builder, &mut sha_jobs, public, recursion, false)
}

/// Derive the exact reciprocal audit digests needed to construct the governed
/// final-State pair binding during dev-only artifact generation. The prepass
/// is not proof authority; final packed circuits constrain the digests and the
/// authenticated release owns the resulting VK identities.
#[cfg(feature = "dev-tools")]
pub(super) fn offline_cash_state_keygen_audit_digests_v1(
    eq_public: &OfflineCashStateRecursivePublicV1,
    ep_public: &OfflineCashStateRecursivePublicV1,
    eq_recursion: &OfflineCashStateParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashStateParityRecursionV1<EpAffine>,
) -> Result<([u8; 32], [u8; 32]), String> {
    if eq_public.parity() != OfflineCashHalo2ParityV1::Eq
        || ep_public.parity() != OfflineCashHalo2ParityV1::Ep
        || !state_common_semantics_match_v1(eq_public, ep_public)
        || !helper_common_semantics_match_v1(eq_recursion, ep_recursion)
        || eq_recursion.guard_bundle_pair_binding != ep_recursion.guard_bundle_pair_binding
    {
        return Err("final-State keygen audit prepass parity/semantics mismatch".to_owned());
    }
    let eq = collect_state_scalar_audit_v1(eq_public, eq_recursion)?
        .audit_sha256()
        .map_err(|error| format!("invalid Eq final-State keygen audit: {error:?}"))?;
    let ep = collect_state_scalar_audit_v1(ep_public, ep_recursion)?
        .audit_sha256()
        .map_err(|error| format!("invalid Ep final-State keygen audit: {error:?}"))?;
    if eq == ep {
        return Err("final-State keygen audit parities unexpectedly alias".to_owned());
    }
    Ok((eq, ep))
}

fn state_common_semantics_match_v1(
    eq: &OfflineCashStateRecursivePublicV1,
    ep: &OfflineCashStateRecursivePublicV1,
) -> bool {
    eq.state
        .words()
        .iter()
        .zip(ep.state.words())
        .enumerate()
        .all(|(index, (eq, ep))| {
            index == STATE_PARITY_WORD
                || (STATE_PROTOCOL_WORD_START..STATE_PROTOCOL_WORD_START + 8).contains(&index)
                || eq == ep
        })
}

fn helper_common_semantics_match_v1(
    eq: &OfflineCashStateParityRecursionV1<EqAffine>,
    ep: &OfflineCashStateParityRecursionV1<EpAffine>,
) -> bool {
    eq.guard_bundle_common
        .words()
        .iter()
        .zip(ep.guard_bundle_common.words())
        .enumerate()
        .all(|(index, (eq, ep))| {
            index == super::helper_abi::HELPER_PARITY_WORD
                || (HELPER_PROTOCOL_WORD_START..HELPER_RELEASE_WORD_START).contains(&index)
                || eq == ep
        })
}

fn build_state_eq_circuit_v1(
    public: &OfflineCashStateRecursivePublicV1,
    recursion: &OfflineCashStateParityRecursionV1<EqAffine>,
    reciprocal: &PoseidonRecursiveScalarAuditV1<EpAffine>,
    stage: StateBuilderStageV1<'_>,
) -> Result<OfflineCashEqStateCircuitV1, String> {
    if public.parity() != OfflineCashHalo2ParityV1::Eq {
        return Err("Eq final-State circuit received the wrong public parity".to_owned());
    }
    let mut builder = state_builder_v1::<Fp>(stage);
    let mut sha_jobs = OfflineCashPackedSha256JobsV1::default();
    let own_audit =
        constrain_state_scalar_v1(&mut builder, &mut sha_jobs, public, recursion, true)?;
    constrain_state_reciprocal_v1::<EpAffine, _>(&mut builder, &mut sha_jobs, public, reciprocal)?;
    let trace = compile_state_trace_v1(&builder, &sha_jobs)?;
    let break_points = match stage {
        StateBuilderStageV1::Keygen => Vec::new(),
        StateBuilderStageV1::Prover(value) => value.to_vec(),
    };
    Ok(OfflineCashEqStateCircuitV1 {
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

fn build_state_ep_circuit_v1(
    public: &OfflineCashStateRecursivePublicV1,
    recursion: &OfflineCashStateParityRecursionV1<EpAffine>,
    reciprocal: &PoseidonRecursiveScalarAuditV1<EqAffine>,
    stage: StateBuilderStageV1<'_>,
) -> Result<OfflineCashEpStateCircuitV1, String> {
    if public.parity() != OfflineCashHalo2ParityV1::Ep {
        return Err("Ep final-State circuit received the wrong public parity".to_owned());
    }
    let mut builder = state_builder_v1::<Fq>(stage);
    let mut sha_jobs = OfflineCashPackedSha256JobsV1::default();
    let own_audit =
        constrain_state_scalar_v1(&mut builder, &mut sha_jobs, public, recursion, true)?;
    constrain_state_reciprocal_v1::<EqAffine, _>(&mut builder, &mut sha_jobs, public, reciprocal)?;
    let trace = compile_state_trace_v1(&builder, &sha_jobs)?;
    let break_points = match stage {
        StateBuilderStageV1::Keygen => Vec::new(),
        StateBuilderStageV1::Prover(value) => value.to_vec(),
    };
    Ok(OfflineCashEpStateCircuitV1 {
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

pub(super) fn build_state_keygen_pair_v1(
    eq_public: &OfflineCashStateRecursivePublicV1,
    ep_public: &OfflineCashStateRecursivePublicV1,
    eq_recursion: &OfflineCashStateParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashStateParityRecursionV1<EpAffine>,
) -> Result<(OfflineCashEqStateCircuitV1, OfflineCashEpStateCircuitV1), String> {
    build_state_pair_v1(
        eq_public,
        ep_public,
        eq_recursion,
        ep_recursion,
        StateBuilderStageV1::Keygen,
        StateBuilderStageV1::Keygen,
    )
}

pub(super) fn build_state_prover_pair_v1(
    eq_public: &OfflineCashStateRecursivePublicV1,
    ep_public: &OfflineCashStateRecursivePublicV1,
    eq_recursion: &OfflineCashStateParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashStateParityRecursionV1<EpAffine>,
    eq_break_points: &[Vec<usize>],
    ep_break_points: &[Vec<usize>],
) -> Result<(OfflineCashEqStateCircuitV1, OfflineCashEpStateCircuitV1), String> {
    build_state_pair_v1(
        eq_public,
        ep_public,
        eq_recursion,
        ep_recursion,
        StateBuilderStageV1::Prover(eq_break_points),
        StateBuilderStageV1::Prover(ep_break_points),
    )
}

fn build_state_pair_v1(
    eq_public: &OfflineCashStateRecursivePublicV1,
    ep_public: &OfflineCashStateRecursivePublicV1,
    eq_recursion: &OfflineCashStateParityRecursionV1<EqAffine>,
    ep_recursion: &OfflineCashStateParityRecursionV1<EpAffine>,
    eq_stage: StateBuilderStageV1<'_>,
    ep_stage: StateBuilderStageV1<'_>,
) -> Result<(OfflineCashEqStateCircuitV1, OfflineCashEpStateCircuitV1), String> {
    let eq_binding = eq_public
        .state
        .recursive_pair_binding()
        .map_err(|error| format!("invalid Eq final-State binding: {error}"))?;
    let ep_binding = ep_public
        .state
        .recursive_pair_binding()
        .map_err(|error| format!("invalid Ep final-State binding: {error}"))?;
    if eq_public.parity() != OfflineCashHalo2ParityV1::Eq
        || ep_public.parity() != OfflineCashHalo2ParityV1::Ep
        || eq_binding != ep_binding
        || !state_common_semantics_match_v1(eq_public, ep_public)
        || eq_recursion.guard_bundle_pair_binding != ep_recursion.guard_bundle_pair_binding
        || !helper_common_semantics_match_v1(eq_recursion, ep_recursion)
        || eq_binding
            .validate_state_child_binding(&eq_recursion.guard_bundle_pair_binding)
            .is_err()
    {
        return Err("final-State reciprocal public/GuardBundle pair mismatch".to_owned());
    }
    let eq_audit = collect_state_scalar_audit_v1(eq_public, eq_recursion)?;
    let ep_audit = collect_state_scalar_audit_v1(ep_public, ep_recursion)?;
    let eq_digest = eq_audit
        .audit_sha256()
        .map_err(|error| format!("invalid Eq final-State scalar audit: {error:?}"))?;
    let ep_digest = ep_audit
        .audit_sha256()
        .map_err(|error| format!("invalid Ep final-State scalar audit: {error:?}"))?;
    if eq_binding.eq_audit_digest != eq_digest
        || eq_binding.ep_audit_digest != ep_digest
        || eq_digest == ep_digest
    {
        return Err("final-State public audit digests do not match scalar prepasses".to_owned());
    }
    let eq = build_state_eq_circuit_v1(eq_public, eq_recursion, &ep_audit, eq_stage)?;
    let ep = build_state_ep_circuit_v1(ep_public, ep_recursion, &eq_audit, ep_stage)?;
    Ok((eq, ep))
}

impl OfflineCashEqStateCircuitV1 {
    pub(super) fn break_points(&self) -> Vec<Vec<usize>> {
        self.break_points.clone()
    }
}

impl OfflineCashEpStateCircuitV1 {
    pub(super) fn break_points(&self) -> Vec<Vec<usize>> {
        self.break_points.clone()
    }
}

// Compile-time same-parity contract: Eq children are verified in Fp and Ep
// children in Fq. A cross-parity transcript verifier cannot type-check here.
const _: fn(OfflineCashStateParityRecursionV1<EqAffine>) = core::mem::drop;
const _: fn(OfflineCashStateParityRecursionV1<EpAffine>) = core::mem::drop;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::pasta_ipa_recursion::{
        PastaIpaInstanceQueryV1, PastaIpaProofShapeV1, pasta_ipa_augmented_proof_shape_v1,
    };
    use ff::Field as _;

    fn configured_final_state_shape_v1<F>(
        configure: impl FnOnce(&mut ConstraintSystem<F>) -> OfflineCashStateRecursiveConfigV1,
    ) -> PastaIpaProofShapeV1
    where
        F: ff::Field + ScalarField,
    {
        let mut constraints = ConstraintSystem::<F>::default();
        let _ = configure(&mut constraints);
        pasta_ipa_augmented_proof_shape_v1(
            &constraints,
            OFFLINE_CASH_HALO2_K_V1,
            PastaIpaInstanceQueryV1::Direct,
        )
        .expect("configured final-State proof shape")
    }

    #[test]
    fn final_state_configured_shape_is_within_target_and_hard_cap() {
        let eq = configured_final_state_shape_v1(configure_state_eq_v1);
        let ep = configured_final_state_shape_v1(configure_state_ep_v1);
        for shape in [&eq, &ep] {
            eprintln!(
                "final-State shape: degree={} advice={}/queries={} instance={}/queries={} fixed_queries={} selectors={} lookups={} permutation={}/chunks={} point_sets={} commitments={} evaluations={} ordinary={} augmented={}",
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
                shape.augmented_proof_bytes(),
            );
            assert!(
                shape.ordinary_proof_bytes()
                    <= (iroha_data_model::offline::OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1 / 2)
                        as u32,
                "final-State ordinary proof exceeds the 3,072-byte qualification target"
            );
            assert!(
                shape.ordinary_proof_bytes()
                    <= iroha_data_model::offline::OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 as u32,
                "final-State ordinary proof exceeds the 3,200-byte hard cap"
            );
        }
        assert_eq!(eq, ep);
    }

    #[test]
    fn guard_bundle_child_carried_column_substitution_is_rejected_exactly() {
        let common = (0..HELPER_INSTANCE_CELLS)
            .map(|value| Fp::from(value as u64 + 1))
            .collect::<Vec<_>>();
        let pair = (0..OFFLINE_CASH_RECURSIVE_PAIR_BINDING_INSTANCE_CELLS_V1 as usize)
            .map(|value| Fp::from(value as u64 + 101))
            .collect::<Vec<_>>();
        let carried = (0..OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_U32_V1 as usize)
            .map(|value| Fp::from(value as u64 + 201))
            .collect::<Vec<_>>();
        let exact = vec![common.clone(), pair.clone(), carried.clone()];
        assert!(guard_child_instance_columns_match_v1(
            &exact, &common, &pair, &carried
        ));

        let mut substituted = exact;
        substituted[2][17] += Fp::ONE;
        assert!(!guard_child_instance_columns_match_v1(
            &substituted,
            &common,
            &pair,
            &carried
        ));
        assert!(substituted[0] == common && substituted[1] == pair);
    }
}
