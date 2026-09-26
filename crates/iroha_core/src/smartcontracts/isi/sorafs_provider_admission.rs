//! Atomic provider admission effects from signed genesis or certified Parliament enactment.

mod genesis;

use crate::{
    query::provider_admission::{
        self as native, AdmissionHistoryRecordV1, ProviderAdmissionErrorV1,
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};
use iroha_data_model::{
    isi::error::{InstructionExecutionError, InvalidParameterError},
    sorafs::provider_admission::{
        ProviderAdmissionCouncilPolicyV1,
        governance::{
            PROVIDER_ADMISSION_HISTORY_MAX_BYTES_V1, PROVIDER_ADMISSION_MAX_PROVIDERS_V1,
            PROVIDER_ADMISSION_MAX_REVISIONS_V1, PROVIDER_ADMISSION_MAX_REVOCATION_BYTES_V1,
            ProviderAdmissionGovernanceActionV1 as Action, decode_frame,
        },
    },
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::{
    ProviderAdmissionEnvelopeV1, ProviderAdmissionRenewalV1, ProviderAdmissionRevocationV1,
};

fn rejected(error: impl std::fmt::Display) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        error.to_string(),
    ))
}

/// Exact current effect head used by Parliament's certificate compare-and-set.
pub(super) fn governed_head(
    world: &impl WorldReadOnly,
    action: &Action,
) -> Result<Option<(u64, Vec<u8>)>, InstructionExecutionError> {
    let subject = action.provider_id().map_err(rejected)?;
    native::read_head(world, subject)
        .map_err(rejected)?
        .map(|head| {
            native::encode(&head)
                .map(|bytes| (head.revision, bytes))
                .map_err(rejected)
        })
        .transpose()
}

/// Apply one exact action after Parliament authenticated its subject, effect and current head.
pub(super) fn apply(
    action: Action,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<bool, InstructionExecutionError> {
    apply_inner(action, tx, None).map_err(rejected)
}

fn apply_inner(
    action: Action,
    tx: &mut StateTransaction<'_, '_>,
    genesis_origin: Option<native::GenesisAdmissionOriginV1>,
) -> Result<bool, ProviderAdmissionErrorV1> {
    let invalid = ProviderAdmissionErrorV1;
    let subject = match (&action, &genesis_origin) {
        (Action::Admit(bytes), Some(_)) => {
            let projection: ProviderAdmissionEnvelopeV1 =
                decode_frame(bytes).map_err(|_| invalid)?;
            Some(iroha_data_model::sorafs::capacity::ProviderId::new(
                projection.proposal.provider_id,
            ))
        }
        _ => action.provider_id().map_err(|_| invalid)?,
    };
    let current = native::read_head(tx.world(), subject)?;
    let height = tx._curr_block.height().get();
    let now = tx.block_unix_timestamp_ms();
    if now == 0
        || now == u64::MAX
        || current
            .as_ref()
            .is_some_and(|old| old.height > height || old.recorded_at_unix_ms > now || old.revoked)
    {
        return Err(invalid);
    }
    let revision = current
        .as_ref()
        .map_or(Some(1), |old| old.revision.checked_add(1))
        .ok_or(invalid)?;
    if revision > PROVIDER_ADMISSION_MAX_REVISIONS_V1
        || (subject.is_some()
            && !matches!(action, Action::Revoke(_))
            && revision == PROVIDER_ADMISSION_MAX_REVISIONS_V1)
    {
        return Err(invalid);
    }
    let network_id = *tx.network_id().as_bytes();
    let mut count_update = None;
    let (material, revoked, owner) = match &action {
        Action::ConfigureCouncil(bytes) => {
            let policy: ProviderAdmissionCouncilPolicyV1 =
                decode_frame(bytes).map_err(|_| invalid)?;
            policy.validate().map_err(|_| invalid)?;
            if policy.network_id != network_id || policy.revision != revision {
                return Err(invalid);
            }
            if let Some(previous) = native::read_policy(tx.world())? {
                policy.validate_successor(&previous).map_err(|_| invalid)?;
            } else if revision != 1 || policy.predecessor_policy_digest.is_some() {
                return Err(invalid);
            }
            (bytes.clone(), false, None)
        }
        Action::Admit(bytes) => {
            if current.is_some() {
                return Ok(false);
            }
            let provider = subject.ok_or(invalid)?;
            let owner = tx
                .world
                .provider_owners
                .get(&provider)
                .cloned()
                .ok_or(invalid)?;
            let policy = native::read_policy(tx.world())?.ok_or(invalid)?;
            let envelope: ProviderAdmissionEnvelopeV1 = decode_frame(bytes).map_err(|_| invalid)?;
            if policy.network_id != network_id || envelope.admission_revision != 1 {
                return Err(invalid);
            }
            if genesis_origin.is_none() {
                policy
                    .verify_envelope_policy_claim(&envelope, now / 1000)
                    .map_err(|_| invalid)?;
            }
            let count_path = native::path(None, "provider_count");
            let count: u64 = tx
                .world
                .smart_contract_state
                .get(&count_path)
                .map(|bytes| decode_frame(bytes))
                .transpose()
                .map_err(|_| invalid)?
                .unwrap_or(0);
            let next = count
                .checked_add(1)
                .filter(|next| *next <= PROVIDER_ADMISSION_MAX_PROVIDERS_V1)
                .ok_or(invalid)?;
            count_update = Some((count_path, native::encode(&next)?));
            (bytes.clone(), false, Some(owner))
        }
        Action::Renew(bytes) => {
            let previous = current.as_ref().ok_or(invalid)?;
            let provider = subject.ok_or(invalid)?;
            let owner = tx
                .world
                .provider_owners
                .get(&provider)
                .cloned()
                .ok_or(invalid)?;
            if previous.owner.as_ref() != Some(&owner) {
                return Err(invalid);
            }
            let policy = native::read_policy(tx.world())?.ok_or(invalid)?;
            let renewal: ProviderAdmissionRenewalV1 = decode_frame(bytes).map_err(|_| invalid)?;
            let old: ProviderAdmissionEnvelopeV1 =
                decode_frame(&previous.material).map_err(|_| invalid)?;
            let old_digest = sorafs_manifest::provider_admission::compute_envelope_digest(&old)
                .map_err(|_| invalid)?;
            let next = &renewal.envelope;
            if policy.network_id != network_id
                || next.admission_revision != revision
                || next.policy_id != old.policy_id
                || next.policy_revision < old.policy_revision
                || renewal.previous_envelope_digest != old_digest
                || next.expected_current_event_digest != Some(old_digest)
                || next.retention_epoch < old.retention_epoch
                || next.issued_at < old.issued_at
                || next.proposal.provider_id != old.proposal.provider_id
                || renewal.envelope_digest
                    != sorafs_manifest::provider_admission::compute_envelope_digest(next)
                        .map_err(|_| invalid)?
            {
                return Err(invalid);
            }
            policy
                .verify_envelope_policy_claim(next, now / 1000)
                .map_err(|_| invalid)?;
            (native::encode(next)?, false, Some(owner))
        }
        Action::Revoke(bytes) => {
            let previous = current.as_ref().ok_or(invalid)?;
            let policy = native::read_policy(tx.world())?.ok_or(invalid)?;
            let revoke: ProviderAdmissionRevocationV1 = decode_frame(bytes).map_err(|_| invalid)?;
            let old: ProviderAdmissionEnvelopeV1 =
                decode_frame(&previous.material).map_err(|_| invalid)?;
            let old_digest = sorafs_manifest::provider_admission::compute_envelope_digest(&old)
                .map_err(|_| invalid)?;
            if policy.network_id != network_id
                || revoke.network_id != network_id
                || revoke.policy_id != policy.policy_id
                || revoke.policy_revision != policy.revision
                || revoke.policy_digest != policy.canonical_digest().map_err(|_| invalid)?
                || revoke.transition_revision != revision
                || revoke.expected_current_event_digest != old_digest
                || revoke.envelope_digest != old_digest
                || revoke.revoked_at > now / 1000
                || revoke.revoked_at < old.issued_at
            {
                return Err(invalid);
            }
            // Emergency revocation remains possible under a paused council.
            sorafs_manifest::provider_admission::verify_revocation_signatures(
                &revoke,
                &native::council(&policy)?,
            )
            .map_err(|_| invalid)?;
            (bytes.clone(), true, previous.owner.clone())
        }
    };
    let next = AdmissionHistoryRecordV1 {
        network_id,
        genesis_origin,
        revision,
        predecessor: current.as_ref().map(native::digest).transpose()?,
        height,
        recorded_at_unix_ms: now,
        owner,
        revoked,
        material,
    };
    let frame = native::encode(&next)?;
    let emergency = matches!(action, Action::Revoke(_));
    let retained_bytes_path = native::path(
        None,
        if emergency {
            "revocation_bytes"
        } else {
            "history_bytes"
        },
    );
    let retained_bytes: u64 = tx
        .world
        .smart_contract_state
        .get(&retained_bytes_path)
        .map(|bytes| decode_frame(bytes))
        .transpose()
        .map_err(|_| invalid)?
        .unwrap_or(0);
    // Two identical retained rows (head and history) plus fixed counter overhead are budgeted.
    // Reserve enough terminal space for all admitted identities even when normal history is full.
    let maximum = if emergency {
        PROVIDER_ADMISSION_MAX_PROVIDERS_V1
            * (2 * PROVIDER_ADMISSION_MAX_REVOCATION_BYTES_V1 as u64 + 4096)
    } else {
        PROVIDER_ADMISSION_HISTORY_MAX_BYTES_V1
    };
    let next_bytes = retained_bytes
        .checked_add(2 * frame.len() as u64 + 64)
        .filter(|bytes| *bytes <= maximum)
        .ok_or(invalid)?;
    let retained_bytes_frame = native::encode(&next_bytes)?;
    let record_path = native::path(subject, &format!("history/{revision}"));
    if tx.world.smart_contract_state.get(&record_path).is_some() {
        return Err(invalid);
    }
    // Validate and encode every write before publishing the single transactional effect.
    tx.world
        .smart_contract_state
        .insert(record_path, frame.clone());
    tx.world
        .smart_contract_state
        .insert(native::path(subject, "head"), frame);
    tx.world
        .smart_contract_state
        .insert(retained_bytes_path, retained_bytes_frame);
    if let Some((path, bytes)) = count_update {
        tx.world.smart_contract_state.insert(path, bytes);
    }
    Ok(true)
}

/// Native admission fixture, available only to the existing test-support feature.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub mod test_fixture;
#[cfg(test)]
mod tests;
