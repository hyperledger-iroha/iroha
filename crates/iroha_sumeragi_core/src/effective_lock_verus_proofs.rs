//! Source-linked Verus kernels for exact body ownership, retirement, and bounded service.

use vstd::prelude::*;

verus! {

/// Verus-side shape of a fixed-width reducer incarnation.
#[derive(Copy, Clone)]
pub struct ProductionTagProjection {
    /// Height consuming the body pipeline.
    pub height: u64,
    /// View consuming the body pipeline.
    pub view: u64,
    /// Reducer generation consuming the body pipeline.
    pub generation: u64,
}

// ---------------------------------------------------------------------------
// Source-linked body ownership and bounded-service kernels
// ---------------------------------------------------------------------------

/// Verus-side result of the exact three-class runtime ingress selector.
#[derive(Copy, Clone)]
pub struct BoundedServiceSelectionProjection {
    /// Selected class (`0` means no ready class).
    pub selected: u8,
    /// Persistent cursor for the next invocation.
    pub next: u8,
}

/// Exact branch relation called by production `BoundedIngress::pop_next`.
pub closed spec fn bounded_service_selection(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
) -> BoundedServiceSelectionProjection {
    bounded_service_selection_body!(
        cursor,
        completion_ready,
        progress_ready,
        normal_ready,
        BoundedServiceSelectionProjection,
    )
}

/// Executable Verus instance of the production class selector.
pub fn verified_bounded_service_selection(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
) -> (selection: BoundedServiceSelectionProjection)
    ensures
        selection == bounded_service_selection(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
        ),
{
    let selection = bounded_service_selection_body!(
        cursor,
        completion_ready,
        progress_ready,
        normal_ready,
        BoundedServiceSelectionProjection,
    );
    proof {
        reveal(bounded_service_selection);
    }
    selection
}

/// A ready class is selected in the same invocation, and invalid cursors
/// cannot select work.
pub proof fn bounded_service_selection_is_ready_or_fail_closed(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
)
    ensures
        (cursor < 1 || cursor > 3) ==>
            bounded_service_selection(
                cursor,
                completion_ready,
                progress_ready,
                normal_ready,
            ).selected == 0,
        (cursor >= 1 && cursor <= 3
            && (completion_ready || progress_ready || normal_ready)) ==>
            bounded_service_selection(
                cursor,
                completion_ready,
                progress_ready,
                normal_ready,
            ).selected != 0,
        bounded_service_selection(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
        ).selected == 1 ==> completion_ready,
        bounded_service_selection(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
        ).selected == 2 ==> progress_ready,
        bounded_service_selection(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
        ).selected == 3 ==> normal_ready,
{
    reveal(bounded_service_selection);
}

/// When all classes stay ready, the production cursor serves each class once
/// in three invocations. This is a bounded arbitration fact, not a temporal
/// claim that the host invokes the runtime.
pub proof fn bounded_service_all_ready_cycle(cursor: u8)
    requires
        cursor >= 1,
        cursor <= 3,
    ensures
        ({
            let first = bounded_service_selection(cursor, true, true, true);
            let second = bounded_service_selection(first.next, true, true, true);
            let third = bounded_service_selection(second.next, true, true, true);
            first.selected != second.selected
                && first.selected != third.selected
                && second.selected != third.selected
                && first.next != 0
                && second.next != 0
                && third.next == cursor
        }),
{
    reveal(bounded_service_selection);
}

/// Verus-side exact residual counters after body supersession.
#[derive(Copy, Clone)]
pub struct ExactBodyRetirementAccountingProjection {
    /// Remaining reconstructed/retained byte-owner aggregate.
    pub ready_after: u64,
    /// Remaining pending-store byte-owner aggregate.
    pub store_after: u64,
}

/// Exact source-linked body supersession accounting relation.
pub closed spec fn exact_body_retirement_accounting(
    ready_before: u64,
    retained_bytes: u64,
    ready_bytes: u64,
    store_before: u64,
    store_bytes: u64,
) -> Option<ExactBodyRetirementAccountingProjection> {
    exact_body_retirement_accounting_body!(
        ready_before,
        retained_bytes,
        ready_bytes,
        store_before,
        store_bytes,
        ExactBodyRetirementAccountingProjection,
    )
}

/// Executable Verus instance of production supersession accounting.
pub fn verified_exact_body_retirement_accounting(
    ready_before: u64,
    retained_bytes: u64,
    ready_bytes: u64,
    store_before: u64,
    store_bytes: u64,
) -> (accounting: Option<ExactBodyRetirementAccountingProjection>)
    ensures
        accounting == exact_body_retirement_accounting(
            ready_before,
            retained_bytes,
            ready_bytes,
            store_before,
            store_bytes,
        ),
        accounting.is_some() ==> ({
            let residual = accounting.unwrap();
            residual.ready_after as int + retained_bytes as int + ready_bytes as int
                == ready_before as int
                && residual.store_after as int + store_bytes as int == store_before as int
        }),
{
    let accounting = exact_body_retirement_accounting_body!(
        ready_before,
        retained_bytes,
        ready_bytes,
        store_before,
        store_bytes,
        ExactBodyRetirementAccountingProjection,
    );
    proof {
        reveal(exact_body_retirement_accounting);
    }
    accounting
}

/// Source-linked classification of one completion stage across its two
/// serialized owners (`0 = vacant`, `1 = exact`, `2 = invalid`).
pub closed spec fn exact_body_completion_ownership(
    ingress_owners: usize,
    ingress_exact: usize,
    deferred_owners: usize,
    deferred_exact: usize,
) -> u8 {
    exact_body_completion_ownership_body!(
        ingress_owners,
        ingress_exact,
        deferred_owners,
        deferred_exact,
        0u8,
        1u8,
        2u8,
    )
}

/// The exact classifier admits one evidence-matching owner in only one lane.
pub proof fn exact_body_completion_owner_is_unique(
    ingress_owners: usize,
    ingress_exact: usize,
    deferred_owners: usize,
    deferred_exact: usize,
)
    ensures
        exact_body_completion_ownership(
            ingress_owners,
            ingress_exact,
            deferred_owners,
            deferred_exact,
        ) == 1 <==>
            ((ingress_owners == 1 && ingress_exact == 1
                && deferred_owners == 0 && deferred_exact == 0)
            || (ingress_owners == 0 && ingress_exact == 0
                && deferred_owners == 1 && deferred_exact == 1)),
{
    reveal(exact_body_completion_ownership);
}

/// Verus-side typed identity of one production exact-body owner.
#[derive(Copy, Clone)]
pub struct ProductionExactBodyOwnerProjection {
    /// Reducer incarnation consuming the next body-stage completion.
    pub tag: ProductionTagProjection,
    /// Complete `(round, subject)` identity.
    pub key: int,
    /// Optional canonical manifest identity during certified acquisition.
    pub manifest_hash: Option<int>,
}

/// Verus-side result of typed exact-body owner binding.
#[derive(Copy, Clone)]
pub struct ProductionExactBodyOwnerBindingProjection {
    /// Exact owner after monotonic manifest enrichment.
    pub owner: ProductionExactBodyOwnerProjection,
    /// Whether the key already had an owner before this binding.
    pub already_owned: bool,
}

/// Exact source-linked owner binding relation called by production.
pub closed spec fn production_exact_body_owner_binding(
    current: Option<ProductionExactBodyOwnerProjection>,
    incoming: ProductionExactBodyOwnerProjection,
) -> Option<ProductionExactBodyOwnerBindingProjection> {
    exact_body_owner_binding_body!(
        current,
        incoming,
        ProductionExactBodyOwnerProjection,
        ProductionExactBodyOwnerBindingProjection
    )
}

/// Executable Verus instance of the production owner-binding branches.
pub fn verified_production_exact_body_owner_binding(
    current: Option<ProductionExactBodyOwnerProjection>,
    incoming: ProductionExactBodyOwnerProjection,
) -> (binding: Option<ProductionExactBodyOwnerBindingProjection>)
    ensures
        binding == production_exact_body_owner_binding(current, incoming),
{
    let binding = exact_body_owner_binding_body!(
        current,
        incoming,
        ProductionExactBodyOwnerProjection,
        ProductionExactBodyOwnerBindingProjection
    );
    proof {
        reveal(production_exact_body_owner_binding);
    }
    binding
}

/// Exact immutable stage-owner identity relation called by Store/Validate.
pub closed spec fn production_exact_body_stage_is_owned(
    owner: ProductionExactBodyOwnerProjection,
    stage: ProductionExactBodyOwnerProjection,
) -> bool {
    exact_body_owner_equal_body!(owner, stage)
}

/// A successful binding cannot replace tag, key, or existing manifest
/// evidence; it may only fill an absent manifest identity.
pub proof fn production_exact_body_binding_is_monotonic(
    current: ProductionExactBodyOwnerProjection,
    incoming: ProductionExactBodyOwnerProjection,
    binding: ProductionExactBodyOwnerBindingProjection,
)
    requires
        production_exact_body_owner_binding(Some(current), incoming) == Some(binding),
    ensures
        binding.already_owned,
        binding.owner.tag == current.tag,
        binding.owner.key == current.key,
        current.manifest_hash.is_some() ==>
            binding.owner.manifest_hash == current.manifest_hash,
        incoming.manifest_hash.is_some() ==>
            binding.owner.manifest_hash == incoming.manifest_hash,
{
    reveal(production_exact_body_owner_binding);
}

/// The four production body stages carry one immutable reducer/key identity
/// and a monotonic manifest identity. A certified Fetch may begin without a
/// manifest; BodyAvailable fills it exactly once, after which Store/Validate
/// retain it. Each premise is the shared predicate called by the corresponding
/// executor admission or completion boundary.
pub proof fn production_fetch_available_store_validate_owner_is_immutable(
    fetch: ProductionExactBodyOwnerProjection,
    available: ProductionExactBodyOwnerProjection,
    store: ProductionExactBodyOwnerProjection,
    validate: ProductionExactBodyOwnerProjection,
    binding: ProductionExactBodyOwnerBindingProjection,
)
    requires
        production_exact_body_owner_binding(Some(fetch), available) == Some(binding),
        binding.owner == available,
        production_exact_body_stage_is_owned(available, store),
        production_exact_body_stage_is_owned(store, validate),
    ensures
        fetch.tag == validate.tag,
        fetch.key == validate.key,
        available.manifest_hash == validate.manifest_hash,
        fetch.manifest_hash.is_some() ==>
            fetch.manifest_hash == validate.manifest_hash,
{
    reveal(production_exact_body_owner_binding);
    reveal(production_exact_body_stage_is_owned);
}

/// Exact source-linked rebind relation called after the service/runtime owner
/// acknowledges a transfer.
pub closed spec fn production_exact_body_owner_rebind(
    current: ProductionExactBodyOwnerProjection,
    previous: ProductionExactBodyOwnerProjection,
    rebound_tag: ProductionTagProjection,
) -> Option<ProductionExactBodyOwnerProjection> {
    exact_body_owner_rebind_body!(
        current,
        previous,
        rebound_tag,
        ProductionExactBodyOwnerProjection
    )
}

/// Executable Verus instance of the exact production rebind branches.
pub fn verified_production_exact_body_owner_rebind(
    current: ProductionExactBodyOwnerProjection,
    previous: ProductionExactBodyOwnerProjection,
    rebound_tag: ProductionTagProjection,
) -> (rebound: Option<ProductionExactBodyOwnerProjection>)
    ensures
        rebound == production_exact_body_owner_rebind(current, previous, rebound_tag),
        rebound.is_some() ==> ({
            let owner = rebound.unwrap();
            owner.tag == rebound_tag
                && owner.key == previous.key
                && owner.manifest_hash == previous.manifest_hash
                && previous.tag.height == rebound_tag.height
                && previous.tag.view < rebound_tag.view
                && previous.tag.generation < rebound_tag.generation
        }),
{
    let rebound = exact_body_owner_rebind_body!(
        current,
        previous,
        rebound_tag,
        ProductionExactBodyOwnerProjection
    );
    proof {
        reveal(production_exact_body_owner_rebind);
    }
    rebound
}

} // verus!
