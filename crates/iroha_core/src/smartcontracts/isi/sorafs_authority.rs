//! Selected Parliament authority host and exclusive native permission projection.

use crate::execution_attempt::ExecutionAttemptError;
use crate::{
    role::RoleIdWithOwner,
    state::{GovernanceProposalStatus, StateReadOnly, StateTransaction, WorldReadOnly},
};
use iroha_allocation::{AllocationBudget, AllocatorRefusal, ChargedBuffer, ChargedBufferError};
use iroha_data_model::{
    governance::types::{
        GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceCertificateId,
        GovernanceCertificateV1, GovernanceExpectedHeadPresentV1, GovernanceExpectedHeadV1,
        ProposalContentId, parliament_expected_head_root_v1,
    },
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs_authority::{
            SorafsAuthorityActionV1, SorafsAuthorityProposalV1, SorafsOperatingAuthorityV1,
        },
    },
    prelude::*,
};
use iroha_executor_data_model::permission::sorafs::{
    CanCompleteSorafsReplicationOrder, CanSetSorafsReservePolicy, CanUpsertSorafsProviderCredit,
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use std::sync::LazyLock;

/// Reserved role whose membership alone controls gateway compliance operations.
pub const COMPLIANCE_ROLE_NAME: &str = "sorafs_gateway_compliance_operator";
static COMPLIANCE_ROLE: LazyLock<RoleId> = LazyLock::new(|| {
    COMPLIANCE_ROLE_NAME
        .parse()
        .expect("static canonical compliance role name")
});

/// Retained hosted content with the latest shared-attempt lifecycle status.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::sorafs_authority::SorafsAuthorityProposalRecordV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityProposalRecordV1 {
    /// Bonded citizen who submitted the immutable content.
    pub proposer: AccountId,
    /// Exact network/root-bound authority decision.
    pub proposal: SorafsAuthorityProposalV1,
    /// Committed proposal creation height.
    pub created_height: u64,
    /// Latest shared Parliament attempt status.
    pub status: GovernanceProposalStatus,
    /// Selection-owned deadline closure; the shared reducer transcript remains unchanged.
    pub deadline_closure: Option<SorafsAuthorityDeadlineClosureV1>,
}

/// Deterministic end of a still-pending decision's execution eligibility.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::sorafs_authority::SorafsAuthorityDeadlineClosureV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityDeadlineClosureV1 {
    /// Exact canonical approval cutoff or early-revocation term end.
    pub closed_at_height: u64,
}

/// Monotonic authority head and the exact receipt of its last successful change.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::sorafs_authority::SorafsAuthorityHeadV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsAuthorityHeadV1 {
    /// Exact two-decision lease plan owning this authority incarnation.
    pub plan_id: [u8; 32],
    /// Canonical nonoverlapping term number.
    pub term: u64,
    /// Irrevocably approved terminal height.
    pub expires_at_height: u64,
    /// Exact authority and provider scope.
    pub authority: SorafsOperatingAuthorityV1,
    /// Canonical authority holder.
    pub account: AccountId,
    /// Successful-change counter; revoked records retain this counter.
    pub revision: u64,
    /// Whether the exact native execution projection is currently installed.
    pub granted: bool,
    /// Immutable decision that last changed this subject.
    pub proposal_content_id: ProposalContentId,
    /// Successful shared attempt.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Exact retained certificate identity.
    pub certificate_id: GovernanceCertificateId,
    /// Exact due height of the successful effect.
    pub enacted_at_height: u64,
}

mod closure;
pub(crate) use closure::{
    close_expired_proposals, proposal_deadline, proposal_is_closed,
    retained_attempt_validation_height,
};
mod terms;
pub(crate) use terms::SorafsAuthorityLeaseV1;
#[cfg(test)]
pub(crate) mod test_support;

fn invalid(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

/// Whether the current immutable services profile selects this authority owner.
pub(crate) fn active(world: &impl WorldReadOnly) -> Result<bool, InstructionExecutionError> {
    crate::runtime::sorafs_authority_enabled(world.runtime_selection())
        .map_err(|error| invalid(format!("unsupported SoraFS authority services: {error}")))
}

fn require_active(world: &impl WorldReadOnly) -> Result<(), InstructionExecutionError> {
    if active(world)? {
        Ok(())
    } else {
        Err(invalid("SoraFS authority governance is inactive"))
    }
}

/// Match protected names before payload decoding, so malformed tokens cannot bypass ownership.
pub(crate) fn protected_permission(permission: &Permission) -> bool {
    matches!(
        permission.name(),
        "CanCompleteSorafsReplicationOrder"
            | "CanSetSorafsReservePolicy"
            | "CanUpsertSorafsProviderCredit"
    )
}

// The Initial executor's empty model denotes its fixed built-in permission table.
// Every explicit/custom declaration must retain the projection's three token names.
fn executor_declares_authorities(world: &impl WorldReadOnly) -> bool {
    let permissions = world.executor_data_model().permissions();
    (permissions.is_empty() && matches!(world.executor(), crate::executor::Executor::Initial))
        || [
            "CanCompleteSorafsReplicationOrder",
            "CanSetSorafsReservePolicy",
            "CanUpsertSorafsProviderCredit",
        ]
        .iter()
        .all(|name| permissions.contains(*name))
}

fn permission(authority: SorafsOperatingAuthorityV1) -> Option<Permission> {
    match authority {
        SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id } => {
            Some(CanCompleteSorafsReplicationOrder { provider_id }.into())
        }
        SorafsOperatingAuthorityV1::SetReservePolicy => Some(CanSetSorafsReservePolicy.into()),
        SorafsOperatingAuthorityV1::UpsertProviderCredit => {
            Some(CanUpsertSorafsProviderCredit.into())
        }
        SorafsOperatingAuthorityV1::GatewayComplianceOperator => None,
    }
}

/// Reject every ordinary direct or role-contained permission mutation under the selected owner.
pub(crate) fn guard_permission_mutation(
    world: &impl WorldReadOnly,
    token: &Permission,
) -> Result<(), InstructionExecutionError> {
    if protected_permission(token) && active(world)? {
        return Err(invalid(
            "SoraFS operating authority changes require a Parliament certificate",
        ));
    }
    Ok(())
}

/// Reject reserved-role edits and delegation or deletion of a role carrying protected tokens.
pub(crate) fn guard_role_mutation(
    world: &impl WorldReadOnly,
    role: &RoleId,
) -> Result<(), InstructionExecutionError> {
    if (role == &*COMPLIANCE_ROLE
        || world
            .roles()
            .get(role)
            .is_some_and(|role| role.permissions().any(protected_permission)))
        && active(world)?
    {
        return Err(invalid(
            "SoraFS operating authority roles are owned by Parliament",
        ));
    }
    Ok(())
}

/// Account removal or rekeying cannot silently revoke or transfer a voted authority.
pub(crate) fn guard_account_removal(
    world: &impl WorldReadOnly,
    account: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if !active(world)? {
        return Ok(());
    }
    if world
        .account_permissions()
        .get(account)
        .is_some_and(|tokens| tokens.iter().any(protected_permission))
        || world.account_roles_iter(account).any(|id| {
            id == &*COMPLIANCE_ROLE
                || world
                    .roles()
                    .get(id)
                    .is_some_and(|role| role.permissions().any(protected_permission))
        })
    {
        return Err(invalid(
            "voted SoraFS authorities must be revoked before account removal or rekeying",
        ));
    }
    Ok(())
}

/// Fail closed at activation instead of importing an unvoted direct or inherited authority.
/// This single cutover traversal borrows the already admitted resident tables. Its B+tree
/// cursors use fixed stack paths; it neither collects the inventory nor clones payloads.
/// Work is linear in the committed inventory and scratch memory is independent of its size.
/// No node-local limit converts an unfinished scan into a deterministic activation lapse.
pub(crate) fn validate_activation_inventory(world: &impl WorldReadOnly) -> Result<(), String> {
    if !executor_declares_authorities(world) {
        return Err(
            "SoraFS authority activation requires exact executor permission declarations".into(),
        );
    }
    if world
        .account_permissions()
        .iter()
        .any(|(_, tokens)| tokens.iter().any(protected_permission))
    {
        return Err(
            "SoraFS authority activation requires no preexisting protected direct grants".into(),
        );
    }
    if world
        .roles()
        .iter()
        .any(|(id, role)| id == &*COMPLIANCE_ROLE || role.permissions().any(protected_permission))
        || world
            .account_roles()
            .iter()
            .any(|(membership, ())| membership.id == *COMPLIANCE_ROLE)
    {
        return Err("SoraFS authority activation requires no protected role grants and an absent compliance role".into());
    }
    if world.sorafs_authority_leases().iter().next().is_some()
        || world.sorafs_authority_heads().iter().next().is_some()
        || world.sorafs_authority_proposals().iter().next().is_some()
    {
        return Err(
            "SoraFS authority activation requires empty uninstalled authority tables".into(),
        );
    }
    Ok(())
}

fn projected(
    world: &impl WorldReadOnly,
    authority: SorafsOperatingAuthorityV1,
    account: &AccountId,
) -> bool {
    permission(authority).map_or_else(
        || {
            world
                .account_roles_iter(account)
                .any(|id| id == &*COMPLIANCE_ROLE)
        },
        |token| world.account_contains_inherent_permission(account, &token),
    )
}

fn effective_predecessor(
    proposal: &SorafsAuthorityProposalV1,
    world: &impl WorldReadOnly,
) -> (u64, bool) {
    let head = world
        .sorafs_authority_heads()
        .get(&proposal.governed_subject_id_v1());
    let Some(head) = head else {
        return (0, false);
    };
    if proposal.action != SorafsAuthorityActionV1::Revoke
        && head.granted
        && head.term < proposal.term
        && proposal
            .grid
            .window(proposal.term)
            .is_some_and(|window| head.expires_at_height <= window.start_height)
        && world
            .sorafs_authority_leases()
            .get(&head.plan_id)
            .is_some_and(|lease| {
                lease.terminal_approval.is_some()
                    && lease.activation.is_some()
                    && lease.completion.is_none()
            })
    {
        return (head.revision.saturating_add(1), false);
    }
    (head.revision, head.granted)
}

/// Derive an observed head without requiring mutable eligibility to remain true.
/// Missing accounts or changed policies therefore supersede certificates instead of panicking.
pub(crate) fn expected_head(
    proposal: &SorafsAuthorityProposalV1,
    world: &impl WorldReadOnly,
) -> GovernanceExpectedHeadV1 {
    let subject_id = proposal.governed_subject_id_v1();
    let (revision, granted) = effective_predecessor(proposal, world);
    let account_present = world.account(&proposal.account).is_ok();
    let (owner, policy) = match (proposal.authority, proposal.action) {
        (
            SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id },
            SorafsAuthorityActionV1::Grant | SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
        ) => (
            world.provider_owners().get(&provider_id).cloned(),
            world
                .provider_ingest_completion_authorities()
                .get(&provider_id)
                .cloned(),
        ),
        _ => (None, None),
    };
    let head_root =
        parliament_expected_head_root_v1(&(revision, granted, account_present, owner, policy));
    GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
        subject_id,
        version: revision.max(1),
        head_root,
    })
}

/// Reconstruct the only predecessor that valid admission can capture for this content.
/// Unlike an observed head, this is independent of current mutable eligibility.
pub(crate) fn admitted_expected_head(
    proposal: &SorafsAuthorityProposalV1,
) -> GovernanceExpectedHeadV1 {
    let policy = match (proposal.authority, proposal.action) {
        (
            SorafsOperatingAuthorityV1::CompleteReplicationOrder { .. },
            SorafsAuthorityActionV1::Grant | SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
        ) => proposal.completion_policy.clone(),
        _ => None,
    };
    let owner = policy.as_ref().map(|policy| policy.provider_owner.clone());
    GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
        subject_id: proposal.governed_subject_id_v1(),
        version: (proposal.expected_revision
            + u64::from(proposal.action == SorafsAuthorityActionV1::Revoke))
        .max(1),
        head_root: parliament_expected_head_root_v1(&(
            proposal.expected_revision
                + u64::from(proposal.action == SorafsAuthorityActionV1::Revoke),
            proposal.action == SorafsAuthorityActionV1::Revoke,
            true,
            owner,
            policy,
        )),
    })
}

/// Validate new proposal admission or an exact effect immediately before applying it.
pub(crate) fn validate_proposal(
    proposal: &SorafsAuthorityProposalV1,
    tx: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    require_active(&tx.world)?;
    if let Some(message) = proposal.invariant_error() {
        return Err(invalid(message));
    }
    if &proposal.network != tx.network_id() {
        return Err(invalid("SoraFS authority proposal network mismatch"));
    }
    let window = proposal
        .grid
        .window(proposal.term)
        .ok_or_else(|| invalid("invalid authority term"))?;
    let epoch_length = match tx.world.sumeragi_npos_parameters() {
        Ok(Some(policy)) => policy.epoch_length_blocks.get(),
        Ok(None) => return Err(invalid("authority terms require committed NPoS cadence")),
        Err(ExecutionAttemptError::Rejected(reason)) => return Err(invalid(reason)),
        Err(ExecutionAttemptError::Deferred(original)) => {
            tx.world.record_execution_deferral(original);
            return Err(invalid("authority cadence read is locally deferred"));
        }
    };
    if proposal.grid.epoch_length_blocks != epoch_length {
        return Err(invalid(
            "authority term grid differs from committed native cadence",
        ));
    }
    let height = tx.block_height();
    if proposal.action == SorafsAuthorityActionV1::Revoke {
        let lease = tx
            .world
            .sorafs_authority_leases()
            .get(&proposal.plan_id())
            .ok_or_else(|| invalid("early revocation must name its exact active lease"))?;
        if lease.activation.is_none()
            || lease.completion.is_some()
            || height < window.start_height
            || height >= window.end_height
        {
            return Err(invalid("early revocation is outside its active term"));
        }
    } else if height < window.admission_height || height >= window.cutoff_height {
        return Err(invalid(
            "authority approval is outside the canonical admission window",
        ));
    }
    validate_eligibility(proposal, tx, false)?;
    if proposal.authority == SorafsOperatingAuthorityV1::GatewayComplianceOperator
        && tx.world.roles().get(&*COMPLIANCE_ROLE).is_some_and(|role| {
            role.permissions().next().is_some() || !role.permission_epochs().is_empty()
        })
    {
        return Err(invalid(
            "reserved compliance role must have no permissions or permission epochs",
        ));
    }
    Ok(())
}

fn validate_eligibility(
    proposal: &SorafsAuthorityProposalV1,
    tx: &StateTransaction<'_, '_>,
    starting: bool,
) -> Result<(), InstructionExecutionError> {
    tx.world.account(&proposal.account)?;
    let head = tx
        .world
        .sorafs_authority_heads()
        .get(&proposal.governed_subject_id_v1());
    let projected_grant = head.is_some_and(|head| head.granted);
    if projected_grant != projected(&tx.world, proposal.authority, &proposal.account) {
        return Err(invalid(
            "SoraFS authority projection differs from its retained head",
        ));
    }
    let (revision, granted) = if starting {
        (head.map_or(0, |h| h.revision), projected_grant)
    } else {
        effective_predecessor(proposal, &tx.world)
    };
    let revoke = proposal.action == SorafsAuthorityActionV1::Revoke;
    if revision != proposal.expected_revision + u64::from(revoke) || granted != revoke {
        return Err(invalid(
            "authority plan predecessor is stale or already granted",
        ));
    }
    if revoke && head.is_none_or(|head| head.plan_id != proposal.plan_id()) {
        return Err(invalid(
            "early revocation names another authority incarnation",
        ));
    }
    if !revoke
        && tx.world.sorafs_authority_leases().iter().any(|(_, lease)| {
            lease.plan.family_id() == proposal.family_id() && lease.activation.is_some()
        })
    {
        return Err(invalid(
            "an authority family permits only one grant per term",
        ));
    }
    if !revoke && let Some(policy) = &proposal.completion_policy {
        let SorafsOperatingAuthorityV1::CompleteReplicationOrder { provider_id } =
            proposal.authority
        else {
            return Err(invalid("unexpected completion policy"));
        };
        if tx.world.provider_owners().get(&provider_id) != Some(&policy.provider_owner)
            || tx
                .world
                .provider_ingest_completion_authorities()
                .get(&provider_id)
                != Some(policy)
        {
            return Err(invalid(
                "completion approval differs from current owner-approved signer policy",
            ));
        }
    }
    Ok(())
}

fn validate_start(
    proposal: &SorafsAuthorityProposalV1,
    tx: &StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    require_active(&tx.world)?;
    // The plan is admitted work, but installing authority adds exposure. It may
    // continue under admission closure, never under completion-only containment.
    let instruction = iroha_data_model::isi::sorafs_authority::ProposeSorafsAuthorityGovernanceV1 {
        proposal: proposal.clone(),
    }
    .into();
    crate::runtime::resolve(
        &instruction,
        tx.world.runtime_selection(),
        tx.block_height(),
        || Some(crate::runtime::obligations::Step::Continue),
    )?
    .ok_or_else(|| invalid("authority start has no selected implementation"))?;
    validate_eligibility(proposal, tx, true)
}

/// Apply a retained, exact, due certificate through the exclusive native projection owner.
/// The caller owns the isolated State transaction and its failure rollback boundary.
pub(crate) fn apply_certified_effect(
    record: &SorafsAuthorityProposalRecordV1,
    certificate: &GovernanceCertificateV1,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let proposal = &record.proposal;
    let id = proposal.fingerprint();
    let attempt = tx
        .world
        .parliament_attempts()
        .get(&certificate.governance_attempt_id)
        .ok_or_else(|| invalid("SoraFS effect has no retained certified attempt"))?;
    certificate
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    attempt
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    if attempt.attempt().status != GovernanceAttemptStatusV1::Certified
        || attempt.certificate() != Some(certificate)
        || tx.world.sorafs_authority_proposals().get(&id) != Some(record)
        || certificate.proposal_content_id != proposal.proposal_content_id()
        || certificate.effect_preimage_hash != proposal.effect_preimage_hash_v1()
        || certificate.enact_at_height != tx.block_height()
        || certificate.expected_head != expected_head(proposal, &tx.world)
        || certificate.expected_head != admitted_expected_head(proposal)
    {
        return Err(invalid(
            "SoraFS effect differs from its exact retained due certificate",
        ));
    }
    validate_proposal(proposal, tx)?;
    match proposal.action {
        SorafsAuthorityActionV1::Grant | SorafsAuthorityActionV1::AuthorizeTerminalRevoke => {
            terms::approve(proposal, certificate, tx)
        }
        SorafsAuthorityActionV1::Revoke => terms::early_revoke(proposal, certificate, tx),
    }
}

fn project(
    proposal: &SorafsAuthorityProposalV1,
    proposer: &AccountId,
    granted: bool,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if let Some(token) = permission(proposal.authority) {
        if granted {
            tx.world
                .add_account_permission(&proposal.account, token.clone());
            tx.world.emit_events(Some(AccountEvent::PermissionAdded(
                AccountPermissionChanged {
                    account: proposal.account.clone(),
                    permission: token,
                },
            )));
        } else {
            if !tx
                .world
                .remove_account_permission(&proposal.account, &token)
            {
                return Err(invalid("SoraFS authority direct projection disappeared"));
            }
            tx.world.emit_events(Some(AccountEvent::PermissionRemoved(
                AccountPermissionChanged {
                    account: proposal.account.clone(),
                    permission: token,
                },
            )));
        }
    } else {
        if granted && tx.world.roles.get(&*COMPLIANCE_ROLE).is_none() {
            let role = Role::new(COMPLIANCE_ROLE.clone(), proposal.account.clone()).build(proposer);
            tx.world.roles.insert(COMPLIANCE_ROLE.clone(), role.clone());
            tx.world.emit_events(Some(RoleEvent::Created(role)));
        }
        let key = RoleIdWithOwner::new(proposal.account.clone(), COMPLIANCE_ROLE.clone());
        if granted {
            if tx.world.account_roles.insert(key, ()).is_some() {
                return Err(invalid("SoraFS compliance membership already exists"));
            }
            tx.world
                .emit_events(Some(AccountEvent::RoleGranted(AccountRoleChanged {
                    account: proposal.account.clone(),
                    role: COMPLIANCE_ROLE.clone(),
                })));
        } else {
            if tx.world.account_roles.remove(key).is_none() {
                return Err(invalid("SoraFS compliance membership disappeared"));
            }
            tx.world
                .emit_events(Some(AccountEvent::RoleRevoked(AccountRoleChanged {
                    account: proposal.account.clone(),
                    role: COMPLIANCE_ROLE.clone(),
                })));
        }
    }
    tx.invalidate_permission_cache_for_account(&proposal.account);
    Ok(())
}

/// Advance captured term capabilities before ordinary authority use.
pub(crate) fn advance_terms(
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    terms::advance(tx)
}

/// Aggregate one role's retained proposal-wide counters across every corrected plan.
/// The initial unit is reserved when content starts its first attempt; transport
/// retries and newer attempts of the same content do not reserve it again.
pub(crate) fn family_randomness_units(
    world: &impl WorldReadOnly,
    proposal: &SorafsAuthorityProposalV1,
    replacement: Option<&crate::governance::parliament::ParliamentAttemptStateV1>,
) -> Result<u32, crate::governance::parliament::ParliamentReducerErrorV1> {
    use crate::governance::parliament::{
        ParliamentReducerErrorV1, canonical_governance_attempt_ids_v1,
    };
    let mut used = 0_u32;
    for (_, record) in world
        .sorafs_authority_proposals()
        .iter()
        .filter(|(_, record)| {
            record.proposal.family_id() == proposal.family_id()
                && record.proposal.action == proposal.action
        })
    {
        let latest = canonical_governance_attempt_ids_v1(record.proposal.proposal_content_id())
            .rev()
            .find_map(|id| {
                replacement
                    .filter(|attempt| attempt.attempt().id == id)
                    .or_else(|| world.parliament_attempts().get(&id))
            });
        if let Some(attempt) = latest {
            used = used
                .checked_add(
                    attempt
                        .randomness_redraws_used_v1()?
                        .checked_add(1)
                        .ok_or(ParliamentReducerErrorV1::RandomnessRedrawLimitExceeded)?,
                )
                .ok_or(ParliamentReducerErrorV1::RandomnessRedrawLimitExceeded)?;
        }
    }
    if used > iroha_data_model::isi::sorafs_authority::SORAFS_AUTHORITY_ROLE_RANDOMNESS_UNITS_V1 {
        return Err(ParliamentReducerErrorV1::RandomnessRedrawLimitExceeded);
    }
    Ok(used)
}

pub(crate) fn validate_attempt_budget(
    world: &impl WorldReadOnly,
    attempt: &crate::governance::parliament::ParliamentAttemptStateV1,
) -> Result<(), crate::governance::parliament::ParliamentReducerErrorV1> {
    if let Some(record) = world
        .sorafs_authority_proposals()
        .get(attempt.proposal_content_id().as_bytes())
    {
        family_randomness_units(world, &record.proposal, Some(attempt))?;
    }
    Ok(())
}

// Every successful change retains its proposal and attempt. Reconstruct the revision order
// independently of the head table so deleting a revoked tombstone cannot reset a subject.
fn validate_successful_history(
    world: &impl WorldReadOnly,
    budget: &AllocationBudget,
) -> Result<(), ExecutionAttemptError<String>> {
    type HistoryEntry = ([u8; 32], u64, [u8; 32], bool, u64);
    let count = world
        .sorafs_authority_leases()
        .iter()
        .try_fold(0_usize, |count, (_, lease)| {
            count.checked_add(
                usize::from(lease.activation.is_some()) + usize::from(lease.completion.is_some()),
            )
        })
        .ok_or_else(|| {
            ExecutionAttemptError::Deferred(
                iroha_allocation::AllocationRefusal::DemandOverflow.into(),
            )
        })?;
    let mut histories = ChargedBuffer::<HistoryEntry>::new(count, budget).map_err(|original| {
        ExecutionAttemptError::Deferred(match original {
            ChargedBufferError::Admission(original) => original.into(),
            original @ ChargedBufferError::Allocator { .. } => AllocatorRefusal::try_from(original)
                .expect("matched original allocator refusal")
                .into(),
        })
    })?;
    for (id, lease) in world.sorafs_authority_leases().iter() {
        terms::validate_lease(world, *id, lease)?;
        for (execution, granted) in [(&lease.activation, true), (&lease.completion, false)] {
            if let Some(execution) = execution {
                histories.push_reserved((
                    lease.plan.governed_subject_id_v1(),
                    execution.revision,
                    *id,
                    granted,
                    execution.at_height,
                ));
            }
        }
    }
    histories
        .as_mut_slice()
        .sort_unstable_by_key(|entry| (entry.0, entry.1));
    for history in histories
        .as_slice()
        .chunk_by(|left, right| left.0 == right.0)
    {
        let subject = history[0].0;
        let head = world
            .sorafs_authority_heads()
            .get(&subject)
            .ok_or("successful SoraFS authority history has no retained head")?;
        let mut preceding_revision = 0_u64;
        let mut preceding_height = 0_u64;
        let mut granted = false;
        for (_, revision, id, next_granted, height) in history {
            if preceding_revision.checked_add(1) != Some(*revision)
                || *next_granted == granted
                || *height < preceding_height
            {
                return Err(
                    "successful SoraFS authority history is not contiguous alternating changes"
                        .into(),
                );
            }
            preceding_revision = *revision;
            preceding_height = *height;
            granted = *next_granted;
            if *revision == head.revision && *id != head.plan_id {
                return Err("SoraFS authority head refers to the wrong successful revision".into());
            }
        }
        if head.revision != preceding_revision || head.granted != granted {
            return Err("SoraFS authority head is not the latest successful decision".into());
        }
    }
    Ok(())
}

/// Verify every restored head, receipt and native permission/role projection in both directions.
pub(crate) fn validate_projection(
    world: &impl WorldReadOnly,
    network: &iroha_data_model::NetworkId,
    budget: &AllocationBudget,
) -> Result<(), ExecutionAttemptError<String>> {
    validate_successful_history(world, budget)?;
    let selected = active(world).map_err(|e| e.to_string())?;
    if selected && !executor_declares_authorities(world) {
        return Err(
            "SoraFS authority projection lacks exact executor permission declarations".into(),
        );
    }
    // Retirement may retain historical receipts, but cannot strand a vote or granted head.
    // The unselected baseline retains its original ordinary delegation semantics.
    if !selected
        && (world
            .sorafs_authority_heads()
            .iter()
            .any(|(_, head)| head.granted)
            || world
                .sorafs_authority_proposals()
                .iter()
                .any(|(_, record)| {
                    record.status == GovernanceProposalStatus::Proposed
                        && record.deadline_closure.is_none()
                })
            || world
                .sorafs_authority_leases()
                .iter()
                .any(|(_, lease)| !lease.start_processed))
    {
        return Err("inactive SoraFS authority host retains a live obligation".into());
    }
    for (id, head) in world.sorafs_authority_heads().iter() {
        let lease = world
            .sorafs_authority_leases()
            .get(&head.plan_id)
            .ok_or("authority head has no retained lease")?;
        let execution = lease
            .completion
            .as_ref()
            .or(lease.activation.as_ref())
            .ok_or("authority head has no actual execution")?;
        let window = lease
            .plan
            .grid
            .window(lease.plan.term)
            .ok_or("invalid authority term")?;
        if lease.plan.network != *network
            || *id != lease.plan.governed_subject_id_v1()
            || head.account != lease.plan.account
            || head.authority != lease.plan.authority
            || head.term != lease.plan.term
            || head.expires_at_height != window.end_height
            || head.revision != execution.revision
            || head.enacted_at_height != execution.at_height
            || head.proposal_content_id != execution.approval.proposal_content_id
            || head.governance_attempt_id != execution.approval.governance_attempt_id
            || head.certificate_id != execution.approval.certificate_id
            || head.granted != lease.completion.is_none()
            || (selected && head.granted != projected(world, head.authority, &head.account))
            || (head.granted && world.account(&head.account).is_err())
        {
            return Err("authority head, lease receipt and projection disagree".into());
        }
    }
    for (_, record) in world
        .sorafs_authority_proposals()
        .iter()
        .filter(|(_, record)| record.status == GovernanceProposalStatus::Enacted)
    {
        let lease = world
            .sorafs_authority_leases()
            .get(&record.proposal.plan_id())
            .ok_or("enacted authority decision lost its lease receipt")?;
        let approval = match record.proposal.action {
            SorafsAuthorityActionV1::Grant => lease.grant_approval.as_ref(),
            SorafsAuthorityActionV1::AuthorizeTerminalRevoke => lease.terminal_approval.as_ref(),
            SorafsAuthorityActionV1::Revoke => lease.completion.as_ref().map(|e| &e.approval),
        };
        if approval.is_none_or(|a| a.proposal_content_id != record.proposal.proposal_content_id()) {
            return Err("enacted authority decision has no exact retained receipt".into());
        }
    }
    if !selected {
        return Ok(());
    }
    for (_, role) in world.roles().iter() {
        if role.permissions().any(protected_permission)
            || (role.id() == &*COMPLIANCE_ROLE
                && (role.permissions().next().is_some() || !role.permission_epochs().is_empty()))
        {
            return Err("SoraFS operating authority escaped into a role".into());
        }
    }
    for (account, tokens) in world.account_permissions().iter() {
        for token in tokens.iter().filter(|token| protected_permission(token)) {
            let authority = if let Ok(token) = CanCompleteSorafsReplicationOrder::try_from(token) {
                SorafsOperatingAuthorityV1::CompleteReplicationOrder {
                    provider_id: token.provider_id,
                }
            } else if token == &Permission::from(CanSetSorafsReservePolicy) {
                SorafsOperatingAuthorityV1::SetReservePolicy
            } else if token == &Permission::from(CanUpsertSorafsProviderCredit) {
                SorafsOperatingAuthorityV1::UpsertProviderCredit
            } else {
                return Err("malformed protected SoraFS authority token".into());
            };
            let id = SorafsAuthorityProposalV1::subject_id(
                network,
                iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
                authority,
                account,
            );
            if !world
                .sorafs_authority_heads()
                .get(&id)
                .is_some_and(|head| head.granted)
            {
                return Err("SoraFS direct authority has no active voted head".into());
            }
        }
    }
    for (membership, ()) in world.account_roles().iter() {
        if membership.id == *COMPLIANCE_ROLE {
            let id = SorafsAuthorityProposalV1::subject_id(
                network,
                iroha_data_model::runtime::lifecycle::RootScopeV1::Universal,
                SorafsOperatingAuthorityV1::GatewayComplianceOperator,
                &membership.account,
            );
            if world.roles().get(&*COMPLIANCE_ROLE).is_none()
                || !world
                    .sorafs_authority_heads()
                    .get(&id)
                    .is_some_and(|head| head.granted)
            {
                return Err("SoraFS compliance membership has no active voted head".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::terms::{SorafsAuthorityApprovalV1, SorafsAuthorityExecutionV1};
    use super::*;
    use iroha_model_base::metadata::Metadata;
    use crate::{
        executor::Executor,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        block::BlockHeader, runtime::lifecycle::RootScopeV1, sorafs::capacity::ProviderId,
    };
    use iroha_primitives::json::Json;
    use std::num::NonZeroU64;

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    }

    fn state() -> State {
        State::new_for_testing(
            World::with(
                [],
                [
                    Account::new(account(1)).build(&account(1)),
                    Account::new(account(2)).build(&account(1)),
                ],
                [],
            ),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn header() -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            1_700_000_000_000,
            0,
        )
    }

    fn activate(tx: &mut StateTransaction<'_, '_>) {
        crate::governance::parliament::seed_npos_sortition_cadence_for_tests(tx, 100);
        validate_activation_inventory(&tx.world).unwrap();
        let change = crate::runtime::draft_addition(
            &tx.world,
            *tx.network_id(),
            "iroha.sorafs.authority_governance.v1",
            2,
            [1; 32],
        )
        .unwrap();
        let applied = change
            .apply(
                tx.world.runtime_selection(),
                &crate::runtime::change_set::PublishedDefinitions(tx.world.runtime_definitions()),
            )
            .unwrap();
        for (id, definition) in applied.published {
            tx.world.runtime_definitions.insert(id, definition);
        }
        *tx.world.runtime_selection.get_mut() = applied.selection;
        assert!(active(&tx.world).unwrap());
    }

    fn proposal(tx: &StateTransaction<'_, '_>) -> SorafsAuthorityProposalV1 {
        SorafsAuthorityProposalV1 {
            grid: iroha_data_model::isi::sorafs_authority::SorafsAuthorityTermGridV1 {
                epoch_length_blocks: 100,
            },
            term: 1,
            network: *tx.network_id(),
            root: RootScopeV1::Universal,
            account: account(1),
            authority: SorafsOperatingAuthorityV1::SetReservePolicy,
            action: SorafsAuthorityActionV1::Grant,
            expected_revision: 0,
            completion_policy: None,
        }
    }

    fn head(
        proposal: &SorafsAuthorityProposalV1,
        revision: u64,
        granted: bool,
    ) -> SorafsAuthorityHeadV1 {
        SorafsAuthorityHeadV1 {
            plan_id: proposal.plan_id(),
            term: proposal.term,
            expires_at_height: proposal.grid.window(proposal.term).unwrap().end_height,
            authority: proposal.authority,
            account: proposal.account.clone(),
            revision,
            granted,
            proposal_content_id: proposal.proposal_content_id(),
            governance_attempt_id: GovernanceAttemptId::derive_v1(
                proposal.proposal_content_id(),
                0,
            ),
            certificate_id: GovernanceCertificateId::new([1; 32]),
            enacted_at_height: 2,
        }
    }

    #[test]
    fn existing_selection_retains_delegation_but_selected_owner_blocks_native_and_executor_paths() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        let token: Permission = CanSetSorafsReservePolicy.into();
        assert!(!active(&tx.world).unwrap());
        guard_permission_mutation(&tx.world, &token).unwrap();
        activate(&mut tx);
        tx.world.add_account_permission(&account(1), token.clone());
        let before = tx.world.account_permissions().get(&account(1)).cloned();
        for instruction in [
            Grant::account_permission(token.clone(), account(2)).into(),
            Revoke::account_permission(token.clone(), account(1)).into(),
        ] {
            assert!(
                Executor::Initial
                    .execute_instruction(&mut tx, &account(1), &instruction)
                    .is_err()
            );
        }
        assert!(
            Grant::account_permission(token.clone(), account(2))
                .execute(&account(1), &mut tx)
                .is_err()
        );
        assert!(
            Revoke::account_permission(token.clone(), account(1))
                .execute(&account(1), &mut tx)
                .is_err()
        );
        assert_eq!(
            tx.world.account_permissions().get(&account(1)),
            before.as_ref()
        );
        assert!(
            !tx.world
                .account_contains_inherent_permission(&account(2), &token)
        );
        assert!(guard_account_removal(&tx.world, &account(1)).is_err());
        guard_account_removal(&tx.world, &account(2)).unwrap();
        for name in [
            "CanCompleteSorafsReplicationOrder",
            "CanUpsertSorafsProviderCredit",
        ] {
            let malformed = Permission::new(name.to_owned(), iroha_primitives::json::Json::new(()));
            assert!(guard_permission_mutation(&tx.world, &malformed).is_err());
        }
    }

    // Actual executor dispatch needs immutable root metadata and its genuine
    // genesis carrier, unlike the direct native-handler component fixtures.
    fn custom_executor_chain() -> crate::sumeragi::test_chain::CertifiedTestChain {
        use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
        CertifiedTestChain::start(TestChainConfig::new(
            World::with(
                [],
                [
                    Account::new(account(1)).build(&account(1)),
                    Account::new(account(2)).build(&account(1)),
                ],
                [],
            ),
            1_000,
        ))
        .unwrap()
    }

    #[test]
    fn approving_custom_executor_cannot_delegate_or_remove_protected_permissions() {
        let chain = custom_executor_chain();
        let state = chain.state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        let executor = crate::executor::allowing_executor_for_testing();
        assert!(matches!(executor, Executor::UserProvided(_)));
        // This real IVM executor approves its input. A harmless native grant
        // proves the test reaches application after that verdict.
        let ordinary = Permission::new("UnprotectedTestPermission".into(), Json::new(()));
        executor
            .execute_instruction(
                &mut tx,
                &account(1),
                &Grant::account_permission(ordinary.clone(), account(2)).into(),
            )
            .unwrap();
        assert!(
            tx.world
                .account_contains_inherent_permission(&account(2), &ordinary)
        );
        for token in [
            Permission::from(CanCompleteSorafsReplicationOrder {
                provider_id: ProviderId::new([0x42; 32]),
            }),
            Permission::from(CanSetSorafsReservePolicy),
            Permission::from(CanUpsertSorafsProviderCredit),
            Permission::new("CanCompleteSorafsReplicationOrder".into(), Json::new(())),
        ] {
            // Seed only the component's projection, not a governance receipt.
            tx.world.add_account_permission(&account(1), token.clone());
            let before = tx.world.account_permissions().get(&account(1)).cloned();
            for instruction in [
                Grant::account_permission(token.clone(), account(2)).into(),
                Revoke::account_permission(token.clone(), account(1)).into(),
            ] {
                let error = executor
                    .execute_instruction(&mut tx, &account(1), &instruction)
                    .unwrap_err();
                assert!(matches!(error,
                    ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(message)
                    )) if message.contains("SoraFS operating authority changes require a Parliament certificate")
                ));
                assert_eq!(
                    tx.world.account_permissions().get(&account(1)),
                    before.as_ref()
                );
                assert!(
                    !tx.world
                        .account_contains_inherent_permission(&account(2), &token)
                );
            }
        }
        let error = executor
            .execute_instruction(
                &mut tx,
                &account(1),
                &Unregister::account(account(1)).into(),
            )
            .unwrap_err();
        assert!(matches!(error,
            ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            )) if message.contains("revoked before account removal or rekeying")
        ));
        assert!(tx.world.account(&account(1)).is_ok());
    }

    #[test]
    fn approving_custom_executor_cannot_create_edit_or_delegate_authority_roles() {
        let chain = custom_executor_chain();
        let state = chain.state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        let executor = crate::executor::allowing_executor_for_testing();
        let ordinary_role: RoleId = "custom_executor_role".parse().unwrap();
        executor
            .execute_instruction(
                &mut tx,
                &account(1),
                &Register::role(Role::new(ordinary_role.clone(), account(1))).into(),
            )
            .unwrap();
        // Retain the native compliance projection as a component fixture.
        let reserved = Role::new(COMPLIANCE_ROLE.clone(), account(1)).build(&account(1));
        tx.world
            .roles
            .insert(COMPLIANCE_ROLE.clone(), reserved.clone());
        tx.world.account_roles.insert(
            RoleIdWithOwner::new(account(1), COMPLIANCE_ROLE.clone()),
            (),
        );
        let ordinary_before = tx.world.roles().get(&ordinary_role).cloned();
        let protected_role: RoleId = "protected_custom_role".parse().unwrap();
        for instruction in [
            Register::role(
                Role::new(protected_role.clone(), account(1))
                    .add_permission(CanSetSorafsReservePolicy),
            )
            .into(),
            Register::role(Role::new(COMPLIANCE_ROLE.clone(), account(2))).into(),
            Grant::role_permission(
                Permission::from(CanUpsertSorafsProviderCredit),
                ordinary_role.clone(),
            )
            .into(),
            Grant::account_role(COMPLIANCE_ROLE.clone(), account(2)).into(),
            Revoke::account_role(COMPLIANCE_ROLE.clone(), account(1)).into(),
            Grant::role_permission(
                Permission::from(CanSetSorafsReservePolicy),
                COMPLIANCE_ROLE.clone(),
            )
            .into(),
            Revoke::role_permission(
                Permission::from(CanSetSorafsReservePolicy),
                COMPLIANCE_ROLE.clone(),
            )
            .into(),
            Unregister::role(COMPLIANCE_ROLE.clone()).into(),
        ] {
            let error = executor
                .execute_instruction(&mut tx, &account(1), &instruction)
                .unwrap_err();
            assert!(matches!(error,
                ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                )) if message.starts_with("SoraFS operating authority")
            ));
            assert_eq!(
                tx.world.roles().get(&ordinary_role),
                ordinary_before.as_ref()
            );
            assert_eq!(tx.world.roles().get(&*COMPLIANCE_ROLE), Some(&reserved));
            assert!(tx.world.roles().get(&protected_role).is_none());
            assert!(
                tx.world
                    .account_roles()
                    .get(&RoleIdWithOwner::new(account(1), COMPLIANCE_ROLE.clone()))
                    .is_some()
            );
            assert!(
                tx.world
                    .account_roles()
                    .get(&RoleIdWithOwner::new(account(2), COMPLIANCE_ROLE.clone()))
                    .is_none()
            );
        }
    }

    #[test]
    fn reserved_role_and_inherited_tokens_cannot_delegate_or_change() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        assert!(
            Register::role(Role::new(COMPLIANCE_ROLE.clone(), account(1)))
                .execute(&account(1), &mut tx)
                .is_err()
        );
        let role = Role::new(COMPLIANCE_ROLE.clone(), account(1)).build(&account(1));
        tx.world.roles.insert(COMPLIANCE_ROLE.clone(), role);
        tx.world.account_roles.insert(
            RoleIdWithOwner::new(account(1), COMPLIANCE_ROLE.clone()),
            (),
        );
        for instruction in [
            Grant::account_role(COMPLIANCE_ROLE.clone(), account(2)).into(),
            Revoke::account_role(COMPLIANCE_ROLE.clone(), account(1)).into(),
            Grant::role_permission(
                Permission::from(CanSetSorafsReservePolicy),
                COMPLIANCE_ROLE.clone(),
            )
            .into(),
            Unregister::role(COMPLIANCE_ROLE.clone()).into(),
        ] {
            assert!(
                Executor::Initial
                    .execute_instruction(&mut tx, &account(1), &instruction)
                    .is_err()
            );
        }
        assert!(
            Grant::account_role(COMPLIANCE_ROLE.clone(), account(2))
                .execute(&account(1), &mut tx)
                .is_err()
        );
        assert!(
            Revoke::account_role(COMPLIANCE_ROLE.clone(), account(1))
                .execute(&account(1), &mut tx)
                .is_err()
        );
        assert!(guard_account_removal(&tx.world, &account(1)).is_err());
        let other: RoleId = "otherwise_ordinary".parse().unwrap();
        let role = Role::new(other.clone(), account(1))
            .add_permission(CanUpsertSorafsProviderCredit)
            .build(&account(1));
        tx.world.roles.insert(other.clone(), role);
        assert!(guard_role_mutation(&tx.world, &other).is_err());
    }

    #[test]
    fn delegated_trigger_cannot_exercise_selected_holder_credit_authority() {
        use iroha_data_model::{
            events::execute_trigger::ExecuteTriggerEventFilter,
            isi::sorafs::UpsertProviderCredit,
            sorafs::pricing::ProviderCreditRecord,
            trigger::{
                Trigger,
                action::{Action, Repeats},
            },
        };
        use iroha_executor_data_model::permission::trigger::{
            CanExecuteTrigger, CanRegisterTrigger,
        };
        use iroha_primitives::numeric::Quantity;

        for (selected, trigger_predates_activation) in [(false, false), (true, false), (true, true)]
        {
            let chain = custom_executor_chain();
            let state = chain.state();
            let mut block = state.block(header());
            let mut tx = block.transaction_for_callback_testing();
            let holder = account(1);
            let caller = account(2);
            let provider = ProviderId::new([0x73; 32]);
            let trigger_id: TriggerId = "delegated_sorafs_credit".parse().unwrap();
            if selected && !trigger_predates_activation {
                activate(&mut tx);
            }
            // These are genuine ordinary capability grants and registration, including
            // a capability and trigger retained from before the selected service began.
            Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &holder,
                    &Grant::account_permission(
                        CanRegisterTrigger {
                            authority: holder.clone(),
                        },
                        caller.clone(),
                    )
                    .into(),
                )
                .unwrap();
            let credit = ProviderCreditRecord::new(
                provider,
                Quantity::from(2_u32),
                Quantity::from(1_u32),
                Quantity::zero(),
                Quantity::zero(),
                1,
                1,
                Metadata::default(),
            );
            let effect = UpsertProviderCredit {
                expected_current: None,
                record: credit.clone(),
            };
            let trigger = Trigger::new(
                trigger_id.clone(),
                Action::new(
                    vec![InstructionBox::from(effect.clone())],
                    Repeats::Indefinitely,
                    holder.clone(),
                    ExecuteTriggerEventFilter::new(),
                )
                .unwrap(),
            );
            Executor::Initial
                .execute_instruction(&mut tx, &caller, &Register::trigger(trigger).into())
                .expect("ordinary trigger registration grants no operating authority");
            Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &holder,
                    &Grant::account_permission(
                        CanExecuteTrigger {
                            trigger: trigger_id.clone(),
                        },
                        caller.clone(),
                    )
                    .into(),
                )
                .unwrap();
            if selected && trigger_predates_activation {
                activate(&mut tx);
            }
            // Seed an existing authority projection and real owner-funded reserve custody.
            // The regression exercises use-time nondelegation, not Parliament admission.
            let permission: Permission = CanUpsertSorafsProviderCredit.into();
            tx.world.add_account_permission(&holder, permission.clone());
            tx.world.provider_owners.insert(provider, holder.clone());
            super::super::sorafs_reserve::seed_verified_provider_bond_for_test(
                &mut tx,
                provider,
                &holder,
                1,
                Quantity::from(1_u32),
            )
            .unwrap();
            assert!(
                !tx.world
                    .account_contains_inherent_permission(&caller, &permission)
            );
            let permissions_before = tx.world.account_permissions().get(&holder).cloned();
            let result = Executor::Initial.execute_instruction(
                &mut tx,
                &caller,
                &ExecuteTrigger::new(trigger_id).into(),
            );
            if selected {
                let error = result.expect_err("a caller may not borrow the holder's authority");
                assert!(matches!(error,
                    ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(message)
                    )) if message.contains("nondelegable SoraFS operating authority")
                ));
                assert!(tx.world.provider_credit_ledger.get(&provider).is_none());
                assert_eq!(
                    tx.world.account_permissions().get(&holder),
                    permissions_before.as_ref()
                );
                Executor::Initial
                    .execute_instruction(&mut tx, &holder, &effect.into())
                    .expect("the exact holder's direct instruction remains authorized");
            } else {
                result.expect("the predecessor's delegated trigger behavior is unchanged");
            }
            assert_eq!(
                tx.world.provider_credit_ledger.get(&provider),
                Some(&credit)
            );
            assert!(
                !tx.world
                    .account_contains_inherent_permission(&caller, &permission)
            );
        }
    }

    #[test]
    fn predecessor_holders_can_surrender_exact_tokens_but_selected_owner_refuses() {
        // Genuine genesis/root metadata reaches Initial executor authorization. Grants below
        // are only the component's existing-inventory fixture, not voted projection records.
        let chain = custom_executor_chain();
        let state = chain.state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        assert!(!active(&tx.world).unwrap());
        let ordinary: Permission =
            iroha_executor_data_model::permission::parameter::CanSetParameters.into();
        let tokens = [
            Permission::from(CanCompleteSorafsReplicationOrder {
                provider_id: ProviderId::new([0x42; 32]),
            }),
            Permission::from(CanSetSorafsReservePolicy),
            Permission::from(CanUpsertSorafsProviderCredit),
        ];
        tx.world
            .add_account_permission(&account(1), ordinary.clone());
        for token in &tokens {
            tx.world.add_account_permission(&account(1), token.clone());
        }
        assert!(validate_activation_inventory(&tx.world).is_err());
        for token in &tokens {
            let before = tx.world.account_permissions().get(&account(1)).cloned();
            let error = Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &account(2),
                    &Revoke::account_permission(token.clone(), account(1)).into(),
                )
                .unwrap_err();
            assert!(matches!(error, ValidationFail::NotPermitted(_)));
            assert_eq!(
                tx.world.account_permissions().get(&account(1)),
                before.as_ref()
            );
            Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &account(1),
                    &Revoke::account_permission(token.clone(), account(1)).into(),
                )
                .expect("exact holder may surrender under the predecessor selection");
            assert!(
                !tx.world
                    .account_contains_inherent_permission(&account(1), token)
            );
            assert!(
                tx.world
                    .account_contains_inherent_permission(&account(1), &ordinary)
            );
            assert!(
                !tx.world
                    .account_contains_inherent_permission(&account(2), token)
            );
            assert!(
                Executor::Initial
                    .execute_instruction(
                        &mut tx,
                        &account(1),
                        &Revoke::account_permission(token.clone(), account(1)).into(),
                    )
                    .is_err()
            );
        }
        validate_activation_inventory(&tx.world).unwrap();
        activate(&mut tx);
        for token in &tokens {
            // The active-owner refusal must be the native ownership guard, not an unrelated
            // executor/root-metadata failure. No ordinary self-surrender exception is added.
            tx.world.add_account_permission(&account(1), token.clone());
            let before = tx.world.account_permissions().get(&account(1)).cloned();
            let error = Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &account(1),
                    &Revoke::account_permission(token.clone(), account(1)).into(),
                )
                .unwrap_err();
            assert!(matches!(error,
                ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(message)
                )) if message.contains("SoraFS operating authority changes require a Parliament certificate")
            ));
            assert_eq!(
                tx.world.account_permissions().get(&account(1)),
                before.as_ref()
            );
        }
    }

    #[test]
    fn predecessor_role_surrender_leaves_inventory_until_authorized_explicit_cleanup() {
        let chain = custom_executor_chain();
        let state = chain.state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        let token: Permission = CanSetSorafsReservePolicy.into();
        let ordinary: Permission =
            iroha_executor_data_model::permission::parameter::CanSetParameters.into();
        let role_id: RoleId = "authority_surrender_fixture".parse().unwrap();
        let role = Role::new(role_id.clone(), account(1))
            .add_permission_with_epoch(token.clone(), 1)
            .add_permission_with_epoch(ordinary.clone(), 2)
            .build(&account(1));
        // Seed existing stored origins; no registration/grant authorization is being inferred.
        tx.world.roles.insert(role_id.clone(), role.clone());
        for member in [account(1), account(2)] {
            tx.world
                .account_roles
                .insert(RoleIdWithOwner::new(member, role_id.clone()), ());
        }
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &account(1),
                &Revoke::account_role(role_id.clone(), account(1)).into(),
            )
            .expect("existing member may surrender a role whose tokens it may revoke");
        assert!(
            tx.world
                .account_roles()
                .get(&RoleIdWithOwner::new(account(1), role_id.clone()))
                .is_none()
        );
        assert_eq!(tx.world.roles().get(&role_id), Some(&role));
        assert!(validate_activation_inventory(&tx.world).is_err());
        assert!(
            Executor::Initial
                .execute_instruction(
                    &mut tx,
                    &account(1),
                    &Revoke::role_permission(token.clone(), role_id.clone()).into(),
                )
                .is_err(),
            "former membership grants no role-editing authority"
        );
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &account(2),
                &Revoke::role_permission(token.clone(), role_id.clone()).into(),
            )
            .expect("remaining exact member may explicitly remove a contained token");
        let remaining = tx.world.roles().get(&role_id).unwrap();
        assert!(
            !remaining
                .permissions()
                .any(|permission| permission == &token)
        );
        assert_eq!(remaining.permission_epoch(&token), None);
        assert_eq!(remaining.permission_epoch(&ordinary), Some(2));
        assert!(
            tx.world
                .account_roles()
                .get(&RoleIdWithOwner::new(account(2), role_id))
                .is_some()
        );
        validate_activation_inventory(&tx.world).unwrap();

        let reserved = Role::new(COMPLIANCE_ROLE.clone(), account(2)).build(&account(2));
        tx.world
            .roles
            .insert(COMPLIANCE_ROLE.clone(), reserved.clone());
        tx.world.account_roles.insert(
            RoleIdWithOwner::new(account(2), COMPLIANCE_ROLE.clone()),
            (),
        );
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &account(2),
                &Revoke::account_role(COMPLIANCE_ROLE.clone(), account(2)).into(),
            )
            .unwrap();
        assert_eq!(tx.world.roles().get(&*COMPLIANCE_ROLE), Some(&reserved));
        assert!(validate_activation_inventory(&tx.world).is_err());
        let error = Executor::Initial
            .execute_instruction(
                &mut tx,
                &account(2),
                &Unregister::role(COMPLIANCE_ROLE.clone()).into(),
            )
            .unwrap_err();
        assert!(
            matches!(error, ValidationFail::NotPermitted(message) if message.contains("Can't unregister role"))
        );
        assert_eq!(tx.world.roles().get(&*COMPLIANCE_ROLE), Some(&reserved));
        // Existing manager authority is fixture input, not a post-genesis bootstrap mechanism.
        tx.world.add_account_permission(
            &account(1),
            iroha_executor_data_model::permission::role::CanManageRoles.into(),
        );
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &account(1),
                &Unregister::role(COMPLIANCE_ROLE.clone()).into(),
            )
            .expect("only an already-authorized role manager performs explicit deletion");
        assert!(tx.world.roles().get(&*COMPLIANCE_ROLE).is_none());
        validate_activation_inventory(&tx.world).unwrap();
    }

    #[test]
    fn activation_refuses_direct_role_and_reserved_name_inventory() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        validate_activation_inventory(&tx.world).unwrap();
        let token: Permission = CanUpsertSorafsProviderCredit.into();
        tx.world.add_account_permission(&account(1), token.clone());
        assert!(validate_activation_inventory(&tx.world).is_err());
        tx.world.remove_account_permission(&account(1), &token);
        let role = Role::new(COMPLIANCE_ROLE.clone(), account(1)).build(&account(1));
        tx.world.roles.insert(COMPLIANCE_ROLE.clone(), role);
        assert!(validate_activation_inventory(&tx.world).is_err());
        tx.world.roles.remove(COMPLIANCE_ROLE.clone());
        let id: RoleId = "hidden_target_permission".parse().unwrap();
        tx.world.roles.insert(
            id.clone(),
            Role::new(id, account(1))
                .add_permission(token)
                .build(&account(1)),
        );
        assert!(validate_activation_inventory(&tx.world).is_err());
    }

    #[test]
    fn activation_requires_explicit_executor_declarations_or_the_initial_builtin_model() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        assert!(matches!(tx.world.executor.get(), Executor::Initial));
        assert!(tx.world.executor_data_model.get().permissions().is_empty());
        validate_activation_inventory(&tx.world).unwrap();
        *tx.world.executor.get_mut() = crate::executor::allowing_executor_for_testing();
        assert!(matches!(tx.world.executor.get(), Executor::UserProvided(_)));
        let error = validate_activation_inventory(&tx.world).unwrap_err();
        assert!(
            error.contains("exact executor permission declarations"),
            "{error}"
        );
        let complete = crate::executor::initial_executor_data_model_fallback();
        for omitted in [
            "CanCompleteSorafsReplicationOrder",
            "CanSetSorafsReservePolicy",
            "CanUpsertSorafsProviderCredit",
        ] {
            let mut partial = complete.clone();
            assert!(partial.permissions.remove(omitted));
            *tx.world.executor_data_model.get_mut() = partial;
            let error = validate_activation_inventory(&tx.world).unwrap_err();
            assert!(
                error.contains("exact executor permission declarations"),
                "{error}"
            );
        }
        *tx.world.executor_data_model.get_mut() = complete;
        validate_activation_inventory(&tx.world).unwrap();
    }

    #[test]
    fn selected_compliance_role_rejects_orphan_permission_epochs() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        let mut proposal = proposal(&tx);
        proposal.authority = SorafsOperatingAuthorityV1::GatewayComplianceOperator;
        let mut role = Role::new(COMPLIANCE_ROLE.clone(), account(1)).build(&account(1));
        role.permission_epochs
            .insert(CanSetSorafsReservePolicy.into(), 1);
        tx.world.roles.insert(COMPLIANCE_ROLE.clone(), role);
        assert!(validate_proposal(&proposal, &tx).is_err());
        let error = validate_projection(
            &tx.world,
            tx.network_id(),
            tx.world.sccp_committee_generations.allocation_budget(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("escaped into a role"), "{error}");
    }

    #[test]
    fn revoked_revision_never_recreates_an_old_head_and_absent_accounts_are_observable() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        let proposal = proposal(&tx);
        validate_proposal(&proposal, &tx).unwrap();
        let original = expected_head(&proposal, &tx.world);
        assert_eq!(original, admitted_expected_head(&proposal));
        tx.world
            .sorafs_authority_heads
            .insert(proposal.governed_subject_id_v1(), head(&proposal, 1, true));
        let granted = expected_head(&proposal, &tx.world);
        tx.world
            .sorafs_authority_heads
            .insert(proposal.governed_subject_id_v1(), head(&proposal, 2, false));
        let revoked = expected_head(&proposal, &tx.world);
        assert_ne!(original, granted);
        assert_ne!(original, revoked);
        assert_ne!(granted, revoked);
        assert!(validate_proposal(&proposal, &tx).is_err());
        let mut next = proposal.clone();
        next.expected_revision = 2;
        validate_proposal(&next, &tx).unwrap();
        assert_eq!(
            expected_head(&next, &tx.world),
            admitted_expected_head(&next)
        );
        tx.world.accounts.remove(account(1));
        assert_ne!(revoked, expected_head(&next, &tx.world));
        assert!(validate_proposal(&next, &tx).is_err());
    }

    #[test]
    fn last_reachable_grant_keeps_a_terminal_revision() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        let mut plan = proposal(&tx);
        plan.expected_revision = u64::MAX - 3;
        tx.world.sorafs_authority_heads.insert(
            plan.governed_subject_id_v1(),
            head(&plan, plan.expected_revision, false),
        );
        validate_proposal(&plan, &tx).unwrap();
        for action in [
            SorafsAuthorityActionV1::Grant,
            SorafsAuthorityActionV1::AuthorizeTerminalRevoke,
            SorafsAuthorityActionV1::Revoke,
        ] {
            plan.action = action;
            assert!(plan.invariant_error().is_none());
            assert_eq!(plan.expected_revision.checked_add(2), Some(u64::MAX - 1));
            plan.expected_revision = u64::MAX - 2;
            assert!(plan.invariant_error().is_some());
            plan.expected_revision = u64::MAX - 3;
        }
    }

    #[test]
    fn projection_refuses_unvoted_direct_and_inherited_authority() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        validate_projection(
            &tx.world,
            tx.network_id(),
            tx.world.sccp_committee_generations.allocation_budget(),
        )
        .unwrap();
        let token: Permission = CanSetSorafsReservePolicy.into();
        tx.world.add_account_permission(&account(1), token.clone());
        assert!(
            validate_projection(
                &tx.world,
                tx.network_id(),
                tx.world.sccp_committee_generations.allocation_budget()
            )
            .is_err()
        );
        tx.world.remove_account_permission(&account(1), &token);
        tx.world.account_roles.insert(
            RoleIdWithOwner::new(account(1), COMPLIANCE_ROLE.clone()),
            (),
        );
        assert!(
            validate_projection(
                &tx.world,
                tx.network_id(),
                tx.world.sccp_committee_generations.allocation_budget()
            )
            .is_err()
        );
    }

    #[test]
    fn family_budget_retains_exhausted_content_across_corrected_plans() {
        use crate::governance::parliament::{
            MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1, PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            ParliamentAttemptStateV1, sorafs_authority_attempt_policy_v1,
        };
        use iroha_data_model::{
            governance::types::{
                GovernanceAttemptV1, GovernanceStageV1,
                MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1,
            },
            isi::sorafs_authority::{
                CreateSorafsAuthorityParliamentAttemptV1, ProposeSorafsAuthorityGovernanceV1,
            },
        };
        use iroha_executor_data_model::permission::governance::CanManageParliament;

        let state = state();
        {
            let mut block = state.block(header());
            let mut tx = block.transaction();
            activate(&mut tx);
            tx.apply();
            block.commit_world_overlay_for_testing().unwrap();
        }
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(2_000).unwrap(),
            None,
            None,
            1_700_000_000_000,
            0,
        ));
        let mut tx = block.transaction();
        let grant = proposal(&tx);
        tx.world
            .add_account_permission(&account(2), CanManageParliament.into());
        tx.world.citizens.insert(
            account(2),
            crate::state::CitizenshipRecord {
                owner: account(2),
                amount: tx.gov.citizenship_bond_amount.clone(),
                bonded_height: 1,
            },
        );
        let revoke = grant.clone();
        let attempt = test_support::exhausted_attempt(&revoke, 10);
        assert_eq!(
            attempt.attempt().status,
            GovernanceAttemptStatusV1::Rejected
        );
        assert_eq!(
            attempt.randomness_redraws_used_v1().unwrap(),
            MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1
        );
        attempt.validate().unwrap();
        let encoded = norito::to_bytes(&attempt).unwrap();
        let restored = norito::decode_from_bytes::<ParliamentAttemptStateV1>(&encoded).unwrap();
        restored.validate().unwrap();
        tx.world.put_parliament_attempt(restored).unwrap();
        let rejected = SorafsAuthorityProposalRecordV1 {
            deadline_closure: None,
            proposer: account(1),
            proposal: revoke.clone(),
            created_height: 2,
            status: GovernanceProposalStatus::Rejected,
        };
        tx.world
            .sorafs_authority_proposals
            .insert(revoke.fingerprint(), rejected.clone());
        for (sequence, message) in [
            (1, "redraw budget exhausted"),
            (
                MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1 + 1,
                "first-release retry limit",
            ),
        ] {
            let error = CreateSorafsAuthorityParliamentAttemptV1 {
                proposal: revoke.clone(),
                attempt_sequence: sequence,
            }
            .execute(&account(2), &mut tx)
            .unwrap_err();
            assert!(
                matches!(
                    &error,
                    InstructionExecutionError::InvalidParameter(
                        iroha_data_model::isi::error::InvalidParameterError::SmartContract(reason)
                    ) if reason.contains(message)
                ),
                "{error:?}"
            );
        }
        ProposeSorafsAuthorityGovernanceV1 {
            proposal: revoke.clone(),
        }
        .execute(&account(2), &mut tx)
        .unwrap();
        assert_eq!(
            tx.world
                .sorafs_authority_proposals
                .get(&revoke.fingerprint()),
            Some(&rejected)
        );
        assert_eq!(
            family_randomness_units(&tx.world, &grant, None).unwrap(),
            17
        );
        let mut corrected = grant.clone();
        corrected.expected_revision = 2;
        assert_eq!(corrected.family_id(), grant.family_id());
        assert_ne!(corrected.plan_id(), grant.plan_id());
        let corrected_record = SorafsAuthorityProposalRecordV1 {
            deadline_closure: None,
            proposer: account(2),
            proposal: corrected.clone(),
            created_height: 3,
            status: GovernanceProposalStatus::Proposed,
        };
        tx.world
            .sorafs_authority_proposals
            .insert(corrected.fingerprint(), corrected_record);
        let (risk_tier, required) = sorafs_authority_attempt_policy_v1();
        let correction = ParliamentAttemptStateV1::try_new(
            GovernanceAttemptV1 {
                id: GovernanceAttemptId::derive_v1(corrected.proposal_content_id(), 0),
                proposal_content_id: corrected.proposal_content_id(),
                sequence: 0,
                risk_tier,
                stage: GovernanceStageV1::Qualification,
                status: GovernanceAttemptStatusV1::Active,
            },
            PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
            1,
            3,
            corrected.effect_preimage_hash_v1(),
            admitted_expected_head(&corrected),
            required,
        )
        .unwrap();
        assert!(validate_attempt_budget(&tx.world, &correction).is_err());
        assert_eq!(
            family_randomness_units(&tx.world, &grant, None).unwrap(),
            17
        );
        assert!(tx.world.sorafs_authority_heads.iter().next().is_none());
    }

    #[test]
    fn successful_history_requires_exact_lease_receipts_and_preserves_capacity_refusal() {
        let state = state();
        let mut block = state.block(header());
        let mut tx = block.transaction();
        activate(&mut tx);
        let plan = proposal(&tx);
        let fake = SorafsAuthorityApprovalV1 {
            proposal_content_id: plan.proposal_content_id(),
            governance_attempt_id: GovernanceAttemptId::derive_v1(plan.proposal_content_id(), 0),
            certificate_id: GovernanceCertificateId::new([3; 32]),
            approved_at_height: 60,
        };
        let binding = tx
            .world
            .runtime_selection()
            .bindings
            .iter()
            .find(|b| b.wire_id == "iroha.sorafs.authority.propose.v1")
            .unwrap();
        let execution = SorafsAuthorityExecutionV1 {
            revision: 1,
            at_height: plan.grid.window(plan.term).unwrap().start_height,
            approval: fake.clone(),
            services_profile: tx.world.runtime_selection().services_profile,
            implementation: binding.implementation,
        };
        tx.world.sorafs_authority_leases.insert(
            plan.plan_id(),
            SorafsAuthorityLeaseV1 {
                plan: plan.clone(),
                grant_approval: Some(fake),
                terminal_approval: None,
                activation: Some(execution),
                completion: None,
                start_processed: true,
            },
        );
        let budget = AllocationBudget::new(4096);
        let held = budget.try_reserve_bytes(4096).unwrap();
        let refused = validate_successful_history(&tx.world, &budget).unwrap_err();
        let ExecutionAttemptError::Deferred(original) = refused else {
            panic!("capacity refusal cannot become invalid history");
        };
        assert!(original.allocation_refusal().is_some());
        drop(held);
        assert!(matches!(
            validate_successful_history(&tx.world, &budget),
            Err(ExecutionAttemptError::Rejected(_))
        ));
        tx.world
            .sorafs_authority_heads
            .insert(plan.governed_subject_id_v1(), head(&plan, 1, true));
        assert!(
            validate_projection(&tx.world, tx.network_id(), &budget).is_err(),
            "a head cannot authenticate an invented certificate"
        );
        tx.world.sorafs_authority_leases.remove(plan.plan_id());
        assert!(
            validate_projection(&tx.world, tx.network_id(), &budget).is_err(),
            "deleting lease history cannot revive a head"
        );
    }
}
