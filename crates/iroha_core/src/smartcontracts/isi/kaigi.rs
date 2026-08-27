//! Host-side execution of Kaigi instruction family.
use crate::{
    smartcontracts::limits,
    state::{StateTransaction, World, WorldReadOnly},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    HasMetadata, Identifiable,
    account::rekey::{AccountAlias, AccountRekeyTransitionProvenance},
    events::{
        data::prelude::{
            KaigiRelayHealthSummary, KaigiRelayManifestSummary, KaigiRelayRegistrationSummary,
            KaigiRelayUnregistrationSummary, KaigiRosterSummary, KaigiStatusSummary,
            KaigiUsageSummary,
        },
        prelude::{DomainEvent, MetadataChanged},
    },
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        kaigi::{
            CreateKaigi, EndKaigi, JoinKaigi, LeaveKaigi, RecordKaigiUsage, RegisterKaigiRelay,
            ReportKaigiRelayHealth, SetKaigiRelayManifest, UnregisterKaigiRelay,
        },
    },
    kaigi::{
        KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1, KAIGI_RELAY_MANIFEST_MAX_HOPS_V1,
        KAIGI_RELAY_MANIFEST_MIN_HOPS_V1, KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1, KaigiId,
        KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode, KaigiRecord,
        KaigiRelayAllowlist, KaigiRelayFeedback, KaigiRelayManifest, KaigiRelayRegistration,
        KaigiStatus, kaigi_metadata_key, kaigi_relay_allowlist_key, kaigi_relay_feedback_key,
        kaigi_relay_metadata_key,
    },
    prelude::{AccountId, DomainId, Json, Name},
    query::error::FindError,
};
use mv::storage::StorageReadOnly;
use privacy::{HostPrivacyArtifacts, PrivacyArtifacts};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryFrom,
};
mod privacy;
/// Signature-authenticated account authorizing a Kaigi state transition.
#[derive(Clone, Copy, Debug)]
enum KaigiAuthorization<'a> {
    SignedAccount(&'a AccountId),
}
impl<'a> KaigiAuthorization<'a> {
    fn signed_account(self) -> &'a AccountId {
        match self {
            Self::SignedAccount(authority) => authority,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccessGrant {
    Default,
    PrivacyAuthorized,
    NoRecordUpdate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordUpdateEffect {
    None,
    Roster,
    Usage,
    RelayManifest,
    Status,
}
use super::super::Execute;
trait ExecuteKaigiAuthorized {
    fn execute_authorized(
        self,
        authorization: KaigiAuthorization<'_>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error>;
}
impl ExecuteKaigiAuthorized for CreateKaigi {
    fn execute_authorized(
        self,
        authorization: KaigiAuthorization<'_>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let CreateKaigi {
            call: template,
            commitment,
            nullifier,
            roster_root,
            proof,
        } = self;
        if template.max_participants == Some(0) {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "Kaigi max_participants must be greater than zero when provided".into(),
                ),
            ));
        }
        let authority = authorization.signed_account();
        if authority != template.host() {
            return Err(unauthorized("only the host account may create a Kaigi"));
        }
        state_transaction.world.account(authority)?;
        if resolve_active_kaigi_account(state_transaction, authority)?.as_ref() != Some(authority) {
            return Err(Error::InvariantViolation(
                "Kaigi host must be the unique registered terminal account-id rekey successor"
                    .into(),
            ));
        }
        if let Some(billing_account) = template.billing_account.as_ref() {
            if authority != billing_account {
                return Err(unauthorized(
                    "Kaigi billing account must be the signed host until delegated billing authorization is supported",
                ));
            }
            state_transaction.world.account(billing_account)?;
        }
        match template.privacy_mode {
            KaigiPrivacyMode::Transparent => {
                privacy::ensure_transparent_payload(&PrivacyArtifacts {
                    #[cfg(feature = "kaigi_privacy_mocks")]
                    subject: authority,
                    #[cfg(feature = "kaigi_privacy_mocks")]
                    host: template.host(),
                    commitment: commitment.as_ref(),
                    nullifier: nullifier.as_ref(),
                    roster_root: roster_root.as_ref(),
                    proof: proof.as_deref(),
                })?;
            }
            KaigiPrivacyMode::ZkRosterV1 => {
                let has_privacy_artifacts = commitment.is_some()
                    || nullifier.is_some()
                    || roster_root.is_some()
                    || proof.is_some();
                if has_privacy_artifacts {
                    let host_artifacts = HostPrivacyArtifacts {
                        commitment: commitment.as_ref(),
                        nullifier: nullifier.as_ref(),
                        roster_root: roster_root.as_ref(),
                        proof: proof.as_deref(),
                    };
                    let expected_root = kaigi_zk::empty_roster_root_hash();
                    privacy::verify_host_create(
                        state_transaction,
                        &host_artifacts,
                        &expected_root,
                    )?;
                }
            }
        }
        if let Some(manifest) = template.relay_manifest() {
            validate_relay_manifest(manifest)?;
            ensure_manifest_relays_registered(state_transaction, manifest)?;
        }
        let key = metadata_key(template.id())?;
        let domain_id = template.id().domain_id.clone();
        let domain = state_transaction.world.domain_mut(&domain_id)?;
        if domain.metadata().contains(&key) {
            return Err(Error::InvariantViolation("Kaigi already exists".into()));
        }
        let creation_ms = state_transaction._curr_block.creation_time().as_millis();
        let created_at_ms = u64::try_from(creation_ms).map_err(|_| {
            Error::InvariantViolation("block creation time exceeds u64::MAX milliseconds".into())
        })?;
        let mut record = KaigiRecord::from_new(&template, created_at_ms);
        if template.privacy_mode == KaigiPrivacyMode::ZkRosterV1 {
            record.host_commitment = commitment;
            if let Some(nullifier) = nullifier {
                record.push_nullifier(nullifier);
            }
        }
        store_record(state_transaction, &domain_id, key, &record)?;
        emit_status_summary(state_transaction, &record);
        emit_roster_summary(state_transaction, &record);
        if let Some(manifest) = record.relay_manifest.as_ref() {
            emit_relay_manifest_summary(state_transaction, &record.id, Some(manifest));
            #[cfg(feature = "telemetry")]
            state_transaction.telemetry.record_kaigi_manifest_update(
                &record.id.domain_id,
                "set",
                u32::try_from(manifest.hops.len()).unwrap_or(u32::MAX),
            );
        }
        emit_usage_summary(state_transaction, &record);
        Ok(())
    }
}
impl Execute for CreateKaigi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.execute_authorized(
            KaigiAuthorization::SignedAccount(authority),
            state_transaction,
        )
    }
}
impl ExecuteKaigiAuthorized for JoinKaigi {
    fn execute_authorized(
        self,
        authorization: KaigiAuthorization<'_>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let JoinKaigi {
            call_id,
            participant,
            commitment,
            nullifier,
            roster_root,
            proof,
        } = self;
        let mut commitment = commitment;
        let mut nullifier = nullifier;
        let mut roster_root = roster_root;
        let allow_unassociated = authorization.signed_account() == &participant;
        apply_with_record_authorized(
            state_transaction,
            &call_id,
            authorization,
            allow_unassociated,
            RecordUpdateEffect::Roster,
            |stx, record| {
                stx.world.account(&participant)?;
                process_join(
                    stx,
                    record,
                    authorization,
                    &participant,
                    commitment.take(),
                    nullifier.take(),
                    roster_root.take(),
                    proof.as_deref(),
                )
            },
        )?;
        Ok(())
    }
}
impl Execute for JoinKaigi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.execute_authorized(
            KaigiAuthorization::SignedAccount(authority),
            state_transaction,
        )
    }
}
impl Execute for LeaveKaigi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let LeaveKaigi {
            call_id,
            participant,
            commitment,
            nullifier,
            roster_root,
            proof,
        } = self;
        let mut commitment = commitment;
        let mut nullifier = nullifier;
        let mut roster_root = roster_root;
        apply_with_record(
            state_transaction,
            &call_id,
            authority,
            false,
            RecordUpdateEffect::Roster,
            |stx, record| {
                process_leave(
                    stx,
                    record,
                    authority,
                    &participant,
                    commitment.take(),
                    nullifier.take(),
                    roster_root.take(),
                    proof.as_deref(),
                )
            },
        )?;
        Ok(())
    }
}
impl ExecuteKaigiAuthorized for EndKaigi {
    fn execute_authorized(
        self,
        authorization: KaigiAuthorization<'_>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let EndKaigi {
            call_id,
            ended_at_ms,
            commitment,
            nullifier,
            roster_root,
            proof,
        } = self;
        let end_ms = state_transaction._curr_block.creation_time().as_millis();
        let default_end_ms = u64::try_from(end_ms).map_err(|_| {
            Error::InvariantViolation("block creation time exceeds u64::MAX milliseconds".into())
        })?;
        let resolved_end = ended_at_ms.unwrap_or(default_end_ms);
        apply_with_record_authorized(
            state_transaction,
            &call_id,
            authorization,
            false,
            RecordUpdateEffect::Status,
            move |stx, record| {
                if record.status == KaigiStatus::Ended {
                    return Err(Error::InvariantViolation("Kaigi already ended".into()));
                }
                let authority = authorization.signed_account();
                if !accounts_share_active_lineage(stx, authority, &record.host)? {
                    return Err(unauthorized("only the host may end a Kaigi"));
                }
                if resolved_end < record.created_at_ms {
                    return Err(Error::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "Kaigi end timestamp must not precede creation".into(),
                        ),
                    ));
                }
                if resolved_end > default_end_ms {
                    return Err(Error::InvalidParameter(
                        InvalidParameterError::SmartContract(
                            "Kaigi end timestamp must not exceed the current block time".into(),
                        ),
                    ));
                }
                match record.privacy_mode {
                    KaigiPrivacyMode::Transparent => {
                        privacy::ensure_transparent_payload(&PrivacyArtifacts {
                            #[cfg(feature = "kaigi_privacy_mocks")]
                            subject: authority,
                            #[cfg(feature = "kaigi_privacy_mocks")]
                            host: &record.host,
                            commitment: commitment.as_ref(),
                            nullifier: nullifier.as_ref(),
                            roster_root: roster_root.as_ref(),
                            proof: proof.as_deref(),
                        })?;
                    }
                    KaigiPrivacyMode::ZkRosterV1 => {
                        if let Some(stored_commitment) = record.host_commitment.as_ref() {
                            let provided_nullifier = nullifier
                                .as_ref()
                                .ok_or_else(|| privacy_error("privacy mode requires nullifier"))?;
                            let host_artifacts = HostPrivacyArtifacts {
                                commitment: commitment.as_ref(),
                                nullifier: Some(provided_nullifier),
                                roster_root: roster_root.as_ref(),
                                proof: proof.as_deref(),
                            };
                            let expected_root = record.roster_root();
                            privacy::verify_host_action(
                                stx,
                                &host_artifacts,
                                &expected_root,
                                stored_commitment,
                            )?;
                            if record.has_nullifier(provided_nullifier) {
                                return Err(Error::InvalidParameter(
                                    InvalidParameterError::SmartContract(
                                        "nullifier already used".into(),
                                    ),
                                ));
                            }
                            record.push_nullifier(provided_nullifier.clone());
                        } else if commitment.is_some()
                            || nullifier.is_some()
                            || roster_root.is_some()
                            || proof.is_some()
                        {
                            return Err(privacy_error(
                                "privacy host artifacts require a stored host commitment",
                            ));
                        }
                    }
                }
                record.status = KaigiStatus::Ended;
                record.ended_at_ms = Some(resolved_end);
                Ok(match record.privacy_mode {
                    KaigiPrivacyMode::Transparent => AccessGrant::Default,
                    KaigiPrivacyMode::ZkRosterV1 => AccessGrant::PrivacyAuthorized,
                })
            },
        )?;
        Ok(())
    }
}
impl Execute for EndKaigi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.execute_authorized(
            KaigiAuthorization::SignedAccount(authority),
            state_transaction,
        )
    }
}
impl Execute for RecordKaigiUsage {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let RecordKaigiUsage {
            call_id,
            duration_ms,
            billed_gas,
            usage_commitment,
            proof,
        } = self;
        if duration_ms == 0 {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract("usage duration must be positive".into()),
            ));
        }
        let mut usage_commitment = usage_commitment;
        apply_with_record(
            state_transaction,
            &call_id,
            authority,
            false,
            RecordUpdateEffect::Usage,
            |stx, record| {
                if !accounts_share_active_lineage(stx, authority, &record.host)? {
                    return Err(unauthorized("only the host may record usage for a Kaigi"));
                }
                ensure_kaigi_active(record)?;
                let total_duration_ms = record
                    .total_duration_ms
                    .checked_add(duration_ms)
                    .ok_or_else(|| usage_error("Kaigi total duration exceeds u64::MAX"))?;
                let total_billed_gas = record
                    .total_billed_gas
                    .checked_add(billed_gas)
                    .ok_or_else(|| usage_error("Kaigi total billed gas exceeds u64::MAX"))?;
                let segments_recorded = record
                    .segments_recorded
                    .checked_add(1)
                    .ok_or_else(|| usage_error("Kaigi usage segment count exceeds u32::MAX"))?;
                match record.privacy_mode {
                    KaigiPrivacyMode::Transparent => {
                        if usage_commitment.is_some() || proof.is_some() {
                            return Err(privacy_error(
                                "transparent Kaigi usage must not include privacy payload",
                            ));
                        }
                    }
                    KaigiPrivacyMode::ZkRosterV1 => {
                        let commitment = usage_commitment.take().ok_or_else(|| {
                            privacy_error("privacy mode requires usage commitment")
                        })?;
                        let segment_index = u64::from(record.segments_recorded);
                        let expected = kaigi_zk::compute_usage_commitment_hash(
                            duration_ms,
                            billed_gas,
                            segment_index,
                        );
                        if expected != commitment {
                            return Err(privacy_error(
                                "usage commitment does not match payload parameters",
                            ));
                        }
                        privacy::verify_usage_commitment(stx, proof.as_deref(), &commitment)?;
                        record.push_usage_commitment(commitment);
                    }
                }
                record.total_duration_ms = total_duration_ms;
                record.total_billed_gas = total_billed_gas;
                record.segments_recorded = segments_recorded;
                Ok(AccessGrant::Default)
            },
        )?;
        Ok(())
    }
}
impl Execute for SetKaigiRelayManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let SetKaigiRelayManifest {
            call_id,
            relay_manifest,
        } = self;
        let mut relay_manifest = relay_manifest;
        apply_with_record(
            state_transaction,
            &call_id,
            authority,
            false,
            RecordUpdateEffect::RelayManifest,
            |stx, record| {
                if !accounts_share_active_lineage(stx, authority, &record.host)? {
                    return Err(unauthorized(
                        "only the host may update the Kaigi relay manifest",
                    ));
                }
                ensure_kaigi_active(record)?;
                let previous_manifest = record.relay_manifest.clone();
                if let Some(manifest) = relay_manifest.take() {
                    validate_relay_manifest(&manifest)?;
                    ensure_manifest_relays_registered(stx, &manifest)?;
                    if previous_manifest.as_ref() == Some(&manifest) {
                        return Ok(AccessGrant::NoRecordUpdate);
                    }
                    record.set_relay_manifest(Some(manifest));
                } else {
                    if previous_manifest.is_none() {
                        return Ok(AccessGrant::NoRecordUpdate);
                    }
                    record.set_relay_manifest(None);
                }
                Ok(AccessGrant::Default)
            },
        )?;
        Ok(())
    }
}
impl Execute for RegisterKaigiRelay {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let registration = self.relay;
        if authority != &registration.relay_id {
            return Err(unauthorized(
                "only the relay account may register or update itself",
            ));
        }
        state_transaction.world.account(authority)?;
        validate_relay_hpke_public_key(&registration.hpke_public_key)?;
        if registration.bandwidth_class == 0 {
            return Err(relay_error(
                "relay registration requires a non-zero bandwidth class",
            ));
        }
        let rekey_graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
        let key = kaigi_relay_metadata_key(&registration.relay_id).map_err(|err| {
            Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
        })?;
        let domain_id =
            relay_domain_with_graph(state_transaction, &registration.relay_id, &rekey_graph)?;
        let existing = state_transaction
            .world
            .domain(&domain_id)?
            .metadata()
            .get(&key)
            .cloned()
            .map(|value| decode_stored_relay_registration(&domain_id, &key, value))
            .transpose()?;
        let indexed_domain = state_transaction
            .world
            .kaigi_relay_registry
            .get(&registration.relay_id);
        match (existing.is_some(), indexed_domain) {
            (true, Some(indexed_domain)) if indexed_domain == &domain_id => {}
            (false, None) => {}
            _ => {
                return Err(Error::InvariantViolation(
                    "Kaigi relay metadata and registry index disagree".into(),
                ));
            }
        }
        if existing.as_ref() == Some(&registration) {
            return Ok(());
        }
        ensure_relay_allowed_by_governance_with_graph(
            state_transaction,
            &registration.relay_id,
            &rekey_graph,
        )?;
        if existing.is_none()
            && validated_kaigi_relay_registry_count_with_graph(state_transaction, &rekey_graph)?
                >= KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1
        {
            return Err(relay_error(format!(
                "Kaigi relay registry is at its {KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1}-entry limit"
            )));
        }
        let value = Json::try_new(registration.clone())
            .map_err(|err| Error::Conversion(err.to_string()))?;
        limits::enforce_json_size(
            state_transaction,
            &value,
            "max_metadata_value_bytes",
            limits::DEFAULT_JSON_LIMIT,
        )?;
        {
            let domain = state_transaction.world.domain_mut(&domain_id)?;
            domain.metadata_mut().insert(key.clone(), value.clone());
        }
        if existing.is_none() {
            let replaced = state_transaction
                .world
                .kaigi_relay_registry
                .insert(registration.relay_id.clone(), domain_id.clone());
            debug_assert!(replaced.is_none());
        }
        state_transaction
            .world
            .emit_internal_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
                target: domain_id.clone(),
                key,
                value: value.clone(),
            })));
        emit_relay_registration_summary(state_transaction, &domain_id, &registration);
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_kaigi_relay_registration(&domain_id, registration.bandwidth_class);
        Ok(())
    }
}
impl Execute for UnregisterKaigiRelay {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.account(authority)?;
        let rekey_graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
        if !accounts_share_active_lineage_with_graph(
            state_transaction,
            authority,
            &self.relay_id,
            &rekey_graph,
        )? {
            return Err(unauthorized(
                "only the relay account or its active account-id rekey successor may unregister it",
            ));
        }
        let domain_id = relay_domain_with_graph(state_transaction, &self.relay_id, &rekey_graph)?;
        let registration_key = kaigi_relay_metadata_key(&self.relay_id).map_err(|err| {
            Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
        })?;
        let indexed_domain = state_transaction
            .world
            .kaigi_relay_registry
            .get(&self.relay_id)
            .ok_or_else(|| Error::Find(FindError::MetadataKey(registration_key.clone())))?;
        if indexed_domain != &domain_id {
            return Err(Error::InvariantViolation(
                "Kaigi relay metadata and registry index disagree".into(),
            ));
        }
        let registration_value = state_transaction
            .world
            .domain(&domain_id)?
            .metadata()
            .get(&registration_key)
            .cloned()
            .ok_or_else(|| Error::Find(FindError::MetadataKey(registration_key.clone())))?;
        let registration = decode_stored_relay_registration(
            &domain_id,
            &registration_key,
            registration_value.clone(),
        )?;
        if registration.relay_id != self.relay_id {
            return Err(Error::InvariantViolation(
                "stored Kaigi relay registration identifier does not match unregister target"
                    .into(),
            ));
        }
        let feedback_key = kaigi_relay_feedback_key(&self.relay_id).map_err(|err| {
            Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
        })?;
        let feedback_value = state_transaction
            .world
            .domain(&domain_id)?
            .metadata()
            .get(&feedback_key)
            .cloned();
        if let Some(value) = feedback_value.as_ref() {
            let feedback: KaigiRelayFeedback = value
                .clone()
                .try_into_any_norito()
                .map_err(|err| Error::Conversion(err.to_string()))?;
            if feedback.relay_id != self.relay_id {
                return Err(Error::InvariantViolation(
                    "stored Kaigi relay feedback identifier does not match unregister target"
                        .into(),
                ));
            }
        }

        {
            let domain = state_transaction.world.domain_mut(&domain_id)?;
            domain.metadata_mut().remove(&registration_key);
            if feedback_value.is_some() {
                domain.metadata_mut().remove(&feedback_key);
            }
        }
        let removed = state_transaction
            .world
            .kaigi_relay_registry
            .remove(self.relay_id.clone());
        debug_assert_eq!(removed.as_ref(), Some(&domain_id));
        let mut internal_events = vec![DomainEvent::MetadataRemoved(MetadataChanged {
            target: domain_id.clone(),
            key: registration_key,
            value: registration_value,
        })];
        if let Some(value) = feedback_value {
            internal_events.push(DomainEvent::MetadataRemoved(MetadataChanged {
                target: domain_id.clone(),
                key: feedback_key,
                value,
            }));
        }
        state_transaction
            .world
            .emit_internal_events(internal_events);
        emit_relay_unregistration_summary(state_transaction, &domain_id, &self.relay_id);
        Ok(())
    }
}
impl Execute for ReportKaigiRelayHealth {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let ReportKaigiRelayHealth {
            call_id,
            relay_id,
            status,
            reported_at_ms,
            notes,
        } = self;
        if let Some(ref text) = notes
            && text.chars().count() > 512
        {
            return Err(relay_error(
                "relay health notes must not exceed 512 characters",
            ));
        }
        apply_with_record(
            state_transaction,
            &call_id,
            authority,
            false,
            RecordUpdateEffect::None,
            |stx, record| {
                if !accounts_share_active_lineage(stx, authority, &record.host)? {
                    return Err(unauthorized("only the host may report Kaigi relay health"));
                }
                ensure_kaigi_active(record)?;
                let manifest_contains_relay =
                    record.relay_manifest.as_ref().is_some_and(|manifest| {
                        manifest.hops.iter().any(|hop| hop.relay_id == relay_id)
                    });
                if !manifest_contains_relay {
                    return Err(relay_error(
                        "relay health updates require the relay to appear in the active manifest",
                    ));
                }
                if reported_at_ms > stx.block_unix_timestamp_ms() {
                    return Err(relay_error(
                        "relay health report timestamp exceeds the current block time",
                    ));
                }
                let feedback = KaigiRelayFeedback {
                    relay_id: relay_id.clone(),
                    call: record.id.clone(),
                    reported_by: authority.clone(),
                    status,
                    reported_at_ms,
                    notes: notes.clone(),
                };
                if let Some(current) = load_relay_feedback(stx, &relay_id)? {
                    if feedback.reported_at_ms < current.reported_at_ms {
                        return Err(relay_error(
                            "relay health report is older than the latest stored feedback",
                        ));
                    }
                    if feedback.reported_at_ms == current.reported_at_ms {
                        if feedback == current {
                            return Ok(AccessGrant::NoRecordUpdate);
                        }
                        return Err(relay_error(
                            "relay health report conflicts with stored feedback at the same timestamp",
                        ));
                    }
                }
                let relay_domain = store_relay_feedback(stx, &feedback)?;
                emit_relay_health_summary(stx, &relay_domain, &feedback);
                #[cfg(feature = "telemetry")]
                stx.telemetry
                    .record_kaigi_relay_health(&relay_domain, &relay_id, status);
                Ok(AccessGrant::NoRecordUpdate)
            },
        )?;
        Ok(())
    }
}
fn apply_with_record<F>(
    state_transaction: &mut StateTransaction<'_, '_>,
    call_id: &KaigiId,
    authority: &AccountId,
    allow_unassociated: bool,
    update_effect: RecordUpdateEffect,
    f: F,
) -> Result<(), Error>
where
    F: FnMut(&mut StateTransaction<'_, '_>, &mut KaigiRecord) -> Result<AccessGrant, Error>,
{
    apply_with_record_authorized(
        state_transaction,
        call_id,
        KaigiAuthorization::SignedAccount(authority),
        allow_unassociated,
        update_effect,
        f,
    )
}
fn apply_with_record_authorized<F>(
    state_transaction: &mut StateTransaction<'_, '_>,
    call_id: &KaigiId,
    authorization: KaigiAuthorization<'_>,
    allow_unassociated: bool,
    update_effect: RecordUpdateEffect,
    mut f: F,
) -> Result<(), Error>
where
    F: FnMut(&mut StateTransaction<'_, '_>, &mut KaigiRecord) -> Result<AccessGrant, Error>,
{
    let key = metadata_key(call_id)?;
    let domain_id = call_id.domain_id.clone();
    let domain = state_transaction.world.domain_mut(&domain_id)?;
    let current = domain
        .metadata()
        .get(&key)
        .cloned()
        .ok_or_else(|| Error::Find(FindError::MetadataKey(key.clone())))?;
    let mut record: KaigiRecord = current
        .try_into_any_norito()
        .map_err(|err| Error::Conversion(err.to_string()))?;
    if record.id != *call_id {
        return Err(Error::InvariantViolation(
            "stored Kaigi record identifier does not match metadata key".into(),
        ));
    }
    if record
        .relay_manifest
        .as_ref()
        .is_some_and(|manifest| validate_relay_manifest(manifest).is_err())
    {
        return Err(Error::InvariantViolation(
            "stored Kaigi relay manifest violates V1 constraints".into(),
        ));
    }
    #[cfg(feature = "telemetry")]
    let previous_manifest = record.relay_manifest.clone();
    let authority = authorization.signed_account();
    let mut associated = accounts_share_active_lineage(state_transaction, authority, &record.host)?
        || record_has_participant_in_active_lineage(state_transaction, &record, authority)?;
    let grant: AccessGrant = f(state_transaction, &mut record)?;
    if matches!(grant, AccessGrant::PrivacyAuthorized) {
        associated = true;
    }
    if !allow_unassociated && !associated {
        return Err(unauthorized("account is not associated with the Kaigi"));
    }
    if matches!(grant, AccessGrant::NoRecordUpdate) {
        return Ok(());
    }
    store_record(state_transaction, &domain_id, key, &record)?;
    match update_effect {
        RecordUpdateEffect::None => {}
        RecordUpdateEffect::Roster => emit_roster_summary(state_transaction, &record),
        RecordUpdateEffect::Usage => emit_usage_summary(state_transaction, &record),
        RecordUpdateEffect::RelayManifest => {
            emit_relay_manifest_summary(
                state_transaction,
                &record.id,
                record.relay_manifest.as_ref(),
            );
            #[cfg(feature = "telemetry")]
            {
                let hop_count = record.relay_manifest.as_ref().map_or(0, |manifest| {
                    u32::try_from(manifest.hops.len()).unwrap_or(u32::MAX)
                });
                let action_label =
                    match (previous_manifest.as_ref(), record.relay_manifest.as_ref()) {
                        (_, None) => "clear",
                        (None, Some(_)) => "set",
                        (Some(previous), Some(current)) if previous == current => "set",
                        (Some(_), Some(_)) => "rotate",
                    };
                state_transaction.telemetry.record_kaigi_manifest_update(
                    &record.id.domain_id,
                    action_label,
                    hop_count,
                );
                if action_label == "rotate" {
                    state_transaction.telemetry.record_kaigi_failover(
                        &record.id.domain_id,
                        &record.id.call_name,
                        hop_count,
                    );
                }
            }
        }
        RecordUpdateEffect::Status => emit_status_summary(state_transaction, &record),
    }
    Ok(())
}
fn store_record(
    state_transaction: &mut StateTransaction<'_, '_>,
    domain_id: &DomainId,
    key: Name,
    record: &KaigiRecord,
) -> Result<(), Error> {
    let mut stored_record = record.clone();
    clear_ledger_visible_privacy_hints(&mut stored_record);
    let value = Json::try_new(stored_record).map_err(|err| Error::Conversion(err.to_string()))?;
    limits::enforce_json_size(
        state_transaction,
        &value,
        "max_metadata_value_bytes",
        limits::DEFAULT_JSON_LIMIT,
    )?;
    let domain = state_transaction.world.domain_mut(domain_id)?;
    domain.metadata_mut().insert(key.clone(), value.clone());
    state_transaction
        .world
        .emit_internal_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
            target: domain_id.clone(),
            key,
            value,
        })));
    Ok(())
}
fn clear_ledger_visible_privacy_hints(record: &mut KaigiRecord) {
    if let Some(host_commitment) = record.host_commitment.as_mut() {
        host_commitment.alias_tag = None;
    }
    for commitment in &mut record.roster_commitments {
        commitment.alias_tag = None;
    }
    for nullifier in &mut record.nullifier_log {
        nullifier.issued_at_ms = 0;
    }
}
fn store_relay_feedback(
    state_transaction: &mut StateTransaction<'_, '_>,
    feedback: &KaigiRelayFeedback,
) -> Result<DomainId, Error> {
    let key = kaigi_relay_feedback_key(&feedback.relay_id).map_err(|err| {
        Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
    })?;
    let value =
        Json::try_new(feedback.clone()).map_err(|err| Error::Conversion(err.to_string()))?;
    limits::enforce_json_size(
        state_transaction,
        &value,
        "max_metadata_value_bytes",
        limits::DEFAULT_JSON_LIMIT,
    )?;
    let domain_id = relay_domain(state_transaction, &feedback.relay_id)?;
    {
        let domain = state_transaction.world.domain_mut(&domain_id)?;
        domain.metadata_mut().insert(key.clone(), value.clone());
    }
    state_transaction
        .world
        .emit_internal_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
            target: domain_id.clone(),
            key,
            value,
        })));
    Ok(domain_id)
}
fn metadata_key(call_id: &KaigiId) -> Result<Name, Error> {
    kaigi_metadata_key(&call_id.call_name).map_err(|err| {
        Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
    })
}
/// Return whether a domain metadata key is owned by native Kaigi state.
///
/// The governance-managed relay allowlist deliberately remains outside these
/// prefixes so domain governance can update it through ordinary metadata ISIs.
pub(crate) fn is_reserved_kaigi_metadata_key(key: &Name) -> bool {
    let literal = key.as_ref();
    literal.starts_with("kaigi__")
        || literal.starts_with("kaigi_relay__")
        || literal.starts_with("kaigi_relay_feedback__")
}
/// Reject moving a registered relay's primary alias outside its storage domain.
///
/// Relay descriptors and feedback are domain metadata keyed by the relay ID.
/// A relay must execute `UnregisterKaigiRelay` before changing its primary alias
/// domain so protected state is not stranded in the previous domain.
pub(crate) fn ensure_kaigi_relay_home_change_allowed(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
    current_domain: &DomainId,
    new_domain: Option<&DomainId>,
) -> Result<(), Error> {
    if new_domain == Some(current_domain) {
        return Ok(());
    }
    let domain = state_transaction.world.domain(current_domain)?;
    let relay_ids = domain
        .metadata()
        .iter()
        .filter(|(key, _)| {
            key.as_ref().starts_with("kaigi_relay__")
                || key.as_ref().starts_with("kaigi_relay_feedback__")
        })
        .map(|(key, value)| {
            if key.as_ref().starts_with("kaigi_relay_feedback__") {
                let feedback: KaigiRelayFeedback = value
                    .clone()
                    .try_into_any_norito()
                    .map_err(|err| Error::Conversion(err.to_string()))?;
                let expected_key = kaigi_relay_feedback_key(&feedback.relay_id).map_err(|err| {
                    Error::InvariantViolation(
                        format!("invalid retained Kaigi relay feedback identity: {err}").into(),
                    )
                })?;
                if key != &expected_key {
                    return Err(Error::InvariantViolation(
                        "retained Kaigi relay feedback key does not match its relay ID".into(),
                    ));
                }
                Ok(feedback.relay_id)
            } else {
                let registration: KaigiRelayRegistration = value
                    .clone()
                    .try_into_any_norito()
                    .map_err(|err| Error::Conversion(err.to_string()))?;
                let expected_key =
                    kaigi_relay_metadata_key(&registration.relay_id).map_err(|err| {
                        Error::InvariantViolation(
                            format!("invalid retained Kaigi relay registration identity: {err}")
                                .into(),
                        )
                    })?;
                if key != &expected_key {
                    return Err(Error::InvariantViolation(
                        "retained Kaigi relay registration key does not match its relay ID".into(),
                    ));
                }
                Ok(registration.relay_id)
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;
    for relay_id in relay_ids {
        if accounts_share_active_lineage(state_transaction, account, &relay_id)? {
            return Err(relay_error(
                "registered Kaigi relay primary alias domain is pinned while relay state exists",
            ));
        }
    }
    Ok(())
}
/// Reject account removal while native Kaigi state still depends on the account.
pub(crate) fn ensure_kaigi_account_can_unregister(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<(), Error> {
    let aliases = state_transaction
        .world
        .account_aliases_by_account
        .get(account)
        .cloned()
        .unwrap_or_default();
    ensure_kaigi_account_rekey_records_can_be_removed(
        state_transaction,
        &aliases,
        "unregistering the alias owner",
    )?;
    ensure_kaigi_account_has_no_stranded_dependencies(state_transaction, account, "unregister")
}
/// Reject an aliasless account-id rekey while native Kaigi state needs durable continuity.
pub(crate) fn ensure_kaigi_account_can_rekey_without_continuity(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<(), Error> {
    ensure_kaigi_account_has_no_stranded_dependencies(
        state_transaction,
        account,
        "rekey without durable account-alias continuity for",
    )
}
fn ensure_kaigi_account_has_no_stranded_dependencies(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
    operation: &str,
) -> Result<(), Error> {
    let entries = state_transaction
        .world
        .domains_iter()
        .flat_map(|domain| {
            domain
                .metadata()
                .iter()
                .filter(|(key, _)| is_reserved_kaigi_metadata_key(key))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (key, value) in entries {
        let literal = key.as_ref();
        if literal.starts_with("kaigi__") {
            let record: KaigiRecord = value
                .try_into_any_norito()
                .map_err(|err| Error::Conversion(err.to_string()))?;
            if record.status != KaigiStatus::Active {
                continue;
            }
            let is_host = accounts_share_active_lineage(state_transaction, account, &record.host)?;
            let is_participant =
                record_has_participant_in_active_lineage(state_transaction, &record, account)?;
            let is_manifest_relay = if let Some(manifest) = record.relay_manifest.as_ref() {
                let mut found = false;
                for hop in &manifest.hops {
                    if accounts_share_active_lineage(state_transaction, account, &hop.relay_id)? {
                        found = true;
                        break;
                    }
                }
                found
            } else {
                false
            };
            if is_host || is_participant || is_manifest_relay {
                return Err(Error::InvariantViolation(
                    format!(
                        "cannot {operation} account {account}: it is referenced by active Kaigi {}",
                        record.id
                    )
                    .into(),
                ));
            }
        } else if literal.starts_with("kaigi_relay_feedback__") {
            let feedback: KaigiRelayFeedback = value
                .try_into_any_norito()
                .map_err(|err| Error::Conversion(err.to_string()))?;
            let expected_key = kaigi_relay_feedback_key(&feedback.relay_id).map_err(|err| {
                Error::InvariantViolation(
                    format!("invalid retained Kaigi relay feedback identity: {err}").into(),
                )
            })?;
            if key != expected_key {
                return Err(Error::InvariantViolation(
                    "retained Kaigi relay feedback key does not match its relay ID".into(),
                ));
            }
            if accounts_share_active_lineage(state_transaction, account, &feedback.relay_id)? {
                return Err(Error::InvariantViolation(
                    format!(
                        "cannot {operation} account {account}: it owns retained Kaigi relay feedback"
                    )
                    .into(),
                ));
            }
        } else if literal.starts_with("kaigi_relay__") {
            let registration: KaigiRelayRegistration = value
                .try_into_any_norito()
                .map_err(|err| Error::Conversion(err.to_string()))?;
            let expected_key = kaigi_relay_metadata_key(&registration.relay_id).map_err(|err| {
                Error::InvariantViolation(
                    format!("invalid retained Kaigi relay registration identity: {err}").into(),
                )
            })?;
            if key != expected_key {
                return Err(Error::InvariantViolation(
                    "retained Kaigi relay registration key does not match its relay ID".into(),
                ));
            }
            if accounts_share_active_lineage(state_transaction, account, &registration.relay_id)? {
                return Err(Error::InvariantViolation(
                    format!(
                        "cannot {operation} account {account}: it owns retained Kaigi relay registration"
                    )
                    .into(),
                ));
            }
        }
    }
    Ok(())
}
/// Reject removal of alias continuity records still needed by native Kaigi state.
pub(crate) fn ensure_kaigi_account_rekey_records_can_be_removed(
    state_transaction: &StateTransaction<'_, '_>,
    aliases: &BTreeSet<AccountAlias>,
    operation: &str,
) -> Result<(), Error> {
    for alias in aliases {
        let Some(record) = state_transaction.world.account_rekey_records.get(alias) else {
            continue;
        };
        if &record.label != alias
            || record.previous_account_ids.len() != record.transition_provenance.len()
        {
            return Err(Error::InvariantViolation(
                "cannot remove malformed account-id rekey history used by Kaigi".into(),
            ));
        }
        let mut sequence = record.previous_account_ids.clone();
        sequence.push(record.active_account_id.clone());
        for (index, provenance) in record.transition_provenance.iter().enumerate() {
            if *provenance != AccountRekeyTransitionProvenance::AccountIdRekey {
                continue;
            }
            for identity in [&sequence[index], &sequence[index + 1]] {
                if let Err(error) = ensure_kaigi_account_has_no_stranded_dependencies(
                    state_transaction,
                    identity,
                    "remove retained rekey continuity for",
                ) {
                    return Err(Error::InvariantViolation(
                        format!(
                            "cannot remove account alias {alias:?} while {operation}: its canonical account-id rekey history is required by native Kaigi state ({error})"
                        )
                        .into(),
                    ));
                }
            }
        }
    }
    Ok(())
}
/// Reject activating an account ID retained as a canonical rekey predecessor.
pub(crate) fn ensure_account_id_is_not_retired_rekey_predecessor(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<(), Error> {
    for (label, record) in state_transaction.world.account_rekey_records.iter() {
        for (index, predecessor) in record.previous_account_ids.iter().enumerate() {
            if predecessor != account {
                continue;
            }
            let Some(provenance) = record.transition_provenance.get(index) else {
                return Err(Error::InvariantViolation(
                    "cannot inspect malformed retained account-id rekey history".into(),
                ));
            };
            if *provenance == AccountRekeyTransitionProvenance::AccountIdRekey {
                if label != &record.label {
                    return Err(Error::InvariantViolation(
                        "retained account-id rekey history key does not match its embedded alias"
                            .into(),
                    ));
                }
                return Err(Error::InvariantViolation(
                    format!(
                        "cannot activate account {account}: it is retained as a retired canonical account-id rekey predecessor"
                    )
                    .into(),
                ));
            }
        }
    }
    Ok(())
}
/// Reject domain removal while it contains protected native Kaigi state.
pub(crate) fn ensure_kaigi_domain_can_unregister(
    state_transaction: &StateTransaction<'_, '_>,
    domain_id: &DomainId,
) -> Result<(), Error> {
    let domain = state_transaction.world.domain(domain_id)?;
    if let Some((key, _)) = domain
        .metadata()
        .iter()
        .find(|(key, _)| is_reserved_kaigi_metadata_key(key))
    {
        return Err(Error::InvariantViolation(
            format!(
                "cannot unregister domain {domain_id}: it contains protected native Kaigi state at metadata key {key}"
            )
            .into(),
        ));
    }
    let account_ids = state_transaction
        .world
        .accounts_in_domain_iter(domain_id)
        .map(|account| account.id().clone())
        .collect::<Vec<_>>();
    let mut aliases = BTreeSet::new();
    for account_id in account_ids {
        let Some(account_aliases) = state_transaction
            .world
            .account_aliases_by_account
            .get(&account_id)
        else {
            continue;
        };
        for alias in account_aliases {
            let alias_domain = alias
                .domain_id(state_transaction.world.dataspace_catalog())
                .map_err(|error| {
                    Error::InvariantViolation(
                        format!(
                            "cannot inspect account alias {alias:?} before domain teardown: {error}"
                        )
                        .into(),
                    )
                })?;
            if alias_domain.as_ref() == Some(domain_id) {
                aliases.insert(alias.clone());
            }
        }
    }
    ensure_kaigi_account_rekey_records_can_be_removed(
        state_transaction,
        &aliases,
        "unregistering its alias domain",
    )?;
    Ok(())
}
fn unauthorized(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}
fn privacy_error(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}
fn relay_error(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}
fn usage_error(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}
fn ensure_kaigi_active(record: &KaigiRecord) -> Result<(), Error> {
    if record.status != KaigiStatus::Active {
        return Err(Error::InvariantViolation("Kaigi is not active".into()));
    }
    Ok(())
}
fn resolve_active_kaigi_account(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<Option<AccountId>, Error> {
    let graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
    resolve_active_kaigi_account_with_graph(state_transaction, account, &graph)
}
fn resolve_active_kaigi_account_with_graph(
    state_transaction: &StateTransaction<'_, '_>,
    account: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<Option<AccountId>, Error> {
    let live = crate::sns::resolve_active_account_id_rekey_lineage(
        &state_transaction.world,
        state_transaction.world.dataspace_catalog(),
        account,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|err| {
        Error::InvariantViolation(
            format!("failed to resolve Kaigi account-id rekey lineage: {err}").into(),
        )
    })?;
    let persisted =
        resolve_persisted_kaigi_rekey_successor(&state_transaction.world, account, graph)?;
    if let (Some(live), Some(persisted)) = (&live, &persisted)
        && live != persisted
    {
        return Err(Error::InvariantViolation(
            "live and persisted account-id rekey lineage resolve to different successors".into(),
        ));
    }
    Ok(live.or(persisted))
}
fn accounts_share_active_lineage(
    state_transaction: &StateTransaction<'_, '_>,
    left: &AccountId,
    right: &AccountId,
) -> Result<bool, Error> {
    let graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
    accounts_share_active_lineage_with_graph(state_transaction, left, right, &graph)
}
fn accounts_share_active_lineage_with_graph(
    state_transaction: &StateTransaction<'_, '_>,
    left: &AccountId,
    right: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<bool, Error> {
    let left_active = resolve_active_kaigi_account_with_graph(state_transaction, left, graph)?;
    let right_active = resolve_active_kaigi_account_with_graph(state_transaction, right, graph)?;
    Ok(left_active.is_some() && left_active == right_active)
}
fn resolve_persisted_kaigi_rekey_successor(
    world: &impl WorldReadOnly,
    account: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<Option<AccountId>, Error> {
    let component = persisted_kaigi_rekey_component(&graph.neighbours, account);
    let mut terminal = account.clone();
    while let Some(successor) = graph.forward.get(&terminal) {
        terminal = successor.clone();
    }
    let registered = component
        .into_iter()
        .filter(|candidate| world.account(candidate).is_ok())
        .collect::<BTreeSet<_>>();
    if registered.is_empty() {
        return Ok(None);
    }
    if registered.len() != 1 || !registered.contains(&terminal) {
        return Err(Error::InvariantViolation(
            "persisted account-id rekey history does not have one registered terminal successor"
                .into(),
        ));
    }
    Ok(Some(terminal))
}
#[derive(Default)]
struct PersistedKaigiRekeyGraph {
    forward: BTreeMap<AccountId, AccountId>,
    neighbours: BTreeMap<AccountId, BTreeSet<AccountId>>,
}
fn persisted_kaigi_rekey_graph(
    world: &impl WorldReadOnly,
) -> Result<PersistedKaigiRekeyGraph, Error> {
    let mut graph = PersistedKaigiRekeyGraph::default();
    let mut reverse = BTreeMap::<AccountId, AccountId>::new();
    for (label, record) in world.account_rekey_records().iter() {
        if label != &record.label {
            return Err(Error::InvariantViolation(
                "account-id rekey history key does not match its embedded alias".into(),
            ));
        }
        if record.previous_account_ids.len() != record.transition_provenance.len() {
            return Err(Error::InvariantViolation(
                "malformed account-id rekey history while resolving Kaigi continuity".into(),
            ));
        }
        let mut sequence = record.previous_account_ids.clone();
        sequence.push(record.active_account_id.clone());
        for (index, provenance) in record.transition_provenance.iter().enumerate() {
            if *provenance != AccountRekeyTransitionProvenance::AccountIdRekey {
                continue;
            }
            let predecessor = sequence[index].clone();
            let successor = sequence[index + 1].clone();
            if predecessor == successor
                || graph
                    .forward
                    .insert(predecessor.clone(), successor.clone())
                    .is_some_and(|existing| existing != successor)
                || reverse
                    .insert(successor.clone(), predecessor.clone())
                    .is_some_and(|existing| existing != predecessor)
            {
                return Err(Error::InvariantViolation(
                    "ambiguous account-id rekey history while resolving Kaigi continuity".into(),
                ));
            }
            graph
                .neighbours
                .entry(predecessor.clone())
                .or_default()
                .insert(successor.clone());
            graph
                .neighbours
                .entry(successor)
                .or_default()
                .insert(predecessor);
        }
    }
    let mut completed = BTreeSet::new();
    for start in graph.forward.keys() {
        if completed.contains(start) {
            continue;
        }
        let mut path = BTreeSet::new();
        let mut current = start;
        while let Some(successor) = graph.forward.get(current) {
            if completed.contains(current) {
                break;
            }
            if !path.insert(current.clone()) {
                return Err(Error::InvariantViolation(
                    "cyclic account-id rekey history while resolving Kaigi continuity".into(),
                ));
            }
            current = successor;
        }
        completed.extend(path);
    }
    Ok(graph)
}
fn persisted_kaigi_rekey_component(
    neighbours: &BTreeMap<AccountId, BTreeSet<AccountId>>,
    account: &AccountId,
) -> BTreeSet<AccountId> {
    let mut frontier = vec![account.clone()];
    let mut visited = BTreeSet::new();
    while let Some(account) = frontier.pop() {
        if !visited.insert(account.clone()) {
            continue;
        }
        if let Some(next) = neighbours.get(&account) {
            frontier.extend(next.iter().cloned());
        }
    }
    visited
}
fn record_has_participant_in_active_lineage(
    state_transaction: &StateTransaction<'_, '_>,
    record: &KaigiRecord,
    account: &AccountId,
) -> Result<bool, Error> {
    for participant in &record.participants {
        if accounts_share_active_lineage(state_transaction, participant, account)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn validate_relay_manifest(manifest: &KaigiRelayManifest) -> Result<(), Error> {
    if manifest.hops.len() < KAIGI_RELAY_MANIFEST_MIN_HOPS_V1 {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(
                "relay manifest must include at least three hops".into(),
            ),
        ));
    }
    if manifest.hops.len() > KAIGI_RELAY_MANIFEST_MAX_HOPS_V1 {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "relay manifest must not include more than {KAIGI_RELAY_MANIFEST_MAX_HOPS_V1} hops"
            )),
        ));
    }
    let mut seen_relays = BTreeSet::new();
    for hop in &manifest.hops {
        validate_relay_hpke_public_key(&hop.hpke_public_key)?;
        if hop.weight == 0 {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract("relay weights must be non-zero".into()),
            ));
        }
        if !seen_relays.insert(hop.relay_id.clone()) {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "relay manifest must not contain duplicate relays".into(),
                ),
            ));
        }
    }
    Ok(())
}
fn validate_relay_hpke_public_key(hpke_public_key: &[u8]) -> Result<(), Error> {
    if hpke_public_key.is_empty() {
        return Err(relay_error("relay HPKE public key must be non-empty"));
    }
    if hpke_public_key.len() > KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 {
        return Err(relay_error(format!(
            "relay HPKE public key must not exceed {KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1} bytes"
        )));
    }
    Ok(())
}
fn decode_stored_relay_registration(
    domain_id: &DomainId,
    key: &Name,
    value: Json,
) -> Result<KaigiRelayRegistration, Error> {
    let registration: KaigiRelayRegistration = value.try_into_any_norito().map_err(|err| {
        Error::InvariantViolation(
            format!("malformed Kaigi relay registration in domain {domain_id}: {err}").into(),
        )
    })?;
    let expected_key = kaigi_relay_metadata_key(&registration.relay_id).map_err(|err| {
        Error::InvariantViolation(
            format!("invalid stored Kaigi relay registration identity: {err}").into(),
        )
    })?;
    if key != &expected_key {
        return Err(Error::InvariantViolation(
            "stored Kaigi relay registration key does not match its relay ID".into(),
        ));
    }
    validate_relay_hpke_public_key(&registration.hpke_public_key).map_err(|error| {
        Error::InvariantViolation(
            format!("stored Kaigi relay registration has an invalid HPKE key: {error}").into(),
        )
    })?;
    if registration.bandwidth_class == 0 {
        return Err(Error::InvariantViolation(
            "stored Kaigi relay registration has an invalid zero bandwidth class".into(),
        ));
    }
    Ok(registration)
}
fn collect_kaigi_relay_registry(
    world: &impl WorldReadOnly,
) -> Result<BTreeMap<AccountId, DomainId>, Error> {
    let mut rebuilt = BTreeMap::new();
    for domain in world.domains_iter() {
        for (key, value) in domain.metadata().iter() {
            if !key.as_ref().starts_with("kaigi_relay__") {
                continue;
            }
            let registration = decode_stored_relay_registration(domain.id(), key, value.clone())?;
            if rebuilt
                .insert(registration.relay_id, domain.id().clone())
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    "duplicate Kaigi relay registration found across domains".into(),
                ));
            }
        }
    }
    Ok(rebuilt)
}
/// Rebuild the derived relay-to-domain index from authoritative domain metadata.
///
/// The first-release limit is an admission constraint, not a restore constraint:
/// valid legacy over-cap state remains loadable so relays can retire it.
///
/// # Errors
///
/// Returns an error when an authoritative relay row is malformed or duplicated.
pub(crate) fn rebuild_kaigi_relay_registry(world: &mut World) -> Result<(), String> {
    let rebuilt = collect_kaigi_relay_registry(&world.view()).map_err(|error| error.to_string())?;
    world.kaigi_relay_registry = rebuilt.into_iter().collect();
    Ok(())
}
/// Validate rebuilt relay membership against persisted account lineage and home domains.
///
/// # Errors
///
/// Returns an error when the index disagrees with authoritative metadata or relay lineage.
pub(crate) fn validate_rebuilt_kaigi_relay_registry(
    world: &impl WorldReadOnly,
) -> Result<(), String> {
    let graph = persisted_kaigi_rekey_graph(world).map_err(|error| error.to_string())?;
    for (relay_id, domain_id) in world.kaigi_relay_registry().iter() {
        let key = kaigi_relay_metadata_key(relay_id).map_err(|error| error.to_string())?;
        let value = world
            .domain(domain_id)
            .map_err(|error| error.to_string())?
            .metadata()
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Kaigi relay registry entry for {relay_id} is missing authoritative metadata"
                )
            })?;
        let registration = decode_stored_relay_registration(domain_id, &key, value)
            .map_err(|error| error.to_string())?;
        if &registration.relay_id != relay_id {
            return Err(format!(
                "Kaigi relay registry key {relay_id} does not match its metadata registration"
            ));
        }
        let resolved_domain =
            persisted_relay_domain(world, relay_id, &graph).map_err(|error| error.to_string())?;
        if &resolved_domain != domain_id {
            return Err(format!(
                "Kaigi relay {relay_id} is stored in {domain_id}, outside its persisted home domain {resolved_domain}"
            ));
        }
    }
    Ok(())
}
fn validated_kaigi_relay_registry_count(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<usize, Error> {
    let graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
    validated_kaigi_relay_registry_count_with_graph(state_transaction, &graph)
}
fn validated_kaigi_relay_registry_count_with_graph(
    state_transaction: &StateTransaction<'_, '_>,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<usize, Error> {
    let mut count = 0usize;
    for (relay_id, domain_id) in state_transaction.world.kaigi_relay_registry.iter() {
        let key = kaigi_relay_metadata_key(relay_id).map_err(|err| {
            Error::InvariantViolation(
                format!("invalid indexed Kaigi relay registration identity: {err}").into(),
            )
        })?;
        let value = state_transaction
            .world
            .domain(domain_id)?
            .metadata()
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                Error::InvariantViolation(
                    "Kaigi relay registry entry is missing authoritative metadata".into(),
                )
            })?;
        let registration = decode_stored_relay_registration(domain_id, &key, value)?;
        if &registration.relay_id != relay_id {
            return Err(Error::InvariantViolation(
                "Kaigi relay registry key does not match its metadata registration".into(),
            ));
        }
        let resolved_domain =
            relay_domain_from_persisted_graph(state_transaction, relay_id, graph)?;
        if &resolved_domain != domain_id {
            return Err(Error::InvariantViolation(
                "stored Kaigi relay registration is outside its relay's active home domain".into(),
            ));
        }
        count = count.saturating_add(1);
        if count > KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1 {
            return Ok(count);
        }
    }
    Ok(count)
}
fn ensure_manifest_relays_registered(
    state_transaction: &StateTransaction<'_, '_>,
    manifest: &KaigiRelayManifest,
) -> Result<(), Error> {
    if manifest.expiry_ms <= state_transaction.block_unix_timestamp_ms() {
        return Err(relay_error(
            "relay manifest expiry must be greater than the current block time",
        ));
    }
    let rekey_graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
    let mut active_relays = BTreeSet::new();
    for hop in &manifest.hops {
        let active_relay = resolve_active_kaigi_account_with_graph(
            state_transaction,
            &hop.relay_id,
            &rekey_graph,
        )?
        .ok_or_else(|| relay_error("relay manifest references an unregistered relay account"))?;
        if !active_relays.insert(active_relay) {
            return Err(relay_error(
                "relay manifest must not contain multiple identities from the same active account-id rekey lineage",
            ));
        }
    }
    for hop in &manifest.hops {
        ensure_relay_allowed_by_governance_with_graph(
            state_transaction,
            &hop.relay_id,
            &rekey_graph,
        )?;
        let key = kaigi_relay_metadata_key(&hop.relay_id).map_err(|err| {
            Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
        })?;
        let domain_id = relay_domain_with_graph(state_transaction, &hop.relay_id, &rekey_graph)?;
        let domain = state_transaction.world.domain(&domain_id)?;
        let stored = domain.metadata().get(&key).cloned().ok_or_else(|| {
            relay_error("relay referenced in manifest is not registered in its domain")
        })?;
        let registration: KaigiRelayRegistration = stored
            .try_into_any_norito()
            .map_err(|err| Error::Conversion(err.to_string()))?;
        if registration.relay_id != hop.relay_id {
            return Err(relay_error(
                "stored relay registration identifier does not match its metadata key",
            ));
        }
        if registration.bandwidth_class == 0 {
            return Err(relay_error(
                "stored relay registration has an invalid zero bandwidth class",
            ));
        }
        if registration.hpke_public_key != hop.hpke_public_key {
            return Err(relay_error(
                "relay HPKE public key does not match registered value",
            ));
        }
    }
    Ok(())
}
fn ensure_relay_allowed_by_governance_with_graph(
    state_transaction: &StateTransaction<'_, '_>,
    relay_id: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<(), Error> {
    let domain_id = relay_domain_with_graph(state_transaction, relay_id, graph)?;
    let allowlist = load_allowlist(state_transaction, &domain_id)?;
    if let Some(allowlist) = allowlist {
        let mut allowed = false;
        for allowlisted_relay in &allowlist.allowed_relays {
            if accounts_share_active_lineage_with_graph(
                state_transaction,
                allowlisted_relay,
                relay_id,
                graph,
            )? {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return Err(relay_error(
                "relay is not present in the governance allowlist for its domain",
            ));
        }
    }
    Ok(())
}
fn load_allowlist(
    state_transaction: &StateTransaction<'_, '_>,
    domain_id: &DomainId,
) -> Result<Option<KaigiRelayAllowlist>, Error> {
    let key = kaigi_relay_allowlist_key().map_err(|err| {
        Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
    })?;
    let domain = state_transaction.world.domain(domain_id)?;
    let Some(stored) = domain.metadata().get(&key) else {
        return Ok(None);
    };
    let allowlist: KaigiRelayAllowlist = stored
        .clone()
        .try_into_any_norito()
        .map_err(|err| Error::Conversion(err.to_string()))?;
    Ok(Some(allowlist))
}
fn load_relay_feedback(
    state_transaction: &StateTransaction<'_, '_>,
    relay_id: &AccountId,
) -> Result<Option<KaigiRelayFeedback>, Error> {
    let key = kaigi_relay_feedback_key(relay_id).map_err(|err| {
        Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
    })?;
    let domain_id = relay_domain(state_transaction, relay_id)?;
    let domain = state_transaction.world.domain(&domain_id)?;
    let Some(stored) = domain.metadata().get(&key) else {
        return Ok(None);
    };
    let feedback: KaigiRelayFeedback = stored
        .clone()
        .try_into_any_norito()
        .map_err(|err| Error::Conversion(err.to_string()))?;
    if feedback.relay_id != *relay_id {
        return Err(Error::InvariantViolation(
            "stored Kaigi relay feedback identifier does not match metadata key".into(),
        ));
    }
    Ok(Some(feedback))
}
fn truncate_len(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
fn emit_roster_summary(stx: &mut StateTransaction<'_, '_>, record: &KaigiRecord) {
    let summary = KaigiRosterSummary {
        call: record.id.clone(),
        privacy_mode: record.privacy_mode,
        participant_count: truncate_len(record.participants.len()),
        commitment_count: truncate_len(record.roster_commitments.len()),
        nullifier_count: truncate_len(record.nullifier_log.len()),
        roster_root: match record.privacy_mode {
            KaigiPrivacyMode::ZkRosterV1 => Some(record.roster_root()),
            KaigiPrivacyMode::Transparent => None,
        },
    };
    stx.world
        .emit_events(Some(DomainEvent::KaigiRosterSummary(summary)));
}
fn emit_relay_registration_summary(
    stx: &mut StateTransaction<'_, '_>,
    domain_id: &DomainId,
    registration: &KaigiRelayRegistration,
) {
    let fingerprint = Hash::new(&registration.hpke_public_key);
    let summary = KaigiRelayRegistrationSummary::new(
        domain_id.clone(),
        registration.relay_id.clone(),
        registration.bandwidth_class,
        fingerprint,
    );
    stx.world
        .emit_events(Some(DomainEvent::KaigiRelayRegistered(summary)));
}
fn emit_relay_unregistration_summary(
    stx: &mut StateTransaction<'_, '_>,
    domain_id: &DomainId,
    relay_id: &AccountId,
) {
    let summary = KaigiRelayUnregistrationSummary::new(domain_id.clone(), relay_id.clone());
    stx.world
        .emit_events(Some(DomainEvent::KaigiRelayUnregistered(summary)));
}
fn relay_domain(
    state_transaction: &StateTransaction<'_, '_>,
    relay_id: &AccountId,
) -> Result<DomainId, Error> {
    let graph = persisted_kaigi_rekey_graph(&state_transaction.world)?;
    relay_domain_with_graph(state_transaction, relay_id, &graph)
}
fn relay_domain_with_graph(
    state_transaction: &StateTransaction<'_, '_>,
    relay_id: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<DomainId, Error> {
    let relay_subject =
        resolve_active_kaigi_account_with_graph(state_transaction, relay_id, graph)?.ok_or_else(
            || relay_error("relay account is not registered or in an active rekey lineage"),
        )?;
    active_relay_subject_domain(state_transaction, &relay_subject)
}
fn relay_domain_from_persisted_graph(
    state_transaction: &StateTransaction<'_, '_>,
    relay_id: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<DomainId, Error> {
    let relay_subject =
        resolve_persisted_kaigi_rekey_successor(&state_transaction.world, relay_id, graph)?
            .ok_or_else(|| {
                relay_error("relay account is not registered or in a persisted lineage")
            })?;
    active_relay_subject_domain(state_transaction, &relay_subject)
}
fn active_relay_subject_domain(
    state_transaction: &StateTransaction<'_, '_>,
    relay_subject: &AccountId,
) -> Result<DomainId, Error> {
    let primary_alias = state_transaction
        .world
        .account(relay_subject)?
        .label()
        .cloned()
        .ok_or_else(|| {
            relay_error("relay account requires an active domain-qualified primary alias")
        })?;
    let resolved = crate::sns::resolve_active_account_alias(
        &state_transaction.world,
        state_transaction.world.dataspace_catalog(),
        &primary_alias,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|err| {
        Error::InvariantViolation(
            format!("failed to resolve relay primary account alias `{primary_alias:?}`: {err}")
                .into(),
        )
    })?;
    if resolved.as_ref() != Some(relay_subject) {
        return Err(relay_error(
            "relay account requires an active domain-qualified primary alias",
        ));
    }
    relay_subject_domain(&state_transaction.world, relay_subject)
}
fn relay_subject_domain(
    world: &impl WorldReadOnly,
    relay_subject: &AccountId,
) -> Result<DomainId, Error> {
    let primary_alias = world
        .account(relay_subject)?
        .label()
        .cloned()
        .ok_or_else(|| {
            relay_error("relay account requires an active domain-qualified primary alias")
        })?;
    let domain_id = primary_alias
        .domain_id(world.dataspace_catalog())
        .map_err(|err| {
            Error::InvariantViolation(
                format!("failed to resolve relay primary alias domain `{primary_alias:?}`: {err}")
                    .into(),
            )
        })?
        .ok_or_else(|| {
            relay_error("relay account requires an active domain-qualified primary alias")
        })?;
    world.domain(&domain_id)?;
    Ok(domain_id)
}
fn persisted_relay_domain(
    world: &impl WorldReadOnly,
    relay_id: &AccountId,
    graph: &PersistedKaigiRekeyGraph,
) -> Result<DomainId, Error> {
    let relay_subject = resolve_persisted_kaigi_rekey_successor(world, relay_id, graph)?
        .ok_or_else(|| relay_error("relay account is not registered or in a persisted lineage"))?;
    relay_subject_domain(world, &relay_subject)
}
fn emit_relay_manifest_summary(
    stx: &mut StateTransaction<'_, '_>,
    call_id: &KaigiId,
    manifest: Option<&KaigiRelayManifest>,
) {
    let (hop_count, expiry_ms) = manifest.map_or((0, 0), |manifest| {
        (truncate_len(manifest.hops.len()), manifest.expiry_ms)
    });
    let summary = KaigiRelayManifestSummary {
        call: call_id.clone(),
        hop_count,
        expiry_ms,
    };
    stx.world
        .emit_events(Some(DomainEvent::KaigiRelayManifestUpdated(summary)));
}
fn emit_status_summary(stx: &mut StateTransaction<'_, '_>, record: &KaigiRecord) {
    let summary = KaigiStatusSummary::new(record.id.clone(), record.status, record.ended_at_ms);
    stx.world
        .emit_events(Some(DomainEvent::KaigiStatusChanged(summary)));
}
fn emit_usage_summary(stx: &mut StateTransaction<'_, '_>, record: &KaigiRecord) {
    let summary = KaigiUsageSummary {
        call: record.id.clone(),
        total_duration_ms: record.total_duration_ms,
        total_billed_gas: record.total_billed_gas,
        segments_recorded: truncate_len(record.segments_recorded as usize),
    };
    stx.world
        .emit_events(Some(DomainEvent::KaigiUsageSummary(summary)));
}
fn emit_relay_health_summary(
    stx: &mut StateTransaction<'_, '_>,
    relay_domain: &DomainId,
    feedback: &KaigiRelayFeedback,
) {
    let summary = KaigiRelayHealthSummary::new(
        relay_domain.clone(),
        feedback.call.clone(),
        feedback.relay_id.clone(),
        feedback.status,
        feedback.reported_at_ms,
    );
    stx.world
        .emit_events(Some(DomainEvent::KaigiRelayHealthUpdated(summary)));
}
#[allow(clippy::too_many_arguments)]
fn process_join(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: &mut KaigiRecord,
    authorization: KaigiAuthorization<'_>,
    participant: &AccountId,
    mut commitment: Option<KaigiParticipantCommitment>,
    mut nullifier: Option<KaigiParticipantNullifier>,
    mut roster_root: Option<iroha_crypto::Hash>,
    proof: Option<&[u8]>,
) -> Result<AccessGrant, Error> {
    ensure_kaigi_active(record)?;
    if accounts_share_active_lineage(state_transaction, &record.host, participant)? {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract("host is already part of the call".into()),
        ));
    }
    match record.privacy_mode {
        KaigiPrivacyMode::Transparent => {
            let authority = authorization.signed_account();
            privacy::ensure_transparent_payload(&PrivacyArtifacts {
                #[cfg(feature = "kaigi_privacy_mocks")]
                subject: authority,
                #[cfg(feature = "kaigi_privacy_mocks")]
                host: &record.host,
                commitment: commitment.as_ref(),
                nullifier: nullifier.as_ref(),
                roster_root: roster_root.as_ref(),
                proof,
            })?;
            if authority != participant
                && !accounts_share_active_lineage(state_transaction, authority, &record.host)?
            {
                return Err(unauthorized("only the host may invite other accounts"));
            }
            if record_has_participant_in_active_lineage(state_transaction, record, participant)? {
                return Err(Error::InvariantViolation(
                    "participant already joined".into(),
                ));
            }
            if let Some(limit) = record.max_participants
                && record.participants.len() >= limit as usize
            {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract("participant limit reached".into()),
                ));
            }
            record.push_participant(participant.clone());
            Ok(AccessGrant::Default)
        }
        KaigiPrivacyMode::ZkRosterV1 => {
            let proof_subject = authorization.signed_account();
            if proof_subject != participant {
                return Err(unauthorized(
                    "signed privacy-mode joins must be submitted by the participant",
                ));
            }
            let commitment = commitment
                .take()
                .ok_or_else(|| privacy_error("privacy mode requires commitment"))?;
            let nullifier = nullifier
                .take()
                .ok_or_else(|| privacy_error("privacy mode requires nullifier"))?;
            let proof_bytes = proof.ok_or_else(|| privacy_error("privacy mode requires proof"))?;
            let provided_root = roster_root
                .take()
                .ok_or_else(|| privacy_error("privacy mode requires roster root"))?;
            let artifacts = PrivacyArtifacts {
                #[cfg(feature = "kaigi_privacy_mocks")]
                subject: proof_subject,
                #[cfg(feature = "kaigi_privacy_mocks")]
                host: &record.host,
                commitment: Some(&commitment),
                nullifier: Some(&nullifier),
                roster_root: Some(&provided_root),
                proof: Some(proof_bytes),
            };
            let expected_root = record.roster_root();
            privacy::verify_roster_join(state_transaction, &artifacts, &expected_root)?;
            if record.has_commitment(&commitment) {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract("commitment already registered".into()),
                ));
            }
            if record.has_nullifier(&nullifier) {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract("nullifier already used".into()),
                ));
            }
            if let Some(limit) = record.max_participants
                && record.roster_commitments.len() >= limit as usize
            {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract("participant limit reached".into()),
                ));
            }
            record.push_commitment(commitment);
            record.push_nullifier(nullifier);
            Ok(AccessGrant::PrivacyAuthorized)
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn process_leave(
    state_transaction: &mut StateTransaction<'_, '_>,
    record: &mut KaigiRecord,
    authority: &AccountId,
    participant: &AccountId,
    mut commitment: Option<KaigiParticipantCommitment>,
    mut nullifier: Option<KaigiParticipantNullifier>,
    mut roster_root: Option<iroha_crypto::Hash>,
    proof: Option<&[u8]>,
) -> Result<AccessGrant, Error> {
    ensure_kaigi_active(record)?;
    match record.privacy_mode {
        KaigiPrivacyMode::Transparent => {
            privacy::ensure_transparent_payload(&PrivacyArtifacts {
                #[cfg(feature = "kaigi_privacy_mocks")]
                subject: authority,
                #[cfg(feature = "kaigi_privacy_mocks")]
                host: &record.host,
                commitment: commitment.as_ref(),
                nullifier: nullifier.as_ref(),
                roster_root: roster_root.as_ref(),
                proof,
            })?;
            if !accounts_share_active_lineage(state_transaction, authority, participant)?
                && !accounts_share_active_lineage(state_transaction, authority, &record.host)?
            {
                return Err(unauthorized(
                    "only the host or participant may remove a participant",
                ));
            }
            if accounts_share_active_lineage(state_transaction, &record.host, participant)? {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "host cannot leave the call without ending it".into(),
                    ),
                ));
            }
            let mut matching_index = None;
            for (index, stored_participant) in record.participants.iter().enumerate() {
                if !accounts_share_active_lineage(
                    state_transaction,
                    stored_participant,
                    participant,
                )? {
                    continue;
                }
                if matching_index.replace(index).is_some() {
                    return Err(Error::InvariantViolation(
                        "multiple Kaigi participants resolve to the same active account-id rekey lineage"
                            .into(),
                    ));
                }
            }
            let matching_index = matching_index
                .ok_or_else(|| Error::Find(FindError::Account(participant.clone())))?;
            let stored_participant = record.participants.remove(matching_index);
            record.participant_metadata.remove(&stored_participant);
            Ok(AccessGrant::Default)
        }
        KaigiPrivacyMode::ZkRosterV1 => {
            let _ = (
                state_transaction,
                authority,
                participant,
                commitment.take(),
                nullifier.take(),
                roster_root.take(),
                proof,
            );
            Err(privacy_error(
                "privacy-mode Kaigi leave is off-chain only; use local session disconnect or host end",
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World, WorldReadOnly},
    };
    use core::num::NonZeroU64;
    use iroha_data_model::{
        events::{
            data::prelude::{
                DataEvent, DomainEvent, KaigiRelayRegistrationSummary,
                KaigiRelayUnregistrationSummary, KaigiStatusSummary,
            },
            prelude::EventBox,
        },
        kaigi::{KaigiRelayHop, KaigiRelayManifest, KaigiRelayRegistration, NewKaigi},
        prelude::*,
    };
    use iroha_test_samples::{ALICE_ID, gen_account_in};
    use std::str::FromStr;
    #[test]
    fn unauthorized_error_contains_message() {
        match unauthorized("test message") {
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                assert_eq!(msg, "test message");
            }
            other => panic!("unexpected error variant {other:?}"),
        }
    }
    #[test]
    fn native_kaigi_metadata_key_reservation_excludes_governance_allowlist() {
        let (_domain, host, _) = sample_ids();
        let call_name = Name::from_str("reserved-key").expect("call name");
        for key in [
            kaigi_metadata_key(&call_name).expect("call metadata key"),
            kaigi_relay_metadata_key(&host).expect("relay metadata key"),
            kaigi_relay_feedback_key(&host).expect("relay feedback key"),
        ] {
            assert!(
                is_reserved_kaigi_metadata_key(&key),
                "native Kaigi key {key} must be protected"
            );
        }
        assert!(
            !is_reserved_kaigi_metadata_key(
                &kaigi_relay_allowlist_key().expect("allowlist metadata key")
            ),
            "domain governance must retain the ordinary allowlist update path"
        );
        let unrelated = Name::from_str("kaigi_topic").expect("unrelated metadata key");
        assert!(!is_reserved_kaigi_metadata_key(&unrelated));
    }
    #[test]
    fn transparent_preconditions_accept_empty_payload() {
        #[cfg(feature = "kaigi_privacy_mocks")]
        let (_domain, host, participant) = sample_ids();
        let artifacts = PrivacyArtifacts {
            #[cfg(feature = "kaigi_privacy_mocks")]
            subject: &participant,
            #[cfg(feature = "kaigi_privacy_mocks")]
            host: &host,
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        };
        assert!(privacy::ensure_transparent_payload(&artifacts).is_ok());
    }
    #[test]
    fn transparent_preconditions_reject_privacy_artifacts() {
        #[cfg(feature = "kaigi_privacy_mocks")]
        let (_domain, host, participant) = sample_ids();
        let commitment = sample_commitment();
        let artifacts = PrivacyArtifacts {
            #[cfg(feature = "kaigi_privacy_mocks")]
            subject: &participant,
            #[cfg(feature = "kaigi_privacy_mocks")]
            host: &host,
            commitment: Some(&commitment),
            nullifier: None,
            roster_root: None,
            proof: None,
        };
        let err = privacy::ensure_transparent_payload(&artifacts)
            .expect_err("transparent mode should reject privacy payloads");
        match err {
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                assert!(msg.contains("not accepted"))
            }
            other => panic!("unexpected error variant {other:?}"),
        }
    }
    #[cfg(feature = "kaigi_privacy_mocks")]
    #[test]
    fn zk_preconditions_require_mock_verifier() {
        let (_domain, host, participant) = sample_ids();
        let commitment = sample_commitment();
        let nullifier = sample_nullifier(0xAB);
        let proof = [9, 9, 9];
        let expected_root = iroha_crypto::Hash::prehashed([0u8; 32]);
        let artifacts = PrivacyArtifacts {
            #[cfg(feature = "kaigi_privacy_mocks")]
            subject: &participant,
            #[cfg(feature = "kaigi_privacy_mocks")]
            host: &host,
            commitment: Some(&commitment),
            nullifier: Some(&nullifier),
            roster_root: Some(&expected_root),
            proof: Some(&proof),
        };
        with_state_transaction(|stx| {
            privacy::verify_roster_join(stx, &artifacts, &expected_root).unwrap();
        });
    }
    #[test]
    fn relay_hpke_public_key_validation_enforces_v1_byte_boundaries() {
        assert!(validate_relay_hpke_public_key(&[]).is_err());
        assert!(
            validate_relay_hpke_public_key(&vec![0xA5; KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1])
                .is_ok()
        );
        assert!(
            validate_relay_hpke_public_key(&vec![
                0xA5;
                KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1
            ])
            .is_err()
        );
    }
    #[test]
    fn relay_manifest_validation_enforces_rules() {
        let manifest = KaigiRelayManifest {
            hops: Vec::new(),
            expiry_ms: 1,
        };
        assert!(validate_relay_manifest(&manifest).is_err());
        let (hop_a, _) = gen_account_in("nexus");
        let (hop_b, _) = gen_account_in("nexus");
        let (hop_c, _) = gen_account_in("nexus");
        let valid = KaigiRelayManifest {
            hops: vec![
                KaigiRelayHop {
                    relay_id: hop_a.clone(),
                    hpke_public_key: vec![1, 2, 3],
                    weight: 1,
                },
                KaigiRelayHop {
                    relay_id: hop_b.clone(),
                    hpke_public_key: vec![4, 5, 6],
                    weight: 1,
                },
                KaigiRelayHop {
                    relay_id: hop_c.clone(),
                    hpke_public_key: vec![7, 8, 9],
                    weight: 1,
                },
            ],
            expiry_ms: 42,
        };
        assert!(validate_relay_manifest(&valid).is_ok());
        let mut invalid = valid.clone();
        invalid.hops[1].hpke_public_key.clear();
        assert!(validate_relay_manifest(&invalid).is_err());
        let mut oversized_key = valid.clone();
        oversized_key.hops[1].hpke_public_key =
            vec![0xA5; KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1];
        assert!(validate_relay_manifest(&oversized_key).is_err());
        let mut zero_weight = valid.clone();
        zero_weight.hops[0].weight = 0;
        assert!(validate_relay_manifest(&zero_weight).is_err());
        let mut duplicate = valid.clone();
        duplicate.hops[2].relay_id = hop_a;
        assert!(validate_relay_manifest(&duplicate).is_err());
        let mut too_many = valid;
        while too_many.hops.len() <= KAIGI_RELAY_MANIFEST_MAX_HOPS_V1 {
            too_many.hops.push(too_many.hops[0].clone());
        }
        let error = validate_relay_manifest(&too_many)
            .expect_err("a nine-hop first-release relay route must be rejected");
        assert_smart_contract_error(error, "must not include more than 8 hops");
    }
    #[test]
    fn relay_manifest_rejects_two_ids_from_one_active_rekey_lineage() {
        let (domain, retired_relay, _) = sample_ids();
        let (active_relay, _) = gen_account_in("nexus");
        let (third_relay, _) = gen_account_in("nexus");
        let manifest = KaigiRelayManifest {
            hops: vec![
                KaigiRelayHop {
                    relay_id: retired_relay.clone(),
                    hpke_public_key: vec![1],
                    weight: 1,
                },
                KaigiRelayHop {
                    relay_id: active_relay.clone(),
                    hpke_public_key: vec![2],
                    weight: 1,
                },
                KaigiRelayHop {
                    relay_id: third_relay.clone(),
                    hpke_public_key: vec![3],
                    weight: 1,
                },
            ],
            expiry_ms: 1,
        };
        with_seeded_kaigi_state_transaction(&domain, &[active_relay.clone(), third_relay], |stx| {
            seed_active_account_id_rekey_lineage(
                stx,
                "kaigi-relay-lineage",
                &retired_relay,
                &active_relay,
            );
            let error = ensure_manifest_relays_registered(stx, &manifest)
                .expect_err("one relay lineage must not fill two route hops");
            assert_smart_contract_error(error, "same active account-id rekey lineage");
        });
    }
    #[test]
    fn persisted_account_id_rekey_history_survives_alias_reassignment() {
        use iroha_data_model::{
            account::rekey::{AccountAlias, AccountRekeyRecord},
            nexus::DataSpaceId,
        };
        let (domain, retired, _) = sample_ids();
        let (active, _) = gen_account_in("nexus");
        let (new_alias_owner, _) = gen_account_in("nexus");
        with_seeded_kaigi_state_transaction(
            &domain,
            &[active.clone(), new_alias_owner.clone()],
            |stx| {
                let alias = AccountAlias::domainless(
                    "persisted-kaigi-lineage"
                        .parse()
                        .expect("lineage alias label"),
                    DataSpaceId::UNIVERSAL,
                );
                let history = AccountRekeyRecord::new(alias.clone(), retired.clone())
                    .repoint_for_account_id_rekey(active.clone())
                    .expect("canonical account-id rekey")
                    .reassign_alias_to_account(new_alias_owner.clone())
                    .expect("later independent alias reassignment");
                stx.world
                    .account_rekey_records
                    .insert(alias.clone(), history);
                stx.world
                    .insert_account_alias_binding(alias.clone(), new_alias_owner.clone());

                assert!(
                    accounts_share_active_lineage(stx, &retired, &active)
                        .expect("persisted canonical lineage"),
                    "later alias reassignment must not erase the canonical rekey edge"
                );
                assert!(
                    !accounts_share_active_lineage(stx, &retired, &new_alias_owner)
                        .expect("independent alias owner"),
                    "alias reassignment must never grant Kaigi lineage authority"
                );
                assert_eq!(
                    resolve_active_kaigi_account(stx, &retired)
                        .expect("resolve persisted successor"),
                    Some(active.clone())
                );
                let call = KaigiId::new(
                    domain.clone(),
                    Name::from_str("persisted-host").expect("call name"),
                );
                let record = KaigiRecord::from_new(
                    &NewKaigi::with_defaults(call.clone(), retired.clone()),
                    0,
                );
                store_record(
                    stx,
                    &domain,
                    kaigi_metadata_key(&call.call_name).expect("metadata key"),
                    &record,
                )
                .expect("seed predecessor-hosted call");
                let error = ensure_kaigi_account_can_unregister(stx, &active)
                    .expect_err("the successor must remain protected after alias reassignment");
                assert_invariant_error(error, "referenced by active Kaigi");
                let error = Unregister::account(new_alias_owner.clone())
                    .execute(&ALICE_ID, stx)
                    .expect_err("alias-owner teardown must preserve the canonical rekey edge");
                assert_invariant_error(error, "history is required by native Kaigi state");
                assert!(stx.world.account(&new_alias_owner).is_ok());
                assert_eq!(
                    stx.world.account_aliases.get(&alias),
                    Some(&new_alias_owner)
                );
                assert!(stx.world.account_rekey_records.get(&alias).is_some());
            },
        );
    }
    #[test]
    fn persisted_rekey_history_rejects_a_registered_predecessor() {
        use iroha_data_model::{
            account::rekey::{AccountAlias, AccountRekeyRecord},
            nexus::DataSpaceId,
        };
        let (domain, predecessor, _) = sample_ids();
        let (terminal, _) = gen_account_in("nexus");
        with_seeded_kaigi_state_transaction(
            &domain,
            &[predecessor.clone(), terminal.clone()],
            |stx| {
                let alias = AccountAlias::domainless(
                    "resurrected-kaigi-predecessor"
                        .parse()
                        .expect("lineage alias label"),
                    DataSpaceId::UNIVERSAL,
                );
                let history = AccountRekeyRecord::new(alias.clone(), predecessor.clone())
                    .repoint_for_account_id_rekey(terminal.clone())
                    .expect("canonical account-id rekey");
                stx.world.account_rekey_records.insert(alias, history);

                let error = resolve_active_kaigi_account(stx, &predecessor)
                    .expect_err("a registered predecessor must not regain Kaigi authority");
                assert_invariant_error(error, "one registered terminal successor");
                let error = accounts_share_active_lineage(stx, &predecessor, &predecessor)
                    .expect_err("same-ID comparison must not bypass lineage validation");
                assert_invariant_error(error, "one registered terminal successor");
                let call = KaigiId::new(
                    domain.clone(),
                    Name::from_str("resurrected-host-create").expect("call name"),
                );
                let error = CreateKaigi {
                    call: NewKaigi::with_defaults(call.clone(), predecessor.clone()),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&predecessor, stx)
                .expect_err("a resurrected predecessor must not create stuck Kaigi state");
                assert_invariant_error(error, "one registered terminal successor");
                let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
                assert!(
                    stx.world
                        .domain(&domain)
                        .expect("domain")
                        .metadata()
                        .get(&key)
                        .is_none()
                );
            },
        );
    }
    #[test]
    fn account_registration_rejects_a_retired_rekey_predecessor() {
        use iroha_data_model::{
            account::rekey::{AccountAlias, AccountRekeyRecord},
            nexus::DataSpaceId,
        };
        let (domain, predecessor, _) = sample_ids();
        let (terminal, _) = gen_account_in("nexus");
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&terminal), |stx| {
            let alias = AccountAlias::domainless(
                "retired-registration".parse().expect("lineage alias label"),
                DataSpaceId::UNIVERSAL,
            );
            let history = AccountRekeyRecord::new(alias.clone(), predecessor.clone())
                .repoint_for_account_id_rekey(terminal.clone())
                .expect("canonical account-id rekey");
            stx.world.account_rekey_records.insert(alias, history);

            let error = Register::account(Account::new(predecessor.clone()))
                .execute(&ALICE_ID, stx)
                .expect_err("canonical rekey predecessors must remain retired");
            assert_invariant_error(error, "retired canonical account-id rekey predecessor");
            assert!(matches!(
                stx.world.account(&predecessor),
                Err(FindError::Account(account)) if account == predecessor
            ));
        });
    }
    #[test]
    fn persisted_rekey_history_rejects_a_mismatched_storage_alias() {
        use iroha_data_model::{
            account::rekey::{AccountAlias, AccountRekeyRecord},
            nexus::DataSpaceId,
        };
        let (domain, predecessor, _) = sample_ids();
        let (terminal, _) = gen_account_in("nexus");
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&terminal), |stx| {
            let stored_alias = AccountAlias::domainless(
                "stored-kaigi-lineage".parse().expect("stored alias label"),
                DataSpaceId::UNIVERSAL,
            );
            let embedded_alias = AccountAlias::domainless(
                "embedded-kaigi-lineage"
                    .parse()
                    .expect("embedded alias label"),
                DataSpaceId::UNIVERSAL,
            );
            let history = AccountRekeyRecord::new(embedded_alias, predecessor.clone())
                .repoint_for_account_id_rekey(terminal.clone())
                .expect("canonical account-id rekey");
            stx.world
                .account_rekey_records
                .insert(stored_alias, history);

            let error = resolve_active_kaigi_account(stx, &predecessor)
                .expect_err("the persisted alias key must authenticate its payload");
            assert_invariant_error(error, "key does not match its embedded alias");
        });
    }
    fn sample_ids() -> (DomainId, AccountId, AccountId) {
        let domain = DomainId::try_new("nexus", "universal").expect("domain id");
        let (host, _) = gen_account_in("nexus");
        let (participant, _) = gen_account_in("nexus");
        (domain, host, participant)
    }
    fn new_record(mode: KaigiPrivacyMode) -> (KaigiRecord, AccountId, AccountId) {
        let (domain, host, participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("daily").unwrap());
        let mut template = NewKaigi::with_defaults(call, host.clone());
        template.privacy_mode = mode;
        let record = KaigiRecord::from_new(&template, 1);
        (record, host, participant)
    }
    fn sample_commitment() -> KaigiParticipantCommitment {
        KaigiParticipantCommitment {
            commitment: iroha_crypto::Hash::prehashed([0x11; 32]),
            alias_tag: None,
        }
    }
    fn sample_nullifier(tag: u8) -> KaigiParticipantNullifier {
        KaigiParticipantNullifier {
            digest: iroha_crypto::Hash::prehashed([tag; 32]),
            issued_at_ms: 0,
        }
    }
    fn with_state_transaction<F>(f: F)
    where
        F: FnMut(&mut StateTransaction<'_, '_>),
    {
        with_state_transaction_at(0, f);
    }
    fn with_state_transaction_at<F>(creation_time_ms: u64, mut f: F)
    where
        F: FnMut(&mut StateTransaction<'_, '_>),
    {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query);
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            creation_time_ms,
            0,
        );
        let mut block = state.block(header);
        {
            let mut stx = block.transaction();
            f(&mut stx);
        }
    }
    fn with_seeded_kaigi_state_transaction<F>(domain: &DomainId, accounts: &[AccountId], mut f: F)
    where
        F: FnMut(&mut StateTransaction<'_, '_>),
    {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let seeded_domain = Domain::new(domain.clone()).build(&ALICE_ID);
        let seeded_accounts = accounts
            .iter()
            .cloned()
            .map(|account| Account::new(account).build(&ALICE_ID));
        let state = State::new(
            World::with(
                [seeded_domain],
                seeded_accounts,
                std::iter::empty::<AssetDefinition>(),
            ),
            kura,
            query,
        );
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        {
            let mut stx = block.transaction();
            f(&mut stx);
        }
    }
    fn seed_active_account_id_rekey_lineage(
        stx: &mut StateTransaction<'_, '_>,
        label: &str,
        retired: &AccountId,
        active: &AccountId,
    ) {
        use iroha_data_model::{
            account::{
                AccountAddress,
                rekey::{AccountAlias, AccountRekeyRecord},
            },
            nexus::DataSpaceId,
            sns::{NameControllerV1, NameRecordV1},
        };
        let alias = AccountAlias::domainless(
            label.parse().expect("lineage alias label"),
            DataSpaceId::UNIVERSAL,
        );
        let selector =
            crate::sns::selector_for_account_alias(&alias, stx.world.dataspace_catalog())
                .expect("lineage alias selector");
        let address = AccountAddress::from_account_id(active).expect("active account address");
        let lease = NameRecordV1::new(
            selector.clone(),
            active.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        stx.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&lease),
        );
        stx.world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(alias.clone(), retired.clone())
                .repoint_for_account_id_rekey(active.clone())
                .expect("canonical account-id rekey fixture"),
        );
        stx.world
            .insert_account_alias_binding(alias, active.clone());
    }
    #[test]
    fn transparent_join_and_leave_updates_participants_only() {
        let (mut record, host, participant) = new_record(KaigiPrivacyMode::Transparent);
        with_state_transaction(|stx| {
            let grant = process_join(
                stx,
                &mut record,
                KaigiAuthorization::SignedAccount(&participant),
                &participant,
                None,
                None,
                None,
                None,
            )
            .expect("transparent join");
            assert_eq!(grant, AccessGrant::Default);
            assert!(record.has_participant(&participant));
            assert!(record.roster_commitments.is_empty());
            let leave_grant = process_leave(
                stx,
                &mut record,
                &participant,
                &participant,
                None,
                None,
                None,
                None,
            )
            .expect("transparent leave");
            assert_eq!(leave_grant, AccessGrant::Default);
            assert!(!record.has_participant(&participant));
            let _ = process_join(
                stx,
                &mut record,
                KaigiAuthorization::SignedAccount(&host),
                &participant,
                None,
                None,
                None,
                None,
            )
            .expect("host can re-invite");
        });
    }
    #[test]
    fn record_storage_scrubs_legacy_clear_privacy_hints() {
        let (mut record, _host, _participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        let mut host_commitment = sample_commitment();
        host_commitment.alias_tag = Some("host".to_owned());
        let mut roster_commitment = sample_commitment();
        roster_commitment.alias_tag = Some("participant".to_owned());
        let mut nullifier = sample_nullifier(0xAA);
        nullifier.issued_at_ms = 123;
        record.host_commitment = Some(host_commitment);
        record.roster_commitments.push(roster_commitment);
        record.nullifier_log.push(nullifier);

        clear_ledger_visible_privacy_hints(&mut record);

        assert_eq!(
            record
                .host_commitment
                .as_ref()
                .and_then(|commitment| commitment.alias_tag.as_ref()),
            None
        );
        assert!(
            record
                .roster_commitments
                .iter()
                .all(|commitment| commitment.alias_tag.is_none())
        );
        assert!(
            record
                .nullifier_log
                .iter()
                .all(|nullifier| nullifier.issued_at_ms == 0)
        );
    }
    #[cfg(feature = "kaigi_privacy_mocks")]
    #[test]
    fn privacy_join_updates_commitments_and_leave_stays_off_chain() {
        let (mut record, _host, participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        let commitment = sample_commitment();
        let join_nullifier = sample_nullifier(0xAA);
        let join_proof = [1u8];
        let leave_proof = [2u8];
        with_state_transaction(|stx| {
            let join_root = record.roster_root();
            let grant = process_join(
                stx,
                &mut record,
                KaigiAuthorization::SignedAccount(&participant),
                &participant,
                Some(commitment.clone()),
                Some(join_nullifier.clone()),
                Some(join_root),
                Some(&join_proof),
            )
            .expect("privacy join");
            assert_eq!(grant, AccessGrant::PrivacyAuthorized);
            assert!(record.roster_commitments.len() == 1);
            assert!(record.nullifier_log.len() == 1);
            assert!(record.participants.is_empty());
            let leave_nullifier = sample_nullifier(0xBB);
            let leave_root = record.roster_root();
            let leave_error = process_leave(
                stx,
                &mut record,
                &participant,
                &participant,
                Some(commitment.clone()),
                Some(leave_nullifier.clone()),
                Some(leave_root),
                Some(&leave_proof),
            )
            .expect_err("privacy leave remains off-chain in the first-release profile");
            assert_smart_contract_error(leave_error, "off-chain only");
            assert_eq!(record.roster_commitments.len(), 1);
            assert_eq!(record.nullifier_log.len(), 1);
            let retry_root = record.roster_root();
            let err = process_join(
                stx,
                &mut record,
                KaigiAuthorization::SignedAccount(&participant),
                &participant,
                Some(commitment.clone()),
                Some(join_nullifier.clone()),
                Some(retry_root),
                Some(&join_proof),
            )
            .expect_err("duplicate commitment rejected");
            assert!(matches!(
                err,
                Error::InvalidParameter(InvalidParameterError::SmartContract(_))
            ));
        });
    }
    #[cfg(feature = "kaigi_privacy_mocks")]
    #[test]
    fn mock_privacy_join_respects_max_participant_limit() {
        let (mut record, _host, participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        record.max_participants = Some(1);
        let first_commitment = sample_commitment();
        let first_nullifier = sample_nullifier(0xCC);
        let second_commitment = KaigiParticipantCommitment {
            commitment: iroha_crypto::Hash::prehashed([0x22; 32]),
            alias_tag: None,
        };
        let second_nullifier = sample_nullifier(0xDD);
        with_state_transaction(|stx| {
            let first_root = record.roster_root();
            process_join(
                stx,
                &mut record,
                KaigiAuthorization::SignedAccount(&participant),
                &participant,
                Some(first_commitment.clone()),
                Some(first_nullifier.clone()),
                Some(first_root),
                Some(&[1u8]),
            )
            .expect("first join succeeds within limit");
            let second_root = record.roster_root();
            let err = process_join(
                stx,
                &mut record,
                KaigiAuthorization::SignedAccount(&participant),
                &participant,
                Some(second_commitment.clone()),
                Some(second_nullifier.clone()),
                Some(second_root),
                Some(&[2u8]),
            )
            .expect_err("privacy join beyond limit rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert_eq!(msg, "participant limit reached");
                }
                other => panic!("unexpected error variant {other:?}"),
            }
        });
    }
    #[test]
    fn create_kaigi_emits_roster_summary_and_manifest() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("relayed").unwrap());
        let manifest = sample_manifest();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            let events = stx.world.take_external_events();
            let summary = extract_roster_summary(&events).expect("roster summary event");
            assert_eq!(summary.call, call);
            assert_eq!(summary.privacy_mode, KaigiPrivacyMode::ZkRosterV1);
            assert_eq!(summary.participant_count, 0);
            assert_eq!(summary.commitment_count, 0);
            assert_eq!(summary.nullifier_count, 0);
            let relay = extract_manifest_summary(&events).expect("relay manifest summary");
            assert_eq!(relay.call, call);
            assert_eq!(relay.hop_count, 3);
            assert_eq!(relay.expiry_ms, 42);
        });
    }
    #[test]
    fn create_kaigi_requires_registered_relays() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("registered-relays").expect("call name"),
        );
        let manifest = sample_manifest();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            for hop in &manifest.hops {
                Register::account(Account::new(hop.relay_id.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay account");
                add_relay_to_allowlist(stx, &domain, &hop.relay_id);
            }
            stx.world.take_external_events();

            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            let error = CreateKaigi {
                call: template.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("create must reject an unregistered manifest relay");
            assert_smart_contract_error(error, "not registered");
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            assert!(
                stx.world
                    .domain(&domain)
                    .expect("domain")
                    .metadata()
                    .get(&key)
                    .is_none()
            );
            assert!(stx.world.take_external_events().is_empty());

            for hop in &manifest.hops {
                RegisterKaigiRelay {
                    relay: KaigiRelayRegistration {
                        relay_id: hop.relay_id.clone(),
                        hpke_public_key: hop.hpke_public_key.clone(),
                        bandwidth_class: 1,
                    },
                }
                .execute(&hop.relay_id, stx)
                .expect("register relay descriptor");
            }
            stx.world.take_external_events();
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create succeeds after every relay is registered");
        });
    }
    #[test]
    fn create_kaigi_rejects_zero_participant_limit_without_mutation() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("zero-participant-limit").expect("call name"),
        );
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&host), |stx| {
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.max_participants = Some(0);
            let error = CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("zero max_participants must reject");
            assert_smart_contract_error(error, "greater than zero");
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            assert!(
                stx.world
                    .domain(&domain)
                    .expect("domain")
                    .metadata()
                    .get(&key)
                    .is_none()
            );
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn create_kaigi_rejects_unregistered_host_without_mutation() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("unregistered-host").expect("call name"),
        );
        with_seeded_kaigi_state_transaction(&domain, &[], |stx| {
            let error = CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("an unregistered host must not create a Kaigi record");
            assert!(
                matches!(&error, Error::Find(FindError::Account(account)) if account == &host),
                "unexpected unregistered-host rejection: {error:?}",
            );
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            assert!(
                stx.world
                    .domain(&domain)
                    .expect("domain")
                    .metadata()
                    .get(&key)
                    .is_none()
            );
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn create_kaigi_rejects_third_party_billing_account_without_mutation() {
        let (domain, host, billing_account) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("third-party-billing").expect("call name"),
        );
        with_seeded_kaigi_state_transaction(
            &domain,
            &[host.clone(), billing_account.clone()],
            |stx| {
                let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
                template.billing_account = Some(billing_account.clone());
                let error = CreateKaigi {
                    call: template,
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&host, stx)
                .expect_err("an unsigned billing account must not be charged");
                assert_smart_contract_error(error, "must be the signed host");
                let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
                assert!(
                    stx.world
                        .domain(&domain)
                        .expect("domain")
                        .metadata()
                        .get(&key)
                        .is_none()
                );
                assert!(stx.world.take_external_events().is_empty());
            },
        );
    }
    #[test]
    fn create_kaigi_accepts_explicit_host_billing_account() {
        let (domain, host, _) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("host-billing").expect("call name"),
        );
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&host), |stx| {
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.billing_account = Some(host.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("the signed host may explicitly bill itself");
            assert_eq!(
                load_call_record(stx, &call).billing_account,
                Some(host.clone())
            );
        });
    }
    #[test]
    fn create_and_set_kaigi_reject_expired_relay_manifests() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("manifest-expiry").expect("call name"),
        );
        let mut manifest = sample_manifest();
        manifest.expiry_ms = 42;
        with_state_transaction_at(42, |stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            register_manifest_relays(stx, &domain, &manifest);

            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            let error = CreateKaigi {
                call: template.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("create must reject a manifest expiring at the current block");
            assert_smart_contract_error(error, "expiry");
            assert!(stx.world.take_external_events().is_empty());

            template
                .relay_manifest
                .as_mut()
                .expect("manifest")
                .expiry_ms = 43;
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("future-dated manifest is admitted");
            let current = load_call_record(stx, &call);
            stx.world.take_external_events();

            let error = SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect_err("replacement must reject an expired manifest");
            assert_smart_contract_error(error, "expiry");
            assert_eq!(load_call_record(stx, &call), current);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn private_create_stores_host_commitment() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("private-host").unwrap());
        let commitment = sample_commitment();
        let nullifier = sample_nullifier(0xA1);
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&host), |stx| {
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
            CreateKaigi {
                call: template,
                commitment: Some(commitment.clone()),
                nullifier: Some(nullifier.clone()),
                roster_root: Some(kaigi_zk::empty_roster_root_hash()),
                proof: Some(vec![1, 2, 3]),
            }
            .execute(&host, stx)
            .expect("create privacy-mode Kaigi");
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let domain = stx.world.domain(&call.domain_id).expect("domain");
            let record: KaigiRecord = domain
                .metadata()
                .get(&key)
                .expect("record metadata")
                .clone()
                .try_into_any_norito()
                .expect("deserialize metadata");
            assert_eq!(record.host_commitment.as_ref(), Some(&commitment));
            assert!(record.has_nullifier(&nullifier));
            assert_eq!(record.status, KaigiStatus::Active);
        });
    }
    #[test]
    fn private_end_requires_host_signature_even_with_valid_host_proof() {
        let (domain, host, participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("private-end").unwrap());
        let commitment = sample_commitment();
        let create_nullifier = sample_nullifier(0xA2);
        let end_nullifier = sample_nullifier(0xA3);
        with_seeded_kaigi_state_transaction(&domain, &[host.clone(), participant.clone()], |stx| {
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
            CreateKaigi {
                call: template,
                commitment: Some(commitment.clone()),
                nullifier: Some(create_nullifier.clone()),
                roster_root: Some(kaigi_zk::empty_roster_root_hash()),
                proof: Some(vec![4, 5, 6]),
            }
            .execute(&host, stx)
            .expect("create privacy-mode Kaigi");
            EndKaigi {
                call_id: call.clone(),
                ended_at_ms: Some(0),
                commitment: Some(commitment.clone()),
                nullifier: Some(create_nullifier.clone()),
                roster_root: Some(kaigi_zk::empty_roster_root_hash()),
                proof: Some(vec![7, 8, 9]),
            }
            .execute(&host, stx)
            .expect_err("the host-create nullifier must not be reusable");
            let copied_proof = EndKaigi {
                call_id: call.clone(),
                ended_at_ms: Some(0),
                commitment: Some(commitment.clone()),
                nullifier: Some(end_nullifier.clone()),
                roster_root: Some(kaigi_zk::empty_roster_root_hash()),
                proof: Some(vec![7, 8, 9]),
            };
            copied_proof
                .clone()
                .execute(&participant, stx)
                .expect_err("a copied host proof must not authorize another signer");
            copied_proof
                .execute(&host, stx)
                .expect("the signed host may end a privacy-mode Kaigi");
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let domain = stx.world.domain(&call.domain_id).expect("domain");
            let record: KaigiRecord = domain
                .metadata()
                .get(&key)
                .expect("record metadata")
                .clone()
                .try_into_any_norito()
                .expect("deserialize metadata");
            assert_eq!(record.status, KaigiStatus::Ended);
            assert_eq!(record.ended_at_ms, Some(0));
            assert!(
                record
                    .nullifier_log
                    .iter()
                    .any(|entry| entry.digest == end_nullifier.digest)
            );
        });
    }
    #[test]
    fn signed_host_can_end_private_call_created_without_host_commitment() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("private-end-signed").unwrap(),
        );
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&host), |stx| {
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create privacy-mode Kaigi without host commitment");
            EndKaigi {
                call_id: call.clone(),
                ended_at_ms: Some(0),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("signed host may end without optional privacy artifacts");
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let domain = stx.world.domain(&call.domain_id).expect("domain");
            let record: KaigiRecord = domain
                .metadata()
                .get(&key)
                .expect("record metadata")
                .clone()
                .try_into_any_norito()
                .expect("deserialize metadata");
            assert_eq!(record.status, KaigiStatus::Ended);
        });
    }
    #[test]
    fn host_can_update_relay_manifest() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("manifest-update").unwrap());
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            stx.world.take_external_events();
            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();
            let manifest = sample_manifest();
            for hop in &manifest.hops {
                let relay_domain: DomainId =
                    DomainId::try_new("relay", "universal").expect("relay domain");
                if stx.world.domain(&relay_domain).is_err() {
                    Register::domain(Domain::new(relay_domain.clone()))
                        .execute(&ALICE_ID, stx)
                        .expect("register relay domain");
                }
                Register::account(Account::new(hop.relay_id.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay account");
                add_relay_to_allowlist(stx, &relay_domain, &hop.relay_id);
                RegisterKaigiRelay {
                    relay: KaigiRelayRegistration {
                        relay_id: hop.relay_id.clone(),
                        hpke_public_key: hop.hpke_public_key.clone(),
                        bandwidth_class: 1,
                    },
                }
                .execute(&hop.relay_id, stx)
                .expect("register relay");
                stx.world.take_external_events();
            }
            SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect("set manifest");
            let events = stx.world.take_external_events();
            let summary = extract_manifest_summary(&events).expect("manifest summary event");
            assert_eq!(summary.call, call);
            assert_eq!(summary.hop_count, 3);
            assert_eq!(summary.expiry_ms, 42);
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let domain = stx.world.domain(&call.domain_id).expect("domain");
            let record: KaigiRecord = domain
                .metadata()
                .get(&key)
                .expect("record metadata")
                .clone()
                .try_into_any_norito()
                .expect("deserialize metadata");
            let stored_manifest = record.relay_manifest.expect("stored relay manifest");
            assert_eq!(stored_manifest.expiry_ms, manifest.expiry_ms);
            assert_eq!(stored_manifest.hops.len(), manifest.hops.len());
            for (stored_hop, expected_hop) in stored_manifest.hops.iter().zip(&manifest.hops) {
                assert_eq!(
                    stored_hop.relay_id.subject_id(),
                    expected_hop.relay_id.subject_id()
                );
                assert_eq!(stored_hop.hpke_public_key, expected_hop.hpke_public_key);
                assert_eq!(stored_hop.weight, expected_hop.weight);
            }
        });
    }
    #[test]
    fn non_host_cannot_update_relay_manifest() {
        let (domain, host, participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("manifest-authz").unwrap());
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            Register::account(Account::new(participant.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register participant");
            stx.world.take_external_events();
            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();
            let err = SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(sample_manifest()),
            }
            .execute(&participant, stx)
            .expect_err("non-host update rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert_eq!(msg, "only the host may update the Kaigi relay manifest");
                }
                other => panic!("unexpected error variant {other:?}"),
            }
        });
    }
    #[test]
    fn relay_registration_enforces_allowlist() {
        let (domain, host, _) = sample_ids();
        let (relay_id, _) = gen_account_in(domain.clone());
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            seed_relay_primary_alias_for_testing(stx, &domain, &relay_id);
            stx.world.take_external_events();
            let allowlist_key = kaigi_relay_allowlist_key().expect("allowlist key");
            let allowlist = KaigiRelayAllowlist::default();
            let allowlist_json = Json::try_new(allowlist).expect("serialize allowlist");
            stx.world
                .domain_mut(&domain)
                .expect("domain exists")
                .metadata_mut()
                .insert(allowlist_key, allowlist_json);
            let registration = KaigiRelayRegistration {
                relay_id: relay_id.clone(),
                hpke_public_key: vec![0xAA],
                bandwidth_class: 1,
            };
            let result = RegisterKaigiRelay {
                relay: registration,
            }
            .execute(&relay_id, stx);
            assert!(
                matches!(result, Err(Error::InvalidParameter(_))),
                "registration should fail when relay is not allowlisted"
            );
        });
    }
    #[test]
    fn relay_registration_rejects_allowlist_only_domain_selection_without_mutation() {
        let domain = DomainId::try_new("relay-home", "universal").expect("domain id");
        let (relay_id, _) = gen_account_in("relay-home");
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            let allowlist_key = kaigi_relay_allowlist_key().expect("allowlist key");
            let mut allowlist = KaigiRelayAllowlist::default();
            allowlist.allowed_relays.insert(relay_id.clone());
            stx.world
                .domain_mut(&domain)
                .expect("domain exists")
                .metadata_mut()
                .insert(
                    allowlist_key,
                    Json::try_new(allowlist).expect("serialize allowlist"),
                );
            stx.world.take_external_events();

            let error = RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: relay_id.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                },
            }
            .execute(&relay_id, stx)
            .expect_err("an allowlist must not choose an aliasless relay's home domain");
            assert_smart_contract_error(error, "domain-qualified primary alias");
            let registration_key = kaigi_relay_metadata_key(&relay_id).expect("registration key");
            assert!(
                stx.world
                    .domain(&domain)
                    .expect("domain exists")
                    .metadata()
                    .get(&registration_key)
                    .is_none()
            );
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn unrelated_malformed_allowlist_cannot_block_relay_registration() {
        let home = DomainId::try_new("relay-home", "universal").expect("home domain");
        let unrelated = DomainId::try_new("unrelated", "universal").expect("unrelated domain");
        let (relay_id, _) = gen_account_in("relay-home");
        with_state_transaction(|stx| {
            for domain in [&home, &unrelated] {
                Register::domain(Domain::new(domain.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register domain");
            }
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            add_relay_to_allowlist(stx, &home, &relay_id);
            stx.world
                .domain_mut(&unrelated)
                .expect("unrelated domain exists")
                .metadata_mut()
                .insert(
                    kaigi_relay_allowlist_key().expect("allowlist key"),
                    Json::try_new("not a Kaigi relay allowlist").expect("serialize invalid value"),
                );
            stx.world.take_external_events();

            RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: relay_id.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                },
            }
            .execute(&relay_id, stx)
            .expect("only the relay's authenticated home-domain allowlist is relevant");
            assert!(
                stx.world
                    .domain(&home)
                    .expect("home domain exists")
                    .metadata()
                    .contains(&kaigi_relay_metadata_key(&relay_id).expect("registration key"))
            );
        });
    }
    #[test]
    fn registered_relay_primary_alias_domain_is_pinned() {
        let home = DomainId::try_new("relay-home", "universal").expect("home domain");
        let other = DomainId::try_new("other-home", "universal").expect("other domain");
        let (relay_id, _) = gen_account_in("relay-home");
        with_state_transaction(|stx| {
            for domain in [&home, &other] {
                Register::domain(Domain::new(domain.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register domain");
            }
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            add_relay_to_allowlist(stx, &home, &relay_id);
            RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: relay_id.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                },
            }
            .execute(&relay_id, stx)
            .expect("register relay");

            ensure_kaigi_relay_home_change_allowed(stx, &relay_id, &home, Some(&home))
                .expect("another primary alias in the same domain preserves relay storage");
            let error = ensure_kaigi_relay_home_change_allowed(stx, &relay_id, &home, Some(&other))
                .expect_err("registered relay state must not be stranded in its old domain");
            assert_smart_contract_error(error, "primary alias domain is pinned");
            let error = ensure_kaigi_relay_home_change_allowed(stx, &relay_id, &home, None)
                .expect_err("registered relay must not clear its domain-qualified primary alias");
            assert_smart_contract_error(error, "primary alias domain is pinned");
        });
    }
    #[test]
    fn retained_relay_feedback_alone_pins_primary_alias_domain() {
        let home = DomainId::try_new("relay-home", "universal").expect("home domain");
        let other = DomainId::try_new("other-home", "universal").expect("other domain");
        let (relay_id, _) = gen_account_in("relay-home");
        with_seeded_kaigi_state_transaction(&home, std::slice::from_ref(&relay_id), |stx| {
            let feedback = KaigiRelayFeedback {
                call: KaigiId::new(
                    home.clone(),
                    Name::from_str("retained-feedback").expect("call name"),
                ),
                relay_id: relay_id.clone(),
                reported_by: relay_id.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 1,
                notes: None,
            };
            let key = kaigi_relay_feedback_key(&relay_id).expect("feedback key");
            stx.world
                .domain_mut(&home)
                .expect("home domain")
                .metadata_mut()
                .insert(key, Json::try_new(feedback).expect("serialize feedback"));

            let error = ensure_kaigi_relay_home_change_allowed(stx, &relay_id, &home, Some(&other))
                .expect_err("retained feedback must pin relay storage even without a descriptor");
            assert_smart_contract_error(error, "primary alias domain is pinned");
        });
    }
    #[test]
    fn corrupt_retained_relay_keys_fail_closed_for_alias_move_and_unregister() {
        let home = DomainId::try_new("relay-home", "universal").expect("home domain");
        let other = DomainId::try_new("other-home", "universal").expect("other domain");
        let (key_relay, _) = gen_account_in("relay-home");
        let (payload_relay, _) = gen_account_in("relay-home");
        with_seeded_kaigi_state_transaction(
            &home,
            &[key_relay.clone(), payload_relay.clone()],
            |stx| {
                let descriptor_key = kaigi_relay_metadata_key(&key_relay).expect("descriptor key");
                let descriptor = KaigiRelayRegistration {
                    relay_id: payload_relay.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                };
                stx.world
                    .domain_mut(&home)
                    .expect("home domain")
                    .metadata_mut()
                    .insert(
                        descriptor_key.clone(),
                        Json::try_new(descriptor).expect("serialize descriptor"),
                    );
                for error in [
                    ensure_kaigi_relay_home_change_allowed(stx, &key_relay, &home, Some(&other))
                        .expect_err("corrupt descriptor key must reject alias movement"),
                    ensure_kaigi_account_can_unregister(stx, &key_relay)
                        .expect_err("corrupt descriptor key must reject account removal"),
                ] {
                    assert_invariant_error(error, "registration key does not match its relay ID");
                }
                stx.world
                    .domain_mut(&home)
                    .expect("home domain")
                    .metadata_mut()
                    .remove(&descriptor_key);

                let feedback_key = kaigi_relay_feedback_key(&key_relay).expect("feedback key");
                let feedback = KaigiRelayFeedback {
                    call: KaigiId::new(
                        home.clone(),
                        Name::from_str("corrupt-feedback").expect("call name"),
                    ),
                    relay_id: payload_relay.clone(),
                    reported_by: payload_relay.clone(),
                    status: KaigiRelayHealthStatus::Healthy,
                    reported_at_ms: 1,
                    notes: None,
                };
                stx.world
                    .domain_mut(&home)
                    .expect("home domain")
                    .metadata_mut()
                    .insert(
                        feedback_key,
                        Json::try_new(feedback).expect("serialize feedback"),
                    );
                for error in [
                    ensure_kaigi_relay_home_change_allowed(stx, &key_relay, &home, Some(&other))
                        .expect_err("corrupt feedback key must reject alias movement"),
                    ensure_kaigi_account_can_unregister(stx, &key_relay)
                        .expect_err("corrupt feedback key must reject account removal"),
                ] {
                    assert_invariant_error(error, "feedback key does not match its relay ID");
                }
            },
        );
    }
    #[test]
    fn retired_relay_allowlist_entry_authorizes_only_the_active_successor_id() {
        let home = DomainId::try_new("relay-home", "universal").expect("home domain");
        let (retired_relay, _) = gen_account_in("relay-home");
        let (active_relay, _) = gen_account_in("relay-home");
        with_state_transaction(|stx| {
            Register::domain(Domain::new(home.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(active_relay.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register active relay account");
            seed_active_account_id_rekey_lineage(
                stx,
                "retired-relay-lineage",
                &retired_relay,
                &active_relay,
            );
            seed_relay_primary_alias_for_testing(stx, &home, &active_relay);
            let mut allowlist = KaigiRelayAllowlist::default();
            allowlist.allowed_relays.insert(retired_relay.clone());
            stx.world
                .domain_mut(&home)
                .expect("home domain exists")
                .metadata_mut()
                .insert(
                    kaigi_relay_allowlist_key().expect("allowlist key"),
                    Json::try_new(allowlist).expect("serialize allowlist"),
                );
            stx.world.take_external_events();

            let error = RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: retired_relay.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                },
            }
            .execute(&active_relay, stx)
            .expect_err("the successor must not write a descriptor under the retired ID");
            assert_smart_contract_error(error, "only the relay account");
            assert!(stx.world.take_external_events().is_empty());

            RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: active_relay.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                },
            }
            .execute(&active_relay, stx)
            .expect("an explicit rekey successor inherits relay governance membership");
            assert!(
                stx.world
                    .domain(&home)
                    .expect("home domain exists")
                    .metadata()
                    .contains(&kaigi_relay_metadata_key(&active_relay).expect("registration key"))
            );
        });
    }
    #[test]
    fn call_feedback_does_not_override_manifest_governance() {
        let (domain, host, _) = sample_ids();
        let (relay_a, _) = gen_account_in(domain.clone());
        let (relay_b, _) = gen_account_in(domain.clone());
        let (relay_c, _) = gen_account_in(domain.clone());
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            for relay in [&relay_a, &relay_b, &relay_c] {
                Register::account(Account::new(relay.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay account");
                seed_relay_primary_alias_for_testing(stx, &domain, relay);
            }
            stx.world.take_external_events();
            let allowlist_key = kaigi_relay_allowlist_key().expect("allowlist key");
            let mut allowlist = KaigiRelayAllowlist::default();
            allowlist
                .allowed_relays
                .extend([relay_a.clone(), relay_b.clone(), relay_c.clone()]);
            let allowlist_json = Json::try_new(allowlist).expect("serialize allowlist");
            stx.world
                .domain_mut(&domain)
                .expect("domain exists")
                .metadata_mut()
                .insert(allowlist_key, allowlist_json);
            let registrations = [
                (relay_a.clone(), vec![1, 2, 3]),
                (relay_b.clone(), vec![4, 5, 6]),
                (relay_c.clone(), vec![7, 8, 9]),
            ];
            for (relay, hpke_key) in &registrations {
                RegisterKaigiRelay {
                    relay: KaigiRelayRegistration {
                        relay_id: relay.clone(),
                        hpke_public_key: hpke_key.clone(),
                        bandwidth_class: 1,
                    },
                }
                .execute(relay, stx)
                .expect("register relay");
            }
            stx.world.take_external_events();
            let call = KaigiId::new(
                domain.clone(),
                Name::from_str("relay-outage").expect("call name"),
            );
            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();
            let feedback = KaigiRelayFeedback {
                relay_id: relay_a.clone(),
                call: call.clone(),
                reported_by: host.clone(),
                status: KaigiRelayHealthStatus::Unavailable,
                reported_at_ms: 7,
                notes: Some("offline".to_owned()),
            };
            store_relay_feedback(stx, &feedback).expect("store feedback");
            stx.world.take_external_events();
            let manifest = KaigiRelayManifest {
                hops: vec![
                    KaigiRelayHop {
                        relay_id: relay_a.clone(),
                        hpke_public_key: vec![1, 2, 3],
                        weight: 1,
                    },
                    KaigiRelayHop {
                        relay_id: relay_b.clone(),
                        hpke_public_key: vec![4, 5, 6],
                        weight: 1,
                    },
                    KaigiRelayHop {
                        relay_id: relay_c.clone(),
                        hpke_public_key: vec![7, 8, 9],
                        weight: 1,
                    },
                ],
                expiry_ms: 100,
            };
            SetKaigiRelayManifest {
                call_id: call,
                relay_manifest: Some(manifest),
            }
            .execute(&host, stx)
            .expect("call-scoped health feedback must not become a global manifest-admission veto");
            assert_eq!(
                load_relay_feedback(stx, &relay_a)
                    .expect("load feedback")
                    .expect("stored feedback"),
                feedback
            );
        });
    }
    #[test]
    fn host_can_clear_relay_manifest() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("manifest-clear").unwrap());
        let manifest = sample_manifest();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();
            SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: None,
            }
            .execute(&host, stx)
            .expect("clear manifest");
            let events = stx.world.take_external_events();
            let summary = extract_manifest_summary(&events).expect("manifest summary event");
            assert_eq!(summary.call, call);
            assert_eq!(summary.hop_count, 0);
            assert_eq!(summary.expiry_ms, 0);
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let domain = stx.world.domain(&call.domain_id).expect("domain");
            let record: KaigiRecord = domain
                .metadata()
                .get(&key)
                .expect("record metadata")
                .clone()
                .try_into_any_norito()
                .expect("deserialize metadata");
            assert!(record.relay_manifest.is_none());
        });
    }
    #[test]
    fn unchanged_relay_manifest_update_is_event_free() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("manifest-idempotent").unwrap(),
        );
        let manifest = sample_manifest();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create Kaigi with relay manifest");
            stx.world.take_external_events();

            SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect("repeat identical manifest");
            assert!(
                stx.world.take_external_events().is_empty(),
                "an identical manifest must not emit metadata or summary events"
            );

            SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: None,
            }
            .execute(&host, stx)
            .expect("clear manifest");
            assert!(
                extract_manifest_summary(&stx.world.take_external_events()).is_some(),
                "the first clear must emit a manifest summary"
            );

            SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: None,
            }
            .execute(&host, stx)
            .expect("repeat clear on an empty manifest");
            assert!(
                stx.world.take_external_events().is_empty(),
                "clearing an already-empty manifest must not emit events"
            );
        });
    }
    #[test]
    fn relay_registration_persists_metadata_and_emits_summary() {
        let (relay_id, _relay_account) = gen_account_in("relay");
        let domain_id = DomainId::try_new("relay", "universal").expect("domain id");
        let registration = KaigiRelayRegistration {
            relay_id: relay_id.clone(),
            hpke_public_key: vec![0x10, 0x20, 0x30],
            bandwidth_class: 5,
        };
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            add_relay_to_allowlist(stx, &domain_id, &relay_id);
            stx.world.take_external_events();
            let oversized_error = RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: relay_id.clone(),
                    hpke_public_key: vec![0xA5; KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1],
                    bandwidth_class: 5,
                },
            }
            .execute(&relay_id, stx)
            .expect_err("oversized relay key must fail before persistence or event emission");
            assert_smart_contract_error(oversized_error, "must not exceed 4096 bytes");
            let key = kaigi_relay_metadata_key(&relay_id).expect("metadata key");
            assert!(
                stx.world
                    .domain(&domain_id)
                    .expect("domain lookup")
                    .metadata()
                    .get(&key)
                    .is_none()
            );
            assert!(stx.world.take_external_events().is_empty());
            RegisterKaigiRelay {
                relay: registration.clone(),
            }
            .execute(&relay_id, stx)
            .expect("register relay");
            let events = stx.world.take_external_events();
            assert_eq!(events.len(), 1, "full relay metadata must remain internal");
            let summary =
                extract_registration_summary(&events).expect("relay registration summary event");
            assert_eq!(summary.relay().subject_id(), relay_id.subject_id());
            assert_eq!(*summary.bandwidth_class(), 5);
            let domain = stx.world.domain(&domain_id).expect("domain lookup");
            let stored = domain
                .metadata()
                .get(&key)
                .cloned()
                .expect("relay metadata");
            let decoded: KaigiRelayRegistration = stored
                .try_into_any_norito()
                .expect("decode relay registration");
            assert_eq!(
                decoded.relay_id.subject_id(),
                registration.relay_id.subject_id()
            );
            assert_eq!(decoded.hpke_public_key, registration.hpke_public_key);
            assert_eq!(decoded.bandwidth_class, registration.bandwidth_class);
            assert_eq!(
                stx.world.kaigi_relay_registry.get(&relay_id),
                Some(&domain_id)
            );

            stx.world
                .domain_mut(&domain_id)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    kaigi_relay_allowlist_key().expect("allowlist key"),
                    Json::try_new(KaigiRelayAllowlist::default())
                        .expect("serialize empty allowlist"),
                );
            let internal_len = stx.world.internal_event_buf.len();
            RegisterKaigiRelay {
                relay: registration.clone(),
            }
            .execute(&relay_id, stx)
            .expect("identical relay registration is an idempotent lookup even after delisting");
            assert!(stx.world.take_external_events().is_empty());
            assert_eq!(stx.world.internal_event_buf.len(), internal_len);

            add_relay_to_allowlist(stx, &domain_id, &relay_id);
            let mut rotated = registration.clone();
            rotated.bandwidth_class = 6;
            RegisterKaigiRelay {
                relay: rotated.clone(),
            }
            .execute(&relay_id, stx)
            .expect("changed relay descriptor updates in place");
            let events = stx.world.take_external_events();
            assert_eq!(events.len(), 1, "descriptor updates expose only a summary");
            assert_eq!(
                *extract_registration_summary(&events)
                    .expect("updated registration summary")
                    .bandwidth_class(),
                6
            );
            assert_eq!(
                load_relay_registration(stx, &domain_id, &relay_id),
                Some(rotated)
            );
        });
    }
    #[test]
    fn relay_registry_cap_rejects_new_entries_but_allows_update_and_retirement() {
        let domain_id = DomainId::try_new("relay-cap", "universal").expect("domain id");
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay domain");
            let mut relay_ids = Vec::with_capacity(KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1 + 1);
            for index in 0..=KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1 {
                let (relay_id, _) = gen_account_in("relay-cap");
                Register::account(Account::new(relay_id.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay account");
                seed_relay_primary_alias_for_testing(stx, &domain_id, &relay_id);
                if index < KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1 {
                    let registration = KaigiRelayRegistration {
                        relay_id: relay_id.clone(),
                        hpke_public_key: vec![u8::try_from(index % 251).expect("bounded byte") + 1],
                        bandwidth_class: 1,
                    };
                    stx.world
                        .domain_mut(&domain_id)
                        .expect("relay domain")
                        .metadata_mut()
                        .insert(
                            kaigi_relay_metadata_key(&relay_id).expect("registration key"),
                            Json::try_new(registration).expect("serialize registration"),
                        );
                    stx.world
                        .kaigi_relay_registry
                        .insert(relay_id.clone(), domain_id.clone());
                }
                relay_ids.push(relay_id);
            }
            let existing = relay_ids[0].clone();
            let new_relay = relay_ids.last().expect("extra relay account").clone();
            add_relay_to_allowlist(stx, &domain_id, &existing);
            add_relay_to_allowlist(stx, &domain_id, &new_relay);
            stx.world.take_external_events();

            let rotated = KaigiRelayRegistration {
                relay_id: existing.clone(),
                hpke_public_key: vec![0xAA, 0xBB],
                bandwidth_class: 2,
            };
            RegisterKaigiRelay {
                relay: rotated.clone(),
            }
            .execute(&existing, stx)
            .expect("an existing descriptor may rotate at the registry cap");
            assert_eq!(
                load_relay_registration(stx, &domain_id, &existing),
                Some(rotated)
            );
            stx.world.take_external_events();

            let rejected = KaigiRelayRegistration {
                relay_id: new_relay.clone(),
                hpke_public_key: vec![0xCC],
                bandwidth_class: 1,
            };
            let error = RegisterKaigiRelay {
                relay: rejected.clone(),
            }
            .execute(&new_relay, stx)
            .expect_err("a new descriptor must be rejected at the registry cap");
            assert_smart_contract_error(error, "500-entry limit");
            assert_eq!(load_relay_registration(stx, &domain_id, &new_relay), None);
            assert!(stx.world.take_external_events().is_empty());

            stx.world
                .domain_mut(&domain_id)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    kaigi_relay_metadata_key(&new_relay).expect("registration key"),
                    Json::try_new(rejected).expect("serialize legacy over-cap registration"),
                );
            stx.world
                .kaigi_relay_registry
                .insert(new_relay.clone(), domain_id.clone());
            assert_eq!(
                collect_kaigi_relay_registry(&stx.world)
                    .expect("valid legacy over-cap registry remains rebuildable")
                    .len(),
                KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1 + 1
            );
            assert_eq!(
                validated_kaigi_relay_registry_count(stx).expect("validate over-cap registry"),
                KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1 + 1
            );
            UnregisterKaigiRelay {
                relay_id: existing.clone(),
            }
            .execute(&existing, stx)
            .expect("retirement must remain available above the cap");
            assert_eq!(
                validated_kaigi_relay_registry_count(stx).expect("validate repaired registry"),
                KAIGI_RELAY_REGISTRY_MAX_ENTRIES_V1
            );
        });
    }
    #[test]
    fn relay_registry_rebuild_fails_closed_on_malformed_row() {
        let domain_id = DomainId::try_new("relay-corrupt", "universal").expect("domain id");
        let (corrupt_key_owner, _) = gen_account_in("relay-corrupt");
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay domain");
            Register::account(Account::new(corrupt_key_owner.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            seed_relay_primary_alias_for_testing(stx, &domain_id, &corrupt_key_owner);
            let corrupt_key =
                kaigi_relay_metadata_key(&corrupt_key_owner).expect("corrupt registration key");
            stx.world
                .domain_mut(&domain_id)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    corrupt_key,
                    Json::try_new("not a relay registration").expect("serialize corrupt row"),
                );
            let error = collect_kaigi_relay_registry(&stx.world)
                .expect_err("a malformed registry row must fail closed during rebuild");
            assert_invariant_error(error, "malformed Kaigi relay registration");
        });
    }
    #[test]
    fn relay_registry_rebuild_rejects_duplicate_relay_rows() {
        let first = DomainId::try_new("relay-first", "universal").expect("first domain id");
        let second = DomainId::try_new("relay-second", "universal").expect("second domain id");
        let (relay_id, _) = gen_account_in("relay-first");
        with_state_transaction(|stx| {
            for domain_id in [&first, &second] {
                Register::domain(Domain::new(domain_id.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay domain");
                let registration = KaigiRelayRegistration {
                    relay_id: relay_id.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                };
                stx.world
                    .domain_mut(domain_id)
                    .expect("relay domain")
                    .metadata_mut()
                    .insert(
                        kaigi_relay_metadata_key(&relay_id).expect("registration key"),
                        Json::try_new(registration).expect("serialize registration"),
                    );
            }

            let error = collect_kaigi_relay_registry(&stx.world)
                .expect_err("duplicate relay rows must fail state rebuild");
            assert_invariant_error(error, "duplicate Kaigi relay registration");
        });
    }
    #[test]
    fn rebuilt_relay_registry_rejects_mis_homed_metadata() {
        let home = DomainId::try_new("relay-home", "universal").expect("home domain id");
        let wrong = DomainId::try_new("relay-wrong", "universal").expect("wrong domain id");
        let (relay_id, _) = gen_account_in("relay-home");
        with_state_transaction(|stx| {
            for domain_id in [&home, &wrong] {
                Register::domain(Domain::new(domain_id.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay domain");
            }
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            seed_relay_primary_alias_for_testing(stx, &home, &relay_id);
            let registration = KaigiRelayRegistration {
                relay_id: relay_id.clone(),
                hpke_public_key: vec![0xAA],
                bandwidth_class: 1,
            };
            stx.world
                .domain_mut(&wrong)
                .expect("wrong domain")
                .metadata_mut()
                .insert(
                    kaigi_relay_metadata_key(&relay_id).expect("registration key"),
                    Json::try_new(registration).expect("serialize registration"),
                );
            let rebuilt =
                collect_kaigi_relay_registry(&stx.world).expect("structurally valid registry");
            for (relay_id, domain_id) in rebuilt {
                stx.world.kaigi_relay_registry.insert(relay_id, domain_id);
            }

            let error = validate_rebuilt_kaigi_relay_registry(&stx.world)
                .expect_err("mis-homed relay metadata must fail state rebuild");
            assert!(
                error.contains("outside its persisted home domain"),
                "unexpected rebuild error: {error}"
            );
        });
    }
    #[test]
    fn relay_registration_fails_closed_when_metadata_and_index_disagree() {
        let domain_id = DomainId::try_new("relay-index", "universal").expect("domain id");
        let (relay_id, _) = gen_account_in("relay-index");
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay domain");
            Register::account(Account::new(relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register relay account");
            seed_relay_primary_alias_for_testing(stx, &domain_id, &relay_id);
            add_relay_to_allowlist(stx, &domain_id, &relay_id);
            let registration = KaigiRelayRegistration {
                relay_id: relay_id.clone(),
                hpke_public_key: vec![0xAA],
                bandwidth_class: 1,
            };
            stx.world
                .domain_mut(&domain_id)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    kaigi_relay_metadata_key(&relay_id).expect("registration key"),
                    Json::try_new(registration.clone()).expect("serialize registration"),
                );

            let error = RegisterKaigiRelay {
                relay: registration,
            }
            .execute(&relay_id, stx)
            .expect_err("an unindexed metadata row must not pass as an idempotent registration");
            assert_invariant_error(error, "metadata and registry index disagree");
        });
    }
    #[test]
    fn relay_retirement_is_atomic_and_active_manifests_remain_self_contained() {
        let (domain, host, _) = sample_ids();
        let call_id = KaigiId::new(
            domain.clone(),
            Name::from_str("retired-relay").expect("call name"),
        );
        let manifest = sample_manifest();
        let retired_relay = manifest.hops[0].relay_id.clone();
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&host), |stx| {
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call_id.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create call with pinned relay manifest");
            let feedback = KaigiRelayFeedback {
                relay_id: retired_relay.clone(),
                call: call_id.clone(),
                reported_by: host.clone(),
                status: KaigiRelayHealthStatus::Degraded,
                reported_at_ms: 0,
                notes: Some("retiring".to_owned()),
            };
            store_relay_feedback(stx, &feedback).expect("seed relay feedback");
            stx.world.take_external_events();
            let internal_len = stx.world.internal_event_buf.len();

            UnregisterKaigiRelay {
                relay_id: retired_relay.clone(),
            }
            .execute(&retired_relay, stx)
            .expect("relay may retire while an existing manifest pins its descriptor");
            let events = stx.world.take_external_events();
            assert_eq!(events.len(), 1, "removal metadata must remain internal");
            let summary = extract_unregistration_summary(&events)
                .expect("relay unregistration summary event");
            assert_eq!(summary.relay(), &retired_relay);
            assert_eq!(summary.domain(), &domain);
            assert_eq!(stx.world.internal_event_buf.len(), internal_len + 3);
            assert_eq!(load_relay_registration(stx, &domain, &retired_relay), None);
            assert!(stx.world.kaigi_relay_registry.get(&retired_relay).is_none());
            assert_eq!(
                load_relay_feedback(stx, &retired_relay).expect("load removed feedback"),
                None
            );
            assert_eq!(
                load_call_record(stx, &call_id).relay_manifest,
                Some(manifest.clone()),
                "retirement must not rewrite an active call's pinned manifest"
            );

            let error = SetKaigiRelayManifest {
                call_id: call_id.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect_err("retired descriptors must be unavailable to future manifest admission");
            assert_smart_contract_error(error, "not registered");
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn relay_retirement_validates_feedback_before_removing_either_row() {
        let domain_id = DomainId::try_new("relay-atomic", "universal").expect("domain id");
        let (relay_id, _) = gen_account_in("relay-atomic");
        let (other_relay, _) = gen_account_in("relay-atomic");
        with_seeded_kaigi_state_transaction(
            &domain_id,
            &[relay_id.clone(), other_relay.clone()],
            |stx| {
                add_relay_to_allowlist(stx, &domain_id, &relay_id);
                let registration = KaigiRelayRegistration {
                    relay_id: relay_id.clone(),
                    hpke_public_key: vec![0xAA],
                    bandwidth_class: 1,
                };
                RegisterKaigiRelay {
                    relay: registration.clone(),
                }
                .execute(&relay_id, stx)
                .expect("register relay");
                stx.world.take_external_events();
                let error = UnregisterKaigiRelay {
                    relay_id: relay_id.clone(),
                }
                .execute(&other_relay, stx)
                .expect_err("an unrelated account must not retire a relay descriptor");
                assert_smart_contract_error(error, "only the relay account");
                assert_eq!(
                    load_relay_registration(stx, &domain_id, &relay_id),
                    Some(registration.clone())
                );
                assert!(stx.world.take_external_events().is_empty());

                let feedback_key = kaigi_relay_feedback_key(&relay_id).expect("relay feedback key");
                let corrupt_feedback = KaigiRelayFeedback {
                    relay_id: other_relay.clone(),
                    call: KaigiId::new(
                        domain_id.clone(),
                        Name::from_str("corrupt-feedback").expect("call name"),
                    ),
                    reported_by: relay_id.clone(),
                    status: KaigiRelayHealthStatus::Healthy,
                    reported_at_ms: 0,
                    notes: None,
                };
                stx.world
                    .domain_mut(&domain_id)
                    .expect("relay domain")
                    .metadata_mut()
                    .insert(
                        feedback_key.clone(),
                        Json::try_new(corrupt_feedback).expect("serialize corrupt feedback"),
                    );
                stx.world.take_external_events();

                let error = UnregisterKaigiRelay {
                    relay_id: relay_id.clone(),
                }
                .execute(&relay_id, stx)
                .expect_err("corrupt feedback must abort relay retirement");
                assert_invariant_error(error, "feedback identifier does not match");
                assert_eq!(
                    load_relay_registration(stx, &domain_id, &relay_id),
                    Some(registration)
                );
                assert_eq!(
                    stx.world.kaigi_relay_registry.get(&relay_id),
                    Some(&domain_id)
                );
                assert!(
                    stx.world
                        .domain(&domain_id)
                        .expect("relay domain")
                        .metadata()
                        .contains(&feedback_key)
                );
                assert!(stx.world.take_external_events().is_empty());
            },
        );
    }
    #[test]
    fn active_rekey_successor_may_retire_predecessor_relay_descriptor() {
        let domain_id = DomainId::try_new("relay-lineage", "universal").expect("domain id");
        let (retired_relay, _) = gen_account_in("relay-lineage");
        let (active_relay, _) = gen_account_in("relay-lineage");
        with_seeded_kaigi_state_transaction(
            &domain_id,
            &[retired_relay.clone(), active_relay.clone()],
            |stx| {
                add_relay_to_allowlist(stx, &domain_id, &retired_relay);
                RegisterKaigiRelay {
                    relay: KaigiRelayRegistration {
                        relay_id: retired_relay.clone(),
                        hpke_public_key: vec![0xAA],
                        bandwidth_class: 1,
                    },
                }
                .execute(&retired_relay, stx)
                .expect("register predecessor descriptor");
                seed_active_account_id_rekey_lineage(
                    stx,
                    "relay-retirement-lineage",
                    &retired_relay,
                    &active_relay,
                );
                seed_relay_primary_alias_for_testing(stx, &domain_id, &active_relay);
                stx.world.take_external_events();

                UnregisterKaigiRelay {
                    relay_id: retired_relay.clone(),
                }
                .execute(&active_relay, stx)
                .expect("active canonical successor may retire predecessor descriptor");
                assert_eq!(
                    load_relay_registration(stx, &domain_id, &retired_relay),
                    None
                );
                assert!(
                    extract_unregistration_summary(&stx.world.take_external_events()).is_some()
                );
            },
        );
    }
    #[test]
    fn manifest_requires_registered_relays() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("manifest-validate").unwrap());
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            stx.world.take_external_events();
            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();
            let manifest = sample_manifest();
            for _hop in &manifest.hops {
                let relay_domain: DomainId =
                    DomainId::try_new("relay", "universal").expect("relay domain");
                if stx.world.domain(&relay_domain).is_err() {
                    Register::domain(Domain::new(relay_domain))
                        .execute(&ALICE_ID, stx)
                        .expect("register relay domain");
                }
            }
            let err = SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect_err("unregistered relays rejected");
            match err {
                Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                    assert!(msg.contains("relay"));
                }
                other => panic!("unexpected error variant {other:?}"),
            }
            for hop in &manifest.hops {
                let relay_domain: DomainId =
                    DomainId::try_new("relay", "universal").expect("relay domain");
                if stx.world.domain(&relay_domain).is_err() {
                    Register::domain(Domain::new(relay_domain.clone()))
                        .execute(&ALICE_ID, stx)
                        .expect("register relay domain");
                }
                Register::account(Account::new(hop.relay_id.clone()))
                    .execute(&ALICE_ID, stx)
                    .expect("register relay account");
                stx.world.take_external_events();
                add_relay_to_allowlist(stx, &relay_domain, &hop.relay_id);
                RegisterKaigiRelay {
                    relay: KaigiRelayRegistration {
                        relay_id: hop.relay_id.clone(),
                        hpke_public_key: hop.hpke_public_key.clone(),
                        bandwidth_class: 1,
                    },
                }
                .execute(&hop.relay_id, stx)
                .expect("register relay");
                stx.world.take_external_events();
            }
            SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect("manifest accepted after registration");
            let events = stx.world.take_external_events();
            let manifest_summary =
                extract_manifest_summary(&events).expect("relay manifest summary event");
            assert_eq!(manifest_summary.call, call);
            assert_eq!(
                manifest_summary.hop_count,
                truncate_len(manifest.hops.len())
            );
        });
    }
    #[test]
    fn manifest_rejects_corrupt_relay_registration_identity_and_bandwidth() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("manifest-corrupt-registration").expect("call name"),
        );
        let manifest = sample_manifest();
        let relay = manifest.hops[0].clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            register_manifest_relays(stx, &domain, &manifest);
            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            let original_record = load_call_record(stx, &call);
            stx.world.take_external_events();

            let key = kaigi_relay_metadata_key(&relay.relay_id).expect("relay metadata key");
            let corrupt_identity = KaigiRelayRegistration {
                relay_id: manifest.hops[1].relay_id.clone(),
                hpke_public_key: relay.hpke_public_key.clone(),
                bandwidth_class: 1,
            };
            stx.world
                .domain_mut(&domain)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    key.clone(),
                    Json::try_new(corrupt_identity).expect("serialize corrupt registration"),
                );
            let error = SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect_err("mismatched stored relay identity must fail closed");
            assert_smart_contract_error(error, "identifier does not match");
            assert_eq!(load_call_record(stx, &call), original_record);
            assert!(stx.world.take_external_events().is_empty());

            let zero_bandwidth = KaigiRelayRegistration {
                relay_id: relay.relay_id.clone(),
                hpke_public_key: relay.hpke_public_key.clone(),
                bandwidth_class: 0,
            };
            stx.world
                .domain_mut(&domain)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    key,
                    Json::try_new(zero_bandwidth).expect("serialize corrupt registration"),
                );
            let error = SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: Some(manifest.clone()),
            }
            .execute(&host, stx)
            .expect_err("zero-bandwidth stored registration must fail closed");
            assert_smart_contract_error(error, "zero bandwidth");
            assert_eq!(load_call_record(stx, &call), original_record);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn transparent_join_updates_roster_summary() {
        let (record, host, participant) = new_record(KaigiPrivacyMode::Transparent);
        let call = record.id.clone();
        let domain = record.id.domain_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            Register::account(Account::new(participant.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register participant account");
            stx.world.take_external_events();
            create_call(stx, &record, &host);
            stx.world.take_external_events();
            JoinKaigi {
                call_id: call.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("transparent join");
            let events = stx.world.take_external_events();
            let summary = extract_roster_summary(&events).expect("roster summary event");
            assert_eq!(summary.participant_count, 1);
            assert_eq!(summary.commitment_count, 0);
            assert_eq!(summary.nullifier_count, 0);
        });
    }
    #[test]
    fn rejected_record_write_emits_no_roster_summary() {
        use iroha_data_model::parameter::{CustomParameter, CustomParameterId, Parameter};

        let (record, host, participant) = new_record(KaigiPrivacyMode::Transparent);
        let call = record.id.clone();
        let domain = call.domain_id.clone();
        with_seeded_kaigi_state_transaction(&domain, &[host.clone(), participant.clone()], |stx| {
            create_call(stx, &record, &host);
            let original = load_call_record(stx, &call);
            let limit_id: CustomParameterId = "max_metadata_value_bytes"
                .parse()
                .expect("metadata limit parameter id");
            stx.world
                .parameters
                .get_mut()
                .set_parameter(Parameter::Custom(CustomParameter::new(
                    limit_id,
                    Json::new(1_u64),
                )));
            stx.world.take_external_events();

            let error = JoinKaigi {
                call_id: call.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("an oversized updated record must be rejected");
            assert_smart_contract_error(error, "Payload too large");
            assert_eq!(load_call_record(stx, &call), original);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn transparent_join_rejects_unregistered_participant_without_mutation() {
        let (record, host, participant) = new_record(KaigiPrivacyMode::Transparent);
        let call = record.id.clone();
        let domain = call.domain_id.clone();
        with_seeded_kaigi_state_transaction(&domain, std::slice::from_ref(&host), |stx| {
            create_call(stx, &record, &host);
            let original = load_call_record(stx, &call);
            stx.world.take_external_events();

            let error = JoinKaigi {
                call_id: call.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("host must not add an account absent from world state");
            assert!(
                matches!(&error, Error::Find(FindError::Account(account)) if account == &participant),
                "unexpected error: {error:?}"
            );
            assert_eq!(load_call_record(stx, &call), original);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[cfg(feature = "kaigi_privacy_mocks")]
    #[test]
    fn mock_privacy_join_updates_commitment_summary() {
        let (record, host, participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        let call = record.id.clone();
        let domain = call.domain_id.clone();
        let commitment = sample_commitment();
        let join_nullifier = sample_nullifier(0xCC);
        with_seeded_kaigi_state_transaction(&domain, &[host.clone(), participant.clone()], |stx| {
            CreateKaigi {
                call: NewKaigi {
                    privacy_mode: KaigiPrivacyMode::ZkRosterV1,
                    ..NewKaigi::with_defaults(call.clone(), host.clone())
                },
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create privacy Kaigi");
            let join_root = load_call_record(stx, &call).roster_root();
            stx.world.take_external_events();
            JoinKaigi {
                call_id: call.clone(),
                participant: participant.clone(),
                commitment: Some(commitment.clone()),
                nullifier: Some(join_nullifier.clone()),
                roster_root: Some(join_root),
                proof: Some(vec![1_u8]),
            }
            .execute(&participant, stx)
            .expect("privacy join");
            let events = stx.world.take_external_events();
            let summary = extract_roster_summary(&events).expect("roster summary event");
            assert_eq!(summary.participant_count, 0);
            assert_eq!(summary.commitment_count, 1);
            assert_eq!(summary.nullifier_count, 1);
        });
    }
    #[test]
    fn leave_kaigi_updates_roster_summary() {
        let (record, host, participant) = new_record(KaigiPrivacyMode::Transparent);
        let call_id = record.id.clone();
        let domain = call_id.domain_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            Register::account(Account::new(participant.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register participant");
            stx.world.take_external_events();
            CreateKaigi {
                call: NewKaigi::with_defaults(call_id.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            JoinKaigi {
                call_id: call_id.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("join kaigi");
            stx.world.take_external_events();
            LeaveKaigi {
                call_id: call_id.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("leave kaigi");
            let events = stx.world.take_external_events();
            let summary = extract_roster_summary(&events).expect("roster summary event");
            assert_eq!(summary.call, call_id);
            assert_eq!(summary.participant_count, 0);
            assert_eq!(summary.commitment_count, 0);
            assert_eq!(summary.nullifier_count, 0);
        });
    }
    #[test]
    fn record_usage_updates_usage_summary() {
        let (record, host, _participant) = new_record(KaigiPrivacyMode::Transparent);
        let domain = record.id.domain_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            stx.world.take_external_events();
            create_call(stx, &record, &host);
            stx.world.take_external_events();
            RecordKaigiUsage {
                call_id: record.id.clone(),
                duration_ms: 90,
                billed_gas: 900,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("record usage");
            let events = stx.world.take_external_events();
            let summary = extract_usage_summary(&events).expect("usage summary event");
            assert_eq!(summary.call, record.id);
            assert_eq!(summary.total_duration_ms, 90);
            assert_eq!(summary.total_billed_gas, 900);
            assert_eq!(summary.segments_recorded, 1);
        });
    }
    #[test]
    fn ended_kaigi_rejects_all_session_mutations() {
        let (domain, host, participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("ended-freeze").unwrap());
        let manifest = sample_manifest();
        let relay_id = manifest.hops[0].relay_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            Register::account(Account::new(participant.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register participant");
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            JoinKaigi {
                call_id: call.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("join participant");
            EndKaigi {
                call_id: call.clone(),
                ended_at_ms: Some(0),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("end kaigi");
            let ended_record = load_call_record(stx, &call);
            assert_eq!(ended_record.status, KaigiStatus::Ended);
            stx.world.take_external_events();

            let leave_err = LeaveKaigi {
                call_id: call.clone(),
                participant: participant.clone(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("ended Kaigi must reject leave");
            assert_kaigi_not_active(leave_err);

            let usage_err = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 1,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("ended Kaigi must reject usage");
            assert_kaigi_not_active(usage_err);

            let health_err = ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Unavailable,
                reported_at_ms: 11,
                notes: None,
            }
            .execute(&host, stx)
            .expect_err("ended Kaigi must reject relay health reports");
            assert_kaigi_not_active(health_err);

            let manifest_err = SetKaigiRelayManifest {
                call_id: call.clone(),
                relay_manifest: None,
            }
            .execute(&host, stx)
            .expect_err("ended Kaigi must reject relay manifest updates");
            assert_kaigi_not_active(manifest_err);

            assert_eq!(load_call_record(stx, &call), ended_record);
            let feedback_key = kaigi_relay_feedback_key(&relay_id).expect("feedback key");
            assert!(
                stx.world
                    .domain(&domain)
                    .expect("domain")
                    .metadata()
                    .get(&feedback_key)
                    .is_none()
            );
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn record_usage_rejects_counter_overflow_without_mutating_state() {
        let (record, host, _participant) = new_record(KaigiPrivacyMode::Transparent);
        let call = record.id.clone();
        let domain = call.domain_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            create_call(stx, &record, &host);
            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");

            let mut stored = load_call_record(stx, &call);
            stored.total_duration_ms = u64::MAX;
            store_record(stx, &domain, key.clone(), &stored).expect("seed duration limit");
            stx.world.take_external_events();
            let err = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 0,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("duration overflow must reject");
            assert_smart_contract_error(err, "total duration");
            assert_eq!(load_call_record(stx, &call), stored);
            assert!(stx.world.take_external_events().is_empty());

            stored.total_duration_ms = 0;
            stored.total_billed_gas = u64::MAX;
            store_record(stx, &domain, key.clone(), &stored).expect("seed billed gas limit");
            stx.world.take_external_events();
            let err = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 1,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("billed gas overflow must reject");
            assert_smart_contract_error(err, "total billed gas");
            assert_eq!(load_call_record(stx, &call), stored);
            assert!(stx.world.take_external_events().is_empty());

            stored.total_billed_gas = 0;
            stored.segments_recorded = u32::MAX;
            store_record(stx, &domain, key, &stored).expect("seed segment count limit");
            stx.world.take_external_events();
            let err = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 0,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("segment count overflow must reject");
            assert_smart_contract_error(err, "segment count");
            assert_eq!(load_call_record(stx, &call), stored);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn end_kaigi_rejects_impossible_timestamps_without_mutating_state() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("end-timestamp").expect("call name"),
        );
        with_state_transaction_at(100, |stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            let active = load_call_record(stx, &call);
            let create_events = stx.world.take_external_events();
            let created = extract_status_summary(&create_events).expect("active status event");
            assert_eq!(*created.status(), KaigiStatus::Active);
            assert_eq!(*created.ended_at_ms(), None);
            assert!(create_events.iter().all(|event| !matches!(
                event,
                EventBox::Data(data)
                    if matches!(data.as_ref(), DataEvent::Domain(DomainEvent::MetadataInserted(_)))
            )));

            for (timestamp, message) in [(99, "precede creation"), (101, "current block time")] {
                let error = EndKaigi {
                    call_id: call.clone(),
                    ended_at_ms: Some(timestamp),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&host, stx)
                .expect_err("impossible end timestamp must reject");
                assert_smart_contract_error(error, message);
                assert_eq!(load_call_record(stx, &call), active);
                assert!(stx.world.take_external_events().is_empty());
            }

            EndKaigi {
                call_id: call.clone(),
                ended_at_ms: Some(100),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("current block time is a valid end timestamp");
            assert_eq!(load_call_record(stx, &call).ended_at_ms, Some(100));
            let end_events = stx.world.take_external_events();
            assert_eq!(
                end_events.len(),
                1,
                "ending exposes only a compact status event"
            );
            let ended = extract_status_summary(&end_events).expect("ended status event");
            assert_eq!(*ended.status(), KaigiStatus::Ended);
            assert_eq!(*ended.ended_at_ms(), Some(100));
        });
    }
    #[test]
    fn stored_record_identifier_must_match_requested_call() {
        let (record, host, _participant) = new_record(KaigiPrivacyMode::Transparent);
        let call = record.id.clone();
        let domain = call.domain_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            create_call(stx, &record, &host);

            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let mut mismatched = load_call_record(stx, &call);
            mismatched.id = KaigiId::new(
                domain.clone(),
                Name::from_str("different-call").expect("call name"),
            );
            stx.world
                .domain_mut(&domain)
                .expect("domain")
                .metadata_mut()
                .insert(
                    key,
                    Json::try_new(mismatched.clone()).expect("serialize mismatched record"),
                );
            stx.world.take_external_events();

            let err = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 1,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("mismatched stored identifier must reject");
            assert_invariant_error(err, "identifier does not match metadata key");
            assert_eq!(load_call_record(stx, &call), mismatched);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn stored_record_relay_manifest_must_satisfy_v1_constraints() {
        let (record, host, _participant) = new_record(KaigiPrivacyMode::Transparent);
        let call = record.id.clone();
        let domain = call.domain_id.clone();
        with_state_transaction(|stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            create_call(stx, &record, &host);

            let key = kaigi_metadata_key(&call.call_name).expect("metadata key");
            let mut persisted = load_call_record(stx, &call);
            let mut too_many = sample_manifest();
            while too_many.hops.len() <= KAIGI_RELAY_MANIFEST_MAX_HOPS_V1 {
                too_many.hops.push(too_many.hops[0].clone());
            }
            persisted.relay_manifest = Some(too_many);
            stx.world
                .domain_mut(&domain)
                .expect("domain")
                .metadata_mut()
                .insert(
                    key.clone(),
                    Json::try_new(persisted.clone()).expect("serialize oversized manifest"),
                );
            stx.world.take_external_events();

            let error = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 1,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("a persisted overlong relay manifest must fail closed");
            assert_invariant_error(error, "relay manifest violates V1 constraints");
            assert_eq!(load_call_record(stx, &call), persisted);
            assert!(stx.world.take_external_events().is_empty());

            let mut oversized_key = sample_manifest();
            oversized_key.hops[0].hpke_public_key =
                vec![0xA5; KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 + 1];
            persisted.relay_manifest = Some(oversized_key);
            stx.world
                .domain_mut(&domain)
                .expect("domain")
                .metadata_mut()
                .insert(
                    key,
                    Json::try_new(persisted.clone()).expect("serialize oversized relay key"),
                );

            let error = RecordKaigiUsage {
                call_id: call.clone(),
                duration_ms: 1,
                billed_gas: 1,
                usage_commitment: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect_err("a persisted oversized relay key must fail closed");
            assert_invariant_error(error, "relay manifest violates V1 constraints");
            assert_eq!(load_call_record(stx, &call), persisted);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn relay_health_reports_are_monotonic_and_idempotent() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("health-ordering").expect("call name"),
        );
        let manifest = sample_manifest();
        let relay_id = manifest.hops[0].relay_id.clone();
        with_state_transaction_at(20, |stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();

            let initial_record = load_call_record(stx, &call);
            let feedback_key =
                kaigi_relay_feedback_key(&relay_id).expect("relay feedback metadata key");
            let corrupt_feedback = KaigiRelayFeedback {
                relay_id: manifest.hops[1].relay_id.clone(),
                call: call.clone(),
                reported_by: host.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 1,
                notes: None,
            };
            stx.world
                .domain_mut(&domain)
                .expect("relay domain")
                .metadata_mut()
                .insert(
                    feedback_key.clone(),
                    Json::try_new(corrupt_feedback).expect("serialize corrupt feedback"),
                );
            let corrupt_error = ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 10,
                notes: None,
            }
            .execute(&host, stx)
            .expect_err("mismatched stored relay feedback must fail closed");
            assert_invariant_error(corrupt_error, "identifier does not match");
            assert_eq!(load_call_record(stx, &call), initial_record);
            assert!(stx.world.take_external_events().is_empty());
            stx.world
                .domain_mut(&domain)
                .expect("relay domain")
                .metadata_mut()
                .remove(&feedback_key)
                .expect("remove corrupt feedback");

            let future_error = ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Unavailable,
                reported_at_ms: u64::MAX,
                notes: Some("poison".to_owned()),
            }
            .execute(&host, stx)
            .expect_err("a future report must not poison feedback ordering");
            assert_smart_contract_error(future_error, "current block time");
            assert!(
                load_relay_feedback(stx, &relay_id)
                    .expect("load feedback")
                    .is_none()
            );
            assert_eq!(load_call_record(stx, &call), initial_record);
            assert!(stx.world.take_external_events().is_empty());

            ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 10,
                notes: Some("baseline".to_owned()),
            }
            .execute(&host, stx)
            .expect("store baseline feedback");
            let baseline_feedback = load_relay_feedback(stx, &relay_id)
                .expect("load feedback")
                .expect("baseline feedback");
            let baseline_record = load_call_record(stx, &call);
            assert!(!stx.world.take_external_events().is_empty());

            ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 10,
                notes: Some("baseline".to_owned()),
            }
            .execute(&host, stx)
            .expect("an exact duplicate is an idempotent no-op");
            assert_eq!(
                load_relay_feedback(stx, &relay_id)
                    .expect("load feedback")
                    .expect("stored feedback"),
                baseline_feedback
            );
            assert_eq!(load_call_record(stx, &call), baseline_record);
            assert!(stx.world.take_external_events().is_empty());

            let stale_error = ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Unavailable,
                reported_at_ms: 9,
                notes: Some("stale".to_owned()),
            }
            .execute(&host, stx)
            .expect_err("an older report must not replace current feedback");
            assert_smart_contract_error(stale_error, "older than the latest");
            assert_eq!(
                load_relay_feedback(stx, &relay_id)
                    .expect("load feedback")
                    .expect("stored feedback"),
                baseline_feedback
            );
            assert_eq!(load_call_record(stx, &call), baseline_record);
            assert!(stx.world.take_external_events().is_empty());

            let conflict_error = ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Degraded,
                reported_at_ms: 10,
                notes: Some("conflict".to_owned()),
            }
            .execute(&host, stx)
            .expect_err("equal-timestamp conflicting feedback must reject");
            assert_smart_contract_error(conflict_error, "same timestamp");
            assert_eq!(
                load_relay_feedback(stx, &relay_id)
                    .expect("load feedback")
                    .expect("stored feedback"),
                baseline_feedback
            );
            assert_eq!(load_call_record(stx, &call), baseline_record);
            assert!(stx.world.take_external_events().is_empty());

            ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Unavailable,
                reported_at_ms: 11,
                notes: Some("newer".to_owned()),
            }
            .execute(&host, stx)
            .expect("newer feedback replaces the current value");
            let latest = load_relay_feedback(stx, &relay_id)
                .expect("load feedback")
                .expect("latest feedback");
            assert_eq!(latest.status, KaigiRelayHealthStatus::Unavailable);
            assert_eq!(latest.reported_at_ms, 11);
            assert_eq!(load_call_record(stx, &call), baseline_record);
            assert!(!stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn equal_timestamp_relay_health_reports_reject_across_calls() {
        let (domain, first_host, second_host) = sample_ids();
        let first_call = KaigiId::new(
            domain.clone(),
            Name::from_str("health-call-a").expect("call name"),
        );
        let second_call = KaigiId::new(
            domain.clone(),
            Name::from_str("health-call-b").expect("call name"),
        );
        let manifest = sample_manifest();
        let relay_id = manifest.hops[0].relay_id.clone();
        with_state_transaction_at(20, |stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            for host in [first_host.clone(), second_host.clone()] {
                Register::account(Account::new(host))
                    .execute(&ALICE_ID, stx)
                    .expect("register host");
            }
            register_manifest_relays(stx, &domain, &manifest);
            for (call, host) in [
                (first_call.clone(), first_host.clone()),
                (second_call.clone(), second_host.clone()),
            ] {
                let mut template = NewKaigi::with_defaults(call, host.clone());
                template.relay_manifest = Some(manifest.clone());
                CreateKaigi {
                    call: template,
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&host, stx)
                .expect("create kaigi");
            }
            stx.world.take_external_events();

            ReportKaigiRelayHealth {
                call_id: first_call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 10,
                notes: Some("first call".to_owned()),
            }
            .execute(&first_host, stx)
            .expect("store first call feedback");
            stx.world.take_external_events();

            let conflict = ReportKaigiRelayHealth {
                call_id: first_call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Degraded,
                reported_at_ms: 10,
                notes: Some("same call conflict".to_owned()),
            }
            .execute(&first_host, stx)
            .expect_err("same-call equal-timestamp conflict must reject");
            assert_smart_contract_error(conflict, "same timestamp");
            assert!(stx.world.take_external_events().is_empty());

            let cross_call_conflict = ReportKaigiRelayHealth {
                call_id: second_call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Degraded,
                reported_at_ms: 10,
                notes: Some("second call".to_owned()),
            }
            .execute(&second_host, stx)
            .expect_err("equal-timestamp feedback must have deterministic singleton ordering");
            assert_smart_contract_error(cross_call_conflict, "same timestamp");
            let latest = load_relay_feedback(stx, &relay_id)
                .expect("load feedback")
                .expect("stored feedback");
            assert_eq!(latest.call, first_call);
            assert_eq!(latest.status, KaigiRelayHealthStatus::Healthy);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn relay_health_note_limit_counts_unicode_characters() {
        let (domain, host, _participant) = sample_ids();
        let call = KaigiId::new(domain.clone(), Name::from_str("unicode-health").unwrap());
        let manifest = sample_manifest();
        let relay_id = manifest.hops[0].relay_id.clone();
        with_state_transaction_at(20, |stx| {
            Register::domain(Domain::new(domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            register_manifest_relays(stx, &domain, &manifest);
            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(manifest.clone());
            CreateKaigi {
                call: template,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();

            let accepted_notes = "界".repeat(512);
            ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 1,
                notes: Some(accepted_notes.clone()),
            }
            .execute(&host, stx)
            .expect("512 multibyte characters must be accepted");
            let feedback_key = kaigi_relay_feedback_key(&relay_id).expect("feedback key");
            let stored: KaigiRelayFeedback = stx
                .world
                .domain(&domain)
                .expect("domain")
                .metadata()
                .get(&feedback_key)
                .expect("stored feedback")
                .clone()
                .try_into_any_norito()
                .expect("decode feedback");
            assert_eq!(stored.notes.as_deref(), Some(accepted_notes.as_str()));
            stx.world.take_external_events();

            let err = ReportKaigiRelayHealth {
                call_id: call.clone(),
                relay_id: relay_id.clone(),
                status: KaigiRelayHealthStatus::Degraded,
                reported_at_ms: 2,
                notes: Some("界".repeat(513)),
            }
            .execute(&host, stx)
            .expect_err("513 characters must be rejected");
            assert_smart_contract_error(err, "512 characters");
            let unchanged: KaigiRelayFeedback = stx
                .world
                .domain(&domain)
                .expect("domain")
                .metadata()
                .get(&feedback_key)
                .expect("stored feedback")
                .clone()
                .try_into_any_norito()
                .expect("decode feedback");
            assert_eq!(unchanged, stored);
            assert!(stx.world.take_external_events().is_empty());
        });
    }
    #[test]
    fn active_rekey_successor_can_end_predecessor_hosted_kaigi() {
        let (domain, retired_host, unrelated) = sample_ids();
        let (active_host, _) = gen_account_in("nexus");
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("rekey-host-end").expect("call name"),
        );
        with_seeded_kaigi_state_transaction(
            &domain,
            &[active_host.clone(), unrelated.clone()],
            |stx| {
                seed_active_account_id_rekey_lineage(
                    stx,
                    "kaigi-host-lineage",
                    &retired_host,
                    &active_host,
                );
                let template = NewKaigi::with_defaults(call.clone(), retired_host.clone());
                let record = KaigiRecord::from_new(&template, 0);
                store_record(
                    stx,
                    &domain,
                    kaigi_metadata_key(&call.call_name).expect("metadata key"),
                    &record,
                )
                .expect("seed predecessor-hosted call");
                stx.world.take_external_events();

                let error = EndKaigi {
                    call_id: call.clone(),
                    ended_at_ms: Some(0),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&unrelated, stx)
                .expect_err("unrelated account must not inherit host authority");
                assert_smart_contract_error(error, "only the host");
                assert_eq!(load_call_record(stx, &call), record);
                assert!(stx.world.take_external_events().is_empty());

                EndKaigi {
                    call_id: call.clone(),
                    ended_at_ms: Some(0),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&active_host, stx)
                .expect("active rekey successor may end predecessor-hosted call");
                let ended = load_call_record(stx, &call);
                assert_eq!(ended.host, retired_host, "audit identity stays historical");
                assert_eq!(ended.status, KaigiStatus::Ended);
                assert_eq!(ended.ended_at_ms, Some(0));
            },
        );
    }
    #[test]
    fn active_rekey_successor_can_leave_predecessor_roster_entry() {
        let (domain, host, retired_participant) = sample_ids();
        let (active_participant, _) = gen_account_in("nexus");
        let (unrelated, _) = gen_account_in("nexus");
        let call = KaigiId::new(
            domain.clone(),
            Name::from_str("rekey-participant-leave").expect("call name"),
        );
        with_seeded_kaigi_state_transaction(
            &domain,
            &[host.clone(), active_participant.clone(), unrelated.clone()],
            |stx| {
                seed_active_account_id_rekey_lineage(
                    stx,
                    "kaigi-participant-lineage",
                    &retired_participant,
                    &active_participant,
                );
                let template = NewKaigi::with_defaults(call.clone(), host.clone());
                let mut record = KaigiRecord::from_new(&template, 0);
                record.push_participant(retired_participant.clone());
                record
                    .participant_metadata
                    .insert(retired_participant.clone(), Metadata::default());
                store_record(
                    stx,
                    &domain,
                    kaigi_metadata_key(&call.call_name).expect("metadata key"),
                    &record,
                )
                .expect("seed predecessor roster entry");
                stx.world.take_external_events();

                let error = LeaveKaigi {
                    call_id: call.clone(),
                    participant: active_participant.clone(),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&unrelated, stx)
                .expect_err("unrelated account must not inherit participant authority");
                assert_smart_contract_error(error, "only the host or participant");
                assert_eq!(load_call_record(stx, &call), record);
                assert!(stx.world.take_external_events().is_empty());

                LeaveKaigi {
                    call_id: call.clone(),
                    participant: active_participant.clone(),
                    commitment: None,
                    nullifier: None,
                    roster_root: None,
                    proof: None,
                }
                .execute(&active_participant, stx)
                .expect("active successor may remove predecessor roster entry");
                let updated = load_call_record(stx, &call);
                assert!(updated.participants.is_empty());
                assert!(updated.participant_metadata.is_empty());
                assert_eq!(updated.host, host);
            },
        );
    }
    fn register_manifest_relays(
        stx: &mut StateTransaction<'_, '_>,
        domain_id: &DomainId,
        manifest: &KaigiRelayManifest,
    ) {
        for hop in &manifest.hops {
            Register::account(Account::new(hop.relay_id.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register manifest relay account");
            add_relay_to_allowlist(stx, domain_id, &hop.relay_id);
        }
        for hop in &manifest.hops {
            RegisterKaigiRelay {
                relay: KaigiRelayRegistration {
                    relay_id: hop.relay_id.clone(),
                    hpke_public_key: hop.hpke_public_key.clone(),
                    bandwidth_class: 1,
                },
            }
            .execute(&hop.relay_id, stx)
            .expect("register manifest relay");
        }
        stx.world.take_external_events();
    }
    fn add_relay_to_allowlist(
        stx: &mut StateTransaction<'_, '_>,
        domain_id: &DomainId,
        relay_id: &AccountId,
    ) {
        seed_relay_primary_alias_for_testing(stx, domain_id, relay_id);
        let key = kaigi_relay_allowlist_key().expect("allowlist key");
        let domain = stx
            .world
            .domain_mut(domain_id)
            .expect("relay domain exists");
        let mut allowlist: KaigiRelayAllowlist = domain
            .metadata_mut()
            .remove(&key)
            .map(|value| {
                value
                    .try_into_any_norito()
                    .expect("deserialize stored allowlist")
            })
            .unwrap_or_default();
        allowlist.allowed_relays.insert(relay_id.clone());
        let value = Json::try_new(allowlist).expect("serialize allowlist");
        domain.metadata_mut().insert(key, value);
    }
    fn seed_relay_primary_alias_for_testing(
        stx: &mut StateTransaction<'_, '_>,
        domain_id: &DomainId,
        relay_id: &AccountId,
    ) {
        use iroha_data_model::{
            account::{
                AccountAddress,
                rekey::{AccountAlias, AccountAliasDomain, AccountRekeyRecord},
            },
            sns::{NameControllerV1, NameRecordV1},
        };
        if stx
            .world
            .account(relay_id)
            .expect("relay account exists")
            .label()
            .is_some()
        {
            return;
        }
        let metadata_key = kaigi_relay_metadata_key(relay_id).expect("relay metadata key");
        let digest = metadata_key
            .as_ref()
            .strip_prefix("kaigi_relay__")
            .expect("relay metadata key prefix");
        let label = Name::from_str(&format!("relay{}", &digest[..16])).expect("relay alias label");
        let dataspace = stx
            .world
            .dataspace_catalog()
            .by_alias(domain_id.dataspace().as_ref())
            .expect("relay domain dataspace exists")
            .id;
        let alias = AccountAlias::new(
            label,
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            dataspace,
        );
        let selector =
            crate::sns::selector_for_account_alias(&alias, stx.world.dataspace_catalog())
                .expect("relay alias selector");
        let address = AccountAddress::from_account_id(relay_id).expect("relay account address");
        let lease = NameRecordV1::new(
            selector.clone(),
            relay_id.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        stx.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&lease),
        );
        stx.world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(alias.clone(), relay_id.clone()),
        );
        stx.world
            .account_mut(relay_id)
            .expect("relay account exists")
            .set_label(Some(alias.clone()));
        stx.world
            .insert_account_alias_binding(alias, relay_id.clone());
    }
    fn load_call_record(stx: &StateTransaction<'_, '_>, call_id: &KaigiId) -> KaigiRecord {
        let key = kaigi_metadata_key(&call_id.call_name).expect("metadata key");
        stx.world
            .domain(&call_id.domain_id)
            .expect("domain")
            .metadata()
            .get(&key)
            .expect("record metadata")
            .clone()
            .try_into_any_norito()
            .expect("deserialize record")
    }
    fn load_relay_registration(
        stx: &StateTransaction<'_, '_>,
        domain_id: &DomainId,
        relay_id: &AccountId,
    ) -> Option<KaigiRelayRegistration> {
        let key = kaigi_relay_metadata_key(relay_id).expect("relay registration key");
        stx.world
            .domain(domain_id)
            .expect("relay domain")
            .metadata()
            .get(&key)
            .cloned()
            .map(|value| {
                value
                    .try_into_any_norito()
                    .expect("decode relay registration")
            })
    }
    fn assert_kaigi_not_active(error: Error) {
        assert_invariant_error(error, "Kaigi is not active");
    }
    fn assert_invariant_error(error: Error, expected: &str) {
        match error {
            Error::InvariantViolation(message) => {
                assert!(message.contains(expected), "unexpected error: {message}");
            }
            other => panic!("unexpected error variant {other:?}"),
        }
    }
    fn assert_smart_contract_error(error: Error, expected: &str) {
        match error {
            Error::InvalidParameter(InvalidParameterError::SmartContract(message)) => {
                assert!(message.contains(expected), "unexpected error: {message}");
            }
            other => panic!("unexpected error variant {other:?}"),
        }
    }
    fn create_call(stx: &mut StateTransaction<'_, '_>, record: &KaigiRecord, host: &AccountId) {
        CreateKaigi {
            call: NewKaigi::with_defaults(record.id.clone(), host.clone()),
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .execute(host, stx)
        .expect("create kaigi");
    }
    fn sample_manifest() -> KaigiRelayManifest {
        let (relay_a, _) = gen_account_in("relay");
        let (relay_b, _) = gen_account_in("relay");
        let (relay_c, _) = gen_account_in("relay");
        KaigiRelayManifest {
            hops: vec![
                KaigiRelayHop {
                    relay_id: relay_a,
                    hpke_public_key: vec![1, 2, 3],
                    weight: 1,
                },
                KaigiRelayHop {
                    relay_id: relay_b,
                    hpke_public_key: vec![4, 5, 6],
                    weight: 1,
                },
                KaigiRelayHop {
                    relay_id: relay_c,
                    hpke_public_key: vec![7, 8, 9],
                    weight: 1,
                },
            ],
            expiry_ms: 42,
        }
    }
    fn extract_roster_summary(events: &[EventBox]) -> Option<KaigiRosterSummary> {
        events.iter().find_map(|event| {
            if let EventBox::Data(ev) = event {
                if let DataEvent::Domain(DomainEvent::KaigiRosterSummary(summary)) = ev.as_ref() {
                    return Some(summary.clone());
                }
            }
            None
        })
    }
    fn extract_registration_summary(events: &[EventBox]) -> Option<KaigiRelayRegistrationSummary> {
        events.iter().find_map(|event| {
            if let EventBox::Data(ev) = event {
                if let DataEvent::Domain(DomainEvent::KaigiRelayRegistered(summary)) = ev.as_ref() {
                    return Some(summary.clone());
                }
            }
            None
        })
    }
    fn extract_unregistration_summary(
        events: &[EventBox],
    ) -> Option<KaigiRelayUnregistrationSummary> {
        events.iter().find_map(|event| {
            if let EventBox::Data(ev) = event {
                if let DataEvent::Domain(DomainEvent::KaigiRelayUnregistered(summary)) = ev.as_ref()
                {
                    return Some(summary.clone());
                }
            }
            None
        })
    }
    fn extract_status_summary(events: &[EventBox]) -> Option<KaigiStatusSummary> {
        events.iter().find_map(|event| {
            if let EventBox::Data(ev) = event {
                if let DataEvent::Domain(DomainEvent::KaigiStatusChanged(summary)) = ev.as_ref() {
                    return Some(summary.clone());
                }
            }
            None
        })
    }
    fn extract_manifest_summary(events: &[EventBox]) -> Option<KaigiRelayManifestSummary> {
        events.iter().find_map(|event| {
            if let EventBox::Data(ev) = event {
                if let DataEvent::Domain(DomainEvent::KaigiRelayManifestUpdated(summary)) =
                    ev.as_ref()
                {
                    return Some(summary.clone());
                }
            }
            None
        })
    }
    fn extract_usage_summary(events: &[EventBox]) -> Option<KaigiUsageSummary> {
        events.iter().find_map(|event| {
            if let EventBox::Data(ev) = event {
                if let DataEvent::Domain(DomainEvent::KaigiUsageSummary(summary)) = ev.as_ref() {
                    return Some(summary.clone());
                }
            }
            None
        })
    }
}
