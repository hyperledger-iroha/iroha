//! Crash-safe write-ahead log for Sumeragi v2 safety decisions.
//!
//! The log stores opaque, Norito-encoded records behind a small framed envelope.  A frame is
//! acknowledged only after the file has been flushed and synchronised, which lets the consensus
//! reducer order signing and view-change effects after durable state transitions.  The envelope is
//! hash chained so corruption before the final, incomplete crash tail fails closed.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[cfg(test)]
use super::v2_core::{
    SAFETY_WAL_FILE_HEADER_LEN as FILE_HEADER_LEN, SAFETY_WAL_FILE_MAGIC as FILE_MAGIC,
    SAFETY_WAL_FORMAT_VERSION as FORMAT_VERSION, SAFETY_WAL_FRAME_HEADER_LEN as FRAME_HEADER_LEN,
    SAFETY_WAL_FRAME_MAGIC as FRAME_MAGIC,
};
use super::v2_core::{
    SAFETY_WAL_HASH_LEN as HASH_LEN, WalAppendError, WalAppendIo, WalAppendState, WalCodecError,
    WalFileIdentity, WalFrameCorruption, WalHeaderCorruption, WalIdentityField, WalIoStage,
    WalRetirementAuthorization, encode_wal_file_header, recover_wal_file,
};
use thiserror::Error;

#[cfg(test)]
const FILE_HEADER_PREFIX_LEN: usize = FILE_MAGIC.len() + 2 + 2 + HASH_LEN + HASH_LEN;

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
    append_state: WalAppendState,
}

impl SafetyWal {
    /// Open or create a WAL bound to the supplied network, protocol, and consensus-key hashes.
    ///
    /// An incomplete final frame is treated as an unacknowledged crash tail and truncated. Any
    /// earlier structural or hash-chain failure is returned as an error.
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

        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.read_to_end(&mut bytes))
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
        let recovery = recover_wal_file(&bytes, identity, &frame_hash)
            .map_err(|error| map_codec_error(&path, error))?;
        if recovery.has_incomplete_tail() {
            file.set_len(u64::try_from(recovery.valid_prefix_len()).unwrap_or(u64::MAX))
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

        Ok(Self {
            path,
            file,
            records: recovery
                .records()
                .iter()
                .map(|record| RecoveredRecord {
                    sequence: record.sequence(),
                    payload: record.payload().to_vec(),
                })
                .collect(),
            append_state: WalAppendState::from_recovery(&recovery),
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
