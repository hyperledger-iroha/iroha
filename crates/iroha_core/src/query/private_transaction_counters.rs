//! Bounded private transaction counters computed from original certified native inputs.
//!
//! The installed genesis authority owns the policy and sealed manifest. Readers supply
//! neither a candidate set nor labels. Internal original rows never leave this owner;
//! the separate installed member service signs only this freshly computed claim.

use iroha_allocation::{AllocationBudget, AllocationReservation};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    AccountId, ValidationFail,
    account::AccountValue,
    block::consensus::SumeragiRootScope,
    events::data::prelude::{AssetBatchTransferLegStatus, AssetBatchTransferRejectionCode},
    isi::TransferAssetBatch,
    isi::error::{AssetTransferAdmissionError, InstructionExecutionError, MathError},
    prelude::TransactionRejectionReason,
    private_transaction_counters::*,
    query::CommittedTransaction,
    smart_contract::{ContractAddress, ContractArtifactId},
    sumeragi_finality::{WorldStateElementKindV1, WorldStateSnapshotV1, world_state_value_hash_v1},
    transaction::{
        Executable, ExecutableBatchItem, TransactionEntrypoint, executable::ContractInvocation,
    },
};
use iroha_model_base::metadata::Metadata;
use mv::storage::StorageReadOnly;
use sha2::{Digest as _, Sha256};

use crate::{
    query::archive_finality::CertifiedArchiveView,
    smartcontracts::isi::tx::{
        TransactionHistoryWorkLimits, committed_transactions_indexed_snapshot_with_budget,
    },
    state::{State, StateReadOnly, WorldReadOnly, is_stable_state_view_generation},
};

#[path = "private_transaction_counters/compiled_plan.rs"]
mod compiled_plan;

/// Bound the original complete durable hash journal before it is captured.
const MAX_COUNTER_TIP_HEIGHT: u64 = 10_000;
const MAX_CLASSIFICATION_JSON_BYTES: usize = 1024;
// Four MiB cumulative native policy/manifest allocations, two simultaneously live
// maximum carrier/canonical frames (2.125 MiB), and checked finite auxiliary layouts.
// No wire cap or independent State pool grants any of these physical bytes.
const COUNTER_AUXILIARY_BYTES: usize = 8 * 1024 * 1024;
const COUNTER_AUXILIARY_DECODE_BYTES: usize = 4 * 1024 * 1024;
const COUNTER_CURRENT_FRAME_BYTES: usize = 4 * 1024 * 1024;
fn counter_source_decode_limits_v1(max_source_bytes: usize) -> norito::DecodeLimits {
    // Native sequence lengths include byte vectors and complete framed instructions. Every
    // successful sequence element also spends at least one allocation-accounting byte, so
    // source element ceilings derive from this same prepaid allocation envelope. The single
    // enclosing family still charges all original decodes cumulatively; control/output caps
    // and the physical State operation pool grant no additional source bytes.
    norito::DecodeLimits::new(
        max_source_bytes,
        max_source_bytes,
        max_source_bytes,
        max_source_bytes,
        32,
    )
}

fn auxiliary_layout_bytes(
    policy: &PrivateCountersPolicyV1,
) -> Result<usize, PrivateCountersErrorV1> {
    use std::alloc::Layout;
    let entries = usize::from(policy.limits.max_entries);
    let groups = usize::from(policy.limits.max_groups);
    let layouts = [
        Layout::array::<Hash>(MAX_COUNTER_TIP_HEIGHT as usize * 2), // retained and unchanged boundary
        Layout::array::<u16>(entries),
        Layout::array::<CounterSemanticV1>(entries),
        Layout::array::<CounterCategoryV1>(
            entries
                .checked_mul(16)
                .ok_or(PrivateCountersErrorV1::Bounds)?,
        ),
        Layout::array::<CounterPartyV1>(
            entries
                .checked_mul(16)
                .ok_or(PrivateCountersErrorV1::Bounds)?,
        ),
        Layout::array::<Hash>(
            entries
                .checked_mul(2)
                .ok_or(PrivateCountersErrorV1::Bounds)?,
        ),
        Layout::array::<CounterGroupV1>(groups),
    ];
    layouts
        .into_iter()
        .try_fold(2 * MAX_CLASSIFICATION_JSON_BYTES, |total, layout| {
            total
                .checked_add(layout.map_err(|_| PrivateCountersErrorV1::Bounds)?.size())
                .ok_or(PrivateCountersErrorV1::Bounds)
        })
}

/// A computed projection whose original pool charge follows its retained graph.
#[must_use]
pub struct ComputedPrivateCountersV1 {
    claim: PrivateCountersClaimV1,
    allocation: AllocationReservation,
}
impl std::ops::Deref for ComputedPrivateCountersV1 {
    type Target = PrivateCountersClaimV1;
    fn deref(&self) -> &Self::Target {
        &self.claim
    }
}
impl std::fmt::Debug for ComputedPrivateCountersV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputedPrivateCountersV1")
            .field("claim", &self.claim)
            .finish_non_exhaustive()
    }
}
impl ComputedPrivateCountersV1 {
    /// Transfer both payload and original charge; the installed signer retains both until encode.
    pub(crate) fn into_parts(self) -> (PrivateCountersClaimV1, AllocationReservation) {
        (self.claim, self.allocation)
    }
}

struct CounterGraphAllocations {
    source: AllocationReservation,
    retained: AllocationReservation,
    frame: AllocationReservation,
}
impl CounterGraphAllocations {
    fn admit(
        limits: &CounterLimitsV1,
        budget: &AllocationBudget,
    ) -> Result<Self, PrivateCountersErrorV1> {
        limits.validate()?;
        let source =
            usize::try_from(limits.max_source_bytes).map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let retained = usize::try_from(limits.max_retained_bytes)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let frame = source.min(COUNTER_CURRENT_FRAME_BYTES);
        let bytes = source
            .checked_add(retained)
            .and_then(|value| value.checked_add(frame))
            .ok_or(PrivateCountersErrorV1::Bounds)?;
        // Acquire the complete graph/frame demand atomically before World capture or body I/O.
        let mut reservation = budget
            .try_reserve_bytes(bytes)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let source = reservation
            .try_partition_bytes(source)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let retained = reservation
            .try_partition_bytes(retained)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let frame = reservation
            .try_partition_bytes(frame)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        Ok(Self {
            source,
            retained,
            frame,
        })
    }
}
const CONNECTED_CATALOG_SHA256: &str =
    "fbfb076a4859abe0e8eac0ce09c1a0093058d7689dec2349f39274afe495a914";

fn external_artifact_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

struct CompiledAction {
    action_id: u16,
    case_id: u16,
    step: u16,
    original_action: &'static str,
    original_case: &'static str,
    operation: &'static str,
    actor: &'static str,
    transaction_class: &'static str,
    operator_required: bool,
    branch: &'static str,
    role: CounterTransactionRoleV1,
    category: CounterCategoryV1,
    party: CounterPartyV1,
    result: CounterResultV1,
    rejection: CounterRejectionV1,
    movements: &'static [CounterCategoryV1],
    parties: &'static [CounterPartyV1],
    independent_batch: bool,
}

impl CompiledAction {
    fn semantic(&self) -> CounterSemanticV1 {
        CounterSemanticV1 {
            action_id: self.action_id,
            case_id: self.case_id,
            step: self.step,
            role: self.role,
            category: self.category,
            party: self.party,
            result: self.result,
            rejection: self.rejection,
            instruction_movements: self.movements.to_vec(),
            instruction_parties: self.parties.to_vec(),
        }
    }
}

fn actions(purpose: CounterPurposeV1) -> &'static [CompiledAction] {
    match purpose {
        CounterPurposeV1::ConnectedProvider => compiled_plan::CONNECTED,
        CounterPurposeV1::WalkthroughInteractions => compiled_plan::INTERACTIONS,
    }
}

/// Recover the entire sole current semantic plan from exact selected action identities.
/// This admits both supported producer purposes, never an arbitrary caller semantic list.
///
/// # Errors
/// Missing, extra, duplicate, unsorted or mutually exclusive branch action identities.
pub fn compiled_private_counter_plan_v1(
    purpose: CounterPurposeV1,
    action_ids: &[u16],
) -> Result<Vec<CounterSemanticV1>, PrivateCountersErrorV1> {
    let catalog = actions(purpose);
    let expected_len = match purpose {
        CounterPurposeV1::ConnectedProvider => 103,
        CounterPurposeV1::WalkthroughInteractions => 30,
    };
    if action_ids.len() != expected_len || action_ids.windows(2).any(|p| p[0] >= p[1]) {
        return Err(PrivateCountersErrorV1::Context);
    }
    let contains = |id| action_ids.binary_search(&id).is_ok();
    let pairs: &[(u16, u16)] = match purpose {
        CounterPurposeV1::ConnectedProvider => &[(56, 57), (102, 103)],
        CounterPurposeV1::WalkthroughInteractions => &[(11, 12)],
    };
    if pairs
        .iter()
        .any(|(left, right)| contains(*left) == contains(*right))
        || catalog.iter().any(|action| {
            !pairs
                .iter()
                .any(|pair| pair.0 == action.action_id || pair.1 == action.action_id)
                && !contains(action.action_id)
        })
        || action_ids
            .iter()
            .any(|id| !catalog.iter().any(|action| action.action_id == *id))
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    action_ids
        .iter()
        .map(|id| {
            catalog
                .iter()
                .find(|action| action.action_id == *id)
                .map(CompiledAction::semantic)
                .ok_or(PrivateCountersErrorV1::Context)
        })
        .collect()
}

/// Compute one complete private counter claim under the original immutable native cut.
/// The fixed policy authority is an installed genesis identity, never a request field.
/// Replay consumption and installed member signing are owned by Sumeragi's service.
///
/// # Errors
/// Refuses invalid signatures, unavailable/tail-modified account originals, foreign cuts,
/// incomplete indexes/manifests, incorrect original outcomes, and every finite-bound failure.
pub fn compute_private_transaction_counters_v1(
    state: &State,
    installed_policy_authority: &AccountId,
    request: &SignedPrivateCountersRequestV1,
    now_ms: u64,
    budget: &AllocationBudget,
) -> Result<ComputedPrivateCountersV1, PrivateCountersErrorV1> {
    request.verify_signature()?;
    let auxiliary = budget
        .try_reserve_bytes(COUNTER_AUXILIARY_BYTES)
        .map_err(|_| PrivateCountersErrorV1::Bounds)?;
    let generation = state.state_view_generation();
    if generation % 2 != 0 {
        return Err(PrivateCountersErrorV1::Context);
    }
    let view = state
        .try_view_once()
        .map_err(|_| PrivateCountersErrorV1::Context)?;
    let height = u64::try_from(view.height()).map_err(|_| PrivateCountersErrorV1::Bounds)?;
    let durable =
        u64::try_from(view.kura().blocks_count()).map_err(|_| PrivateCountersErrorV1::Bounds)?;
    if height < 2
        || height > MAX_COUNTER_TIP_HEIGHT
        || durable != height
        || request.payload.cut.height != height
        || request.payload.network_id != *view.network_id()
        || crate::sumeragi::lanes::routing::committed_root_scope(view.world())
            != Some(request.payload.scope)
        || !matches!(request.payload.scope, SumeragiRootScope::Dataspace { .. })
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    // This borrowed candidate grants only resource admission. Exact account originals are
    // authenticated below at the certified World cut before any count can be produced.
    let funding_authority = view
        .world()
        .accounts()
        .get(installed_policy_authority)
        .ok_or(PrivateCountersErrorV1::Context)?;
    let auxiliary_limits = norito::DecodeLimits::new(
        1024,
        MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1,
        131_072,
        COUNTER_AUXILIARY_DECODE_BYTES,
        32,
    );
    let (originals, usage) = norito::core::with_decode_limits_measured(auxiliary_limits, || {
        Ok::<_, PrivateCountersErrorV1>((
            read_policy(funding_authority)?,
            read_manifest(funding_authority)?,
        ))
    });
    let (policy, manifest) = originals?;
    let frame_bytes = (MAX_PRIVATE_COUNTER_POLICY_BYTES_V1 + MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1)
        .checked_mul(2)
        .ok_or(PrivateCountersErrorV1::Bounds)?;
    let funded_auxiliary_bytes = COUNTER_AUXILIARY_DECODE_BYTES
        .checked_add(frame_bytes)
        .and_then(|value| value.checked_add(auxiliary_layout_bytes(&policy).ok()?))
        .ok_or(PrivateCountersErrorV1::Bounds)?;
    if funded_auxiliary_bytes > auxiliary.remaining_bytes() {
        return Err(PrivateCountersErrorV1::Bounds);
    }
    if usage.total_allocated_bytes() > COUNTER_AUXILIARY_DECODE_BYTES {
        return Err(PrivateCountersErrorV1::Bounds);
    }
    request.payload.validate_at(&policy, now_ms)?;
    manifest.validate_against_policy(&policy)?;
    let allocations = CounterGraphAllocations::admit(&policy.limits, budget)?;
    let limits = crate::smartcontracts::isi::query::SingularQueryOutputLimits::new(
        allocations.frame.remaining_bytes() as u64,
        allocations.retained.remaining_bytes() as u64,
    );
    let source_limits = counter_source_decode_limits_v1(allocations.source.remaining_bytes());
    let claim = norito::core::with_decode_limits_scope(source_limits, || {
        crate::smartcontracts::isi::query::with_retained_singular_query_limits(limits, || {
            let archive = CertifiedArchiveView::new_with_budget(&view, view.kura(), budget)
        .map_err(|_original_error| {
            #[cfg(test)]
            eprintln!("private counter original refusal at archive constructor: {_original_error:?}");
            PrivateCountersErrorV1::Context
        })?;
            if archive.tip_height() != height {
                return Err(PrivateCountersErrorV1::Context);
            }
            let certified = archive.block(height).map_err(|_original_error| {
                #[cfg(test)]
                eprintln!("private counter original refusal at archive tip: {_original_error:?}");
                PrivateCountersErrorV1::Context
            })?;
            let tip = certified.committed();
            let cut = &request.payload.cut;
            if tip.block_hash() != cut.block_hash
                || tip.id().0.as_ref() != cut.context_id.as_ref()
                || tip.commitment().execution.world_state_root != cut.world_root
                || tip
                    .commitment()
                    .schedule
                    .current
                    .context_id()
                    .map_err(|_| PrivateCountersErrorV1::Context)?
                    != cut.epoch_context_id
            {
                return Err(PrivateCountersErrorV1::Context);
            }
            let mut refusal = None;
            let claim = state
        .with_native_private_counter_accounts_v1(
            tip,
            installed_policy_authority,
            &request.payload.authority,
            budget,
            |snapshot, authority, _requester| {
                #[cfg(test)]
                let mut _original_stage = "certified World root";
                let result = (|| {
                    if snapshot
                        .root()
                        .map_err(|_| PrivateCountersErrorV1::Context)?
                        != cut.world_root
                    {
                        return Err(PrivateCountersErrorV1::Context);
                    }
                    #[cfg(test)]
                    { _original_stage = "authority policy/manifest originals"; }
                    for (key, maximum) in [(PRIVATE_COUNTER_POLICY_METADATA_KEY_V1, MAX_PRIVATE_COUNTER_POLICY_BYTES_V1),
                                           (PRIVATE_COUNTER_MANIFEST_METADATA_KEY_V1, MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1)] {
                        if carrier_hex(authority, key, maximum)? != carrier_hex(funding_authority, key, maximum)? {
                            return Err(PrivateCountersErrorV1::Context);
                        }
                    }
                    #[cfg(test)]
                    { _original_stage = "current reader policy"; }
                    let reader = request.payload.validate_at(&policy, now_ms)?;
                    #[cfg(test)]
                    { _original_stage = "same-cut contract bindings"; }
                    let mut contracts = VerifiedContractOriginals::default();
                    verify_contract_bindings(snapshot, view.world(), &policy, &mut contracts)?;
                    #[cfg(test)]
                    { _original_stage = "current manifest binding"; }
                    manifest.validate_against_policy(&policy)?;
                    if manifest.commitment()? != request.payload.manifest_hash {
                        return Err(PrivateCountersErrorV1::Context);
                    }
                    #[cfg(test)]
                    { _original_stage = "compiled semantic plan binding"; }
                    let mut ids: Vec<_> = manifest
                        .entries
                        .iter()
                        .map(|entry| entry.semantic.action_id)
                        .collect();
                    ids.sort_unstable();
                    let plan = compiled_private_counter_plan_v1(policy.purpose, &ids)?;
                    if counter_plan_commitment_v1(&plan, &policy.expected_executables)?
                        != policy.plan_hash
                        || manifest.entries.iter().any(|entry| {
                            plan.binary_search_by_key(&entry.semantic.action_id, |semantic| {
                                semantic.action_id
                            })
                            .ok()
                            .is_none_or(|index| plan[index] != entry.semantic)
                        })
                    {
                        return Err(PrivateCountersErrorV1::Context);
                    }
                    #[cfg(test)]
                    { _original_stage = "archive before indexed history"; }
                    archive
                        .verify_unchanged()
                        .map_err(|_| PrivateCountersErrorV1::Context)?;
                    let entrypoint_hashes = manifest
                            .entries
                            .iter()
                            .map(|entry| entry.entrypoint_hash)
                            .collect();
                    #[cfg(test)]
                    { _original_stage = "indexed original history"; }
                    let rows = committed_transactions_indexed_snapshot_with_budget(
                        &view,
                        entrypoint_hashes,
                        TransactionHistoryWorkLimits {
                            max_carrier_work: policy.limits.max_carrier_work,
                            max_total_work: policy.limits.max_total_work,
                            max_bytes: policy.limits.max_source_bytes,
                        },
                        manifest.entries.len() as u64,
                        policy.limits.max_retained_bytes,
                        budget,
                        &allocations.retained,
                    )
                    .map_err(|_| PrivateCountersErrorV1::Bounds)?;
                    #[cfg(test)]
                    { _original_stage = "history row cardinality"; }
                    if rows.len() != manifest.entries.len() {
                        return Err(PrivateCountersErrorV1::Context);
                    }
                    #[cfg(test)]
                    { _original_stage = "projection storage admission"; }
                    let mut observed = Vec::new();
                    observed.try_reserve_exact(manifest.entries.len()).map_err(|_| PrivateCountersErrorV1::Bounds)?;
                    let mut groups: Vec<CounterGroupV1> = Vec::new();
                    groups.try_reserve_exact(usize::from(policy.limits.max_groups)).map_err(|_| PrivateCountersErrorV1::Bounds)?;
                    // Indexed projection and these authenticated carriers spend the same
                    // cumulative source family. Retain each selected carrier once; its
                    // exact fixed cache backing is admitted from the same operation pool.
                    let capacity = manifest.entries.len();
                    let cache_layouts = [
                        std::alloc::Layout::array::<u64>(capacity)
                            .map_err(|_| PrivateCountersErrorV1::Bounds)?,
                        std::alloc::Layout::array::<crate::sumeragi::certified_chain::CertifiedBlock>(capacity)
                            .map_err(|_| PrivateCountersErrorV1::Bounds)?,
                    ];
                    let mut cache_storage = budget.try_reserve_layouts(cache_layouts)
                        .map_err(|_| PrivateCountersErrorV1::Bounds)?;
                    let mut carrier_heights = iroha_allocation::ChargedBuffer::<u64>::from_reservation(
                        capacity, &mut cache_storage,
                    ).map_err(|_| PrivateCountersErrorV1::Bounds)?;
                    let mut carriers = iroha_allocation::ChargedBuffer::<crate::sumeragi::certified_chain::CertifiedBlock>::from_reservation(
                        capacity, &mut cache_storage,
                    ).map_err(|_| PrivateCountersErrorV1::Bounds)?;
                    for entry in &manifest.entries {
                        if entry.block_height > height {
                            return Err(PrivateCountersErrorV1::Context);
                        }
                        carrier_heights.push_reserved(entry.block_height);
                    }
                    carrier_heights.as_mut_slice().sort_unstable();
                    let mut previous_height = None;
                    for &carrier_height in carrier_heights.as_slice() {
                        if previous_height == Some(carrier_height) {
                            continue;
                        }
                        previous_height = Some(carrier_height);
                        if carrier_height == tip.height() {
                            // The existing certified tip owns this original through the callback.
                            continue;
                        }
                        // Increasing heights reuse the same full authenticated prefix;
                        // equal-height rows borrow one original rather than decoding it again.
                        #[cfg(test)]
                        { _original_stage = "original selected carrier cache"; }
                        let original = archive.block(carrier_height)
                            .map_err(|_| PrivateCountersErrorV1::Context)?;
                        carriers.push_reserved(original);
                    }
                    for row in rows.iter() {
                        #[cfg(test)]
                        { _original_stage = "manifest row membership"; }
                        let index = manifest
                            .entries
                            .binary_search_by_key(&row.entrypoint_hash, |entry| {
                                entry.entrypoint_hash
                            })
                            .map_err(|_| PrivateCountersErrorV1::Context)?;
                        let entry = &manifest.entries[index];
                        if observed.contains(&row.entrypoint_hash) || entry.block_height > height {
                            return Err(PrivateCountersErrorV1::Context);
                        }
                        observed.push(row.entrypoint_hash);
                        #[cfg(test)]
                        { _original_stage = "original row carrier"; }
                        let original = if entry.block_height == tip.height() {
                            tip
                        } else {
                            carriers.as_slice()
                                .binary_search_by_key(&entry.block_height, |original| original.height())
                                .map(|index| carriers.as_slice()[index].committed())
                                .map_err(|_| PrivateCountersErrorV1::Context)?
                        };
                        #[cfg(test)]
                        { _original_stage = "authenticated row inclusion and authority"; }
                        if !row.verify_selective_in_authenticated_execution(
                            view.network_id(),
                            &original.block().header(),
                            &original.commitment().execution,
                        ) || row.entrypoint.authority() != &entry.authority
                        {
                            return Err(PrivateCountersErrorV1::Context);
                        }
                        // The fixed owner captured these native executable identities from
                        // its authored plan before submission, independently of this receipt.
                        #[cfg(test)]
                        { _original_stage = "pre-submit original executable binding"; }
                        let expected = policy
                            .expected_executables
                            .binary_search_by_key(&entry.semantic.action_id, |binding| {
                                binding.action_id
                            })
                            .map(|index| &policy.expected_executables[index])
                            .map_err(|_| PrivateCountersErrorV1::Context)?;
                        let TransactionEntrypoint::External(signed) = &row.entrypoint else {
                            return Err(PrivateCountersErrorV1::Context);
                        };
                        if signed.authority() != &expected.authority
                            || HashOf::<Executable>::try_new(signed.instructions())
                                .map_err(|_| PrivateCountersErrorV1::Codec)?
                                != expected.executable_hash
                        {
                            return Err(PrivateCountersErrorV1::Context);
                        }
                        #[cfg(test)]
                        { _original_stage = "original semantic classification"; }
                        let action = actions(policy.purpose)
                            .iter()
                            .find(|action| action.action_id == entry.semantic.action_id)
                            .ok_or(PrivateCountersErrorV1::Context)?;
                        verify_original_classification(row, &policy, action)?;
                        #[cfg(test)]
                        { _original_stage = "original invoked contract binding"; }
                        for call in original_calls(&row.entrypoint) {
                            // Successful contracts need no declared rejection variants.
                            // Their original executable already pins address and native code
                            // identity; authenticate the same-cut code against the released
                            // artifact independently of the nominal rejection mapping.
                            contracts.require(
                                snapshot,
                                view.world(),
                                &policy,
                                &call.contract_address,
                                call.expected_code_hash,
                            )?;
                        }
                        #[cfg(test)]
                        { _original_stage = "original terminal result classification"; }
                        verify_original_result(row, &policy, action)?;
                        #[cfg(test)]
                        { _original_stage = "original independent batch classification"; }
                        verify_independent_batch(row, action)?;
                        #[cfg(test)]
                        { _original_stage = "bounded categorical projection"; }
                        let visible = match reader {
                            CounterReaderV1::Operator => true,
                            CounterReaderV1::Psp1 => action.party == CounterPartyV1::Psp1,
                            CounterReaderV1::Psp2 => action.party == CounterPartyV1::Psp2,
                        };
                        if !visible {
                            continue;
                        }
                        let key = CounterGroupKeyV1 {
                            party: if policy.purpose == CounterPurposeV1::WalkthroughInteractions {
                                CounterPartyV1::None
                            } else {
                                action.party
                            },
                            role: action.role,
                            category: action.category,
                            result: action.result,
                            rejection: action.rejection,
                        };
                        if let Some(group) = groups.iter_mut().find(|group| group.key == key) {
                            group.count = group.count.checked_add(1).ok_or(PrivateCountersErrorV1::Bounds)?;
                        } else {
                            if groups.len() >= usize::from(policy.limits.max_groups) {
                                return Err(PrivateCountersErrorV1::Bounds);
                            }
                            groups.push(CounterGroupV1 { key, count: 1 });
                        }
                    }
                    groups.sort_unstable_by_key(|group| group.key);
                    #[cfg(test)]
                    { _original_stage = "original request commitment"; }
                    let claim = PrivateCountersClaimV1 {
                        version: 1,
                        network_id: policy.network_id,
                        scope: policy.scope,
                        request_hash: request.original_hash()?,
                        authority: request.payload.authority.clone(),
                        reader,
                        purpose: policy.purpose,
                        policy_hash: request.payload.policy_hash,
                        manifest_hash: request.payload.manifest_hash,
                        cut: cut.clone(),
                        certified_block_time_ms: tip.block_time_ms(),
                        nonce: request.payload.nonce,
                        groups,
                    };
                    #[cfg(test)]
                    { _original_stage = "categorical claim validation"; }
                    claim.validate()?;
                    #[cfg(test)]
                    { _original_stage = "archive after claim validation"; }
                    archive
                        .verify_unchanged()
                        .map_err(|_| PrivateCountersErrorV1::Context)?;
                    Ok(claim)
                })();
                result.map_err(|error| {
                    #[cfg(test)]
                    eprintln!("private counter original consumer refusal at {_original_stage}: {error:?}");
                    refusal = Some(error);
                    "Private counter computation refused".to_owned()
                })
            },
        )
        .map_err(|_original_error| {
            #[cfg(test)]
            eprintln!("private counter original refusal at certified World accounts: {_original_error:?}");
            refusal.unwrap_or(PrivateCountersErrorV1::Context)
        })?;
            archive
                .verify_unchanged()
                .map_err(|_| PrivateCountersErrorV1::Context)?;
            if !is_stable_state_view_generation(generation, state.state_view_generation()) {
                return Err(PrivateCountersErrorV1::Context);
            }
            Ok(claim)
        })
    })?;
    // Source/rows/frames have been destroyed before their charges release. The small
    // returned native claim remains covered through installed response signing/encoding.
    Ok(ComputedPrivateCountersV1 {
        claim,
        allocation: auxiliary,
    })
}

fn carrier(
    authority: &AccountValue,
    key: &str,
    maximum: usize,
) -> Result<Vec<u8>, PrivateCountersErrorV1> {
    hex::decode(carrier_hex(authority, key, maximum)?).map_err(|_| PrivateCountersErrorV1::Codec)
}
fn carrier_hex<'a>(
    authority: &'a AccountValue,
    key: &str,
    maximum: usize,
) -> Result<&'a [u8], PrivateCountersErrorV1> {
    let value = authority
        .metadata
        .get(key)
        .ok_or(PrivateCountersErrorV1::Context)?;
    let original = value.get().as_bytes();
    if original.len() < 4
        || original.len() > maximum.saturating_mul(2).saturating_add(2)
        || original.first() != Some(&b'"')
        || original.last() != Some(&b'"')
    {
        return Err(PrivateCountersErrorV1::Bounds);
    }
    let hex = &original[1..original.len() - 1];
    if hex.len() % 2 != 0
        || hex
            .iter()
            .any(|b| !b.is_ascii_digit() && !(b'a'..=b'f').contains(b))
    {
        return Err(PrivateCountersErrorV1::Codec);
    }
    Ok(hex)
}
fn read_policy(
    authority: &AccountValue,
) -> Result<PrivateCountersPolicyV1, PrivateCountersErrorV1> {
    PrivateCountersPolicyV1::decode_bounded_canonical(&carrier(
        authority,
        PRIVATE_COUNTER_POLICY_METADATA_KEY_V1,
        MAX_PRIVATE_COUNTER_POLICY_BYTES_V1,
    )?)
}
fn read_manifest(
    authority: &AccountValue,
) -> Result<PrivateCountersManifestV1, PrivateCountersErrorV1> {
    PrivateCountersManifestV1::decode_bounded_canonical(&carrier(
        authority,
        PRIVATE_COUNTER_MANIFEST_METADATA_KEY_V1,
        MAX_PRIVATE_COUNTER_MANIFEST_BYTES_V1,
    )?)
}

fn require_original_table<T: norito::codec::Encode, V: norito::codec::Encode>(
    snapshot: &WorldStateSnapshotV1,
    field: &str,
    key: &T,
    value: &V,
) -> Result<(), PrivateCountersErrorV1> {
    let key_hash = world_state_value_hash_v1(key).map_err(|_| PrivateCountersErrorV1::Codec)?;
    let value_hash = world_state_value_hash_v1(value).map_err(|_| PrivateCountersErrorV1::Codec)?;
    let index = snapshot
        .entries
        .binary_search_by(|entry| {
            (entry.field_id.as_str(), entry.kind, entry.key_hash).cmp(&(
                field,
                WorldStateElementKindV1::Table,
                Some(key_hash),
            ))
        })
        .map_err(|_| PrivateCountersErrorV1::Context)?;
    if snapshot.entries[index].value_hash != value_hash {
        return Err(PrivateCountersErrorV1::Context);
    }
    Ok(())
}

struct VerifiedContractOriginals {
    addresses: [Option<(ContractAddress, Hash)>; 16],
    artifacts: [Option<(ContractArtifactId, [u8; 32])>; 16],
    original_bytes: usize,
}
impl Default for VerifiedContractOriginals {
    fn default() -> Self {
        Self {
            addresses: std::array::from_fn(|_| None),
            artifacts: std::array::from_fn(|_| None),
            original_bytes: 0,
        }
    }
}

impl VerifiedContractOriginals {
    fn require(
        &mut self,
        snapshot: &WorldStateSnapshotV1,
        world: &impl WorldReadOnly,
        policy: &PrivateCountersPolicyV1,
        address: &ContractAddress,
        expected_code_hash: Hash,
    ) -> Result<[u8; 32], PrivateCountersErrorV1> {
        if let Some((_, code_hash)) = self
            .addresses
            .iter()
            .flatten()
            .find(|(key, _)| key == address)
        {
            if *code_hash != expected_code_hash {
                return Err(PrivateCountersErrorV1::Context);
            }
        } else {
            let slot = self
                .addresses
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(PrivateCountersErrorV1::Bounds)?;
            let code_hash = world
                .contract_instances()
                .get(address)
                .ok_or(PrivateCountersErrorV1::Context)?;
            if *code_hash != expected_code_hash {
                return Err(PrivateCountersErrorV1::Context);
            }
            require_original_table(snapshot, "world.contract_instances", address, code_hash)?;
            *slot = Some((address.clone(), *code_hash));
        }
        let artifact = ContractArtifactId::for_address(address, expected_code_hash)
            .map_err(|_| PrivateCountersErrorV1::Context)?;
        if artifact.dataspace_id != policy.scope.dataspace_id() {
            return Err(PrivateCountersErrorV1::Context);
        }
        if let Some((_, original_hash)) = self
            .artifacts
            .iter()
            .flatten()
            .find(|(key, _)| key == &artifact)
        {
            return Ok(*original_hash);
        }
        let slot = self
            .artifacts
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(PrivateCountersErrorV1::Bounds)?;
        let code = world
            .contract_code()
            .get(&artifact)
            .ok_or(PrivateCountersErrorV1::Context)?;
        self.original_bytes = self
            .original_bytes
            .checked_add(code.len())
            .filter(|bytes| *bytes <= 16 * 1024 * 1024)
            .ok_or(PrivateCountersErrorV1::Bounds)?;
        let original_hash = external_artifact_sha256(code);
        if !policy.contracts.contains(&original_hash) {
            return Err(PrivateCountersErrorV1::Context);
        }
        require_original_table(snapshot, "world.contract_code", &artifact, code)?;
        *slot = Some((artifact, original_hash));
        Ok(original_hash)
    }
}

fn verify_contract_bindings(
    snapshot: &WorldStateSnapshotV1,
    world: &impl WorldReadOnly,
    policy: &PrivateCountersPolicyV1,
    originals: &mut VerifiedContractOriginals,
) -> Result<(), PrivateCountersErrorV1> {
    for binding in &policy.contract_errors {
        if originals.require(
            snapshot,
            world,
            policy,
            &binding.contract_address,
            binding.code_hash,
        )? != binding.artifact_hash
        {
            return Err(PrivateCountersErrorV1::Context);
        }
    }
    // A repeated native binding must not carry another artifact digest or code identity.
    for pair in policy.contract_errors.iter().flat_map(|left| {
        policy
            .contract_errors
            .iter()
            .map(move |right| (left, right))
    }) {
        if pair.0.contract_address == pair.1.contract_address
            && (pair.0.code_hash != pair.1.code_hash
                || pair.0.artifact_hash != pair.1.artifact_hash)
        {
            return Err(PrivateCountersErrorV1::Context);
        }
    }
    Ok(())
}

fn metadata_json<'a>(metadata: &'a Metadata, key: &str) -> Result<&'a str, PrivateCountersErrorV1> {
    let value = metadata
        .get(key)
        .ok_or(PrivateCountersErrorV1::Context)?
        .get();
    if value.len() > MAX_CLASSIFICATION_JSON_BYTES {
        return Err(PrivateCountersErrorV1::Bounds);
    }
    Ok(value)
}
fn expect_json(
    metadata: &Metadata,
    key: &str,
    expected: &str,
) -> Result<(), PrivateCountersErrorV1> {
    if metadata_json(metadata, key)? != expected {
        return Err(PrivateCountersErrorV1::Context);
    }
    Ok(())
}
fn expect_text(
    metadata: &Metadata,
    key: &str,
    expected: &str,
) -> Result<(), PrivateCountersErrorV1> {
    // Preserve native JSON string escaping for all admitted original run labels.
    let original = iroha_primitives::json::Json::new(expected);
    expect_json(metadata, key, original.get())
}
fn party_name(party: CounterPartyV1) -> &'static str {
    match party {
        CounterPartyV1::None => "none",
        CounterPartyV1::BankA => "bank_a",
        CounterPartyV1::BankB => "bank_b",
        CounterPartyV1::Court => "court",
        CounterPartyV1::Psp1 => "psp_1",
        CounterPartyV1::Psp2 => "psp_2",
    }
}
fn category_name(category: CounterCategoryV1) -> &'static str {
    match category {
        CounterCategoryV1::Availability => "availability",
        CounterCategoryV1::BankLink => "bank_link",
        CounterCategoryV1::BatchLeg => "batch_leg",
        CounterCategoryV1::Blocked => "blocked",
        CounterCategoryV1::Control => "control",
        CounterCategoryV1::CourtOrder => "court_order",
        CounterCategoryV1::EscrowPending => "escrow_pending",
        CounterCategoryV1::FacilityDraw => "facility_draw",
        CounterCategoryV1::Issuance => "issuance",
        CounterCategoryV1::Mint => "mint",
        CounterCategoryV1::Mixed => "mixed",
        CounterCategoryV1::Policy => "policy",
        CounterCategoryV1::Release => "release",
        CounterCategoryV1::Reserve => "reserve",
        CounterCategoryV1::Seize => "seize",
        CounterCategoryV1::Seizure => "seizure",
        CounterCategoryV1::Transfer => "transfer",
        CounterCategoryV1::WalletRegistration => "wallet_registration",
    }
}
fn rejection_name(rejection: CounterRejectionV1) -> &'static str {
    match rejection {
        CounterRejectionV1::None => "none",
        CounterRejectionV1::BelowMinimum => "BelowMinimum",
        CounterRejectionV1::CapabilityNotRegistered => "CapabilityNotRegistered",
        CounterRejectionV1::HoldingLimitExceeded => "HoldingLimitExceeded",
        CounterRejectionV1::IncomingDisabled => "IncomingDisabled",
        CounterRejectionV1::InsufficientBalance => "InsufficientBalance",
        CounterRejectionV1::NotPermitted => "NotPermitted",
        CounterRejectionV1::WalletInactive => "WalletInactive",
        CounterRejectionV1::WalletLimitExceeded => "WalletLimitExceeded",
    }
}

fn verify_original_classification(
    row: &CommittedTransaction,
    policy: &PrivateCountersPolicyV1,
    action: &CompiledAction,
) -> Result<(), PrivateCountersErrorV1> {
    let metadata = row
        .entrypoint
        .metadata()
        .ok_or(PrivateCountersErrorV1::Context)?;
    match &policy.run_binding {
        CounterRunBindingV1::Connected {
            definition_id,
            logical_baseline_hash,
            session_hash,
            provider_generation_hash,
        } => {
            expect_text(metadata, "boiw_walkthrough_id", definition_id)?;
            expect_text(
                metadata,
                "boiw_logical_baseline_digest",
                &hex::encode(logical_baseline_hash),
            )?;
            expect_text(metadata, "boiw_session_digest", &hex::encode(session_hash))?;
            expect_text(
                metadata,
                "boiw_provider_policy_generation_digest",
                &hex::encode(provider_generation_hash),
            )?;
            expect_text(metadata, "boiw_catalog_sha256", CONNECTED_CATALOG_SHA256)?;
            expect_text(metadata, "boiw_action_id", action.original_action)?;
            expect_json(metadata, "boiw_action_ordinal", &action.step.to_string())?;
            expect_json(metadata, "boiw_case_index", &action.case_id.to_string())?;
            expect_text(metadata, "boiw_actor_id", action.actor)?;
            expect_text(metadata, "boiw_operation", action.operation)?;
            expect_text(metadata, "boiw_transaction_class", action.transaction_class)?;
            expect_json(
                metadata,
                "boiw_operator_required",
                if action.operator_required {
                    "true"
                } else {
                    "false"
                },
            )?;
            if action.branch.is_empty() {
                expect_json(metadata, "boiw_branch_option_id", "null")?;
            } else {
                expect_text(metadata, "boiw_branch_option_id", action.branch)?;
            }
            expect_text(
                metadata,
                "boiw_role",
                if action.role == CounterTransactionRoleV1::Business {
                    "business"
                } else {
                    "control"
                },
            )?;
            expect_text(
                metadata,
                "boiw_movement_kind",
                category_name(action.category),
            )?;
            expect_text(metadata, "boiw_psp", party_name(action.party))?;
            expect_text(
                metadata,
                "boiw_rejection_code",
                rejection_name(action.rejection),
            )?;
        }
        CounterRunBindingV1::Interactions {
            run_id,
            run_namespace,
            definition_id,
            definition_hash,
            bindings_hash,
        } => {
            expect_text(metadata, "boiw_v1_run_id", run_id)?;
            expect_text(metadata, "boiw_v1_namespace", run_namespace)?;
            expect_text(metadata, "boiw_v1_definition_id", definition_id)?;
            expect_text(
                metadata,
                "boiw_v1_definition_digest",
                &hex::encode(definition_hash),
            )?;
            expect_text(
                metadata,
                "boiw_v1_binding_digest",
                &hex::encode(bindings_hash),
            )?;
            expect_text(metadata, "boiw_v1_checkpoint_id", action.original_case)?;
            expect_text(metadata, "boiw_v1_interaction_id", action.original_action)?;
            expect_text(metadata, "boiw_v1_operation_kind", action.operation)?;
            expect_text(
                metadata,
                "boiw_v1_movement_category",
                category_name(action.category),
            )?;
            expect_text(
                metadata,
                "boiw_v1_rejection_category",
                rejection_name(action.rejection),
            )?;
            expect_text(
                metadata,
                "boiw_v1_psp_scope",
                if action.party == CounterPartyV1::None {
                    "operator"
                } else {
                    party_name(action.party)
                },
            )?;
        }
    }
    Ok(())
}

fn verify_original_result(
    row: &CommittedTransaction,
    policy: &PrivateCountersPolicyV1,
    action: &CompiledAction,
) -> Result<(), PrivateCountersErrorV1> {
    match &row.result().0 {
        Ok(_) if action.result == CounterResultV1::Applied => Ok(()),
        Err(reason) if action.result == CounterResultV1::Rejected => {
            let actual = match reason {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(_)) => {
                    CounterRejectionV1::NotPermitted
                }
                TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                    error,
                )) => match error {
                    InstructionExecutionError::Math(MathError::NotEnoughQuantity) => {
                        CounterRejectionV1::InsufficientBalance
                    }
                    InstructionExecutionError::AssetTransferAdmission(
                        AssetTransferAdmissionError::HoldingLimitExceeded(_),
                    ) => CounterRejectionV1::HoldingLimitExceeded,
                    InstructionExecutionError::AssetTransferAdmission(
                        AssetTransferAdmissionError::IncomingDisabled(_),
                    ) => CounterRejectionV1::IncomingDisabled,
                    _ => return Err(PrivateCountersErrorV1::Context),
                },
                TransactionRejectionReason::Validation(ValidationFail::ContractRejected(
                    rejection,
                )) => {
                    let matched = policy
                        .contract_errors
                        .iter()
                        .find(|binding| {
                            binding.contract.as_str() == rejection.contract.as_ref()
                                && binding.error_type == rejection.error_type
                                && binding.schema_hash == rejection.schema_hash
                                && binding.name == rejection.name
                                && binding.code == rejection.code
                                && original_calls(&row.entrypoint).any(|call| {
                                    call.contract_address == binding.contract_address
                                        && call.expected_code_hash == binding.code_hash
                                })
                        })
                        .ok_or(PrivateCountersErrorV1::Context)?;
                    matched.rejection
                }
                _ => return Err(PrivateCountersErrorV1::Context),
            };
            if actual != action.rejection {
                return Err(PrivateCountersErrorV1::Context);
            }
            Ok(())
        }
        _ => Err(PrivateCountersErrorV1::Context),
    }
}

fn original_calls(entrypoint: &TransactionEntrypoint) -> impl Iterator<Item = &ContractInvocation> {
    let executable = match entrypoint {
        TransactionEntrypoint::External(signed) => Some(signed.instructions()),
        _ => None,
    };
    let direct = match executable {
        Some(Executable::ContractCall(call)) => Some(call),
        _ => None,
    };
    let batch = match executable {
        Some(Executable::Batch(items)) => Some(&items[..]),
        _ => None,
    };
    direct
        .into_iter()
        .chain(batch.into_iter().flat_map(|items| {
            items.iter().filter_map(|item| match item {
                ExecutableBatchItem::ContractCall(call) => Some(call),
                _ => None,
            })
        }))
}

fn verify_independent_batch(
    row: &CommittedTransaction,
    action: &CompiledAction,
) -> Result<(), PrivateCountersErrorV1> {
    let outcomes = row.result().batch_transfer_outcomes();
    if !action.independent_batch {
        if !outcomes.is_empty() {
            return Err(PrivateCountersErrorV1::Context);
        }
        return Ok(());
    }
    let TransactionEntrypoint::External(signed) = &row.entrypoint else {
        return Err(PrivateCountersErrorV1::Context);
    };
    let Executable::Instructions(instructions) = signed.instructions() else {
        return Err(PrivateCountersErrorV1::Context);
    };
    let batches: Vec<_> = instructions
        .iter()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<TransferAssetBatch>())
        .collect();
    if batches.len() != 1 || batches[0].entries().len() != 2 || outcomes.len() != 2 {
        return Err(PrivateCountersErrorV1::Context);
    }
    for (index, (entry, outcome)) in batches[0].entries().iter().zip(outcomes).enumerate() {
        if outcome.leg_index != index as u32
            || outcome.leg_id != *entry.leg_id()
            || outcome.asset.account() != entry.from()
            || outcome.asset.definition() != entry.asset_definition()
            || outcome.destination != *entry.to()
            || outcome.amount != *entry.amount()
        {
            return Err(PrivateCountersErrorV1::Context);
        }
    }
    if outcomes[0].status != AssetBatchTransferLegStatus::Applied
        || !matches!(&outcomes[1].status, AssetBatchTransferLegStatus::Rejected(error)
            if error.code == AssetBatchTransferRejectionCode::HoldingLimitExceeded)
    {
        return Err(PrivateCountersErrorV1::Context);
    }
    Ok(())
}

#[cfg(test)]
#[path = "private_transaction_counters/tests.rs"]
pub(crate) mod tests;
