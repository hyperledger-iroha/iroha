//! Descriptor-owned retention of complete, untrusted device response evidence.
//!
//! Framing, hashes and fsync establish retained bytes only. A qualified guard must still
//! authenticate the original device signature, policy, report, credential, release and exact
//! current checkpoint before any response may supply authority. This archive exposes no
//! completion, stock observation, authenticated absence, or history-retirement capability.

use super::{
    KagemushaLaneIdV1, KagemushaRecoveryJournalPrefixV1,
    private_journal::{PrivateJournal, PrivateJournalError, PrivateJournalFormat},
};
use iroha_data_model::{
    kagemusha::{
        KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1, KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1,
        KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1, KagemushaHardwareCredentialV1,
        kagemusha_decode_device_success_response_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use norito::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, path::Path};

const COMMAND_HEADER_BYTES: usize = 80;
const COMMAND_MAX_BYTES: usize = COMMAND_HEADER_BYTES + KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1;
const RECORD_MAX_BYTES: usize = COMMAND_MAX_BYTES
    + KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
    + KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1
    + 2048;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "responses.norito.wal",
    magic: b"IKGRW1\0\0",
    hash_domain: b"iroha:kagemusha:v1:response-evidence-disk:frame\0",
    maximum_payload_bytes: RECORD_MAX_BYTES as u64,
};

/// Response retention failures; no outcome authenticates hardware or monetary state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaResponseEvidenceArchiveErrorV1 {
    /// The private path, syscall or storage device is unavailable.
    #[error("response evidence storage unavailable")]
    StorageUnavailable,
    /// Another writer already owns this response archive.
    #[error("response evidence archive is already open")]
    AlreadyOpen,
    /// File ownership, exact framing or canonical replay failed.
    #[error("response evidence archive integrity failed")]
    JournalCorrupt,
    /// The write or named-file race prevents acknowledgment of durable state.
    #[error("response evidence durability is uncertain")]
    DurabilityUncertain,
    /// The wallet binding or complete correlated frames are malformed or oversized.
    #[error("invalid response evidence binding")]
    InvalidBinding,
    /// An operation and request identifier already retain different immutable frame bytes.
    #[error("conflicting response evidence binding")]
    Conflict,
}

use KagemushaResponseEvidenceArchiveErrorV1 as Error;
type Result<T> = core::result::Result<T, Error>;
type ResponseKey = (u8, [u8; 32]);

/// Exact original qualification context retained as untrusted evidence, never as authority.
///
/// Native verification must resolve the original release and authenticate all these bindings.
/// A newer credential or release cannot replace the context of an older response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct KagemushaResponseEvidenceContextV1 {
    /// Complete original canonical credential, including its governance signature.
    pub canonical_credential: Vec<u8>,
    /// Exact originally authenticated release identity.
    pub release_id: [u8; 32],
    /// Original device hardware-policy identity used by its response signature.
    pub hardware_policy_id: [u8; 32],
    /// Original qualification-report digest used by the response signature.
    pub qualification_report_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::response_evidence_archive::Record")]
enum Record {
    Initialize {
        lane: KagemushaLaneIdV1,
        asset_incarnation: AxtAssetIncarnationV1,
    },
    Observe {
        operation: u8,
        request_id: [u8; 32],
        command: Vec<u8>,
        response: Vec<u8>,
        context: KagemushaResponseEvidenceContextV1,
    },
}

/// Collision-resistant retry identity only, never hardware or checkpoint authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RetainedIdentity {
    command_len: usize,
    response_len: usize,
    credential_len: usize,
    command_digest: [u8; 32],
    response_digest: [u8; 32],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
    credential_digest: [u8; 32],
    release_id: [u8; 32],
}

/// A native-owned, append-only response evidence WAL bound to one wallet incarnation.
///
/// Every complete original frame remains on disk. The in-memory index retains only lengths
/// and SHA-256 identities, so replay allocates at most one bounded full record at a time.
/// There is no history-count limit, expiry or eviction; actual storage exhaustion fails closed.
/// Observation responses may be retained as evidence but never become current observations.
/// TODO: qualified native checkpoint/response verification must select this exact prefix;
/// neither a host digest nor this storage object implements that missing authority.
pub struct KagemushaResponseEvidenceArchiveV1 {
    wal: PrivateJournal,
    lane: KagemushaLaneIdV1,
    retained: BTreeMap<ResponseKey, RetainedIdentity>,
}

impl KagemushaResponseEvidenceArchiveV1 {
    pub(super) fn create_new(
        path: &Path,
        lane: &KagemushaLaneIdV1,
        asset_incarnation: AxtAssetIncarnationV1,
    ) -> Result<Self> {
        validate_owner(lane, asset_incarnation)?;
        let mut archive = Self {
            wal: PrivateJournal::create_new(path, FORMAT).map_err(storage_error)?,
            lane: lane.clone(),
            retained: BTreeMap::new(),
        };
        archive
            .wal
            .append(&encode(&Record::Initialize {
                lane: lane.clone(),
                asset_incarnation,
            })?)
            .map_err(storage_error)?;
        Ok(archive)
    }

    pub(super) fn open_existing(
        path: &Path,
        lane: &KagemushaLaneIdV1,
        asset_incarnation: AxtAssetIncarnationV1,
    ) -> Result<Self> {
        validate_owner(lane, asset_incarnation)?;
        let mut archive = Self {
            wal: PrivateJournal::open_existing(path, FORMAT).map_err(storage_error)?,
            lane: lane.clone(),
            retained: BTreeMap::new(),
        };
        while let Some((sequence, payload)) = archive.wal.replay_next().map_err(storage_error)? {
            let record = decode(&payload)?;
            if sequence == 0 {
                if record
                    != (Record::Initialize {
                        lane: lane.clone(),
                        asset_incarnation,
                    })
                {
                    return Err(Error::JournalCorrupt);
                }
                continue;
            }
            let Record::Observe {
                operation,
                request_id,
                command,
                response,
                context,
            } = record
            else {
                return Err(Error::JournalCorrupt);
            };
            let identity =
                frame_identity(lane, operation, request_id, &command, &response, &context)
                    .map_err(|_| Error::JournalCorrupt)?;
            // Exact retries never append: even a duplicate identical record is invalid history.
            if archive
                .retained
                .insert((operation, request_id), identity)
                .is_some()
            {
                return Err(Error::JournalCorrupt);
            }
        }
        Ok(archive)
    }

    /// Return the live descriptor's complete fsynced and replayed evidence prefix.
    ///
    /// # Errors
    /// Rejects a poisoned owner, replaced or modified file, or incomplete replay.
    pub fn recovery_prefix(&self) -> Result<KagemushaRecoveryJournalPrefixV1> {
        self.wal.recovery_prefix().map_err(storage_error)
    }

    /// Retain both complete correlated frames, including the original response authenticator.
    ///
    /// Returns `true` only after the first append is fsynced. A retry with the same complete
    /// frame lengths and all six original context commitments returns `false` without changing
    /// the original bytes or prefix. This validates framing and correlation, not signatures or
    /// currentness. An expired original credential retains its historical meaning; this method
    /// never substitutes current qualification for an older response.
    ///
    /// # Errors
    /// Rejects malformed/oversized frames, changed immutable evidence for the same operation
    /// and request ID, unavailable storage, and all uncertain writes or lost file ownership.
    pub fn retain_observed_response(
        &mut self,
        operation: u8,
        request_id: [u8; 32],
        full_command: &[u8],
        full_response: &[u8],
        context: &KagemushaResponseEvidenceContextV1,
    ) -> Result<bool> {
        // Check before acknowledging even a cached exact retry or rejecting a conflict.
        self.wal.check_owned().map_err(storage_error)?;
        let identity = frame_identity(
            &self.lane,
            operation,
            request_id,
            full_command,
            full_response,
            context,
        )?;
        let key = (operation, request_id);
        if let Some(original) = self.retained.get(&key) {
            return if original == &identity {
                Ok(false)
            } else {
                Err(Error::Conflict)
            };
        }
        let bytes = encode(&Record::Observe {
            operation,
            request_id,
            command: full_command.to_vec(),
            response: full_response.to_vec(),
            context: context.clone(),
        })?;
        self.wal.append(&bytes).map_err(storage_error)?;
        self.retained.insert(key, identity);
        Ok(true)
    }
}

fn validate_owner(lane: &KagemushaLaneIdV1, incarnation: AxtAssetIncarnationV1) -> Result<()> {
    lane.validate().map_err(|_| Error::InvalidBinding)?;
    incarnation.validate().map_err(|_| Error::InvalidBinding)
}

fn frame_identity(
    lane: &KagemushaLaneIdV1,
    operation: u8,
    request_id: [u8; 32],
    command: &[u8],
    response: &[u8],
    context: &KagemushaResponseEvidenceContextV1,
) -> Result<RetainedIdentity> {
    if !(1..=22).contains(&operation)
        || request_id == [0; 32]
        || !(COMMAND_HEADER_BYTES + 1..=COMMAND_MAX_BYTES).contains(&command.len())
        || response.len() > KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
        || &command[..8] != b"IKGMJCM1"
        || command[8..10] != 1_u16.to_le_bytes()
        || command[10] != operation
        || command[11] != 0
        || command[12..44] != request_id
    {
        return Err(Error::InvalidBinding);
    }
    let payload_len = u32::from_le_bytes(
        command[44..48]
            .try_into()
            .map_err(|_| Error::InvalidBinding)?,
    ) as usize;
    if payload_len != command.len() - COMMAND_HEADER_BYTES
        || command[48..80] != Sha256::digest(&command[COMMAND_HEADER_BYTES..])[..]
    {
        return Err(Error::InvalidBinding);
    }
    kagemusha_decode_device_success_response_v1(response, operation, request_id)
        .map_err(|_| Error::InvalidBinding)?;
    let maximum = KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1;
    if context.canonical_credential.is_empty()
        || context.canonical_credential.len() > maximum
        || context.release_id == [0; 32]
        || context.hardware_policy_id == [0; 32]
        || context.qualification_report_digest == [0; 32]
    {
        return Err(Error::InvalidBinding);
    }
    let credential: KagemushaHardwareCredentialV1 = norito::decode_canonical_with_limits(
        &context.canonical_credential,
        norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(|_| Error::InvalidBinding)?;
    credential
        .validate_shape()
        .map_err(|_| Error::InvalidBinding)?;
    if credential.network_id != lane.network_id
        || credential.lane_commitment != lane.device_lane_id
        || norito::encode_canonical(&credential).map_err(|_| Error::InvalidBinding)?
            != context.canonical_credential
    {
        return Err(Error::InvalidBinding);
    }
    Ok(RetainedIdentity {
        command_len: command.len(),
        response_len: response.len(),
        credential_len: context.canonical_credential.len(),
        command_digest: Sha256::digest(command).into(),
        response_digest: Sha256::digest(response).into(),
        hardware_policy_id: context.hardware_policy_id,
        qualification_report_digest: context.qualification_report_digest,
        credential_digest: Sha256::digest(&context.canonical_credential).into(),
        release_id: context.release_id,
    })
}

fn encode(record: &Record) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(record).map_err(|_| Error::InvalidBinding)?;
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(Error::InvalidBinding);
    }
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<Record> {
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(Error::JournalCorrupt);
    }
    // Decode the complete fixed-schema Norito frame under explicit allocation/work bounds.
    let record: Record = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES * 4,
            RECORD_MAX_BYTES * 8,
            32,
        ),
    )
    .map_err(|_| Error::JournalCorrupt)?;
    if encode(&record).map_err(|_| Error::JournalCorrupt)? != bytes {
        return Err(Error::JournalCorrupt);
    }
    Ok(record)
}

fn storage_error(error: PrivateJournalError) -> Error {
    match error {
        PrivateJournalError::StorageUnavailable => Error::StorageUnavailable,
        PrivateJournalError::AlreadyOpen => Error::AlreadyOpen,
        PrivateJournalError::Corrupt => Error::JournalCorrupt,
        PrivateJournalError::Uncertain => Error::DurabilityUncertain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_state::private_journal::TestPersistenceFailure;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::kagemusha::{
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, kagemusha_device_key_reference_v1,
    };
    use iroha_data_model::{NetworkId, asset::AssetDefinitionId};
    use std::{fs, io::Write as _};

    fn owner() -> (KagemushaLaneIdV1, AxtAssetIncarnationV1) {
        (
            KagemushaLaneIdV1 {
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"response-archive-test-network"),
                )),
                device_lane_id: [2; 32],
                asset: AssetDefinitionId::from_uuid_bytes([
                    0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84,
                    0xfd, 0xcd, 0x2f,
                ])
                .unwrap(),
                scale: 2,
            },
            AxtAssetIncarnationV1::try_from_bytes([3; 32]).unwrap(),
        )
    }

    fn context(lane: &KagemushaLaneIdV1) -> KagemushaResponseEvidenceContextV1 {
        let signing = p256::ecdsa::SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let credential = KagemushaHardwareCredentialV1 {
            version: 1,
            credential_id: [0; 32],
            network_id: lane.network_id,
            hardware_profile_id: [3; 32],
            suite_id: [4; 32],
            firmware_policy_digest: [5; 32],
            policy_epoch: 1,
            lane_commitment: lane.device_lane_id,
            hardware_epoch_id: [6; 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 1,
            expires_at_ms: 2,
            governance_signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap(),
        }
        .seal_credential_id()
        .unwrap();
        KagemushaResponseEvidenceContextV1 {
            canonical_credential: norito::encode_canonical(&credential).unwrap(),
            release_id: [7; 32],
            hardware_policy_id: [8; 32],
            qualification_report_digest: [9; 32],
        }
    }

    // Deliberately unqualified bodies/authenticator: framing never becomes proof authority.
    fn frames(operation: u8, id: [u8; 32], value: u8, length: usize) -> (Vec<u8>, Vec<u8>) {
        let body = vec![value; length];
        let mut command = b"IKGMJCM1".to_vec();
        command.extend_from_slice(&1_u16.to_le_bytes());
        command.extend_from_slice(&[operation, 0]);
        command.extend_from_slice(&id);
        command.extend_from_slice(&(length as u32).to_le_bytes());
        command.extend_from_slice(&Sha256::digest(&body));
        command.extend_from_slice(&body);
        let signature = [value; 64];
        let mut response = b"IKGMJRS1".to_vec();
        response.extend_from_slice(&1_u16.to_le_bytes());
        response.extend_from_slice(&[operation, 0]);
        response.extend_from_slice(&id);
        response.extend_from_slice(&(length as u32).to_le_bytes());
        response.extend_from_slice(&64_u32.to_le_bytes());
        response.extend_from_slice(&Sha256::digest(&body));
        response.extend_from_slice(&Sha256::digest(signature));
        response.extend_from_slice(&body);
        response.extend_from_slice(&signature);
        (command, response)
    }

    #[test]
    fn initialized_archive_reopens_only_for_exact_owner_without_reset() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("archive");
        let (lane, incarnation) = owner();
        let archive =
            KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
        let prefix = archive.recovery_prefix().unwrap();
        assert_eq!(prefix.sequence, 1);
        assert_ne!(prefix.head, [0; 32]);
        assert!(prefix.byte_len > 0);
        assert!(matches!(
            KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation),
            Err(Error::AlreadyOpen)
        ));
        drop(archive);
        let archive =
            KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation).unwrap();
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
        drop(archive);
        let mut changed = lane.clone();
        changed.device_lane_id[0] ^= 1;
        assert!(matches!(
            KagemushaResponseEvidenceArchiveV1::open_existing(&path, &changed, incarnation),
            Err(Error::JournalCorrupt)
        ));
        assert!(matches!(
            KagemushaResponseEvidenceArchiveV1::open_existing(
                &path,
                &lane,
                AxtAssetIncarnationV1::try_from_bytes(
                    *Hash::new(b"different-response-archive-incarnation").as_ref()
                )
                .unwrap()
            ),
            Err(Error::JournalCorrupt)
        ));
        assert!(KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).is_err());
        changed.device_lane_id = [0; 32];
        let invalid = root.path().canonicalize().unwrap().join("invalid");
        assert!(matches!(
            KagemushaResponseEvidenceArchiveV1::create_new(&invalid, &changed, incarnation),
            Err(Error::InvalidBinding)
        ));
        assert!(!invalid.exists());
    }

    #[test]
    fn original_frames_and_prefix_survive_retry_conflict_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("archive");
        let (lane, incarnation) = owner();
        let context = context(&lane);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
        let (command, response) = frames(2, [8; 32], 9, 32);
        assert!(
            archive
                .retain_observed_response(2, [8; 32], &command, &response, &context)
                .unwrap()
        );
        let prefix = archive.recovery_prefix().unwrap();
        let bytes = fs::read(path.join(FORMAT.filename)).unwrap();
        assert!(
            !archive
                .retain_observed_response(2, [8; 32], &command, &response, &context)
                .unwrap()
        );
        let (other_command, other_response) = frames(2, [8; 32], 10, 32);
        for (candidate_command, candidate_response) in
            [(&other_command, &response), (&command, &other_response)]
        {
            assert_eq!(
                archive.retain_observed_response(
                    2,
                    [8; 32],
                    candidate_command,
                    candidate_response,
                    &context
                ),
                Err(Error::Conflict)
            );
        }
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
        assert_eq!(fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
        drop(archive);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation).unwrap();
        assert!(
            !archive
                .retain_observed_response(2, [8; 32], &command, &response, &context)
                .unwrap()
        );
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
        drop(archive);
        let mut wal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        wal.replay_next().unwrap().unwrap();
        let (_, payload) = wal.replay_next().unwrap().unwrap();
        assert_eq!(
            decode(&payload).unwrap(),
            Record::Observe {
                operation: 2,
                request_id: [8; 32],
                command,
                response,
                context: context.clone(),
            }
        );
        assert!(wal.replay_next().unwrap().is_none());
    }

    #[test]
    fn framing_bounds_and_correlation_fail_before_any_append() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("archive");
        let (lane, incarnation) = owner();
        let context = context(&lane);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
        let prefix = archive.recovery_prefix().unwrap();
        let (command, response) = frames(2, [8; 32], 9, 32);
        for cut in 0..COMMAND_HEADER_BYTES + 1 {
            assert_eq!(
                archive.retain_observed_response(2, [8; 32], &command[..cut], &response, &context),
                Err(Error::InvalidBinding)
            );
        }
        for offset in [0, 8, 10, 11, 12, 44, 48, COMMAND_HEADER_BYTES] {
            let mut invalid = command.clone();
            invalid[offset] ^= 1;
            assert_eq!(
                archive.retain_observed_response(2, [8; 32], &invalid, &response, &context),
                Err(Error::InvalidBinding)
            );
        }
        for offset in [0, 8, 10, 11, 12, 44, 48, 52, 84, 116] {
            let mut invalid = response.clone();
            invalid[offset] ^= 1;
            assert_eq!(
                archive.retain_observed_response(2, [8; 32], &command, &invalid, &context),
                Err(Error::InvalidBinding)
            );
        }
        for (operation, id) in [(0, [8; 32]), (23, [8; 32]), (2, [0; 32]), (3, [8; 32])] {
            assert_eq!(
                archive.retain_observed_response(operation, id, &command, &response, &context),
                Err(Error::InvalidBinding)
            );
        }
        let (oversize_command, oversize_response) =
            frames(2, [8; 32], 9, KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1 + 1);
        assert_eq!(
            archive.retain_observed_response(2, [8; 32], &oversize_command, &response, &context),
            Err(Error::InvalidBinding)
        );
        assert_eq!(
            archive.retain_observed_response(2, [8; 32], &command, &oversize_response, &context),
            Err(Error::InvalidBinding)
        );
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
        let (maximum_command, maximum_response) =
            frames(2, [8; 32], 9, KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1);
        assert!(
            archive
                .retain_observed_response(2, [8; 32], &maximum_command, &maximum_response, &context)
                .unwrap()
        );
        drop(archive);
        KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation).unwrap();
    }

    #[test]
    fn stage_key_includes_operation_and_history_has_no_card_slot_limit() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("archive");
        let (lane, incarnation) = owner();
        let context = context(&lane);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
        for operation in 1..=22 {
            let (command, response) = frames(operation, [8; 32], 9, 1);
            assert!(
                archive
                    .retain_observed_response(operation, [8; 32], &command, &response, &context)
                    .unwrap()
            );
        }
        for value in 1..=40 {
            let (command, response) = frames(2, [value; 32], 9, 1);
            let inserted = archive
                .retain_observed_response(2, [value; 32], &command, &response, &context)
                .unwrap();
            assert_eq!(inserted, value != 8);
        }
        assert_eq!(archive.retained.len(), 61);
        let prefix = archive.recovery_prefix().unwrap();
        drop(archive);
        let archive =
            KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation).unwrap();
        assert_eq!(archive.retained.len(), 61);
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
    }

    #[test]
    fn every_original_qualification_binding_is_immutable_and_bounded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("archive");
        let (lane, incarnation) = owner();
        let context = context(&lane);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
        let (command, response) = frames(2, [8; 32], 9, 1);
        archive
            .retain_observed_response(2, [8; 32], &command, &response, &context)
            .unwrap();
        let prefix = archive.recovery_prefix().unwrap();
        let bytes = fs::read(path.join(FORMAT.filename)).unwrap();
        for field in 0..4 {
            let mut changed = context.clone();
            match field {
                0 => changed.release_id[0] ^= 1,
                1 => changed.hardware_policy_id[0] ^= 1,
                2 => changed.qualification_report_digest[0] ^= 1,
                _ => {
                    let mut credential: KagemushaHardwareCredentialV1 =
                        norito::decode_canonical(&changed.canonical_credential).unwrap();
                    credential.governance_signature =
                        KagemushaDeviceSignatureV1::from_raw_bytes(&[2; 64]).unwrap();
                    changed.canonical_credential = norito::encode_canonical(&credential).unwrap();
                }
            }
            assert_eq!(
                archive.retain_observed_response(2, [8; 32], &command, &response, &changed),
                Err(Error::Conflict),
                "context field {field}"
            );
        }
        for field in 0..7 {
            let mut changed = context.clone();
            match field {
                0 => changed.release_id = [0; 32],
                1 => changed.hardware_policy_id = [0; 32],
                2 => changed.qualification_report_digest = [0; 32],
                3 => changed.canonical_credential.clear(),
                4 => changed.canonical_credential.push(0),
                5 => {
                    changed.canonical_credential =
                        vec![0; KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1 + 1];
                }
                _ => {
                    let mut credential: KagemushaHardwareCredentialV1 =
                        norito::decode_canonical(&changed.canonical_credential).unwrap();
                    credential.lane_commitment = [10; 32];
                    credential = credential.seal_credential_id().unwrap();
                    changed.canonical_credential = norito::encode_canonical(&credential).unwrap();
                }
            }
            assert_eq!(
                archive.retain_observed_response(2, [8; 32], &command, &response, &changed),
                Err(Error::InvalidBinding),
                "invalid context field {field}"
            );
        }
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
        assert_eq!(fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
        drop(archive);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation).unwrap();
        assert!(
            !archive
                .retain_observed_response(2, [8; 32], &command, &response, &context)
                .unwrap()
        );
        assert_eq!(archive.recovery_prefix().unwrap(), prefix);
    }

    #[test]
    fn uncertain_writes_poison_all_retry_and_prefix_acknowledgments() {
        for failure in [
            TestPersistenceFailure::PartialWrite,
            TestPersistenceFailure::BeforeSync,
            TestPersistenceFailure::AfterSync,
            TestPersistenceFailure::ReplaceAfterSync,
            TestPersistenceFailure::TruncateAfterSync,
        ] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().canonicalize().unwrap().join("archive");
            let (lane, incarnation) = owner();
            let context = context(&lane);
            let mut archive =
                KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
            let (command, response) = frames(2, [8; 32], 9, 1);
            archive
                .retain_observed_response(2, [8; 32], &command, &response, &context)
                .unwrap();
            archive.wal.failure.set(Some(failure));
            let (next_command, next_response) = frames(3, [8; 32], 9, 1);
            assert_eq!(
                archive.retain_observed_response(
                    3,
                    [8; 32],
                    &next_command,
                    &next_response,
                    &context
                ),
                Err(Error::DurabilityUncertain)
            );
            assert_eq!(
                archive.retain_observed_response(2, [8; 32], &command, &response, &context),
                Err(Error::DurabilityUncertain)
            );
            assert_eq!(archive.recovery_prefix(), Err(Error::DurabilityUncertain));
            drop(archive);
            let reopened =
                KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation);
            if matches!(
                failure,
                TestPersistenceFailure::PartialWrite | TestPersistenceFailure::TruncateAfterSync
            ) {
                assert!(matches!(reopened, Err(Error::JournalCorrupt)));
            } else {
                let mut reopened = reopened.unwrap();
                assert!(
                    !reopened
                        .retain_observed_response(
                            3,
                            [8; 32],
                            &next_command,
                            &next_response,
                            &context
                        )
                        .unwrap()
                );
            }
        }
    }

    #[test]
    fn canonical_record_replay_rejects_trailing_empty_duplicate_and_truncated_history() {
        let (lane, incarnation) = owner();
        let context = context(&lane);
        let initial = encode(&Record::Initialize {
            lane: lane.clone(),
            asset_incarnation: incarnation,
        })
        .unwrap();
        for cut in 0..initial.len() {
            assert!(decode(&initial[..cut]).is_err());
        }
        let mut trailing = initial;
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        assert!(decode(&vec![0; RECORD_MAX_BYTES + 1]).is_err());
        for malformed in 0..4 {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().canonicalize().unwrap().join("archive");
            let archive =
                KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
            drop(archive);
            if malformed < 2 {
                let mut wal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
                while wal.replay_next().unwrap().is_some() {}
                if malformed == 0 {
                    wal.append(
                        &encode(&Record::Initialize {
                            lane: lane.clone(),
                            asset_incarnation: incarnation,
                        })
                        .unwrap(),
                    )
                    .unwrap();
                } else {
                    let (command, response) = frames(2, [8; 32], 9, 1);
                    let record = encode(&Record::Observe {
                        operation: 2,
                        request_id: [8; 32],
                        command,
                        response,
                        context: context.clone(),
                    })
                    .unwrap();
                    wal.append(&record).unwrap();
                    wal.append(&record).unwrap();
                }
            } else {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .append(true)
                    .open(path.join(FORMAT.filename))
                    .unwrap();
                if malformed == 2 {
                    file.set_len(0).unwrap();
                } else {
                    file.write_all(b"truncated").unwrap();
                }
                file.sync_all().unwrap();
            }
            assert!(matches!(
                KagemushaResponseEvidenceArchiveV1::open_existing(&path, &lane, incarnation),
                Err(Error::JournalCorrupt)
            ));
        }
    }

    #[test]
    fn file_replacement_invalidates_even_a_cached_exact_retry() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("archive");
        let (lane, incarnation) = owner();
        let context = context(&lane);
        let mut archive =
            KagemushaResponseEvidenceArchiveV1::create_new(&path, &lane, incarnation).unwrap();
        let (command, response) = frames(2, [8; 32], 9, 1);
        archive
            .retain_observed_response(2, [8; 32], &command, &response, &context)
            .unwrap();
        let displaced = path.join("displaced.wal");
        fs::rename(path.join(FORMAT.filename), &displaced).unwrap();
        fs::copy(&displaced, path.join(FORMAT.filename)).unwrap();
        assert_eq!(
            archive.retain_observed_response(2, [8; 32], &command, &response, &context),
            Err(Error::JournalCorrupt)
        );
        assert_eq!(archive.recovery_prefix(), Err(Error::DurabilityUncertain));
    }
}

#[cfg(test)]
#[test]
fn captured_state_frame_owners() {
    crate::zk::kagemusha_v1_state::state_frame_identity_tests::observed::<Record>(
        "iroha_core::zk::kagemusha_v1_state::response_evidence_archive::Record",
    );
}
