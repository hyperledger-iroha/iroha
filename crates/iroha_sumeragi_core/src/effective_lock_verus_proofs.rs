//! Source-linked Verus kernels for exact body ownership, retirement, and bounded service.
use crate::refinement::{
    CERTIFICATE_EVIDENCE_ABSENT, CERTIFICATE_EVIDENCE_INCOMING, CERTIFICATE_EVIDENCE_LOCAL,
    IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_CONSENSUS_CONTEXT,
    IDENTITY_KIND_CONSENSUS_SUBJECT,
};
use crate::verus_proofs::{
    EnterViewProjection, ProductionTransitionProjection, accepted_core_enter_view_has_exact_fact,
    accepted_core_enter_view_projection_selects_post_install_lock,
    production_enter_view_exact_fact, production_enter_view_preserves_locked_prepare_qc_identity,
    production_enter_view_retains_high_prepare_qc_identity, production_kernel_relation,
};
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
/// Verus mirror of the production effective-lock trace projection.
#[derive(Copy, Clone)]
pub struct EffectiveLockTraceProjection {
    /// Production action discriminator.
    pub kind: u8,
    /// Exact lower-level relation derived from live state.
    pub relation_exact: bool,
    /// Protected-lock evidence before the checked seam.
    pub protected_before: u64,
    /// Protected-lock evidence after the checked seam.
    pub protected_after: u64,
    /// Exact body-owner count before the checked seam.
    pub owner_before: u64,
    /// Exact body-owner count after the checked seam.
    pub owner_after: u64,
    /// Whether an existing owner was reused monotonically.
    pub owner_reused: bool,
    /// Ready/retained byte capacity before retirement.
    pub ready_before: u64,
    /// Retained locked-body bytes retired by the step.
    pub retired_retained: u64,
    /// Ready-body bytes retired by the step.
    pub retired_ready: u64,
    /// Ready/retained byte capacity after retirement.
    pub ready_after: u64,
    /// Pending-store byte capacity before retirement.
    pub store_before: u64,
    /// Pending-store bytes retired by the step.
    pub retired_store: u64,
    /// Pending-store byte capacity after retirement.
    pub store_after: u64,
    /// Persistent bounded-service cursor before selection.
    pub cursor_before: u8,
    /// Whether the trusted completion class is ready.
    pub completion_ready: bool,
    /// Whether the certified progress class is ready.
    pub progress_ready: bool,
    /// Whether the ordinary ingress class is ready.
    pub normal_ready: bool,
    /// Selected bounded-service class.
    pub selected: u8,
    /// Persistent bounded-service cursor after selection.
    pub cursor_after: u8,
}
/// Verus instance of the compact production effective-lock trace relation.
pub closed spec fn effective_lock_trace_step_is_valid(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_step_body!(projection)
}
/// Exact Verus mirror of the production EnterView trace gate.
pub closed spec fn production_enter_view_uses_post_install_effective_lock_kernel(
    trace: EffectiveLockTraceProjection,
    enter_view: EnterViewProjection,
) -> bool {
    effective_lock_trace_claim_body!(trace, 1u8)
        && enter_view_locked_prepare_qc_identity_body!(enter_view)
        && enter_view_high_prepare_qc_control_identity_body!(enter_view)
}
/// Exact Verus mirror of the production body-owner trace gate.
pub closed spec fn production_body_ownership_preserves_effective_lock_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 2u8)
}
/// Exact Verus mirror of the production capacity-retirement trace gate.
pub closed spec fn production_body_capacity_retirement_preserves_effective_lock_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 3u8)
}
/// Exact Verus mirror of the production bounded-service trace gate.
pub closed spec fn production_body_service_refines_async_fairness_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 4u8)
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
                && previous.tag.view <= rebound_tag.view
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
// ---------------------------------------------------------------------------
// Effective-lock production refinement claims
// ---------------------------------------------------------------------------

/// Exact trace projected from the reducer's post-WAL `EnterView` seam.
pub closed spec fn production_enter_view_effective_lock_trace(
    projection: ProductionTransitionProjection,
) -> EffectiveLockTraceProjection {
    let enter_view = projection.enter_view;
    let protected_after = if enter_view.durable_lock_after.present { 1u64 } else { 0u64 };
    let owner_after = if enter_view.following_fetch_lock.present { 1u64 } else { 0u64 };
    EffectiveLockTraceProjection {
        kind: 1u8,
        relation_exact: production_enter_view_exact_fact(projection),
        protected_before: protected_after,
        protected_after: if enter_view.effect_protected_lock.present { 1u64 } else { 0u64 },
        owner_before: enter_view.fetch_count,
        owner_after,
        owner_reused: false,
        ready_before: 0u64,
        retired_retained: 0u64,
        retired_ready: 0u64,
        ready_after: 0u64,
        store_before: 0u64,
        retired_store: 0u64,
        store_after: 0u64,
        cursor_before: 0u8,
        completion_ready: false,
        progress_ready: false,
        normal_ready: false,
        selected: 0u8,
        cursor_after: 0u8,
    }
}
/// Exact trace projected from the executor's body-owner binding seam.
pub closed spec fn production_body_ownership_effective_lock_trace(
    current: Option<ProductionExactBodyOwnerProjection>,
    incoming: ProductionExactBodyOwnerProjection,
    binding: ProductionExactBodyOwnerBindingProjection,
) -> EffectiveLockTraceProjection {
    EffectiveLockTraceProjection {
        kind: 2u8,
        relation_exact: production_exact_body_owner_binding(current, incoming) == Some(binding),
        protected_before: if current.is_some() && current.unwrap().manifest_hash.is_some() {
            1u64
        } else {
            0u64
        },
        protected_after: if binding.owner.manifest_hash.is_some() { 1u64 } else { 0u64 },
        owner_before: if current.is_some() { 1u64 } else { 0u64 },
        owner_after: 1u64,
        owner_reused: binding.already_owned,
        ready_before: 0u64,
        retired_retained: 0u64,
        retired_ready: 0u64,
        ready_after: 0u64,
        store_before: 0u64,
        retired_store: 0u64,
        store_after: 0u64,
        cursor_before: 0u8,
        completion_ready: false,
        progress_ready: false,
        normal_ready: false,
        selected: 0u8,
        cursor_after: 0u8,
    }
}
/// Exact trace projected from either executor body-retirement seam.
pub closed spec fn production_body_capacity_retirement_effective_lock_trace(
    ready_before: u64,
    retained_bytes: u64,
    ready_bytes: u64,
    store_before: u64,
    store_bytes: u64,
    accounting: ExactBodyRetirementAccountingProjection,
) -> EffectiveLockTraceProjection {
    EffectiveLockTraceProjection {
        kind: 3u8,
        relation_exact: exact_body_retirement_accounting(
            ready_before,
            retained_bytes,
            ready_bytes,
            store_before,
            store_bytes,
        ) == Some(accounting),
        protected_before: 0u64,
        protected_after: 0u64,
        owner_before: 0u64,
        owner_after: 0u64,
        owner_reused: false,
        ready_before,
        retired_retained: retained_bytes,
        retired_ready: ready_bytes,
        ready_after: accounting.ready_after,
        store_before,
        retired_store: store_bytes,
        store_after: accounting.store_after,
        cursor_before: 0u8,
        completion_ready: false,
        progress_ready: false,
        normal_ready: false,
        selected: 0u8,
        cursor_after: 0u8,
    }
}
/// Exact trace projected from one runtime bounded-service selection.
pub closed spec fn production_body_service_effective_lock_trace(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
    selection: BoundedServiceSelectionProjection,
) -> EffectiveLockTraceProjection {
    EffectiveLockTraceProjection {
        kind: 4u8,
        relation_exact: selection == bounded_service_selection(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
        ),
        protected_before: 0u64,
        protected_after: 0u64,
        owner_before: 0u64,
        owner_after: 0u64,
        owner_reused: false,
        ready_before: 0u64,
        retired_retained: 0u64,
        retired_ready: 0u64,
        ready_after: 0u64,
        store_before: 0u64,
        retired_store: 0u64,
        store_after: 0u64,
        cursor_before: cursor,
        completion_ready,
        progress_ready,
        normal_ready,
        selected: selection.selected,
        cursor_after: selection.next,
    }
}
/// A reducer-accepted active `EnterView` carries the exact installed lock and
/// matching recovery-fetch evidence into the shared effective-lock trace.
pub proof fn production_enter_view_uses_post_install_effective_lock(
    projection: ProductionTransitionProjection,
)
    requires
        projection.enter_view.active,
        production_kernel_relation(projection),
    ensures
        production_enter_view_uses_post_install_effective_lock_kernel(
            production_enter_view_effective_lock_trace(projection),
            projection.enter_view,
        ),
        production_enter_view_effective_lock_trace(projection).kind == 1u8,
        production_enter_view_effective_lock_trace(projection).protected_after
            == production_enter_view_effective_lock_trace(projection).protected_before,
        production_enter_view_effective_lock_trace(projection).owner_after
            == production_enter_view_effective_lock_trace(projection).owner_before,
        projection.enter_view.effect_protected_lock.present
            == projection.enter_view.durable_lock_after.present,
        projection.enter_view.following_fetch_lock.present
            == projection.enter_view.durable_lock_after.present,
        production_enter_view_retains_high_prepare_qc_identity(projection.enter_view),
        projection.enter_view.prepare_control_slot_present_after
            == projection.enter_view.durable_highest_after.present,
        certificate_identity_equal_body!(
            projection.enter_view.retained_prepare_qc_after,
            projection.enter_view.durable_highest_after
        ),
{
    accepted_core_enter_view_has_exact_fact(projection);
    accepted_core_enter_view_projection_selects_post_install_lock(projection);
    reveal(production_enter_view_effective_lock_trace);
    reveal(production_enter_view_uses_post_install_effective_lock_kernel);
    reveal(effective_lock_trace_step_is_valid);
    reveal(production_enter_view_preserves_locked_prepare_qc_identity);
    reveal(production_enter_view_retains_high_prepare_qc_identity);
    assert(production_enter_view_uses_post_install_effective_lock_kernel(
        production_enter_view_effective_lock_trace(projection),
        projection.enter_view,
    ));
}
/// A successful exact-owner binding creates one owner or reuses it without
/// dropping previously installed manifest evidence.
pub proof fn production_body_ownership_preserves_effective_lock(
    current: Option<ProductionExactBodyOwnerProjection>,
    incoming: ProductionExactBodyOwnerProjection,
    binding: ProductionExactBodyOwnerBindingProjection,
)
    requires
        production_exact_body_owner_binding(current, incoming) == Some(binding),
    ensures
        production_body_ownership_preserves_effective_lock_kernel(
            production_body_ownership_effective_lock_trace(current, incoming, binding),
        ),
        production_body_ownership_effective_lock_trace(
            current,
            incoming,
            binding,
        ).owner_after == 1u64,
        production_body_ownership_effective_lock_trace(
            current,
            incoming,
            binding,
        ).protected_after
            >= production_body_ownership_effective_lock_trace(
                current,
                incoming,
                binding,
            ).protected_before,
        production_body_ownership_effective_lock_trace(
            current,
            incoming,
            binding,
        ).owner_reused
            == (production_body_ownership_effective_lock_trace(
                current,
                incoming,
                binding,
            ).owner_before == 1u64),
{
    reveal(production_body_ownership_effective_lock_trace);
    reveal(production_exact_body_owner_binding);
    reveal(production_body_ownership_preserves_effective_lock_kernel);
    reveal(effective_lock_trace_step_is_valid);
    assert(production_body_ownership_preserves_effective_lock_kernel(
        production_body_ownership_effective_lock_trace(current, incoming, binding),
    ));
}
/// Successful exact retirement accounts for every retained, ready, and store
/// byte without underflow or residual leakage.
pub proof fn production_body_capacity_retirement_preserves_effective_lock(
    ready_before: u64,
    retained_bytes: u64,
    ready_bytes: u64,
    store_before: u64,
    store_bytes: u64,
    accounting: ExactBodyRetirementAccountingProjection,
)
    requires
        exact_body_retirement_accounting(
            ready_before,
            retained_bytes,
            ready_bytes,
            store_before,
            store_bytes,
        ) == Some(accounting),
    ensures
        production_body_capacity_retirement_preserves_effective_lock_kernel(
            production_body_capacity_retirement_effective_lock_trace(
                ready_before,
                retained_bytes,
                ready_bytes,
                store_before,
                store_bytes,
                accounting,
            ),
        ),
        retained_bytes <= ready_before,
        ready_bytes <= ready_before - retained_bytes,
        accounting.ready_after == ready_before - retained_bytes - ready_bytes,
        store_bytes <= store_before,
        accounting.store_after == store_before - store_bytes,
{
    reveal(production_body_capacity_retirement_effective_lock_trace);
    reveal(exact_body_retirement_accounting);
    reveal(production_body_capacity_retirement_preserves_effective_lock_kernel);
    reveal(effective_lock_trace_step_is_valid);
    assert(production_body_capacity_retirement_preserves_effective_lock_kernel(
        production_body_capacity_retirement_effective_lock_trace(
            ready_before,
            retained_bytes,
            ready_bytes,
            store_before,
            store_bytes,
            accounting,
        ),
    ));
}
/// When any class is ready, the bounded-service cursor selects one exact
/// ready class and advances to the unique next cursor.
pub proof fn production_body_service_refines_async_fairness(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
    selection: BoundedServiceSelectionProjection,
)
    requires
        cursor >= 1u8,
        cursor <= 3u8,
        completion_ready || progress_ready || normal_ready,
        selection == bounded_service_selection(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
        ),
    ensures
        production_body_service_refines_async_fairness_kernel(
            production_body_service_effective_lock_trace(
                cursor,
                completion_ready,
                progress_ready,
                normal_ready,
                selection,
            ),
        ),
        selection.selected >= 1u8,
        selection.selected <= 3u8,
        selection.next >= 1u8,
        selection.next <= 3u8,
        selection.selected == 1u8 ==> completion_ready,
        selection.selected == 2u8 ==> progress_ready,
        selection.selected == 3u8 ==> normal_ready,
{
    reveal(production_body_service_effective_lock_trace);
    reveal(bounded_service_selection);
    reveal(production_body_service_refines_async_fairness_kernel);
    reveal(effective_lock_trace_step_is_valid);
    assert(production_body_service_refines_async_fairness_kernel(
        production_body_service_effective_lock_trace(
            cursor,
            completion_ready,
            progress_ready,
            normal_ready,
            selection,
        ),
    ));
}
} // verus!
