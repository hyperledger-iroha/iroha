//! Shared role-11 permission, custody and phase eligibility for consensus and finality readers.

use super::{Error, OperationHeadV1};
use crate::query::stream_token_authority as journal;
use crate::{
    query::stream_token_custody::NativeControl,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::{
    account::AccountId,
    permission::Permission,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            StreamTokenAuthorityActionV1 as Action, StreamTokenCheckPhaseV1 as Phase,
            StreamTokenCheckV1, StreamTokenOutcomeV1, StreamTokenReviewedV1,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::stream_token::stream_token_binding_digest_v1;

fn has_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    permission: Permission,
) -> bool {
    world.accounts().get(authority).is_some()
        && (world.account_contains_inherent_permission(authority, &permission)
            || world
                .account_roles_iter(authority)
                .filter_map(|id| world.roles().get(id))
                .any(|role| role.permissions().any(|token| token == &permission)))
}

pub(crate) fn authorized(
    tx: &impl StateReadOnly,
    authority: &AccountId,
    provider: ProviderId,
    action: &Action,
) -> bool {
    let world = tx.world();
    let is_owner = world.provider_owners().get(&provider) == Some(authority);
    let can_operate = has_permission(
        world,
        authority,
        CanOperateSorafsStreamToken {
            provider_id: provider,
        }
        .into(),
    );
    match action {
        Action::Reserve(_) | Action::Complete(_) => is_owner && can_operate,
        Action::Expire(_) => {
            is_owner && can_operate
                || has_permission(
                    world,
                    authority,
                    CanManageSorafsStreamTokenCustody {
                        provider_id: provider,
                    }
                    .into(),
                )
        }
        Action::Check(check) => {
            authority == &check.expected_observer
                && authority != &check.expected_operator
                && world.provider_owners().get(&provider) == Some(&check.expected_operator)
                && has_permission(
                    world,
                    &check.expected_operator,
                    CanOperateSorafsStreamToken {
                        provider_id: provider,
                    }
                    .into(),
                )
                && has_permission(
                    world,
                    authority,
                    CanCheckSorafsStreamToken {
                        provider_id: provider,
                    }
                    .into(),
                )
        }
    }
}

pub(crate) fn eligible_custody(
    current: &crate::query::stream_token_custody::NativeControl,
    original_record_digest: [u8; 32],
    now: u64,
) -> Result<(), Error> {
    let state = &current.state;
    let active = state.active_head.ok_or(Error::Custody)?;
    if state.signer_revoked
        || state.attester_revoked
        || now < state.policy.active_from_unix_ms
        || now >= state.policy.active_until_unix_ms
        || active.record_digest != original_record_digest
        || active.key_revision != state.policy.binding.key_revision
        || active.policy_revision != state.policy.binding.policy_revision
        || active.policy_digest != state.policy.binding.policy_digest
    {
        return Err(Error::Custody);
    }
    Ok(())
}

pub(crate) fn checked_phase(
    tx: &impl StateReadOnly,
    provider: ProviderId,
    check: &StreamTokenCheckV1,
    head: &OperationHeadV1,
    now: u64,
) -> Result<(StreamTokenReviewedV1, Phase), Error> {
    let id = check.reviewed.request.operation_id;
    match &check.phase {
        Phase::Current(_) => {
            if head.active_operation.is_some()
                || journal::read_history(tx.world(), provider, id)?.is_some()
                || check.reviewed.intent.previous_audit != head.audit
            {
                return Err(Error::Conflict);
            }
            Ok((check.reviewed, Phase::Current(head.audit)))
        }
        Phase::BeforeProvider(_) | Phase::AfterProvider(_) | Phase::BeforeCommit(_) => {
            let history =
                journal::read_history(tx.world(), provider, id)?.ok_or(Error::Conflict)?;
            let record = &history.current;
            let row = &record.operation;
            if row.operation.outcome != StreamTokenOutcomeV1::Reserved
                || head.active_operation != Some(id)
                || head.revision != record.revision
                || head.digest != journal::record_digest(record)?
                || head.audit != row.operation.reviewed.intent.previous_audit
                || now < row.reserved_execution.recorded_at_unix_ms
                || now >= row.operation.reservation.expires_at_unix_ms
            {
                return Err(Error::Conflict);
            }
            let expected = match &check.phase {
                Phase::BeforeProvider(_) => Phase::BeforeProvider(row.clone()),
                Phase::AfterProvider(_) => Phase::AfterProvider(row.clone()),
                Phase::BeforeCommit(_) => Phase::BeforeCommit(row.clone()),
                _ => return Err(Error::Invalid),
            };
            Ok((history.reserved.operation.operation.reviewed, expected))
        }
        Phase::AfterCommit(_) | Phase::BeforeRelease(_) => {
            let history =
                journal::read_history(tx.world(), provider, id)?.ok_or(Error::Conflict)?;
            let row = &history.current.operation;
            if !matches!(row.operation.outcome, StreamTokenOutcomeV1::Completed(_))
                || row
                    .terminal_execution
                    .as_ref()
                    .is_none_or(|terminal| now < terminal.recorded_at_unix_ms)
            {
                return Err(Error::Conflict);
            }
            let expected = match &check.phase {
                Phase::AfterCommit(_) => Phase::AfterCommit(row.clone()),
                Phase::BeforeRelease(_) => Phase::BeforeRelease(row.clone()),
                _ => return Err(Error::Invalid),
            };
            Ok((history.reserved.operation.operation.reviewed, expected))
        }
    }
}

pub(crate) fn check_live_custody(
    current: &NativeControl,
    check: &StreamTokenCheckV1,
    authority: &AccountId,
    now: u64,
) -> Result<(), Error> {
    let reviewed = &check.reviewed.request;
    eligible_custody(current, reviewed.original_custody.record_digest, now)?;
    if *authority != check.expected_observer
        || *authority == check.expected_operator
        || *authority == AccountId::new(current.state.policy.binding.public_key.clone())
        || reviewed.original_custody.control_state_digest != current.index.digest
        || reviewed.binding_digest
            != stream_token_binding_digest_v1(&current.state.policy.binding)
                .map_err(|_| Error::Custody)?
        || now < reviewed.issued_at_unix_ms
        || now >= reviewed.expires_at_unix_ms
    {
        return Err(Error::Custody);
    }
    Ok(())
}
