//! Payload-free immutable audit journal and crash recovery.
#[cfg(feature = "taira-authority-bin")]
use super::protocol::SoftwareSignerPublicBindingV1;
use super::{
    envelope::SoftwareSignerKeyEnvelopeAadV1,
    protocol::{
        SIGNER_AUDIT_MAGIC_V1, SIGNER_MAX_SIGNATURE_BYTES_V1, SIGNER_PROTOCOL_VERSION_V1,
        digest_canonical,
    },
};
use iroha_crypto::{KeyPair, Signature};
use norito::codec::{Decode, Encode};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};
const AUDIT_RECORD_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.audit-record.v1";
const AUDIT_ATTESTATION_DOMAIN_V1: &[u8] = b"iroha.external-signer.audit-attestation.v1";
const AUDIT_RECORD_MAX_BYTES_V1: usize = 32 * 1024;
// These V1 lifetime limits bound both durable journal growth and the replay maps
// needed for operation-id idempotency. Exhaustion requires a reviewed successor;
// immutable audit records are never truncated or evicted in place.
const AUDIT_MAX_RECORDS_V1: u64 = 65_536;
const AUDIT_MAX_TOTAL_BYTES_V1: u64 = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuditRetentionLimitsV1 {
    max_records: u64,
    max_total_bytes: u64,
}
const AUDIT_RETENTION_LIMITS_V1: AuditRetentionLimitsV1 = AuditRetentionLimitsV1 {
    max_records: AUDIT_MAX_RECORDS_V1,
    max_total_bytes: AUDIT_MAX_TOTAL_BYTES_V1,
};
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) enum SoftwareSignerAuditEventV1 {
    Genesis {
        key: SoftwareSignerKeyEnvelopeAadV1,
        envelope_digest: [u8; 32],
    },
    SignCommitted {
        operation_id: [u8; 32],
        request_digest: [u8; 32],
        payload_digest: [u8; 32],
        signature: Vec<u8>,
        signature_digest: [u8; 32],
    },
    EquivocationRejected {
        operation_id: [u8; 32],
        accepted_request_digest: [u8; 32],
        rejected_request_digest: [u8; 32],
    },
    Rotated {
        operation_id: [u8; 32],
        request_digest: [u8; 32],
        previous_key_revision: u64,
        previous_policy_revision: u64,
        new_key: SoftwareSignerKeyEnvelopeAadV1,
        new_envelope_digest: [u8; 32],
    },
    Revoked {
        operation_id: [u8; 32],
        request_digest: [u8; 32],
        key_revision: u64,
        policy_revision: u64,
        reason_digest: [u8; 32],
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct SoftwareSignerAuditRecordBodyV1 {
    magic: [u8; 8],
    version: u16,
    sequence: u64,
    predecessor_digest: [u8; 32],
    event: SoftwareSignerAuditEventV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct SoftwareSignerAuditRecordV1 {
    body: SoftwareSignerAuditRecordBodyV1,
    record_digest: [u8; 32],
    attestation: Vec<u8>,
}
#[derive(Clone, Debug)]
pub(super) struct RecoveredSignCommitV1 {
    pub request_digest: [u8; 32],
    pub payload_digest: [u8; 32],
    pub signature: Vec<u8>,
    pub sequence: u64,
    #[cfg(feature = "taira-authority-bin")]
    pub predecessor_audit_head: [u8; 32],
    pub audit_head: [u8; 32],
}
#[derive(Clone, Copy, Debug)]
pub(super) struct RecoveredAdminCommitV1 {
    pub request_digest: [u8; 32],
}
pub(super) struct RecoveredJournalV1 {
    pub active_key: SoftwareSignerKeyEnvelopeAadV1,
    pub active_envelope_digest: [u8; 32],
    pub audit_genesis_digest: [u8; 32],
    pub sequence: u64,
    pub audit_head: [u8; 32],
    pub revoked: bool,
    pub sign_commits: BTreeMap<[u8; 32], RecoveredSignCommitV1>,
    pub admin_commits: BTreeMap<[u8; 32], RecoveredAdminCommitV1>,
    record_bytes: u64,
}

/// Public-key-verifiable successor data extracted from one old-key-attested
/// rotation journal record.
#[cfg(feature = "taira-authority-bin")]
pub(super) struct VerifiedRotationSuccessorV1 {
    pub operation_id: [u8; 32],
    pub request_digest: [u8; 32],
    pub sequence: u64,
    pub predecessor_audit_head: [u8; 32],
    pub audit_head: [u8; 32],
    pub new_key: SoftwareSignerKeyEnvelopeAadV1,
}
pub(super) struct SoftwareSignerAuditJournalV1 {
    directory: PathBuf,
    sequence: u64,
    audit_head: [u8; 32],
    record_bytes: u64,
}
impl SoftwareSignerAuditJournalV1 {
    pub(super) fn create(
        state_directory: &Path,
        key: SoftwareSignerKeyEnvelopeAadV1,
        envelope_digest: [u8; 32],
        keypair: &KeyPair,
    ) -> Result<(Self, RecoveredJournalV1), SoftwareSignerJournalErrorV1> {
        let directory = state_directory.join("audit-v1");
        create_private_directory(&directory)?;
        let mut journal = Self {
            directory,
            sequence: 0,
            audit_head: [0; 32],
            record_bytes: 0,
        };
        let digest = journal.append(
            SoftwareSignerAuditEventV1::Genesis {
                key: key.clone(),
                envelope_digest,
            },
            keypair,
        )?;
        let record_bytes = journal.record_bytes;
        Ok((
            journal,
            RecoveredJournalV1 {
                active_key: key,
                active_envelope_digest: envelope_digest,
                audit_genesis_digest: digest,
                sequence: 1,
                audit_head: digest,
                revoked: false,
                sign_commits: BTreeMap::new(),
                admin_commits: BTreeMap::new(),
                record_bytes,
            },
        ))
    }
    pub(super) fn open(
        state_directory: &Path,
    ) -> Result<(Self, RecoveredJournalV1), SoftwareSignerJournalErrorV1> {
        let directory = state_directory.join("audit-v1");
        validate_private_directory(&directory)?;
        let mut inventory = scan_audit_directory(&directory, AUDIT_RETENTION_LIMITS_V1)?;
        recover_pending_record(&directory, &mut inventory)?;
        let recovered = validate_records_streaming(&directory, &inventory)?;
        Ok((
            Self {
                directory,
                sequence: recovered.sequence,
                audit_head: recovered.audit_head,
                record_bytes: recovered.record_bytes,
            },
            recovered,
        ))
    }
    pub(super) fn append(
        &mut self,
        event: SoftwareSignerAuditEventV1,
        keypair: &KeyPair,
    ) -> Result<[u8; 32], SoftwareSignerJournalErrorV1> {
        self.append_with_limits(event, keypair, AUDIT_RETENTION_LIMITS_V1)
    }
    fn append_with_limits(
        &mut self,
        event: SoftwareSignerAuditEventV1,
        keypair: &KeyPair,
        limits: AuditRetentionLimitsV1,
    ) -> Result<[u8; 32], SoftwareSignerJournalErrorV1> {
        let sequence = self
            .sequence
            .checked_add(1)
            .filter(|sequence| *sequence <= limits.max_records)
            .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
        let body = SoftwareSignerAuditRecordBodyV1 {
            magic: SIGNER_AUDIT_MAGIC_V1,
            version: SIGNER_PROTOCOL_VERSION_V1,
            sequence,
            predecessor_digest: self.audit_head,
            event,
        };
        let record_digest = digest_canonical(AUDIT_RECORD_DIGEST_DOMAIN_V1, &body)
            .map_err(|()| SoftwareSignerJournalErrorV1::Invalid)?;
        let attestation_message = audit_attestation_message(record_digest, sequence);
        let attestation = Signature::try_new(keypair.private_key(), &attestation_message)
            .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?
            .payload()
            .to_vec();
        if attestation.is_empty() || attestation.len() > SIGNER_MAX_SIGNATURE_BYTES_V1 {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        let record = SoftwareSignerAuditRecordV1 {
            body,
            record_digest,
            attestation,
        };
        let encoded = encode_record(&record)?;
        let next_record_bytes =
            enforce_retention_limits(self.sequence, self.record_bytes, encoded.len(), limits)?.1;
        persist_record(&self.directory, &record, &encoded)?;
        self.sequence = sequence;
        self.audit_head = record_digest;
        self.record_bytes = next_record_bytes;
        Ok(record_digest)
    }
    pub(super) const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub(super) const fn audit_head(&self) -> [u8; 32] {
        self.audit_head
    }

    #[cfg(feature = "taira-authority-bin")]
    pub(super) fn rotation_record_bytes(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, SoftwareSignerJournalErrorV1> {
        if operation_id == [0; 32] {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        for sequence in 2..=self.sequence {
            let record = read_record(&self.directory.join(record_name(sequence)))?;
            if matches!(
                &record.body.event,
                SoftwareSignerAuditEventV1::Rotated {
                    operation_id: found,
                    ..
                } if found == &operation_id
            ) {
                return encode_record(&record);
            }
        }
        Err(SoftwareSignerJournalErrorV1::Invalid)
    }

    #[cfg(feature = "taira-authority-bin")]
    pub(super) fn rotation_record_bytes_from_previous(
        &self,
        previous: &SoftwareSignerPublicBindingV1,
    ) -> Result<Vec<u8>, SoftwareSignerJournalErrorV1> {
        let mut found = None;
        for sequence in 2..=self.sequence {
            let record = read_record(&self.directory.join(record_name(sequence)))?;
            let bytes = encode_record(&record)?;
            if verify_rotation_successor_record(&bytes, previous).is_ok()
                && found.replace(bytes).is_some()
            {
                return Err(SoftwareSignerJournalErrorV1::Invalid);
            }
        }
        found.ok_or(SoftwareSignerJournalErrorV1::Invalid)
    }
}

#[cfg(feature = "taira-authority-bin")]
pub(super) fn verify_rotation_successor_record(
    bytes: &[u8],
    previous: &SoftwareSignerPublicBindingV1,
) -> Result<VerifiedRotationSuccessorV1, SoftwareSignerJournalErrorV1> {
    previous
        .validate()
        .map_err(|()| SoftwareSignerJournalErrorV1::Invalid)?;
    if bytes.is_empty() || bytes.len() > AUDIT_RECORD_MAX_BYTES_V1 {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let record: SoftwareSignerAuditRecordV1 =
        norito::decode_canonical(bytes).map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    if encode_record(&record)? != bytes
        || record.body.magic != SIGNER_AUDIT_MAGIC_V1
        || record.body.version != SIGNER_PROTOCOL_VERSION_V1
        || record.body.sequence <= 1
        || record.body.predecessor_digest == [0; 32]
        || record.record_digest == [0; 32]
        || digest_canonical(AUDIT_RECORD_DIGEST_DOMAIN_V1, &record.body)
            .map_err(|()| SoftwareSignerJournalErrorV1::Invalid)?
            != record.record_digest
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let attestation = Signature::try_from_bytes(&record.attestation)
        .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    attestation
        .verify(
            &previous.public_key,
            &audit_attestation_message(record.record_digest, record.body.sequence),
        )
        .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    let sequence = record.body.sequence;
    let predecessor_audit_head = record.body.predecessor_digest;
    let audit_head = record.record_digest;
    let SoftwareSignerAuditEventV1::Rotated {
        operation_id,
        request_digest,
        previous_key_revision,
        previous_policy_revision,
        new_key,
        new_envelope_digest,
    } = record.body.event
    else {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    };
    new_key
        .validate()
        .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    if operation_id == [0; 32]
        || request_digest == [0; 32]
        || new_envelope_digest == [0; 32]
        || previous_key_revision != previous.key_revision
        || previous_policy_revision != previous.policy_revision
        || new_key.key_revision <= previous.key_revision
        || new_key.policy_revision <= previous.policy_revision
        || new_key.public_key_digest == previous.public_key_digest
        || new_key.backend != previous.backend
        || new_key.handle != previous.handle
        || new_key.service_id != previous.service_id
        || new_key.administrator_id != previous.administrator_id
        || new_key.service_uid != previous.service_uid
        || new_key.client_uid != previous.client_uid
        || new_key.administrator_uid != previous.administrator_uid
        || new_key.role != previous.role
        || new_key.purpose_binding != previous.purpose_binding
        || new_key.domain != previous.domain
        || new_key.max_request_bytes != previous.max_request_bytes
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    Ok(VerifiedRotationSuccessorV1 {
        operation_id,
        request_digest,
        sequence,
        predecessor_audit_head,
        audit_head,
        new_key,
    })
}
#[derive(Debug)]
struct AuditDirectoryInventoryV1 {
    record_count: u64,
    record_bytes: u64,
    pending: Option<(u64, PathBuf)>,
}
#[allow(clippy::too_many_lines)]
fn validate_records_streaming(
    directory: &Path,
    inventory: &AuditDirectoryInventoryV1,
) -> Result<RecoveredJournalV1, SoftwareSignerJournalErrorV1> {
    if inventory.record_count == 0 {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let mut first = Some(read_record(&directory.join(record_name(1)))?);
    let SoftwareSignerAuditEventV1::Genesis {
        key: genesis_key,
        envelope_digest,
    } = &first
        .as_ref()
        .ok_or(SoftwareSignerJournalErrorV1::Invalid)?
        .body
        .event
    else {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    };
    genesis_key
        .validate()
        .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    if *envelope_digest == [0; 32] {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let mut active_key = genesis_key.clone();
    let mut active_envelope_digest = *envelope_digest;
    let mut expected_predecessor = [0; 32];
    let mut revoked = false;
    let mut sign_commits = BTreeMap::new();
    let mut admin_commits = BTreeMap::new();
    let mut genesis_digest = [0; 32];
    for expected_sequence in 1..=inventory.record_count {
        let record = if expected_sequence == 1 {
            first.take().ok_or(SoftwareSignerJournalErrorV1::Invalid)?
        } else {
            read_record(&directory.join(record_name(expected_sequence)))?
        };
        let SoftwareSignerAuditRecordV1 {
            body,
            record_digest,
            attestation,
        } = record;
        if body.magic != SIGNER_AUDIT_MAGIC_V1
            || body.version != SIGNER_PROTOCOL_VERSION_V1
            || body.sequence != expected_sequence
            || body.predecessor_digest != expected_predecessor
            || record_digest == [0; 32]
            || digest_canonical(AUDIT_RECORD_DIGEST_DOMAIN_V1, &body)
                .map_err(|()| SoftwareSignerJournalErrorV1::Invalid)?
                != record_digest
            || attestation.is_empty()
            || attestation.len() != active_key.algorithm.algorithm().signature_payload_len()
        {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        let attestation = Signature::try_from_bytes(&attestation)
            .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
        attestation
            .verify(
                &active_key.public_key,
                &audit_attestation_message(record_digest, body.sequence),
            )
            .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
        match body.event {
            SoftwareSignerAuditEventV1::Genesis {
                key,
                envelope_digest,
            } if expected_sequence == 1
                && key == active_key
                && envelope_digest == active_envelope_digest =>
            {
                genesis_digest = record_digest;
            }
            SoftwareSignerAuditEventV1::SignCommitted {
                operation_id,
                request_digest,
                payload_digest,
                signature,
                signature_digest,
            } if !revoked => {
                if operation_id == [0; 32]
                    || request_digest == [0; 32]
                    || payload_digest == [0; 32]
                    || signature.is_empty()
                    || signature.len() != active_key.algorithm.algorithm().signature_payload_len()
                    || digest_parts_signature(&signature) != signature_digest
                    || sign_commits.contains_key(&operation_id)
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
                sign_commits.insert(
                    operation_id,
                    RecoveredSignCommitV1 {
                        request_digest,
                        payload_digest,
                        signature,
                        sequence: body.sequence,
                        #[cfg(feature = "taira-authority-bin")]
                        predecessor_audit_head: body.predecessor_digest,
                        audit_head: record_digest,
                    },
                );
            }
            SoftwareSignerAuditEventV1::EquivocationRejected {
                operation_id,
                accepted_request_digest,
                rejected_request_digest,
            } if !revoked => {
                let Some(accepted) = sign_commits.get(&operation_id) else {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                };
                if accepted.request_digest != accepted_request_digest
                    || rejected_request_digest == [0; 32]
                    || accepted_request_digest == rejected_request_digest
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
            }
            SoftwareSignerAuditEventV1::Rotated {
                operation_id,
                request_digest,
                previous_key_revision,
                previous_policy_revision,
                new_key,
                new_envelope_digest,
            } if !revoked => {
                new_key
                    .validate()
                    .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
                if operation_id == [0; 32]
                    || request_digest == [0; 32]
                    || admin_commits.contains_key(&operation_id)
                    || previous_key_revision != active_key.key_revision
                    || previous_policy_revision != active_key.policy_revision
                    || new_key.key_revision <= active_key.key_revision
                    || new_key.policy_revision <= active_key.policy_revision
                    || new_key.role != active_key.role
                    || new_key.handle != active_key.handle
                    || new_key.service_id != active_key.service_id
                    || new_key.administrator_id != active_key.administrator_id
                    || new_key.service_uid != active_key.service_uid
                    || new_key.client_uid != active_key.client_uid
                    || new_key.administrator_uid != active_key.administrator_uid
                    || new_envelope_digest == [0; 32]
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
                admin_commits.insert(operation_id, RecoveredAdminCommitV1 { request_digest });
                active_key = new_key;
                active_envelope_digest = new_envelope_digest;
            }
            SoftwareSignerAuditEventV1::Revoked {
                operation_id,
                request_digest,
                key_revision,
                policy_revision,
                reason_digest,
            } if !revoked => {
                if operation_id == [0; 32]
                    || request_digest == [0; 32]
                    || reason_digest == [0; 32]
                    || admin_commits.contains_key(&operation_id)
                    || key_revision != active_key.key_revision
                    || policy_revision != active_key.policy_revision
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
                admin_commits.insert(operation_id, RecoveredAdminCommitV1 { request_digest });
                revoked = true;
            }
            _ => return Err(SoftwareSignerJournalErrorV1::Invalid),
        }
        expected_predecessor = record_digest;
    }
    Ok(RecoveredJournalV1 {
        active_key,
        active_envelope_digest,
        audit_genesis_digest: genesis_digest,
        sequence: inventory.record_count,
        audit_head: expected_predecessor,
        revoked,
        sign_commits,
        admin_commits,
        record_bytes: inventory.record_bytes,
    })
}
fn persist_record(
    directory: &Path,
    record: &SoftwareSignerAuditRecordV1,
    encoded: &[u8],
) -> Result<(), SoftwareSignerJournalErrorV1> {
    let pending = directory.join(pending_name(record.body.sequence));
    let final_path = directory.join(record_name(record.body.sequence));
    if final_path.exists() || pending.exists() {
        return Err(SoftwareSignerJournalErrorV1::Conflict);
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&pending)
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    file.write_all(encoded)
        .and_then(|()| file.sync_all())
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    validate_private_file(&pending)?;
    fs::rename(&pending, &final_path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    sync_directory(directory)?;
    validate_private_file(&final_path)
}
fn encode_record(
    record: &SoftwareSignerAuditRecordV1,
) -> Result<Vec<u8>, SoftwareSignerJournalErrorV1> {
    let encoded =
        norito::encode_canonical(record).map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    if encoded.is_empty() || encoded.len() > AUDIT_RECORD_MAX_BYTES_V1 {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    Ok(encoded)
}
fn enforce_retention_limits(
    current_records: u64,
    current_record_bytes: u64,
    next_record_bytes: usize,
    limits: AuditRetentionLimitsV1,
) -> Result<(u64, u64), SoftwareSignerJournalErrorV1> {
    let records = current_records
        .checked_add(1)
        .filter(|records| *records <= limits.max_records)
        .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
    let next_record_bytes =
        u64::try_from(next_record_bytes).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?;
    let record_bytes = current_record_bytes
        .checked_add(next_record_bytes)
        .filter(|bytes| *bytes <= limits.max_total_bytes)
        .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
    Ok((records, record_bytes))
}
fn scan_audit_directory(
    directory: &Path,
    limits: AuditRetentionLimitsV1,
) -> Result<AuditDirectoryInventoryV1, SoftwareSignerJournalErrorV1> {
    let mut record_count = 0_u64;
    let mut maximum_sequence = 0_u64;
    let mut record_bytes = 0_u64;
    let mut pending = None;
    for entry in fs::read_dir(directory).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)? {
        let entry = entry.map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(SoftwareSignerJournalErrorV1::Invalid)?;
        let path = entry.path();
        if let Some(sequence) = parse_record_name(name) {
            if sequence == 0 {
                return Err(SoftwareSignerJournalErrorV1::Invalid);
            }
            if sequence > limits.max_records {
                return Err(SoftwareSignerJournalErrorV1::Capacity);
            }
            record_count = record_count
                .checked_add(1)
                .filter(|count| *count <= limits.max_records)
                .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
            maximum_sequence = maximum_sequence.max(sequence);
        } else if let Some(sequence) = parse_pending_name(name) {
            if sequence == 0 || pending.is_some() {
                return Err(SoftwareSignerJournalErrorV1::Invalid);
            }
            if sequence > limits.max_records {
                return Err(SoftwareSignerJournalErrorV1::Capacity);
            }
            pending = Some((sequence, path.clone()));
        } else {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        let bytes = validated_private_file_len(&path)?;
        if bytes == 0
            || bytes
                > u64::try_from(AUDIT_RECORD_MAX_BYTES_V1)
                    .map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?
        {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        record_bytes = record_bytes
            .checked_add(bytes)
            .filter(|total| *total <= limits.max_total_bytes)
            .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
    }
    // Canonical names are unique directory entries. For positive sequences,
    // `count == maximum` therefore proves the exact set is `1..=maximum`
    // without retaining or sorting every path.
    if record_count != maximum_sequence {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    if let Some((sequence, _)) = pending.as_ref() {
        let expected = record_count
            .checked_add(1)
            .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
        if *sequence != expected {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
    }
    Ok(AuditDirectoryInventoryV1 {
        record_count,
        record_bytes,
        pending,
    })
}
fn read_record(path: &Path) -> Result<SoftwareSignerAuditRecordV1, SoftwareSignerJournalErrorV1> {
    let file_len = validated_private_file_len(path)?;
    if file_len == 0
        || file_len
            > u64::try_from(AUDIT_RECORD_MAX_BYTES_V1)
                .map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let mut file = File::open(path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(file_len).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?,
    );
    std::io::Read::by_ref(&mut file)
        .take(
            u64::try_from(AUDIT_RECORD_MAX_BYTES_V1 + 1)
                .map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?,
        )
        .read_to_end(&mut bytes)
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    if bytes.is_empty()
        || bytes.len() > AUDIT_RECORD_MAX_BYTES_V1
        || u64::try_from(bytes.len()).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?
            != file_len
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let record =
        norito::decode_canonical(&bytes).map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    if norito::encode_canonical(&record).map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?
        != bytes
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    Ok(record)
}
fn recover_pending_record(
    directory: &Path,
    inventory: &mut AuditDirectoryInventoryV1,
) -> Result<(), SoftwareSignerJournalErrorV1> {
    let Some((sequence, path)) = inventory.pending.take() else {
        return Ok(());
    };
    let final_path = directory.join(record_name(sequence));
    if final_path.exists() {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let record = read_record(&path)?;
    if record.body.sequence != sequence {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    if sequence
        != inventory
            .record_count
            .checked_add(1)
            .ok_or(SoftwareSignerJournalErrorV1::Capacity)?
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    fs::rename(path, final_path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    sync_directory(directory)?;
    inventory.record_count = sequence;
    Ok(())
}
fn record_name(sequence: u64) -> String {
    format!("{sequence:020}.norito")
}
fn pending_name(sequence: u64) -> String {
    format!(".pending-{sequence:020}.norito")
}
fn parse_record_name(name: &str) -> Option<u64> {
    let digits = name.strip_suffix(".norito")?;
    (digits.len() == 20 && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}
fn parse_pending_name(name: &str) -> Option<u64> {
    parse_record_name(name.strip_prefix(".pending-")?)
}
fn audit_attestation_message(record_digest: [u8; 32], sequence: u64) -> [u8; 32] {
    super::protocol::digest_parts(
        AUDIT_ATTESTATION_DOMAIN_V1,
        &[&sequence.to_be_bytes(), &record_digest],
    )
}
pub(super) fn digest_parts_signature(signature: &[u8]) -> [u8; 32] {
    super::protocol::digest_parts(b"iroha.external-signer.signature.v1", &[signature])
}
fn create_private_directory(path: &Path) -> Result<(), SoftwareSignerJournalErrorV1> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(path)
            .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    }
    #[cfg(not(unix))]
    fs::create_dir(path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    validate_private_directory(path)
}
fn validate_private_directory(path: &Path) -> Result<(), SoftwareSignerJournalErrorV1> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    #[cfg(unix)]
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(SoftwareSignerJournalErrorV1::UntrustedPath);
    }
    #[cfg(not(unix))]
    if !metadata.is_dir() {
        return Err(SoftwareSignerJournalErrorV1::UntrustedPath);
    }
    Ok(())
}
fn validated_private_file_len(path: &Path) -> Result<u64, SoftwareSignerJournalErrorV1> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    #[cfg(unix)]
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(SoftwareSignerJournalErrorV1::UntrustedPath);
    }
    #[cfg(not(unix))]
    if !metadata.is_file() {
        return Err(SoftwareSignerJournalErrorV1::UntrustedPath);
    }
    Ok(metadata.len())
}
pub(super) fn validate_private_file(path: &Path) -> Result<(), SoftwareSignerJournalErrorV1> {
    validated_private_file_len(path).map(|_| ())
}
pub(super) fn sync_directory(path: &Path) -> Result<(), SoftwareSignerJournalErrorV1> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)
}
/// Payload-free audit journal validation or persistence failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareSignerJournalErrorV1 {
    /// Record structure, canonical encoding, chain, or attestation is invalid.
    Invalid,
    /// Audit directory or record ownership/mode/link invariants are invalid.
    UntrustedPath,
    /// Immutable sequence or operation identity conflicts with durable state.
    Conflict,
    /// The bounded V1 journal record-count or aggregate-byte capacity was exhausted.
    Capacity,
    /// Filesystem persistence or cryptographic attestation was unavailable.
    Unavailable,
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Algorithm;
    fn write_private_file(path: &Path, bytes: &[u8]) {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut file = options.open(path).expect("create private audit fixture");
        file.write_all(bytes).expect("write audit fixture");
    }
    fn fixture_event() -> SoftwareSignerAuditEventV1 {
        SoftwareSignerAuditEventV1::EquivocationRejected {
            operation_id: [0x11; 32],
            accepted_request_digest: [0x22; 32],
            rejected_request_digest: [0x33; 32],
        }
    }
    #[test]
    fn retention_limits_accept_boundary_and_reject_first_overflow() {
        let limits = AuditRetentionLimitsV1 {
            max_records: 2,
            max_total_bytes: 5,
        };
        assert_eq!(enforce_retention_limits(1, 2, 3, limits), Ok((2, 5)));
        assert_eq!(
            enforce_retention_limits(2, 5, 0, limits),
            Err(SoftwareSignerJournalErrorV1::Capacity)
        );
        assert_eq!(
            enforce_retention_limits(1, 5, 1, limits),
            Err(SoftwareSignerJournalErrorV1::Capacity)
        );
    }
    #[test]
    fn append_rejects_capacity_before_creating_pending_file() {
        let parent = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
            .expect("temporary parent");
        let missing_directory = parent.path().join("missing-audit-v1");
        let keypair =
            KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519).expect("fixture signer key");
        let limits = AuditRetentionLimitsV1 {
            max_records: 2,
            max_total_bytes: 1,
        };
        let mut count_full = SoftwareSignerAuditJournalV1 {
            directory: missing_directory.clone(),
            sequence: 2,
            audit_head: [0x44; 32],
            record_bytes: 1,
        };
        assert_eq!(
            count_full.append_with_limits(fixture_event(), &keypair, limits),
            Err(SoftwareSignerJournalErrorV1::Capacity)
        );
        let mut bytes_full = SoftwareSignerAuditJournalV1 {
            directory: missing_directory.clone(),
            sequence: 1,
            audit_head: [0x55; 32],
            record_bytes: 1,
        };
        assert_eq!(
            bytes_full.append_with_limits(fixture_event(), &keypair, limits),
            Err(SoftwareSignerJournalErrorV1::Capacity)
        );
        assert!(!missing_directory.exists());
        assert_eq!((bytes_full.sequence, bytes_full.record_bytes), (1, 1));
    }
    #[test]
    fn directory_inventory_is_bounded_and_requires_contiguous_records() {
        let parent = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
            .expect("temporary parent");
        let exact = parent.path().join("exact");
        create_private_directory(&exact).expect("private audit directory");
        write_private_file(&exact.join(record_name(1)), &[0x01]);
        write_private_file(&exact.join(record_name(2)), &[0x02]);
        let limits = AuditRetentionLimitsV1 {
            max_records: 2,
            max_total_bytes: 2,
        };
        let inventory = scan_audit_directory(&exact, limits).expect("exact bounded inventory");
        assert_eq!((inventory.record_count, inventory.record_bytes), (2, 2));
        assert!(inventory.pending.is_none());
        write_private_file(&exact.join(record_name(3)), &[0x03]);
        assert!(matches!(
            scan_audit_directory(&exact, limits),
            Err(SoftwareSignerJournalErrorV1::Capacity)
        ));
        let bytes_full = parent.path().join("bytes-full");
        create_private_directory(&bytes_full).expect("private bytes-full directory");
        write_private_file(&bytes_full.join(record_name(1)), &[0x01, 0x02]);
        assert!(matches!(
            scan_audit_directory(
                &bytes_full,
                AuditRetentionLimitsV1 {
                    max_records: 1,
                    max_total_bytes: 1,
                }
            ),
            Err(SoftwareSignerJournalErrorV1::Capacity)
        ));
        let gap = parent.path().join("gap");
        create_private_directory(&gap).expect("private gap directory");
        write_private_file(&gap.join(record_name(1)), &[0x01]);
        write_private_file(&gap.join(record_name(3)), &[0x03]);
        assert!(matches!(
            scan_audit_directory(
                &gap,
                AuditRetentionLimitsV1 {
                    max_records: 3,
                    max_total_bytes: 2,
                }
            ),
            Err(SoftwareSignerJournalErrorV1::Invalid)
        ));
    }
    #[test]
    fn directory_inventory_admits_only_the_exact_pending_successor() {
        let parent = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
            .expect("temporary parent");
        let directory = parent.path().join("audit-v1");
        create_private_directory(&directory).expect("private audit directory");
        write_private_file(&directory.join(record_name(1)), &[0x01]);
        write_private_file(&directory.join(pending_name(2)), &[0x02]);
        let limits = AuditRetentionLimitsV1 {
            max_records: 2,
            max_total_bytes: 2,
        };
        let inventory =
            scan_audit_directory(&directory, limits).expect("exact pending successor inventory");
        assert_eq!((inventory.record_count, inventory.record_bytes), (1, 2));
        assert_eq!(
            inventory.pending.as_ref().map(|(sequence, _)| *sequence),
            Some(2)
        );
        fs::remove_file(directory.join(pending_name(2))).expect("remove exact pending fixture");
        write_private_file(&directory.join(pending_name(1)), &[0x02]);
        assert!(matches!(
            scan_audit_directory(&directory, limits),
            Err(SoftwareSignerJournalErrorV1::Invalid)
        ));
    }
}
