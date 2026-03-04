//! Resolver attestation directory governance ISIs.

use std::time::UNIX_EPOCH;

use hex::encode as hex_encode;
use iroha_data_model::{
    events::data::{DataEvent, soradns::SoradnsDirectoryEvent},
    isi::error::{InstructionExecutionError, InvalidParameterError},
    soradns::{
        DIRECTORY_RECORD_VERSION_V1, DirectoryDraftSubmittedEventV1, DirectoryId,
        DirectoryPolicyUpdatedEventV1, DirectoryPublishedEventV1, DirectoryReleaseSignerEventV1,
        DirectoryRevokedEventV1, DirectoryRotationPolicyV1, DirectoryUnrevokedEventV1,
        PendingDirectoryDraftV1, ResolverDirectoryEventV1, ResolverDirectoryRecordV1,
        ResolverRevocationRecordV1,
    },
};
use norito::json::{self, Value};

use super::*;
use crate::state::StateTransaction;

impl Execute for iroha_data_model::isi::soradns::SubmitDirectoryDraft {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let iroha_data_model::isi::soradns::SubmitDirectoryDraft {
            record,
            car_cid,
            directory_json_sha256,
            builder_public_key,
            builder_signature,
        } = self;

        ensure_release_signer(state_transaction, &builder_public_key)?;
        ensure_directory_record_supported(&record)?;

        if record.directory_json_sha256 != directory_json_sha256 {
            return Err(invalid_parameter(
                "directory_json_sha256 field must match record payload",
            ));
        }

        if record.builder_public_key != builder_public_key {
            return Err(invalid_parameter(
                "builder_public_key inside the record must match the submission payload",
            ));
        }
        if record.builder_signature != builder_signature {
            return Err(invalid_parameter(
                "builder_signature inside the record must match the submission payload",
            ));
        }

        let directory_id = record.root_hash;
        ensure_directory_id_available(state_transaction, directory_id)?;
        ensure_previous_pointer_matches(state_transaction, &record)?;
        verify_builder_signature(&record)?;

        let submitted_at_ms = current_time_millis()?;
        let draft = PendingDirectoryDraftV1 {
            record,
            car_cid: car_cid.clone(),
            directory_json_sha256,
            builder_public_key: builder_public_key.clone(),
            builder_signature: builder_signature.clone(),
            submitted_at_ms,
        };
        state_transaction
            .world
            .soradns_directory_pending
            .insert(directory_id, draft);

        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::DraftSubmitted(DirectoryDraftSubmittedEventV1 {
                directory_id,
                car_cid,
                builder_public_key,
            }),
        );

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::soradns::PublishDirectory {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let iroha_data_model::isi::soradns::PublishDirectory {
            directory_id,
            expected_prev,
        } = self;
        let draft = state_transaction
            .world
            .soradns_directory_pending
            .remove(directory_id)
            .ok_or_else(|| {
                invariant_violation(format!(
                    "pending draft {0} does not exist",
                    hex_directory_id(&directory_id)
                ))
            })?;

        ensure_directory_record_supported(&draft.record)?;
        ensure_previous_pointer_matches(state_transaction, &draft.record)?;
        if let Some(expected_prev) = expected_prev {
            if *state_transaction.world.soradns_directory_latest.get() != Some(expected_prev) {
                return Err(invalid_parameter(format!(
                    "expected previous directory id {} does not match the latest pointer",
                    hex_directory_id(&expected_prev)
                )));
            }
        }

        verify_builder_signature(&draft.record)?;
        enforce_rotation_policy(state_transaction, &draft.record, directory_id)?;

        let previous = *state_transaction.world.soradns_directory_latest.get();
        if draft.record.previous_root.is_none() && previous.is_some() {
            return Err(invalid_parameter(
                "record.previous_root must reference the latest directory when one exists",
            ));
        }

        let history_index = *state_transaction.world.soradns_history_len.get();
        state_transaction
            .world
            .soradns_directory_history
            .insert(history_index, directory_id);
        *state_transaction.world.soradns_history_len.get_mut() = history_index.saturating_add(1);

        if let Some(prev_id) = previous {
            state_transaction
                .world
                .soradns_directory_prev_of
                .insert(directory_id, prev_id);
        }

        state_transaction
            .world
            .soradns_directory_records
            .insert(directory_id, draft.record.clone());
        *state_transaction.world.soradns_directory_latest.get_mut() = Some(directory_id);
        *state_transaction.world.soradns_last_publish_ms.get_mut() = Some(current_time_millis()?);

        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::Published(DirectoryPublishedEventV1 {
                directory_id,
                previous_directory_id: previous,
                directory_json_sha256: draft.directory_json_sha256,
                car_cid: draft.car_cid,
                block_height: state_transaction.block_height(),
            }),
        );

        Ok(())
    }
}

impl Execute for iroha_data_model::isi::soradns::RevokeResolver {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let iroha_data_model::isi::soradns::RevokeResolver {
            resolver_id,
            reason,
        } = self;

        if state_transaction
            .world
            .soradns_directory_revocations
            .get(&resolver_id)
            .is_some()
        {
            return Err(invariant_violation(format!(
                "resolver {} is already revoked",
                hex::encode(resolver_id)
            )));
        }

        let record = ResolverRevocationRecordV1 {
            resolver_id,
            reason,
            revoked_at_ms: current_time_millis()?,
        };
        state_transaction
            .world
            .soradns_directory_revocations
            .insert(resolver_id, record);

        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::Revoked(DirectoryRevokedEventV1 {
                resolver_id,
                reason,
                block_height: state_transaction.block_height(),
            }),
        );
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::soradns::UnrevokeResolver {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let resolver_id = self.resolver_id;
        if state_transaction
            .world
            .soradns_directory_revocations
            .remove(resolver_id)
            .is_none()
        {
            return Err(invalid_parameter(format!(
                "resolver {} is not revoked",
                hex::encode(resolver_id)
            )));
        }

        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::Unrevoked(DirectoryUnrevokedEventV1 {
                resolver_id,
                block_height: state_transaction.block_height(),
            }),
        );
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::soradns::AddReleaseSigner {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .soradns_release_signers
            .get(&self.public_key)
            .is_some()
        {
            return Err(invalid_parameter("release signer already exists"));
        }
        state_transaction
            .world
            .soradns_release_signers
            .insert(self.public_key.clone(), ());
        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::ReleaseSignerAdded(DirectoryReleaseSignerEventV1 {
                public_key: self.public_key,
            }),
        );
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::soradns::RemoveReleaseSigner {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .soradns_release_signers
            .remove(self.public_key.clone())
            .is_none()
        {
            return Err(invalid_parameter("release signer not found"));
        }
        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::ReleaseSignerRemoved(DirectoryReleaseSignerEventV1 {
                public_key: self.public_key,
            }),
        );
        Ok(())
    }
}

impl Execute for iroha_data_model::isi::soradns::SetDirectoryRotationPolicy {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        validate_rotation_policy(&self.policy)?;
        *state_transaction.world.soradns_rotation_policy.get_mut() = self.policy;
        emit_directory_event(
            state_transaction,
            ResolverDirectoryEventV1::PolicyUpdated(DirectoryPolicyUpdatedEventV1 {
                policy: self.policy,
            }),
        );
        Ok(())
    }
}

fn ensure_release_signer(
    state_transaction: &StateTransaction<'_, '_>,
    public_key: &iroha_crypto::PublicKey,
) -> Result<(), Error> {
    if state_transaction
        .world
        .soradns_release_signers
        .get(public_key)
        .is_some()
    {
        Ok(())
    } else {
        Err(invalid_parameter(
            "release signer is not authorized to submit directory drafts",
        ))
    }
}

fn ensure_directory_id_available(
    state_transaction: &StateTransaction<'_, '_>,
    directory_id: DirectoryId,
) -> Result<(), Error> {
    if state_transaction
        .world
        .soradns_directory_records
        .get(&directory_id)
        .is_some()
    {
        return Err(invalid_parameter(format!(
            "directory {} was already published",
            hex_directory_id(&directory_id)
        )));
    }
    if state_transaction
        .world
        .soradns_directory_pending
        .get(&directory_id)
        .is_some()
    {
        return Err(invalid_parameter(format!(
            "directory {} already has a pending draft",
            hex_directory_id(&directory_id)
        )));
    }
    Ok(())
}

fn ensure_directory_record_supported(record: &ResolverDirectoryRecordV1) -> Result<(), Error> {
    if record.record_version != DIRECTORY_RECORD_VERSION_V1 {
        return Err(invalid_parameter(format!(
            "unsupported directory record version {}",
            record.record_version
        )));
    }
    if record.rad_count == 0 {
        return Err(invalid_parameter(
            "directory record must reference at least one resolver attestation",
        ));
    }
    Ok(())
}

fn ensure_previous_pointer_matches(
    state_transaction: &StateTransaction<'_, '_>,
    record: &ResolverDirectoryRecordV1,
) -> Result<(), Error> {
    let latest = *state_transaction.world.soradns_directory_latest.get();
    match (latest, record.previous_root) {
        (Some(latest_id), Some(prev_id)) if latest_id != prev_id => Err(invalid_parameter(
            "record.previous_root does not match the latest published directory",
        )),
        (None, Some(_)) => Err(invalid_parameter(
            "record.previous_root must be empty for the first directory publish",
        )),
        _ => Ok(()),
    }
}

fn enforce_rotation_policy(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: &ResolverDirectoryRecordV1,
    directory_id: DirectoryId,
) -> Result<(), Error> {
    let policy = *state_transaction.world.soradns_rotation_policy.get();
    if policy.min_interval_ms == 0 {
        return Err(invalid_parameter(
            "rotation policy min_interval_ms must be greater than zero",
        ));
    }
    if policy.max_skew_ms == 0 {
        return Err(invalid_parameter(
            "rotation policy max_skew_ms must be greater than zero",
        ));
    }
    if policy.council_threshold == 0 {
        return Err(invalid_parameter(
            "rotation policy council_threshold must be greater than zero",
        ));
    }

    let now_ms = current_time_millis()?;
    if let Some(last_publish) = *state_transaction.world.soradns_last_publish_ms.get() {
        let spacing = now_ms.saturating_sub(last_publish);
        if spacing < policy.min_interval_ms {
            return Err(invalid_parameter(format!(
                "directory {} was published too soon ({spacing} ms since last publish, policy requires {} ms)",
                hex_directory_id(&directory_id),
                policy.min_interval_ms
            )));
        }
    }

    let created_delta = record.created_at_ms.abs_diff(now_ms);
    if created_delta > policy.max_skew_ms {
        return Err(invalid_parameter(format!(
            "directory record timestamp skew ({created_delta} ms) exceeds policy limit {} ms",
            policy.max_skew_ms
        )));
    }

    if policy.require_change
        && *state_transaction.world.soradns_directory_latest.get() == Some(directory_id)
    {
        return Err(invalid_parameter(
            "rotation policy requires the directory to change between publishes",
        ));
    }

    Ok(())
}

fn validate_rotation_policy(policy: &DirectoryRotationPolicyV1) -> Result<(), Error> {
    if policy.min_interval_ms == 0 {
        return Err(invalid_parameter(
            "rotation policy min_interval_ms must be greater than zero",
        ));
    }
    if policy.max_skew_ms == 0 {
        return Err(invalid_parameter(
            "rotation policy max_skew_ms must be greater than zero",
        ));
    }
    if policy.council_threshold == 0 {
        return Err(invalid_parameter(
            "rotation policy council_threshold must be greater than zero",
        ));
    }
    Ok(())
}

fn verify_builder_signature(record: &ResolverDirectoryRecordV1) -> Result<(), Error> {
    let payload = signing_payload_bytes(record)?;
    record
        .builder_signature
        .verify(&record.builder_public_key, &payload)
        .map_err(|err| invalid_parameter(format!("builder signature verification failed: {err}")))
}

fn signing_payload_bytes(record: &ResolverDirectoryRecordV1) -> Result<Vec<u8>, Error> {
    let (_, pk_bytes) = record.builder_public_key.to_bytes();
    let mut payload = norito::json::Map::new();
    payload.insert(
        "record_version".into(),
        Value::from(u64::from(record.record_version)),
    );
    payload.insert("created_at_ms".into(), Value::from(record.created_at_ms));
    payload.insert("rad_count".into(), Value::from(record.rad_count));
    payload.insert(
        "root_hash_hex".into(),
        Value::from(hex_encode(record.root_hash)),
    );
    payload.insert(
        "directory_json_sha256_hex".into(),
        Value::from(hex_encode(record.directory_json_sha256)),
    );
    if let Some(prev) = record.previous_root {
        payload.insert("previous_root_hex".into(), Value::from(hex_encode(prev)));
    }
    payload.insert(
        "proof_manifest_cid".into(),
        Value::from(record.proof_manifest_cid.to_string()),
    );
    payload.insert(
        "builder_public_key_hex".into(),
        Value::from(hex_encode(pk_bytes)),
    );
    payload.insert(
        "published_at_block".into(),
        Value::from(record.published_at_block),
    );
    payload.insert(
        "published_at_unix".into(),
        Value::from(record.published_at_unix),
    );

    json::to_vec(&Value::Object(payload)).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to serialize signing payload: {err}").into(),
        )
    })
}

fn emit_directory_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    event: ResolverDirectoryEventV1,
) {
    let soradns_event = match event {
        ResolverDirectoryEventV1::DraftSubmitted(payload) => {
            SoradnsDirectoryEvent::DraftSubmitted(payload)
        }
        ResolverDirectoryEventV1::Published(payload) => SoradnsDirectoryEvent::Published(payload),
        ResolverDirectoryEventV1::Revoked(payload) => SoradnsDirectoryEvent::Revoked(payload),
        ResolverDirectoryEventV1::Unrevoked(payload) => SoradnsDirectoryEvent::Unrevoked(payload),
        ResolverDirectoryEventV1::ReleaseSignerAdded(payload) => {
            SoradnsDirectoryEvent::ReleaseSignerAdded(payload)
        }
        ResolverDirectoryEventV1::ReleaseSignerRemoved(payload) => {
            SoradnsDirectoryEvent::ReleaseSignerRemoved(payload)
        }
        ResolverDirectoryEventV1::PolicyUpdated(payload) => {
            SoradnsDirectoryEvent::PolicyUpdated(payload)
        }
    };
    state_transaction
        .world
        .emit_events(Some(DataEvent::Soradns(soradns_event)));
}

fn current_time_millis() -> Result<u64, Error> {
    let now = crate::time::now().now;
    let duration = now.duration_since(UNIX_EPOCH).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("system clock is before UNIX_EPOCH: {err}").into(),
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        InstructionExecutionError::InvariantViolation(
            "system clock duration exceeded u64 capacity".into(),
        )
    })
}

fn invalid_parameter(message: impl Into<String>) -> Error {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn invariant_violation(message: impl Into<String>) -> Error {
    InstructionExecutionError::InvariantViolation(message.into().into_boxed_str())
}

fn hex_directory_id(id: &DirectoryId) -> String {
    hex::encode(id)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        account::AccountId, ipfs::IpfsPath, soradns::ResolverDirectoryRecordV1,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn block_header() -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)
    }

    fn make_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let handle = LiveQueryStore::start_test();
        State::new_for_testing(World::new(), kura, handle)
    }

    fn sample_record(builder_keys: &KeyPair) -> ResolverDirectoryRecordV1 {
        let now_ms = current_time_millis().expect("time");
        ResolverDirectoryRecordV1 {
            root_hash: [1; 32],
            record_version: soradns::DIRECTORY_RECORD_VERSION_V1,
            created_at_ms: now_ms,
            rad_count: 1,
            directory_json_sha256: [2; 32],
            previous_root: None,
            published_at_block: 0,
            published_at_unix: now_ms / 1_000,
            proof_manifest_cid: IpfsPath::from_str("/ipfs/bafyreiaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("cid"),
            builder_public_key: builder_keys.public_key().clone(),
            builder_signature: Signature::from_bytes(&[0; 64]),
        }
    }

    fn signed_record() -> (ResolverDirectoryRecordV1, KeyPair) {
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let mut record = sample_record(&keypair);
        let payload = signing_payload_bytes(&record).expect("payload");
        record.builder_signature = Signature::new(keypair.private_key(), &payload);
        (record, keypair)
    }

    #[test]
    fn submit_directory_draft_succeeds() {
        crate::test_alias::ensure();
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let authority = AccountId::from_str("council@sora").expect("account");
        let (record, keypair) = signed_record();
        stx.world
            .soradns_release_signers
            .insert(keypair.public_key().clone(), ());

        let submit = iroha_data_model::isi::soradns::SubmitDirectoryDraft {
            record: record.clone(),
            car_cid: IpfsPath::from_str("/ipfs/bafyreidraftaaaaaaaaaaaaaaaaaaaaaaaa").expect("cid"),
            directory_json_sha256: record.directory_json_sha256,
            builder_public_key: keypair.public_key().clone(),
            builder_signature: record.builder_signature.clone(),
        };
        submit.execute(&authority, &mut stx).expect("submit");
        assert!(
            stx.world
                .soradns_directory_pending
                .get(&record.root_hash)
                .is_some()
        );
    }

    #[test]
    fn publish_directory_moves_draft() {
        crate::test_alias::ensure();
        let state = make_state();
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let authority = AccountId::from_str("council@sora").expect("account");
        let (record, keypair) = signed_record();
        stx.world
            .soradns_release_signers
            .insert(keypair.public_key().clone(), ());

        let submit = iroha_data_model::isi::soradns::SubmitDirectoryDraft {
            record: record.clone(),
            car_cid: IpfsPath::from_str("/ipfs/bafyreisubmitbbbbbbbbbbbbbbbbbbbbbbbb")
                .expect("cid"),
            directory_json_sha256: record.directory_json_sha256,
            builder_public_key: keypair.public_key().clone(),
            builder_signature: record.builder_signature.clone(),
        };
        submit.execute(&authority, &mut stx).expect("submit");

        let publish = iroha_data_model::isi::soradns::PublishDirectory {
            directory_id: record.root_hash,
            expected_prev: None,
        };
        publish.execute(&authority, &mut stx).expect("publish");
        assert!(
            stx.world
                .soradns_directory_records
                .get(&record.root_hash)
                .is_some()
        );
        assert!(
            stx.world
                .soradns_directory_pending
                .get(&record.root_hash)
                .is_none()
        );
    }
}
