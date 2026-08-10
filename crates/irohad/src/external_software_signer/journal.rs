//! Payload-free immutable audit journal and crash recovery.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

use iroha_crypto::{KeyPair, Signature};
use norito::codec::{Decode, Encode};

use super::{
    envelope::SoftwareSignerKeyEnvelopeAadV1,
    protocol::{
        SIGNER_AUDIT_MAGIC_V1, SIGNER_MAX_SIGNATURE_BYTES_V1, SIGNER_PROTOCOL_VERSION_V1,
        digest_canonical,
    },
};

const AUDIT_RECORD_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.audit-record.v1";
const AUDIT_ATTESTATION_DOMAIN_V1: &[u8] = b"iroha.external-signer.audit-attestation.v1";
const AUDIT_RECORD_MAX_BYTES_V1: usize = 32 * 1024;
const AUDIT_MAX_RECORDS_V1: u64 = 10_000_000;

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
}

pub(super) struct SoftwareSignerAuditJournalV1 {
    directory: PathBuf,
    sequence: u64,
    audit_head: [u8; 32],
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
        };
        let digest = journal.append(
            SoftwareSignerAuditEventV1::Genesis {
                key: key.clone(),
                envelope_digest,
            },
            keypair,
        )?;
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
            },
        ))
    }

    pub(super) fn open(
        state_directory: &Path,
    ) -> Result<(Self, RecoveredJournalV1), SoftwareSignerJournalErrorV1> {
        let directory = state_directory.join("audit-v1");
        validate_private_directory(&directory)?;
        recover_pending_record(&directory)?;
        let records = read_records(&directory)?;
        let recovered = validate_records(&records)?;
        Ok((
            Self {
                directory,
                sequence: recovered.sequence,
                audit_head: recovered.audit_head,
            },
            recovered,
        ))
    }

    pub(super) fn append(
        &mut self,
        event: SoftwareSignerAuditEventV1,
        keypair: &KeyPair,
    ) -> Result<[u8; 32], SoftwareSignerJournalErrorV1> {
        let sequence = self
            .sequence
            .checked_add(1)
            .filter(|sequence| *sequence <= AUDIT_MAX_RECORDS_V1)
            .ok_or(SoftwareSignerJournalErrorV1::Capacity)?;
        let body = SoftwareSignerAuditRecordBodyV1 {
            magic: SIGNER_AUDIT_MAGIC_V1,
            version: SIGNER_PROTOCOL_VERSION_V1,
            sequence,
            predecessor_digest: self.audit_head,
            event,
        };
        let record_digest = digest_canonical(AUDIT_RECORD_DIGEST_DOMAIN_V1, &body)
            .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
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
        persist_record(&self.directory, &record)?;
        self.sequence = sequence;
        self.audit_head = record_digest;
        Ok(record_digest)
    }

    pub(super) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) const fn audit_head(&self) -> [u8; 32] {
        self.audit_head
    }
}

fn validate_records(
    records: &[SoftwareSignerAuditRecordV1],
) -> Result<RecoveredJournalV1, SoftwareSignerJournalErrorV1> {
    let first = records
        .first()
        .ok_or(SoftwareSignerJournalErrorV1::Invalid)?;
    let SoftwareSignerAuditEventV1::Genesis {
        key: genesis_key,
        envelope_digest,
    } = &first.body.event
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

    for (index, record) in records.iter().enumerate() {
        let expected_sequence =
            u64::try_from(index + 1).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?;
        if record.body.magic != SIGNER_AUDIT_MAGIC_V1
            || record.body.version != SIGNER_PROTOCOL_VERSION_V1
            || record.body.sequence != expected_sequence
            || record.body.predecessor_digest != expected_predecessor
            || record.record_digest == [0; 32]
            || digest_canonical(AUDIT_RECORD_DIGEST_DOMAIN_V1, &record.body)
                .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?
                != record.record_digest
            || record.attestation.is_empty()
            || record.attestation.len() != active_key.algorithm.algorithm().signature_payload_len()
        {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        let attestation = Signature::try_from_bytes(&record.attestation)
            .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
        attestation
            .verify(
                &active_key.public_key,
                &audit_attestation_message(record.record_digest, record.body.sequence),
            )
            .map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;

        match &record.body.event {
            SoftwareSignerAuditEventV1::Genesis {
                key,
                envelope_digest,
            } if index == 0 && key == &active_key && envelope_digest == &active_envelope_digest => {
                genesis_digest = record.record_digest;
            }
            SoftwareSignerAuditEventV1::SignCommitted {
                operation_id,
                request_digest,
                payload_digest,
                signature,
                signature_digest,
            } if !revoked => {
                if *operation_id == [0; 32]
                    || *request_digest == [0; 32]
                    || *payload_digest == [0; 32]
                    || signature.is_empty()
                    || signature.len() != active_key.algorithm.algorithm().signature_payload_len()
                    || digest_parts_signature(signature) != *signature_digest
                    || sign_commits.contains_key(operation_id)
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
                sign_commits.insert(
                    *operation_id,
                    RecoveredSignCommitV1 {
                        request_digest: *request_digest,
                        payload_digest: *payload_digest,
                        signature: signature.clone(),
                        sequence: record.body.sequence,
                        audit_head: record.record_digest,
                    },
                );
            }
            SoftwareSignerAuditEventV1::EquivocationRejected {
                operation_id,
                accepted_request_digest,
                rejected_request_digest,
            } if !revoked => {
                let Some(accepted) = sign_commits.get(operation_id) else {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                };
                if accepted.request_digest != *accepted_request_digest
                    || *rejected_request_digest == [0; 32]
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
                if *operation_id == [0; 32]
                    || *request_digest == [0; 32]
                    || admin_commits.contains_key(operation_id)
                    || *previous_key_revision != active_key.key_revision
                    || *previous_policy_revision != active_key.policy_revision
                    || new_key.key_revision <= active_key.key_revision
                    || new_key.policy_revision <= active_key.policy_revision
                    || new_key.role != active_key.role
                    || new_key.handle != active_key.handle
                    || new_key.service_id != active_key.service_id
                    || new_key.administrator_id != active_key.administrator_id
                    || new_key.service_uid != active_key.service_uid
                    || new_key.client_uid != active_key.client_uid
                    || new_key.administrator_uid != active_key.administrator_uid
                    || *new_envelope_digest == [0; 32]
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
                admin_commits.insert(
                    *operation_id,
                    RecoveredAdminCommitV1 {
                        request_digest: *request_digest,
                    },
                );
                active_key = new_key.clone();
                active_envelope_digest = *new_envelope_digest;
            }
            SoftwareSignerAuditEventV1::Revoked {
                operation_id,
                request_digest,
                key_revision,
                policy_revision,
                reason_digest,
            } if !revoked => {
                if *operation_id == [0; 32]
                    || *request_digest == [0; 32]
                    || *reason_digest == [0; 32]
                    || admin_commits.contains_key(operation_id)
                    || *key_revision != active_key.key_revision
                    || *policy_revision != active_key.policy_revision
                {
                    return Err(SoftwareSignerJournalErrorV1::Invalid);
                }
                admin_commits.insert(
                    *operation_id,
                    RecoveredAdminCommitV1 {
                        request_digest: *request_digest,
                    },
                );
                revoked = true;
            }
            _ => return Err(SoftwareSignerJournalErrorV1::Invalid),
        }
        expected_predecessor = record.record_digest;
    }

    Ok(RecoveredJournalV1 {
        active_key,
        active_envelope_digest,
        audit_genesis_digest: genesis_digest,
        sequence: u64::try_from(records.len())
            .map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?,
        audit_head: expected_predecessor,
        revoked,
        sign_commits,
        admin_commits,
    })
}

fn persist_record(
    directory: &Path,
    record: &SoftwareSignerAuditRecordV1,
) -> Result<(), SoftwareSignerJournalErrorV1> {
    let encoded =
        norito::encode_canonical(record).map_err(|_| SoftwareSignerJournalErrorV1::Invalid)?;
    if encoded.is_empty() || encoded.len() > AUDIT_RECORD_MAX_BYTES_V1 {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
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
    file.write_all(&encoded)
        .and_then(|()| file.sync_all())
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    validate_private_file(&pending)?;
    fs::rename(&pending, &final_path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    sync_directory(directory)?;
    validate_private_file(&final_path)
}

fn read_records(
    directory: &Path,
) -> Result<Vec<SoftwareSignerAuditRecordV1>, SoftwareSignerJournalErrorV1> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)? {
        let entry = entry.map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(SoftwareSignerJournalErrorV1::Invalid)?;
        if parse_record_name(name).is_none() {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
        paths.push(entry.path());
    }
    paths.sort();
    if paths.is_empty()
        || u64::try_from(paths.len()).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?
            > AUDIT_MAX_RECORDS_V1
    {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            if parse_record_name(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .ok_or(SoftwareSignerJournalErrorV1::Invalid)?,
            ) != Some(
                u64::try_from(index + 1).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?,
            ) {
                return Err(SoftwareSignerJournalErrorV1::Invalid);
            }
            read_record(path)
        })
        .collect()
}

fn read_record(path: &Path) -> Result<SoftwareSignerAuditRecordV1, SoftwareSignerJournalErrorV1> {
    validate_private_file(path)?;
    let mut file = File::open(path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(
            u64::try_from(AUDIT_RECORD_MAX_BYTES_V1 + 1)
                .map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?,
        )
        .read_to_end(&mut bytes)
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    if bytes.is_empty() || bytes.len() > AUDIT_RECORD_MAX_BYTES_V1 {
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

fn recover_pending_record(directory: &Path) -> Result<(), SoftwareSignerJournalErrorV1> {
    let mut pending = Vec::new();
    for entry in fs::read_dir(directory).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)? {
        let entry = entry.map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        };
        if let Some(sequence) = parse_pending_name(name) {
            pending.push((sequence, entry.path()));
        } else if parse_record_name(name).is_none() {
            return Err(SoftwareSignerJournalErrorV1::Invalid);
        }
    }
    if pending.len() > 1 {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    let Some((sequence, path)) = pending.pop() else {
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
    let existing_count = fs::read_dir(directory)
        .map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(parse_record_name)
                .is_some()
        })
        .count();
    let existing_count =
        u64::try_from(existing_count).map_err(|_| SoftwareSignerJournalErrorV1::Capacity)?;
    if sequence != existing_count + 1 {
        return Err(SoftwareSignerJournalErrorV1::Invalid);
    }
    fs::rename(path, final_path).map_err(|_| SoftwareSignerJournalErrorV1::Unavailable)?;
    sync_directory(directory)
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

pub(super) fn validate_private_file(path: &Path) -> Result<(), SoftwareSignerJournalErrorV1> {
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
    Ok(())
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
    /// The bounded V1 record capacity was exhausted.
    Capacity,
    /// Filesystem persistence or cryptographic attestation was unavailable.
    Unavailable,
}
