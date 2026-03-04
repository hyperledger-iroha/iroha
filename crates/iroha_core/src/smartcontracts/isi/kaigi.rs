//! Host-side execution of Kaigi instruction family.
use std::{collections::BTreeSet, convert::TryFrom};

use iroha_crypto::Hash;
use iroha_data_model::{
    HasMetadata,
    events::{
        data::prelude::{
            KaigiRelayHealthSummary, KaigiRelayManifestSummary, KaigiRelayRegistrationSummary,
            KaigiRosterSummary, KaigiUsageSummary,
        },
        prelude::{DomainEvent, MetadataChanged},
    },
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        kaigi::{
            CreateKaigi, EndKaigi, JoinKaigi, LeaveKaigi, RecordKaigiUsage, RegisterKaigiRelay,
            ReportKaigiRelayHealth, SetKaigiRelayManifest,
        },
    },
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRecord, KaigiRelayAllowlist, KaigiRelayFeedback, KaigiRelayHealthStatus,
        KaigiRelayManifest, KaigiRelayRegistration, KaigiStatus, kaigi_metadata_key,
        kaigi_relay_allowlist_key, kaigi_relay_feedback_key, kaigi_relay_metadata_key,
    },
    prelude::{AccountId, DomainId, Json, Name},
    query::error::FindError,
};
use privacy::PrivacyArtifacts;

use crate::{
    smartcontracts::limits,
    state::{StateTransaction, WorldReadOnly},
};

mod privacy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccessGrant {
    Default,
    PrivacyAuthorized,
}

use super::super::Execute;

impl Execute for CreateKaigi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let template = self.call;
        if authority != template.host() {
            return Err(unauthorized("only the host account may create a Kaigi"));
        }

        if let Some(manifest) = template.relay_manifest() {
            validate_relay_manifest(manifest)?;
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
        let record = KaigiRecord::from_new(&template, created_at_ms);

        store_record(state_transaction, &domain_id, key, &record)?;
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

impl Execute for JoinKaigi {
    fn execute(
        self,
        authority: &AccountId,
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

        let allow_unassociated = authority == &participant;
        apply_with_record(
            state_transaction,
            &call_id,
            authority,
            allow_unassociated,
            |stx, record| {
                process_join(
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

impl Execute for EndKaigi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let EndKaigi {
            call_id,
            ended_at_ms,
        } = self;

        let end_ms = state_transaction._curr_block.creation_time().as_millis();
        let default_end_ms = u64::try_from(end_ms).map_err(|_| {
            Error::InvariantViolation("block creation time exceeds u64::MAX milliseconds".into())
        })?;
        let resolved_end = ended_at_ms.unwrap_or(default_end_ms);

        apply_with_record(
            state_transaction,
            &call_id,
            authority,
            false,
            move |_, record| {
                if authority != &record.host {
                    return Err(unauthorized("only the host may end a Kaigi"));
                }
                if record.status == KaigiStatus::Ended {
                    return Err(Error::InvariantViolation("Kaigi already ended".into()));
                }
                record.status = KaigiStatus::Ended;
                record.ended_at_ms = Some(resolved_end);
                Ok(AccessGrant::Default)
            },
        )?;
        Ok(())
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
            |stx, record| {
                if authority != &record.host {
                    return Err(unauthorized("only the host may record usage for a Kaigi"));
                }

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
                        #[cfg(feature = "kaigi_privacy_mocks")]
                        privacy::verify_usage_commitment(proof.as_deref())?;
                        #[cfg(not(feature = "kaigi_privacy_mocks"))]
                        privacy::verify_usage_commitment(stx, proof.as_deref())?;
                        record.push_usage_commitment(commitment);
                    }
                }

                record.total_duration_ms = record.total_duration_ms.saturating_add(duration_ms);
                record.total_billed_gas = record.total_billed_gas.saturating_add(billed_gas);
                record.segments_recorded = record.segments_recorded.saturating_add(1);
                emit_usage_summary(stx, record);
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
            |stx, record| {
                if authority != &record.host {
                    return Err(unauthorized(
                        "only the host may update the Kaigi relay manifest",
                    ));
                }

                let previous_manifest = record.relay_manifest.clone();
                if let Some(manifest) = relay_manifest.take() {
                    validate_relay_manifest(&manifest)?;
                    ensure_manifest_relays_registered(stx, &manifest)?;
                    if previous_manifest.as_ref() == Some(&manifest) {
                        return Ok(AccessGrant::Default);
                    }
                    record.set_relay_manifest(Some(manifest));
                } else {
                    if previous_manifest.is_none() {
                        return Ok(AccessGrant::Default);
                    }
                    record.set_relay_manifest(None);
                }

                emit_relay_manifest_summary(stx, &record.id, record.relay_manifest.as_ref());
                #[cfg(feature = "telemetry")]
                {
                    let hop_count = record
                        .relay_manifest
                        .as_ref()
                        .map_or(0, |m| u32::try_from(m.hops.len()).unwrap_or(u32::MAX));
                    let action_label =
                        match (previous_manifest.as_ref(), record.relay_manifest.as_ref()) {
                            (_, None) => "clear",
                            (None, Some(_)) => "set",
                            (Some(prev), Some(next)) if prev == next => "set",
                            (Some(_), Some(_)) => "rotate",
                        };
                    stx.telemetry.record_kaigi_manifest_update(
                        &record.id.domain_id,
                        action_label,
                        hop_count,
                    );
                    if action_label == "rotate" {
                        stx.telemetry.record_kaigi_failover(
                            &record.id.domain_id,
                            &record.id.call_name,
                            hop_count,
                        );
                    }
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
        if registration.hpke_public_key.is_empty() {
            return Err(relay_error(
                "relay registration requires an HPKE public key",
            ));
        }
        if registration.bandwidth_class == 0 {
            return Err(relay_error(
                "relay registration requires a non-zero bandwidth class",
            ));
        }
        ensure_relay_allowed_by_governance(state_transaction, &registration.relay_id)?;

        let key = kaigi_relay_metadata_key(&registration.relay_id).map_err(|err| {
            Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
        })?;
        let value = Json::try_new(registration.clone())
            .map_err(|err| Error::Conversion(err.to_string()))?;

        limits::enforce_json_size(
            state_transaction,
            &value,
            "max_metadata_value_bytes",
            limits::DEFAULT_JSON_LIMIT,
        )?;

        let domain_id = registration.relay_id.domain.clone();
        {
            let domain = state_transaction.world.domain_mut(&domain_id)?;
            domain.metadata_mut().insert(key.clone(), value.clone());
        }

        state_transaction
            .world
            .emit_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
                target: domain_id,
                key,
                value: value.clone(),
            })));

        emit_relay_registration_summary(state_transaction, &registration);
        #[cfg(feature = "telemetry")]
        state_transaction.telemetry.record_kaigi_relay_registration(
            &registration.relay_id.domain,
            registration.bandwidth_class,
        );
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
            && text.len() > 512
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
            |stx, record| {
                if authority != &record.host {
                    return Err(unauthorized("only the host may report Kaigi relay health"));
                }
                let manifest_contains_relay =
                    record.relay_manifest.as_ref().is_some_and(|manifest| {
                        manifest.hops.iter().any(|hop| hop.relay_id == relay_id)
                    });
                if !manifest_contains_relay {
                    return Err(relay_error(
                        "relay health updates require the relay to appear in the active manifest",
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

                store_relay_feedback(stx, &feedback)?;
                emit_relay_health_summary(stx, &feedback);
                #[cfg(feature = "telemetry")]
                stx.telemetry
                    .record_kaigi_relay_health(&relay_id.domain, &relay_id, status);

                Ok(AccessGrant::Default)
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

    let mut associated = authority == &record.host || record.has_participant(authority);

    let grant: AccessGrant = f(state_transaction, &mut record)?;

    if matches!(grant, AccessGrant::PrivacyAuthorized) {
        associated = true;
    }

    if !allow_unassociated && !associated {
        return Err(unauthorized("account is not associated with the Kaigi"));
    }

    store_record(state_transaction, &domain_id, key, &record)
}

fn store_record(
    state_transaction: &mut StateTransaction<'_, '_>,
    domain_id: &DomainId,
    key: Name,
    record: &KaigiRecord,
) -> Result<(), Error> {
    let value = Json::try_new(record.clone()).map_err(|err| Error::Conversion(err.to_string()))?;

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
        .emit_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
            target: domain_id.clone(),
            key,
            value,
        })));

    Ok(())
}

fn store_relay_feedback(
    state_transaction: &mut StateTransaction<'_, '_>,
    feedback: &KaigiRelayFeedback,
) -> Result<(), Error> {
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

    let domain_id = feedback.relay_id.domain.clone();
    {
        let domain = state_transaction.world.domain_mut(&domain_id)?;
        domain.metadata_mut().insert(key.clone(), value.clone());
    }

    state_transaction
        .world
        .emit_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
            target: domain_id,
            key,
            value,
        })));

    Ok(())
}

fn metadata_key(call_id: &KaigiId) -> Result<Name, Error> {
    kaigi_metadata_key(&call_id.call_name).map_err(|err| {
        Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
    })
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

fn validate_relay_manifest(manifest: &KaigiRelayManifest) -> Result<(), Error> {
    if manifest.hops.len() < 3 {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(
                "relay manifest must include at least three hops".into(),
            ),
        ));
    }

    let mut seen_relays = BTreeSet::new();
    for hop in &manifest.hops {
        if hop.hpke_public_key.is_empty() {
            return Err(Error::InvalidParameter(
                InvalidParameterError::SmartContract("relay hops require HPKE keys".into()),
            ));
        }
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

fn ensure_manifest_relays_registered(
    state_transaction: &StateTransaction<'_, '_>,
    manifest: &KaigiRelayManifest,
) -> Result<(), Error> {
    for hop in &manifest.hops {
        ensure_relay_allowed_by_governance(state_transaction, &hop.relay_id)?;
        let key = kaigi_relay_metadata_key(&hop.relay_id).map_err(|err| {
            Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
        })?;
        let domain_id = hop.relay_id.domain.clone();
        let domain = state_transaction.world.domain(&domain_id)?;
        let stored = domain.metadata().get(&key).cloned().ok_or_else(|| {
            relay_error("relay referenced in manifest is not registered in its domain")
        })?;
        let registration: KaigiRelayRegistration = stored
            .try_into_any_norito()
            .map_err(|err| Error::Conversion(err.to_string()))?;
        if registration.hpke_public_key != hop.hpke_public_key {
            return Err(relay_error(
                "relay HPKE public key does not match registered value",
            ));
        }
        if let Some(feedback) = load_relay_feedback(state_transaction, &hop.relay_id)?
            && feedback.status == KaigiRelayHealthStatus::Unavailable
        {
            return Err(relay_error(
                "relay is marked unavailable by the latest health report",
            ));
        }
    }
    Ok(())
}

fn ensure_relay_allowed_by_governance(
    state_transaction: &StateTransaction<'_, '_>,
    relay_id: &AccountId,
) -> Result<(), Error> {
    let allowlist = load_allowlist(state_transaction, &relay_id.domain)?;
    if let Some(allowlist) = allowlist
        && !allowlist.contains(relay_id)
    {
        return Err(relay_error(
            "relay is not present in the governance allowlist for its domain",
        ));
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
    let domain = state_transaction.world.domain(&relay_id.domain)?;
    let Some(stored) = domain.metadata().get(&key) else {
        return Ok(None);
    };
    let feedback: KaigiRelayFeedback = stored
        .clone()
        .try_into_any_norito()
        .map_err(|err| Error::Conversion(err.to_string()))?;
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
    registration: &KaigiRelayRegistration,
) {
    let fingerprint = Hash::new(&registration.hpke_public_key);
    let summary = KaigiRelayRegistrationSummary::new(
        registration.relay_id.clone(),
        registration.bandwidth_class,
        fingerprint,
    );
    stx.world
        .emit_events(Some(DomainEvent::KaigiRelayRegistered(summary)));
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

fn emit_relay_health_summary(stx: &mut StateTransaction<'_, '_>, feedback: &KaigiRelayFeedback) {
    let summary = KaigiRelayHealthSummary::new(
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
    authority: &AccountId,
    participant: &AccountId,
    mut commitment: Option<KaigiParticipantCommitment>,
    mut nullifier: Option<KaigiParticipantNullifier>,
    mut roster_root: Option<iroha_crypto::Hash>,
    proof: Option<&[u8]>,
) -> Result<AccessGrant, Error> {
    if record.status != KaigiStatus::Active {
        return Err(Error::InvariantViolation("Kaigi is not active".into()));
    }

    match record.privacy_mode {
        KaigiPrivacyMode::Transparent => {
            privacy::ensure_transparent_payload(&PrivacyArtifacts {
                authority,
                host: &record.host,
                commitment: commitment.as_ref(),
                nullifier: nullifier.as_ref(),
                roster_root: roster_root.as_ref(),
                proof,
            })?;
            if authority != participant && authority != &record.host {
                return Err(unauthorized("only the host may invite other accounts"));
            }
            if record.host == *participant {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract("host is already part of the call".into()),
                ));
            }
            if record.has_participant(participant) {
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
            emit_roster_summary(state_transaction, record);
            Ok(AccessGrant::Default)
        }
        KaigiPrivacyMode::ZkRosterV1 => {
            if authority != participant {
                return Err(unauthorized(
                    "privacy mode joins must be submitted by the participant",
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
                authority,
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
            emit_roster_summary(state_transaction, record);
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
    match record.privacy_mode {
        KaigiPrivacyMode::Transparent => {
            privacy::ensure_transparent_payload(&PrivacyArtifacts {
                authority,
                host: &record.host,
                commitment: commitment.as_ref(),
                nullifier: nullifier.as_ref(),
                roster_root: roster_root.as_ref(),
                proof,
            })?;
            if authority != participant && authority != &record.host {
                return Err(unauthorized(
                    "only the host or participant may remove a participant",
                ));
            }
            if record.host == *participant {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "host cannot leave the call without ending it".into(),
                    ),
                ));
            }
            if !record.remove_participant(participant) {
                return Err(Error::Find(FindError::Account(participant.clone())));
            }
            emit_roster_summary(state_transaction, record);
            Ok(AccessGrant::Default)
        }
        KaigiPrivacyMode::ZkRosterV1 => {
            if authority != participant {
                return Err(unauthorized(
                    "privacy mode leaves must be submitted by the participant",
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
                authority,
                host: &record.host,
                commitment: Some(&commitment),
                nullifier: Some(&nullifier),
                roster_root: Some(&provided_root),
                proof: Some(proof_bytes),
            };
            let expected_root = record.roster_root();
            privacy::verify_roster_leave(state_transaction, &artifacts, &expected_root)?;

            if !record.remove_commitment(&commitment) {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract(
                        "commitment not registered with roster".into(),
                    ),
                ));
            }
            if record.has_nullifier(&nullifier) {
                return Err(Error::InvalidParameter(
                    InvalidParameterError::SmartContract("nullifier already used".into()),
                ));
            }
            record.push_nullifier(nullifier);
            emit_roster_summary(state_transaction, record);
            Ok(AccessGrant::PrivacyAuthorized)
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use std::str::FromStr;

    use iroha_data_model::{
        events::{
            data::prelude::{DataEvent, DomainEvent, KaigiRelayRegistrationSummary},
            prelude::EventBox,
        },
        kaigi::{KaigiRelayHop, KaigiRelayManifest, KaigiRelayRegistration, NewKaigi},
        prelude::*,
    };
    use iroha_test_samples::{ALICE_ID, gen_account_in};

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World, WorldReadOnly},
    };

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
    fn transparent_preconditions_accept_empty_payload() {
        let (_domain, host, participant) = sample_ids();
        let artifacts = PrivacyArtifacts {
            authority: &participant,
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
        let (_domain, host, participant) = sample_ids();
        let commitment = sample_commitment();
        let artifacts = PrivacyArtifacts {
            authority: &participant,
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
            authority: &participant,
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

        let mut zero_weight = valid.clone();
        zero_weight.hops[0].weight = 0;
        assert!(validate_relay_manifest(&zero_weight).is_err());

        let mut duplicate = valid.clone();
        duplicate.hops[2].relay_id = hop_a;
        assert!(validate_relay_manifest(&duplicate).is_err());
    }

    fn sample_ids() -> (DomainId, AccountId, AccountId) {
        let (host, _) = gen_account_in("nexus");
        let (participant, _) = gen_account_in("nexus");
        (host.domain.clone(), host, participant)
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
            alias_tag: Some("alice".to_string()),
        }
    }

    fn sample_nullifier(tag: u8) -> KaigiParticipantNullifier {
        KaigiParticipantNullifier {
            digest: iroha_crypto::Hash::prehashed([tag; 32]),
            issued_at_ms: 777,
        }
    }

    fn with_state_transaction<F>(mut f: F)
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
            0,
            0,
        );
        let mut block = state.block(header);
        {
            let mut stx = block.transaction();
            f(&mut stx);
        }
    }

    #[test]
    fn transparent_join_and_leave_updates_participants_only() {
        let (mut record, host, participant) = new_record(KaigiPrivacyMode::Transparent);

        with_state_transaction(|stx| {
            let grant = process_join(
                stx,
                &mut record,
                &participant,
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
                &host,
                &participant,
                None,
                None,
                None,
                None,
            )
            .expect("host can re-invite");
        });
    }

    #[cfg(feature = "kaigi_privacy_mocks")]
    #[test]
    fn privacy_join_and_leave_updates_commitments() {
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
                &participant,
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
            let leave_grant = process_leave(
                stx,
                &mut record,
                &participant,
                &participant,
                Some(commitment.clone()),
                Some(leave_nullifier.clone()),
                Some(leave_root),
                Some(&leave_proof),
            )
            .expect("privacy leave");
            assert_eq!(leave_grant, AccessGrant::PrivacyAuthorized);
            assert!(record.roster_commitments.is_empty());
            assert!(record.nullifier_log.len() == 2);

            let retry_root = record.roster_root();
            let err = process_join(
                stx,
                &mut record,
                &participant,
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
    fn privacy_join_respects_max_participant_limit() {
        let (mut record, _host, participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        record.max_participants = Some(1);

        let first_commitment = sample_commitment();
        let first_nullifier = sample_nullifier(0xCC);
        let second_commitment = KaigiParticipantCommitment {
            commitment: iroha_crypto::Hash::prehashed([0x22; 32]),
            alias_tag: Some("second".to_string()),
        };
        let second_nullifier = sample_nullifier(0xDD);

        with_state_transaction(|stx| {
            let first_root = record.roster_root();
            process_join(
                stx,
                &mut record,
                &participant,
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
                &participant,
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
        let (_domain, host, _participant) = sample_ids();
        let call = KaigiId::new(host.domain.clone(), Name::from_str("relayed").unwrap());

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            stx.world.take_external_events();

            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
            template.relay_manifest = Some(sample_manifest());

            CreateKaigi { call: template }
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
    fn host_can_update_relay_manifest() {
        let (_domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            host.domain.clone(),
            Name::from_str("manifest-update").unwrap(),
        );

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            stx.world.take_external_events();

            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();

            let manifest = sample_manifest();
            for hop in &manifest.hops {
                let relay_domain = hop.relay_id.domain.clone();
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
            assert_eq!(record.relay_manifest.as_ref(), Some(&manifest));
        });
    }

    #[test]
    fn non_host_cannot_update_relay_manifest() {
        let (_domain, host, participant) = sample_ids();
        let call = KaigiId::new(
            host.domain.clone(),
            Name::from_str("manifest-authz").unwrap(),
        );

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
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
    fn manifest_rejects_relays_marked_unavailable() {
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
            let result = SetKaigiRelayManifest {
                call_id: call,
                relay_manifest: Some(manifest),
            }
            .execute(&host, stx);
            assert!(
                matches!(result, Err(Error::InvalidParameter(_))),
                "manifest updates must reject relays flagged as unavailable"
            );
        });
    }

    #[test]
    fn host_can_clear_relay_manifest() {
        let (_domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            host.domain.clone(),
            Name::from_str("manifest-clear").unwrap(),
        );

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host");
            stx.world.take_external_events();

            let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
            template.relay_manifest = Some(sample_manifest());
            CreateKaigi { call: template }
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
    fn relay_registration_persists_metadata_and_emits_summary() {
        let (relay_id, _relay_account) = gen_account_in("relay");
        let domain_id = relay_id.domain.clone();
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
            stx.world.take_external_events();

            RegisterKaigiRelay {
                relay: registration.clone(),
            }
            .execute(&relay_id, stx)
            .expect("register relay");

            let events = stx.world.take_external_events();
            let summary =
                extract_registration_summary(&events).expect("relay registration summary event");
            assert_eq!(summary.relay(), &relay_id);
            assert_eq!(*summary.bandwidth_class(), 5);

            let key = kaigi_relay_metadata_key(&relay_id).expect("metadata key");
            let domain = stx.world.domain(&domain_id).expect("domain lookup");
            let stored = domain
                .metadata()
                .get(&key)
                .cloned()
                .expect("relay metadata");
            let decoded: KaigiRelayRegistration = stored
                .try_into_any_norito()
                .expect("decode relay registration");
            assert_eq!(decoded, registration);
        });
    }

    #[test]
    fn manifest_requires_registered_relays() {
        let (_domain, host, _participant) = sample_ids();
        let call = KaigiId::new(
            host.domain.clone(),
            Name::from_str("manifest-validate").unwrap(),
        );

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            stx.world.take_external_events();

            CreateKaigi {
                call: NewKaigi::with_defaults(call.clone(), host.clone()),
            }
            .execute(&host, stx)
            .expect("create kaigi");
            stx.world.take_external_events();

            let manifest = sample_manifest();
            for hop in &manifest.hops {
                let relay_domain = hop.relay_id.domain.clone();
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
                let relay_domain = hop.relay_id.domain.clone();
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
    fn transparent_join_updates_roster_summary() {
        let (mut record, host, participant) = new_record(KaigiPrivacyMode::Transparent);

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register domain");
            Register::account(Account::new(host.clone()))
                .execute(&ALICE_ID, stx)
                .expect("register host account");
            stx.world.take_external_events();

            create_call(stx, &record, &host);
            stx.world.take_external_events();

            process_join(
                stx,
                &mut record,
                &host,
                &participant,
                None,
                None,
                None,
                None,
            )
            .expect("transparent join");
            let events = stx.world.take_external_events();
            let summary = extract_roster_summary(&events).expect("roster summary event");
            assert_eq!(summary.participant_count, 1);
            assert_eq!(summary.commitment_count, 0);
            assert_eq!(summary.nullifier_count, 0);
        });
    }

    #[test]
    fn privacy_join_updates_commitment_summary() {
        let (mut record, _host, participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        let commitment = sample_commitment();
        let join_nullifier = sample_nullifier(0xCC);

        with_state_transaction(|stx| {
            stx.world.take_external_events();
            let join_root = record.roster_root();
            process_join(
                stx,
                &mut record,
                &participant,
                &participant,
                Some(commitment.clone()),
                Some(join_nullifier.clone()),
                Some(join_root),
                Some(&[1u8]),
            )
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

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
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

        with_state_transaction(|stx| {
            Register::domain(Domain::new(host.domain.clone()))
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

    fn add_relay_to_allowlist(
        stx: &mut StateTransaction<'_, '_>,
        domain_id: &DomainId,
        relay_id: &AccountId,
    ) {
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

    fn create_call(stx: &mut StateTransaction<'_, '_>, record: &KaigiRecord, host: &AccountId) {
        CreateKaigi {
            call: NewKaigi::with_defaults(record.id.clone(), host.clone()),
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
