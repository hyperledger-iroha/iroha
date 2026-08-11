//! Crash-safe write-ahead log for Sumeragi v2 safety decisions.
//!
//! The log stores opaque, Norito-encoded records behind a small framed envelope.  A frame is
//! acknowledged only after the file has been flushed and synchronised, which lets the consensus
//! reducer order signing and view-change effects after durable state transitions.  The envelope is
//! hash chained so corruption before the final, incomplete crash tail fails closed. Recovery
//! verifies frames incrementally and fixed height-local record/payload ceilings are enforced both
//! while opening and before append I/O, keeping valid replay memory bounded.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use super::v2_core::{
    SAFETY_WAL_FILE_HEADER_LEN as FILE_HEADER_LEN, SAFETY_WAL_FRAME_HEADER_LEN as FRAME_HEADER_LEN,
    SAFETY_WAL_FRAME_MAGIC as FRAME_MAGIC, SAFETY_WAL_HASH_LEN as HASH_LEN,
    SAFETY_WAL_MAX_RECORD_BYTES as MAX_RECORD_BYTES, WalAppendError, WalAppendIo, WalAppendState,
    WalCodecError, WalFileIdentity, WalFrameCorruption, WalHeaderCorruption, WalIdentityField,
    WalIoStage, WalRetirementAuthorization, encode_wal_file_header, recover_wal_file,
};
#[cfg(test)]
use super::v2_core::{
    SAFETY_WAL_FILE_MAGIC as FILE_MAGIC, SAFETY_WAL_FORMAT_VERSION as FORMAT_VERSION,
};
use thiserror::Error;

#[cfg(test)]
const FILE_HEADER_PREFIX_LEN: usize = FILE_MAGIC.len() + 2 + 2 + HASH_LEN + HASH_LEN;

/// Maximum complete frames retained by one height-local safety WAL.
pub(crate) const SAFETY_WAL_MAX_RECORDS: usize = 8 * 1024;
/// Maximum combined payload bytes retained by one height-local safety WAL.
pub(crate) const SAFETY_WAL_MAX_TOTAL_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const SAFETY_WAL_RECOVERY_SCRATCH_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WalRetentionLimits {
    max_records: usize,
    max_payload_bytes: usize,
}

const WAL_RETENTION_LIMITS: WalRetentionLimits = WalRetentionLimits {
    max_records: SAFETY_WAL_MAX_RECORDS,
    max_payload_bytes: SAFETY_WAL_MAX_TOTAL_PAYLOAD_BYTES,
};

/// A record recovered from the safety WAL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredRecord {
    /// Monotonic record sequence starting at zero.
    pub sequence: u64,
    /// Opaque Norito payload bytes supplied by the consensus adapter.
    pub payload: Vec<u8>,
}

/// Errors raised while opening, replaying, or appending the safety WAL.
#[derive(Debug, Error)]
pub(crate) enum SafetyWalError {
    /// A filesystem operation failed.
    #[error("sumeragi safety WAL I/O failed at {path}: {source}")]
    Io {
        /// Path being accessed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The file header is missing or malformed.
    #[error("invalid sumeragi safety WAL header at {path}: {reason}")]
    InvalidHeader {
        /// WAL path.
        path: PathBuf,
        /// Validation failure.
        reason: &'static str,
    },
    /// The WAL belongs to another network, protocol, or consensus key.
    #[error("sumeragi safety WAL identity mismatch at {path}: {field}")]
    IdentityMismatch {
        /// WAL path.
        path: PathBuf,
        /// Mismatching header field.
        field: &'static str,
    },
    /// A complete frame is corrupt. Only an incomplete final frame may be discarded.
    #[error("corrupt sumeragi safety WAL frame {sequence} at {path}: {reason}")]
    CorruptFrame {
        /// WAL path.
        path: PathBuf,
        /// Expected frame sequence.
        sequence: u64,
        /// Validation failure.
        reason: &'static str,
    },
    /// A record exceeds the bounded safety-record size.
    #[error("sumeragi safety WAL record is too large: {actual} bytes (maximum {maximum})")]
    RecordTooLarge {
        /// Supplied record size.
        actual: usize,
        /// Maximum accepted record size.
        maximum: usize,
    },
    /// The height-local WAL exceeds its fixed record or aggregate-payload retention bound.
    #[error(
        "sumeragi safety WAL retention bound exceeded at {path}: {records} records/{payload_bytes} payload bytes (maximum {maximum_records} records/{maximum_payload_bytes} bytes)"
    )]
    RetentionLimitExceeded {
        /// WAL path.
        path: PathBuf,
        /// Record count that would cross the bound.
        records: usize,
        /// Aggregate payload bytes that would cross the bound.
        payload_bytes: usize,
        /// Maximum retained record count.
        maximum_records: usize,
        /// Maximum retained aggregate payload bytes.
        maximum_payload_bytes: usize,
    },
    /// The monotonic frame sequence is exhausted.
    #[error("sumeragi safety WAL frame sequence overflow at {path}")]
    SequenceOverflow {
        /// WAL path.
        path: PathBuf,
    },
    /// An ordered append I/O stage failed and poisoned this WAL instance.
    #[error("sumeragi safety WAL append {stage:?} failed at {path}: {source}")]
    AppendIo {
        /// WAL path.
        path: PathBuf,
        /// Exact failed stage.
        stage: WalIoStage,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// A failed append was retried without verified reopen and recovery.
    #[error("sumeragi safety WAL at {path} is failed closed and must be reopened")]
    FailedClosed {
        /// WAL path.
        path: PathBuf,
    },
}

/// Append-only, hash-chained Sumeragi safety WAL.
#[derive(Debug)]
pub(crate) struct SafetyWal {
    path: PathBuf,
    file: File,
    records: Vec<RecoveredRecord>,
    payload_bytes: usize,
    append_state: WalAppendState,
}

impl SafetyWal {
    /// Open or create a WAL bound to the supplied network, protocol, and consensus-key hashes.
    ///
    /// An incomplete final frame is treated as an unacknowledged crash tail and truncated. Any
    /// earlier structural or hash-chain failure, or a complete prefix beyond the fixed retention
    /// bounds, is returned as an error.
    pub(crate) fn open(
        path: impl Into<PathBuf>,
        protocol_version: u16,
        network_id: [u8; HASH_LEN],
        key_hash: [u8; HASH_LEN],
    ) -> Result<Self, SafetyWalError> {
        let path = path.into();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| SafetyWalError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let identity = WalFileIdentity::new(protocol_version, network_id, key_hash);
        let created = !path.exists();
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;

        if created
            || file
                .metadata()
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?
                .len()
                == 0
        {
            let header = encode_wal_file_header(identity, &frame_hash);
            file.write_all(&header)
                .and_then(|()| file.flush())
                .and_then(|()| file.sync_data())
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?;
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                sync_directory(parent).map_err(|source| SafetyWalError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
        }

        let recovery = recover_wal_stream(&mut file, &path, identity, WAL_RETENTION_LIMITS)?;
        if recovery.incomplete_tail {
            file.set_len(recovery.valid_prefix_len)
                .and_then(|()| file.sync_data())
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?;
        }
        file.seek(SeekFrom::End(0))
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;

        let append_state = WalAppendState::from_verified_stream_recovery(
            recovery.next_sequence,
            recovery.last_frame_hash,
        );
        Ok(Self {
            path,
            file,
            records: recovery.records,
            payload_bytes: recovery.payload_bytes,
            append_state,
        })
    }

    /// Return all records recovered during open.
    pub(crate) fn recovered_records(&self) -> &[RecoveredRecord] {
        &self.records
    }

    /// Append and synchronise an opaque Norito record.
    ///
    /// A successful return is the durability acknowledgement used by the reducer. On any error,
    /// callers must fail stop and reopen the WAL before attempting another consensus action.
    pub(crate) fn append(&mut self, payload: &[u8]) -> Result<u64, SafetyWalError> {
        self.append_with_limits(payload, WAL_RETENTION_LIMITS)
    }

    fn append_with_limits(
        &mut self,
        payload: &[u8],
        limits: WalRetentionLimits,
    ) -> Result<u64, SafetyWalError> {
        if payload.len() > MAX_RECORD_BYTES {
            return Err(SafetyWalError::RecordTooLarge {
                actual: payload.len(),
                maximum: MAX_RECORD_BYTES,
            });
        }
        let (_, next_payload_bytes) = enforce_retention_limits(
            &self.path,
            self.records.len(),
            self.payload_bytes,
            payload.len(),
            limits,
        )?;
        let mut io = FileAppendIo {
            file: &mut self.file,
        };
        let receipt = self
            .append_state
            .append(payload, &frame_hash, &mut io)
            .map_err(|error| map_append_error(&self.path, error))?;
        let sequence = receipt.sequence();
        self.records.push(RecoveredRecord {
            sequence,
            payload: payload.to_vec(),
        });
        self.payload_bytes = next_payload_bytes;
        Ok(sequence)
    }

    /// Retire a closed height's WAL after the caller has validated Kura's
    /// durable block-and-finality receipt.
    ///
    /// Consuming the log prevents any safety intent from being appended after
    /// retirement. `authorization` can only be derived after the reducer has
    /// compared the exact typed Kura receipt and consumed the finalized height.
    pub(crate) fn retire(
        self,
        _authorization: WalRetirementAuthorization,
    ) -> Result<(), SafetyWalError> {
        let Self { path, file, .. } = self;
        remove_wal_file(path, file)
    }
}

fn remove_wal_file(path: PathBuf, file: File) -> Result<(), SafetyWalError> {
    file.sync_all().map_err(|source| SafetyWalError::Io {
        path: path.clone(),
        source,
    })?;
    drop(file);
    fs::remove_file(&path).map_err(|source| SafetyWalError::Io {
        path: path.clone(),
        source,
    })?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        sync_directory(parent).map_err(|source| SafetyWalError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

struct StreamingWalRecovery {
    records: Vec<RecoveredRecord>,
    payload_bytes: usize,
    valid_prefix_len: u64,
    incomplete_tail: bool,
    next_sequence: u64,
    last_frame_hash: [u8; HASH_LEN],
}

fn enforce_retention_limits(
    path: &Path,
    current_records: usize,
    current_payload_bytes: usize,
    next_payload_bytes: usize,
    limits: WalRetentionLimits,
) -> Result<(usize, usize), SafetyWalError> {
    let records = current_records.checked_add(1).unwrap_or(usize::MAX);
    let payload_bytes = current_payload_bytes
        .checked_add(next_payload_bytes)
        .unwrap_or(usize::MAX);
    if records > limits.max_records || payload_bytes > limits.max_payload_bytes {
        return Err(SafetyWalError::RetentionLimitExceeded {
            path: path.to_path_buf(),
            records,
            payload_bytes,
            maximum_records: limits.max_records,
            maximum_payload_bytes: limits.max_payload_bytes,
        });
    }
    Ok((records, payload_bytes))
}

#[allow(clippy::too_many_lines)]
fn recover_wal_stream(
    file: &mut File,
    path: &Path,
    identity: WalFileIdentity,
    limits: WalRetentionLimits,
) -> Result<StreamingWalRecovery, SafetyWalError> {
    let file_len = file
        .metadata()
        .map_err(|source| SafetyWalError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    file.seek(SeekFrom::Start(0))
        .map_err(|source| SafetyWalError::Io {
            path: path.to_path_buf(),
            source,
        })?;

    let mut file_header = [0_u8; FILE_HEADER_LEN];
    let header_len = read_up_to(file, &mut file_header).map_err(|source| SafetyWalError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if header_len < FILE_HEADER_LEN {
        return Err(SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: "truncated header",
        });
    }
    recover_wal_file(&file_header, identity, &frame_hash)
        .map_err(|error| map_codec_error(path, error))?;

    let mut records = Vec::new();
    let mut payload_bytes = 0_usize;
    let mut valid_prefix_len = u64::try_from(FILE_HEADER_LEN).expect("WAL header length fits u64");
    let mut incomplete_tail = false;
    let mut expected_sequence = 0_u64;
    let mut previous_hash = [0_u8; HASH_LEN];

    while valid_prefix_len < file_len {
        let frame_start = valid_prefix_len;
        let mut frame_header = [0_u8; FRAME_HEADER_LEN];
        let frame_header_len =
            read_up_to(file, &mut frame_header).map_err(|source| SafetyWalError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        if frame_header_len < FRAME_HEADER_LEN {
            incomplete_tail = true;
            break;
        }

        let mut offset = 0_usize;
        if frame_header[offset..offset + FRAME_MAGIC.len()] != FRAME_MAGIC {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence: expected_sequence,
                    reason: WalFrameCorruption::Magic,
                },
            ));
        }
        offset += FRAME_MAGIC.len();
        let sequence = u64::from_le_bytes(
            frame_header[offset..offset + 8]
                .try_into()
                .expect("fixed-width WAL sequence"),
        );
        offset += 8;
        let payload_len = usize::try_from(u32::from_le_bytes(
            frame_header[offset..offset + 4]
                .try_into()
                .expect("fixed-width WAL payload length"),
        ))
        .unwrap_or(usize::MAX);
        offset += 4;
        let mut encoded_previous = [0_u8; HASH_LEN];
        encoded_previous.copy_from_slice(&frame_header[offset..offset + HASH_LEN]);

        if sequence != expected_sequence {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence: expected_sequence,
                    reason: WalFrameCorruption::Sequence,
                },
            ));
        }
        if payload_len > MAX_RECORD_BYTES {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::RecordLength,
                },
            ));
        }
        if encoded_previous != previous_hash {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::PreviousHash,
                },
            ));
        }

        let frame_len = FRAME_HEADER_LEN
            .checked_add(payload_len)
            .and_then(|length| length.checked_add(HASH_LEN))
            .and_then(|length| u64::try_from(length).ok())
            .ok_or_else(|| {
                map_codec_error(
                    path,
                    WalCodecError::CorruptFrame {
                        sequence,
                        reason: WalFrameCorruption::RecordLength,
                    },
                )
            })?;
        if file_len.saturating_sub(frame_start) < frame_len {
            incomplete_tail = true;
            break;
        }

        let retention =
            enforce_retention_limits(path, records.len(), payload_bytes, payload_len, limits);
        let mut hasher = blake3::Hasher::new();
        hasher.update(&frame_header);
        let payload = if retention.is_ok() {
            let mut payload = vec![0_u8; payload_len];
            file.read_exact(&mut payload)
                .map_err(|source| SafetyWalError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            hasher.update(&payload);
            Some(payload)
        } else {
            let mut scratch = vec![0_u8; SAFETY_WAL_RECOVERY_SCRATCH_BYTES];
            let mut remaining = payload_len;
            while remaining > 0 {
                let chunk_len = remaining.min(scratch.len());
                let chunk = &mut scratch[..chunk_len];
                file.read_exact(chunk)
                    .map_err(|source| SafetyWalError::Io {
                        path: path.to_path_buf(),
                        source,
                    })?;
                hasher.update(chunk);
                remaining -= chunk_len;
            }
            None
        };
        let mut encoded_hash = [0_u8; HASH_LEN];
        file.read_exact(&mut encoded_hash)
            .map_err(|source| SafetyWalError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let calculated_hash = *hasher.finalize().as_bytes();
        if encoded_hash != calculated_hash {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::Checksum,
                },
            ));
        }
        if expected_sequence == u64::MAX {
            return Err(map_codec_error(path, WalCodecError::SequenceOverflow));
        }
        let (_, next_payload_total) = retention?;
        records.push(RecoveredRecord {
            sequence,
            payload: payload.expect("payload is retained when its retention check succeeds"),
        });
        payload_bytes = next_payload_total;
        previous_hash = encoded_hash;
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| map_codec_error(path, WalCodecError::SequenceOverflow))?;
        valid_prefix_len = frame_start.checked_add(frame_len).ok_or_else(|| {
            map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::RecordLength,
                },
            )
        })?;
    }

    Ok(StreamingWalRecovery {
        records,
        payload_bytes,
        valid_prefix_len,
        incomplete_tail,
        next_sequence: expected_sequence,
        last_frame_hash: previous_hash,
    })
}

fn read_up_to(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut read = 0_usize;
    while read < buffer.len() {
        match reader.read(&mut buffer[read..]) {
            Ok(0) => break,
            Ok(count) => read += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(read)
}

struct FileAppendIo<'a> {
    file: &'a mut File,
}

impl WalAppendIo for FileAppendIo<'_> {
    type Error = io::Error;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.file.write_all(bytes)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.file.flush()
    }

    fn sync_data(&mut self) -> Result<(), Self::Error> {
        self.file.sync_data()
    }
}

fn map_append_error(path: &Path, error: WalAppendError<io::Error>) -> SafetyWalError {
    match error {
        WalAppendError::Codec(error) => map_codec_error(path, error),
        WalAppendError::Io { stage, source } => SafetyWalError::AppendIo {
            path: path.to_path_buf(),
            stage,
            source,
        },
        WalAppendError::FailedClosed => SafetyWalError::FailedClosed {
            path: path.to_path_buf(),
        },
    }
}

fn map_codec_error(path: &Path, error: WalCodecError) -> SafetyWalError {
    match error {
        WalCodecError::InvalidHeader(reason) => SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: match reason {
                WalHeaderCorruption::Truncated => "truncated header",
                WalHeaderCorruption::Magic => "magic mismatch",
                WalHeaderCorruption::FormatVersion => "unsupported format version",
                WalHeaderCorruption::Checksum => "checksum mismatch",
            },
        },
        WalCodecError::IdentityMismatch(field) => SafetyWalError::IdentityMismatch {
            path: path.to_path_buf(),
            field: match field {
                WalIdentityField::ProtocolVersion => "protocol version",
                WalIdentityField::NetworkId => "network id",
                WalIdentityField::ConsensusKeyHash => "consensus key hash",
            },
        },
        WalCodecError::CorruptFrame { sequence, reason } => SafetyWalError::CorruptFrame {
            path: path.to_path_buf(),
            sequence,
            reason: match reason {
                WalFrameCorruption::Magic => "frame magic mismatch",
                WalFrameCorruption::Sequence => "non-monotonic sequence",
                WalFrameCorruption::RecordLength => "record length exceeds safety bound",
                WalFrameCorruption::PreviousHash => "previous-frame hash mismatch",
                WalFrameCorruption::Checksum => "frame checksum mismatch",
            },
        },
        WalCodecError::RecordTooLarge { actual, maximum } => {
            SafetyWalError::RecordTooLarge { actual, maximum }
        }
        WalCodecError::SequenceOverflow => SafetyWalError::SequenceOverflow {
            path: path.to_path_buf(),
        },
    }
}

fn frame_hash(bytes: &[u8]) -> [u8; HASH_LEN] {
    *blake3::hash(bytes).as_bytes()
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETWORK_ID: [u8; HASH_LEN] = [0x11; HASH_LEN];
    const KEY: [u8; HASH_LEN] = [0x22; HASH_LEN];
    const PROTOCOL: u16 = iroha_data_model::block::consensus_v2::PROTOCOL_VERSION;

    fn read_test_u16(bytes: &[u8]) -> u16 {
        u16::from_le_bytes(bytes.try_into().expect("two-byte fixture field"))
    }

    #[test]
    fn file_header_uses_the_declared_canonical_layout() {
        let header =
            encode_wal_file_header(WalFileIdentity::new(PROTOCOL, NETWORK_ID, KEY), &frame_hash);
        let format_offset = FILE_MAGIC.len();
        let protocol_offset = format_offset + 2;
        let network_id_offset = protocol_offset + 2;
        let key_offset = network_id_offset + HASH_LEN;

        assert_eq!(&header[..FILE_MAGIC.len()], &FILE_MAGIC);
        assert_eq!(
            read_test_u16(&header[format_offset..protocol_offset]),
            FORMAT_VERSION
        );
        assert_eq!(
            read_test_u16(&header[protocol_offset..network_id_offset]),
            PROTOCOL
        );
        assert_eq!(&header[network_id_offset..key_offset], &NETWORK_ID);
        assert_eq!(&header[key_offset..FILE_HEADER_PREFIX_LEN], &KEY);
        assert_eq!(
            &header[FILE_HEADER_PREFIX_LEN..],
            &frame_hash(&header[..FILE_HEADER_PREFIX_LEN])
        );
    }

    #[test]
    fn append_reopens_and_replays_hash_chained_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            assert_eq!(wal.append(b"prepare").expect("append Prepare"), 0);
            assert_eq!(wal.append(b"commit").expect("append Commit"), 1);
        }

        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("reopen WAL");
        assert_eq!(
            wal.recovered_records(),
            [
                RecoveredRecord {
                    sequence: 0,
                    payload: b"prepare".to_vec(),
                },
                RecoveredRecord {
                    sequence: 1,
                    payload: b"commit".to_vec(),
                },
            ]
        );
        assert_eq!(wal.payload_bytes, b"prepare".len() + b"commit".len());
    }

    #[test]
    fn append_retention_limits_fail_before_file_or_hash_state_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let limits = WalRetentionLimits {
            max_records: 2,
            max_payload_bytes: 5,
        };
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        wal.append_with_limits(b"ab", limits)
            .expect("append first bounded record");
        wal.append_with_limits(b"cde", limits)
            .expect("append exact payload boundary");
        let file_len = wal.file.metadata().expect("WAL metadata").len();
        let append_state = wal.append_state;

        assert!(matches!(
            wal.append_with_limits(b"", limits),
            Err(SafetyWalError::RetentionLimitExceeded {
                records: 3,
                payload_bytes: 5,
                maximum_records: 2,
                maximum_payload_bytes: 5,
                ..
            })
        ));
        assert_eq!(wal.file.metadata().expect("WAL metadata").len(), file_len);
        assert_eq!(wal.append_state, append_state);
        assert_eq!(wal.recovered_records().len(), 2);
        assert_eq!(wal.payload_bytes, 5);

        let payload_path = dir.path().join("sumeragi-v2-payload.wal");
        let mut payload_wal =
            SafetyWal::open(&payload_path, PROTOCOL, NETWORK_ID, KEY).expect("open payload WAL");
        payload_wal
            .append_with_limits(b"12345", limits)
            .expect("append exact aggregate payload boundary");
        let payload_file_len = payload_wal
            .file
            .metadata()
            .expect("payload WAL metadata")
            .len();
        let payload_append_state = payload_wal.append_state;
        assert!(matches!(
            payload_wal.append_with_limits(b"x", limits),
            Err(SafetyWalError::RetentionLimitExceeded {
                records: 2,
                payload_bytes: 6,
                maximum_records: 2,
                maximum_payload_bytes: 5,
                ..
            })
        ));
        assert_eq!(
            payload_wal
                .file
                .metadata()
                .expect("payload WAL metadata")
                .len(),
            payload_file_len
        );
        assert_eq!(payload_wal.append_state, payload_append_state);
        assert_eq!(payload_wal.recovered_records().len(), 1);
        assert_eq!(payload_wal.payload_bytes, 5);
    }

    #[test]
    fn streaming_recovery_accepts_the_boundary_and_rejects_the_next_frame() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            wal.append(b"one").expect("append one");
            wal.append(b"two").expect("append two");
            wal.append(b"three").expect("append three");
        }

        let identity = WalFileIdentity::new(PROTOCOL, NETWORK_ID, KEY);
        let mut file = File::open(&path).expect("open WAL for streaming recovery");
        let exact = recover_wal_stream(
            &mut file,
            &path,
            identity,
            WalRetentionLimits {
                max_records: 3,
                max_payload_bytes: 11,
            },
        )
        .expect("recover exact record and payload boundary");
        assert_eq!(exact.records.len(), 3);
        assert_eq!(exact.payload_bytes, 11);
        assert_eq!(exact.next_sequence, 3);

        let error = match recover_wal_stream(
            &mut file,
            &path,
            identity,
            WalRetentionLimits {
                max_records: 2,
                max_payload_bytes: usize::MAX,
            },
        ) {
            Ok(_) => panic!("the first complete frame above the retention bound must fail closed"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            SafetyWalError::RetentionLimitExceeded {
                records: 3,
                payload_bytes: 11,
                maximum_records: 2,
                maximum_payload_bytes: usize::MAX,
                ..
            }
        ));
    }

    #[test]
    fn incomplete_final_frame_is_truncated_as_unacknowledged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            wal.append(b"durable").expect("append durable record");
        }
        let good_len = fs::metadata(&path).expect("metadata").len();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open append")
            .write_all(b"S2FR\x01\x00")
            .expect("write crash tail");

        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("recover WAL");
        assert_eq!(wal.recovered_records().len(), 1);
        assert_eq!(fs::metadata(path).expect("metadata").len(), good_len);
    }

    #[test]
    fn incomplete_final_payload_and_checksum_are_discarded_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let good_len;
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            wal.append(b"durable decision")
                .expect("append acknowledged decision");
            good_len = wal.file.metadata().expect("metadata").len();
            wal.append(b"unacknowledged next intent")
                .expect("append frame used to model partial write");
        }
        let partial_len = good_len
            .checked_add(u64::try_from(FRAME_HEADER_LEN + 3).expect("partial length"))
            .expect("test file length");
        OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open WAL for truncation")
            .set_len(partial_len)
            .expect("truncate in final payload");

        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("recover WAL");
        assert_eq!(
            wal.recovered_records(),
            [RecoveredRecord {
                sequence: 0,
                payload: b"durable decision".to_vec(),
            }]
        );
        assert_eq!(fs::metadata(path).expect("metadata").len(), good_len);
    }

    #[test]
    fn complete_corrupt_frame_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            wal.append(b"prepare").expect("append record");
        }
        let mut bytes = fs::read(&path).expect("read WAL");
        let payload_offset = FILE_HEADER_LEN + FRAME_HEADER_LEN;
        bytes[payload_offset] ^= 0x80;
        fs::write(&path, bytes).expect("corrupt WAL");

        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY),
            Err(SafetyWalError::CorruptFrame { .. })
        ));
    }

    #[test]
    fn complete_hash_chain_break_after_valid_record_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let first_payload_len = b"prepare".len();
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            wal.append(b"prepare").expect("append first record");
            wal.append(b"decision").expect("append second record");
        }
        let second_frame = FILE_HEADER_LEN + FRAME_HEADER_LEN + first_payload_len + HASH_LEN;
        let previous_hash_offset = second_frame + FRAME_MAGIC.len() + 8 + 4;
        let mut bytes = fs::read(&path).expect("read WAL");
        bytes[previous_hash_offset] ^= 0x80;
        fs::write(&path, bytes).expect("break hash chain");

        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY),
            Err(SafetyWalError::CorruptFrame {
                sequence: 1,
                reason: "previous-frame hash mismatch",
                ..
            })
        ));
    }

    #[test]
    fn identity_mismatch_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        drop(SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL"));

        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, [0x33; HASH_LEN]),
            Err(SafetyWalError::IdentityMismatch {
                field: "consensus key hash",
                ..
            })
        ));
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL.saturating_add(1), NETWORK_ID, KEY),
            Err(SafetyWalError::IdentityMismatch {
                field: "protocol version",
                ..
            })
        ));
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, [0x44; HASH_LEN], KEY),
            Err(SafetyWalError::IdentityMismatch {
                field: "network id",
                ..
            })
        ));
    }

    #[test]
    fn append_io_failure_poisoning_requires_verified_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let read_only = File::open(&path).expect("open read-only WAL handle");
        let writable = std::mem::replace(&mut wal.file, read_only);
        drop(writable);

        assert!(matches!(
            wal.append(b"must fail before acknowledgement"),
            Err(SafetyWalError::AppendIo {
                stage: WalIoStage::Write,
                ..
            })
        ));
        assert!(wal.append_state.is_failed_closed());
        assert!(matches!(
            wal.append(b"retry is forbidden"),
            Err(SafetyWalError::FailedClosed { .. })
        ));
        assert!(wal.recovered_records().is_empty());

        drop(wal);
        let reopened = SafetyWal::open(path, PROTOCOL, NETWORK_ID, KEY).expect("verified reopen");
        assert!(reopened.recovered_records().is_empty());
        assert!(!reopened.append_state.is_failed_closed());
    }

    #[test]
    fn physical_retirement_removes_and_directory_syncs_a_closed_height_log() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        wal.append(b"decision").expect("append decision");
        let SafetyWal { path, file, .. } = wal;
        let retired_path = path.clone();
        remove_wal_file(path, file).expect("retire finalized WAL bytes");
        assert!(!retired_path.exists());
    }
}
