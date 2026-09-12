//! Governed native StreamToken custody configuration, enrollment and terminal revocation.
//! No per-token operation state is stored here; all writes share one native transactional CAS.
use super::Execute;
use crate::{
    query::stream_token_custody::{
        ControlIndexV1, NativeControl, StreamTokenCustodyControlErrorV1 as Error, decode, encode,
        head_key, height_key, key_path, read_active, read_record,
        read_stream_token_custody_control_at_v1, record_digest, record_key, validate_state_binding,
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsStreamTokenCustody,
    },
    permission::Permission,
    sorafs::{
        capacity::ProviderId,
        stream_token_custody::{
            STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1, STREAM_TOKEN_CUSTODY_NORMAL_REVISIONS_V1,
            SorafsStreamTokenCustodyActionV1 as Action, SorafsStreamTokenCustodyRevocationV1,
            StreamTokenCustodyControlRecordV1,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody;
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyActiveHeadV1, SignerCustodyEnrollmentContextV1,
        verify_signer_custody_enrollment_v1,
    },
    stream_token_custody_control::{StreamTokenCustodyControlStateV1, StreamTokenCustodyPolicyV1},
};

fn rejected(error: Error) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        error.to_string(),
    ))
}
fn authorized(
    state: &StateTransaction<'_, '_>,
    authority: &AccountId,
    provider: ProviderId,
) -> bool {
    let permission = Permission::from(CanManageSorafsStreamTokenCustody {
        provider_id: provider,
    });
    state.world.accounts.get(authority).is_some()
        && (state
            .world
            .account_permissions
            .get(authority)
            .is_some_and(|tokens| tokens.contains(&permission))
            || state
                .world
                .account_roles_iter(authority)
                .filter_map(|role| state.world.roles.get(role))
                .any(|role| role.permissions().any(|token| token == &permission)))
}
fn check_key_generation(
    old_revision: u64,
    new_revision: u64,
    old_key: &PublicKey,
    new_key: &PublicKey,
) -> Result<bool, Error> {
    if new_revision < old_revision
        || (new_revision == old_revision && new_key != old_key)
        || (new_revision > old_revision && new_key == old_key)
    {
        return Err(Error::Generation);
    }
    Ok(new_revision > old_revision)
}
fn next_policy(
    current: Option<&NativeControl>,
    next: StreamTokenCustodyPolicyV1,
) -> Result<StreamTokenCustodyControlStateV1, Error> {
    next.validate().map_err(|_| Error::Invalid)?;
    let Some(current) = current else {
        return Ok(StreamTokenCustodyControlStateV1 {
            policy: next,
            next_sequence: 1,
            predecessor_digest: [0; 32],
            active_head: None,
            signer_revoked: false,
            attester_revoked: false,
        });
    };
    let old = &current.state.policy;
    if old == &next {
        return Err(Error::Conflict);
    }
    let new_signer = check_key_generation(
        old.binding.key_revision,
        next.binding.key_revision,
        &old.binding.public_key,
        &next.binding.public_key,
    )?;
    let new_attester = check_key_generation(
        old.attester_authority.key_revision,
        next.attester_authority.key_revision,
        &old.attester_public_key,
        &next.attester_public_key,
    )?;
    if next.binding.policy_revision < old.binding.policy_revision
        || next.attester_authority.policy_revision < old.attester_authority.policy_revision
        || (next.binding.policy_revision == old.binding.policy_revision
            && next.binding.policy_digest != old.binding.policy_digest)
        || (next.attester_authority.policy_revision == old.attester_authority.policy_revision
            && next.attester_authority.policy_digest != old.attester_authority.policy_digest)
    {
        return Err(Error::Generation);
    }
    // Changing signing identities/handles is governed policy work, not same-key renewal.
    let mut same_signer_policy = next.binding.clone();
    same_signer_policy.public_key = old.binding.public_key.clone();
    same_signer_policy.key_revision = old.binding.key_revision;
    if new_signer {
        same_signer_policy
            .key_handle
            .clone_from(&old.binding.key_handle);
    }
    if same_signer_policy != old.binding
        && next.binding.policy_revision <= old.binding.policy_revision
    {
        return Err(Error::Generation);
    }
    let mut same_attester_policy = next.custody_trust();
    same_attester_policy.public_key = old.attester_public_key.clone();
    same_attester_policy.authority.key_revision = old.attester_authority.key_revision;
    let old_trust = old.custody_trust();
    let changed_attester_policy = same_attester_policy.authority != old_trust.authority
        || same_attester_policy.active_from_unix_ms != old_trust.active_from_unix_ms
        || same_attester_policy.active_until_unix_ms != old_trust.active_until_unix_ms
        || same_attester_policy.max_validity_ms != old_trust.max_validity_ms
        || same_attester_policy.max_anchor_age_ms != old_trust.max_anchor_age_ms;
    if changed_attester_policy
        && next.attester_authority.policy_revision <= old.attester_authority.policy_revision
    {
        return Err(Error::Generation);
    }
    Ok(StreamTokenCustodyControlStateV1 {
        policy: next,
        next_sequence: current.state.next_sequence,
        predecessor_digest: current.state.predecessor_digest,
        active_head: None,
        signer_revoked: current.state.signer_revoked && !new_signer,
        attester_revoked: current.state.attester_revoked && !new_attester,
    })
}
fn prepare_keys(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    current: Option<&NativeControl>,
    next: &StreamTokenCustodyPolicyV1,
) -> Result<Vec<StatePath>, Error> {
    let mut new_paths = Vec::with_capacity(2);
    for (signer, key, old_key) in [
        (
            true,
            &next.binding.public_key,
            current.map(|c| &c.state.policy.binding.public_key),
        ),
        (
            false,
            &next.attester_public_key,
            current.map(|c| &c.state.policy.attester_public_key),
        ),
    ] {
        let path = key_path(provider, signer, key)?;
        if old_key == Some(key) {
            // Existing-key renewals require an authentic first-use index, never a missing tombstone.
            let bytes = world
                .smart_contract_state()
                .get(&path)
                .ok_or(Error::CorruptHistory)?;
            let index: ControlIndexV1 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
            let old = current.ok_or(Error::CorruptHistory)?;
            if index.revision == 0 || index.revision > old.index.revision {
                return Err(Error::CorruptHistory);
            }
        } else {
            if world.smart_contract_state().get(&path).is_some() {
                return Err(Error::Generation);
            }
            new_paths.push(path);
        }
    }
    Ok(new_paths)
}
fn request_digest(
    instruction: &MutateSorafsStreamTokenCustody,
    authority: &AccountId,
) -> Result<[u8; 32], Error> {
    // A 16 KiB canonical action plus its native request envelope must fit before allocation.
    if norito::canonical_frame_len(instruction).map_err(|_| Error::Invalid)? > 32 * 1024 {
        return Err(Error::Invalid);
    }
    let request = norito::encode_canonical(instruction).map_err(|_| Error::Invalid)?;
    let authority = encode(authority)?;
    let mut bytes = b"iroha.sorafs.stream-token.custody-request.v1\0".to_vec();
    bytes.extend_from_slice(&request);
    bytes.extend_from_slice(&authority);
    Ok(*Hash::new(bytes).as_ref())
}
fn apply_control(
    instruction: MutateSorafsStreamTokenCustody,
    authority: &AccountId,
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let provider = instruction.provider_id;
    if tx.world.provider_owners.get(&provider).is_none() {
        return Err(Error::BindingMismatch);
    }
    let current = read_active(tx.world(), provider)?;
    if let Some(active) = &current {
        if validate_state_binding(tx, &active.state.policy.binding)? != provider {
            return Err(Error::BindingMismatch);
        }
    }
    let request_digest = request_digest(&instruction, authority)?;
    if current.as_ref().map_or(0, |c| c.index.revision) != instruction.expected_revision
        || current.as_ref().map_or([0; 32], |c| c.index.digest) != instruction.expected_digest
    {
        // Only the exact original authority and full canonical request can retry a retained
        // transition. Current permission/registration has already been checked; no state revives.
        if let Some(active) = &current {
            if instruction.expected_revision < active.index.revision {
                let historical =
                    read_record(tx.world(), provider, instruction.expected_revision + 1)?;
                if historical.record.predecessor_digest == instruction.expected_digest
                    && historical.record.authority == *authority
                    && historical.record.request_digest == request_digest
                {
                    return Ok(());
                }
            }
        }
        return Err(Error::Conflict);
    }
    let revision = instruction
        .expected_revision
        .checked_add(1)
        .ok_or(Error::Capacity)?;
    let emergency = matches!(instruction.action, Action::Revoke(_));
    if revision > STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1
        || (!emergency && revision > STREAM_TOKEN_CUSTODY_NORMAL_REVISIONS_V1)
    {
        return Err(Error::Capacity);
    }
    let parent_height =
        u64::try_from(tx.block_hashes().len()).map_err(|_| Error::HeightUnavailable)?;
    let height = parent_height
        .checked_add(1)
        .ok_or(Error::HeightUnavailable)?;
    let now = tx.block_unix_timestamp_ms();
    if height != tx._curr_block.height().get() || now == 0 || now == u64::MAX {
        return Err(Error::Invalid);
    }
    let ordinal = match current.as_ref() {
        Some(old) if old.index.height > height || old.record.recorded_at_unix_ms > now => {
            return Err(Error::CorruptHistory);
        }
        Some(old) if old.index.height == height => {
            if old.record.recorded_at_unix_ms != now {
                return Err(Error::CorruptHistory);
            }
            old.index.ordinal.checked_add(1).ok_or(Error::Capacity)?
        }
        _ => 0,
    };
    let mut key_paths = Vec::new();
    let state = match instruction.action {
        Action::Configure(bytes) => {
            let policy: StreamTokenCustodyPolicyV1 = decode(&bytes)?;
            if validate_state_binding(tx, &policy.binding)? != provider {
                return Err(Error::BindingMismatch);
            }
            let next = next_policy(current.as_ref(), policy)?;
            key_paths = prepare_keys(tx.world(), provider, current.as_ref(), &next.policy)?;
            next
        }
        Action::Enroll(bytes) => {
            let current = current.as_ref().ok_or(Error::Conflict)?;
            let previous_committed = read_stream_token_custody_control_at_v1(
                tx,
                &current.state.policy.binding,
                parent_height,
            )?
            .ok_or(Error::Conflict)?;
            // A pending Configure/Enroll/Revoke cannot masquerade as the previous committed state.
            if previous_committed.anchor.state_digest != current.index.digest
                || previous_committed.state != current.state
            {
                return Err(Error::Conflict);
            }
            let verified = verify_signer_custody_enrollment_v1(
                &bytes,
                &current.state.policy.binding,
                &current.state.policy.custody_trust(),
                &SignerCustodyEnrollmentContextV1 {
                    now_unix_ms: now,
                    anchor_observed_at_unix_ms: now,
                    current_anchor: previous_committed.anchor,
                    next_sequence: current.state.next_sequence,
                    predecessor_digest: current.state.predecessor_digest,
                    signer_revoked: current.state.signer_revoked,
                    attester_revoked: current.state.attester_revoked,
                },
            )
            .map_err(|_| Error::Enrollment)?;
            let mut next = current.state.clone();
            next.next_sequence = next.next_sequence.checked_add(1).ok_or(Error::Capacity)?;
            next.predecessor_digest = verified.record_digest();
            next.active_head = Some(SignerCustodyActiveHeadV1 {
                record_digest: verified.record_digest(),
                sequence: verified.statement().sequence,
                approved_anchor: previous_committed.anchor,
                key_revision: next.policy.binding.key_revision,
                policy_revision: next.policy.binding.policy_revision,
                policy_digest: next.policy.binding.policy_digest,
            });
            next
        }
        Action::Revoke(SorafsStreamTokenCustodyRevocationV1 { signer, attester }) => {
            let mut next = current.as_ref().ok_or(Error::Conflict)?.state.clone();
            if !(signer && !next.signer_revoked || attester && !next.attester_revoked) {
                return Err(Error::Conflict);
            }
            next.signer_revoked |= signer;
            next.attester_revoked |= attester;
            next
        }
    };
    state.validate().map_err(|_| Error::Invalid)?;
    if validate_state_binding(tx, &state.policy.binding)? != provider {
        return Err(Error::BindingMismatch);
    }
    let record = StreamTokenCustodyControlRecordV1 {
        provider_id: provider,
        revision,
        predecessor_digest: instruction.expected_digest,
        request_digest,
        execution_height: height,
        ordinal,
        recorded_at_unix_ms: now,
        authority: authority.clone(),
        control_state: encode(&state)?,
    };
    let index = ControlIndexV1 {
        revision,
        digest: record_digest(&record)?,
        height,
        ordinal,
    };
    let record_path = record_key(provider, revision);
    let height_path = height_key(provider, height, ordinal);
    if tx.world.smart_contract_state.get(&record_path).is_some()
        || tx.world.smart_contract_state.get(&height_path).is_some()
    {
        return Err(Error::CorruptHistory);
    }
    let record_bytes = encode(&record)?;
    let index_bytes = encode(&index)?;
    // All fallible validation/encoding completes before the single transactional publication.
    tx.world
        .smart_contract_state
        .insert(record_path, record_bytes);
    tx.world
        .smart_contract_state
        .insert(height_path, index_bytes.clone());
    for path in key_paths {
        tx.world
            .smart_contract_state
            .insert(path, index_bytes.clone());
    }
    tx.world
        .smart_contract_state
        .insert(head_key(provider), index_bytes);
    Ok(())
}
impl Execute for MutateSorafsStreamTokenCustody {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if !authorized(state_transaction, authority, self.provider_id) {
            return Err(rejected(Error::BindingMismatch));
        }
        apply_control(self, authority, state_transaction).map_err(rejected)
    }
}

#[cfg(test)]
#[path = "sorafs_stream_token_custody/tests.rs"]
mod tests;
