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

use thiserror::Error;

const FILE_MAGIC: [u8; 8] = *b"SUMV2WAL";
const FRAME_MAGIC: [u8; 4] = *b"S2FR";
const FORMAT_VERSION: u16 = 1;
const HASH_LEN: usize = 32;
const FILE_HEADER_PREFIX_LEN: usize = FILE_MAGIC.len() + 2 + 2 + HASH_LEN + HASH_LEN;
const FILE_HEADER_LEN: usize = FILE_HEADER_PREFIX_LEN + HASH_LEN;
const FRAME_HEADER_LEN: usize = FRAME_MAGIC.len() + 8 + 4 + HASH_LEN;
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

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
    /// The WAL belongs to another chain, protocol, or consensus key.
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
}

/// Append-only, hash-chained Sumeragi safety WAL.
#[derive(Debug)]
pub(crate) struct SafetyWal {
    path: PathBuf,
    file: File,
    records: Vec<RecoveredRecord>,
    next_sequence: u64,
    last_frame_hash: [u8; HASH_LEN],
}

impl SafetyWal {
    /// Open or create a WAL bound to the supplied chain, protocol, and consensus-key hashes.
    ///
    /// An incomplete final frame is treated as an unacknowledged crash tail and truncated. Any
    /// earlier structural or hash-chain failure is returned as an error.
    pub(crate) fn open(
        path: impl Into<PathBuf>,
        protocol_version: u16,
        chain_hash: [u8; HASH_LEN],
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
            let header = encode_file_header(protocol_version, chain_hash, key_hash);
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
        validate_file_header(&path, &bytes, protocol_version, chain_hash, key_hash)?;

        let mut offset = FILE_HEADER_LEN;
        let mut expected_sequence = 0_u64;
        let mut previous_hash = [0_u8; HASH_LEN];
        let mut records = Vec::new();
        let mut truncated_tail = false;

        while offset < bytes.len() {
            if bytes.len().saturating_sub(offset) < FRAME_HEADER_LEN {
                truncated_tail = true;
                break;
            }
            let frame_start = offset;
            if bytes[offset..offset + FRAME_MAGIC.len()] != FRAME_MAGIC {
                return Err(SafetyWalError::CorruptFrame {
                    path,
                    sequence: expected_sequence,
                    reason: "frame magic mismatch",
                });
            }
            offset += FRAME_MAGIC.len();
            let sequence = read_u64(&bytes[offset..offset + 8]);
            offset += 8;
            let payload_len =
                usize::try_from(read_u32(&bytes[offset..offset + 4])).unwrap_or(usize::MAX);
            offset += 4;
            let mut encoded_previous = [0_u8; HASH_LEN];
            encoded_previous.copy_from_slice(&bytes[offset..offset + HASH_LEN]);
            offset += HASH_LEN;

            if sequence != expected_sequence {
                return Err(SafetyWalError::CorruptFrame {
                    path,
                    sequence: expected_sequence,
                    reason: "non-monotonic sequence",
                });
            }
            if payload_len > MAX_RECORD_BYTES {
                return Err(SafetyWalError::CorruptFrame {
                    path,
                    sequence,
                    reason: "record length exceeds safety bound",
                });
            }
            if encoded_previous != previous_hash {
                return Err(SafetyWalError::CorruptFrame {
                    path,
                    sequence,
                    reason: "previous-frame hash mismatch",
                });
            }

            let frame_len = FRAME_HEADER_LEN
                .saturating_add(payload_len)
                .saturating_add(HASH_LEN);
            if bytes.len().saturating_sub(frame_start) < frame_len {
                truncated_tail = true;
                offset = frame_start;
                break;
            }
            let payload_end = offset + payload_len;
            let payload = bytes[offset..payload_end].to_vec();
            let mut encoded_hash = [0_u8; HASH_LEN];
            encoded_hash.copy_from_slice(&bytes[payload_end..payload_end + HASH_LEN]);
            let calculated_hash = frame_hash(&bytes[frame_start..payload_end]);
            if encoded_hash != calculated_hash {
                return Err(SafetyWalError::CorruptFrame {
                    path,
                    sequence,
                    reason: "frame checksum mismatch",
                });
            }

            records.push(RecoveredRecord { sequence, payload });
            previous_hash = encoded_hash;
            expected_sequence = expected_sequence
                .checked_add(1)
                .ok_or_else(|| SafetyWalError::SequenceOverflow { path: path.clone() })?;
            offset = payload_end + HASH_LEN;
        }

        if truncated_tail {
            file.set_len(u64::try_from(offset).unwrap_or(u64::MAX))
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
            records,
            next_sequence: expected_sequence,
            last_frame_hash: previous_hash,
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
        if payload.len() > MAX_RECORD_BYTES {
            return Err(SafetyWalError::RecordTooLarge {
                actual: payload.len(),
                maximum: MAX_RECORD_BYTES,
            });
        }
        let payload_len =
            u32::try_from(payload.len()).map_err(|_| SafetyWalError::RecordTooLarge {
                actual: payload.len(),
                maximum: MAX_RECORD_BYTES,
            })?;
        let sequence = self.next_sequence;
        let next_sequence =
            sequence
                .checked_add(1)
                .ok_or_else(|| SafetyWalError::SequenceOverflow {
                    path: self.path.clone(),
                })?;
        let mut frame = Vec::with_capacity(FRAME_HEADER_LEN + payload.len() + HASH_LEN);
        frame.extend_from_slice(&FRAME_MAGIC);
        frame.extend_from_slice(&sequence.to_le_bytes());
        frame.extend_from_slice(&payload_len.to_le_bytes());
        frame.extend_from_slice(&self.last_frame_hash);
        frame.extend_from_slice(payload);
        let hash = frame_hash(&frame);
        frame.extend_from_slice(&hash);

        self.file
            .write_all(&frame)
            .and_then(|()| self.file.flush())
            .and_then(|()| self.file.sync_data())
            .map_err(|source| SafetyWalError::Io {
                path: self.path.clone(),
                source,
            })?;

        self.records.push(RecoveredRecord {
            sequence,
            payload: payload.to_vec(),
        });
        self.next_sequence = next_sequence;
        self.last_frame_hash = hash;
        Ok(sequence)
    }

    /// Retire a closed height's WAL after the caller has validated Kura's
    /// durable block-and-finality receipt.
    ///
    /// Consuming the log prevents any safety intent from being appended after
    /// retirement. The production consensus adapter is responsible for
    /// comparing the typed Kura receipt before crossing this boundary.
    pub(crate) fn retire(self) -> Result<(), SafetyWalError> {
        let Self { path, file, .. } = self;
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

    /// Clear acknowledged records after the matching block and certificate are durable in Kura.
    #[cfg(test)]
    fn clear_after_commit(&mut self) -> Result<(), SafetyWalError> {
        self.file
            .set_len(u64::try_from(FILE_HEADER_LEN).unwrap_or(u64::MAX))
            .and_then(|()| {
                self.file
                    .seek(SeekFrom::Start(FILE_HEADER_LEN as u64))
                    .map(drop)
            })
            .and_then(|()| self.file.sync_data())
            .map_err(|source| SafetyWalError::Io {
                path: self.path.clone(),
                source,
            })?;
        self.records.clear();
        self.next_sequence = 0;
        self.last_frame_hash = [0; HASH_LEN];
        Ok(())
    }
}

fn encode_file_header(
    protocol_version: u16,
    chain_hash: [u8; HASH_LEN],
    key_hash: [u8; HASH_LEN],
) -> [u8; FILE_HEADER_LEN] {
    let mut header = [0_u8; FILE_HEADER_LEN];
    let mut offset = 0;
    header[offset..offset + FILE_MAGIC.len()].copy_from_slice(&FILE_MAGIC);
    offset += FILE_MAGIC.len();
    header[offset..offset + 2].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    offset += 2;
    header[offset..offset + 2].copy_from_slice(&protocol_version.to_le_bytes());
    offset += 2;
    header[offset..offset + HASH_LEN].copy_from_slice(&chain_hash);
    offset += HASH_LEN;
    header[offset..offset + HASH_LEN].copy_from_slice(&key_hash);
    let checksum = frame_hash(&header[..FILE_HEADER_PREFIX_LEN]);
    header[FILE_HEADER_PREFIX_LEN..].copy_from_slice(&checksum);
    header
}

fn validate_file_header(
    path: &Path,
    bytes: &[u8],
    protocol_version: u16,
    chain_hash: [u8; HASH_LEN],
    key_hash: [u8; HASH_LEN],
) -> Result<(), SafetyWalError> {
    if bytes.len() < FILE_HEADER_LEN {
        return Err(SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: "truncated header",
        });
    }
    if bytes[..FILE_MAGIC.len()] != FILE_MAGIC {
        return Err(SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: "magic mismatch",
        });
    }
    let mut offset = FILE_MAGIC.len();
    if read_u16(&bytes[offset..offset + 2]) != FORMAT_VERSION {
        return Err(SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: "unsupported format version",
        });
    }
    offset += 2;
    if read_u16(&bytes[offset..offset + 2]) != protocol_version {
        return Err(SafetyWalError::IdentityMismatch {
            path: path.to_path_buf(),
            field: "protocol version",
        });
    }
    offset += 2;
    if bytes[offset..offset + HASH_LEN] != chain_hash {
        return Err(SafetyWalError::IdentityMismatch {
            path: path.to_path_buf(),
            field: "chain hash",
        });
    }
    offset += HASH_LEN;
    if bytes[offset..offset + HASH_LEN] != key_hash {
        return Err(SafetyWalError::IdentityMismatch {
            path: path.to_path_buf(),
            field: "consensus key hash",
        });
    }
    let expected = frame_hash(&bytes[..FILE_HEADER_PREFIX_LEN]);
    if bytes[FILE_HEADER_PREFIX_LEN..FILE_HEADER_LEN] != expected {
        return Err(SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: "checksum mismatch",
        });
    }
    Ok(())
}

fn frame_hash(bytes: &[u8]) -> [u8; HASH_LEN] {
    *blake3::hash(bytes).as_bytes()
}

fn read_u16(bytes: &[u8]) -> u16 {
    let mut array = [0_u8; 2];
    array.copy_from_slice(bytes);
    u16::from_le_bytes(array)
}

fn read_u32(bytes: &[u8]) -> u32 {
    let mut array = [0_u8; 4];
    array.copy_from_slice(bytes);
    u32::from_le_bytes(array)
}

fn read_u64(bytes: &[u8]) -> u64 {
    let mut array = [0_u8; 8];
    array.copy_from_slice(bytes);
    u64::from_le_bytes(array)
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAIN: [u8; HASH_LEN] = [0x11; HASH_LEN];
    const KEY: [u8; HASH_LEN] = [0x22; HASH_LEN];

    #[test]
    fn file_header_uses_the_declared_canonical_layout() {
        let header = encode_file_header(2, CHAIN, KEY);
        let format_offset = FILE_MAGIC.len();
        let protocol_offset = format_offset + 2;
        let chain_offset = protocol_offset + 2;
        let key_offset = chain_offset + HASH_LEN;

        assert_eq!(&header[..FILE_MAGIC.len()], &FILE_MAGIC);
        assert_eq!(
            read_u16(&header[format_offset..protocol_offset]),
            FORMAT_VERSION
        );
        assert_eq!(read_u16(&header[protocol_offset..chain_offset]), 2);
        assert_eq!(&header[chain_offset..key_offset], &CHAIN);
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
            let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
            assert_eq!(wal.append(b"prepare").expect("append Prepare"), 0);
            assert_eq!(wal.append(b"commit").expect("append Commit"), 1);
        }

        let wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("reopen WAL");
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
            let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
            wal.append(b"durable").expect("append durable record");
        }
        let good_len = fs::metadata(&path).expect("metadata").len();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open append")
            .write_all(b"S2FR\x01\x00")
            .expect("write crash tail");

        let wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("recover WAL");
        assert_eq!(wal.recovered_records().len(), 1);
        assert_eq!(fs::metadata(path).expect("metadata").len(), good_len);
    }

    #[test]
    fn incomplete_final_payload_and_checksum_are_discarded_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let good_len;
        {
            let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
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

        let wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("recover WAL");
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
            let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
            wal.append(b"prepare").expect("append record");
        }
        let mut bytes = fs::read(&path).expect("read WAL");
        let payload_offset = FILE_HEADER_LEN + FRAME_HEADER_LEN;
        bytes[payload_offset] ^= 0x80;
        fs::write(&path, bytes).expect("corrupt WAL");

        assert!(matches!(
            SafetyWal::open(&path, 2, CHAIN, KEY),
            Err(SafetyWalError::CorruptFrame { .. })
        ));
    }

    #[test]
    fn complete_hash_chain_break_after_valid_record_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let first_payload_len = b"prepare".len();
        {
            let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
            wal.append(b"prepare").expect("append first record");
            wal.append(b"decision").expect("append second record");
        }
        let second_frame = FILE_HEADER_LEN + FRAME_HEADER_LEN + first_payload_len + HASH_LEN;
        let previous_hash_offset = second_frame + FRAME_MAGIC.len() + 8 + 4;
        let mut bytes = fs::read(&path).expect("read WAL");
        bytes[previous_hash_offset] ^= 0x80;
        fs::write(&path, bytes).expect("break hash chain");

        assert!(matches!(
            SafetyWal::open(&path, 2, CHAIN, KEY),
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
        drop(SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL"));

        assert!(matches!(
            SafetyWal::open(&path, 2, CHAIN, [0x33; HASH_LEN]),
            Err(SafetyWalError::IdentityMismatch {
                field: "consensus key hash",
                ..
            })
        ));
        assert!(matches!(
            SafetyWal::open(&path, 3, CHAIN, KEY),
            Err(SafetyWalError::IdentityMismatch {
                field: "protocol version",
                ..
            })
        ));
        assert!(matches!(
            SafetyWal::open(&path, 2, [0x44; HASH_LEN], KEY),
            Err(SafetyWalError::IdentityMismatch {
                field: "chain hash",
                ..
            })
        ));
    }

    #[test]
    fn clear_after_commit_keeps_header_and_resets_sequence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
        wal.append(b"decision").expect("append decision");
        wal.clear_after_commit().expect("clear committed state");
        assert!(wal.recovered_records().is_empty());
        assert_eq!(wal.append(b"next-height").expect("append next height"), 0);

        let reopened = SafetyWal::open(path, 2, CHAIN, KEY).expect("reopen WAL");
        assert_eq!(reopened.recovered_records().len(), 1);
        assert_eq!(reopened.recovered_records()[0].sequence, 0);
    }

    #[test]
    fn sequence_overflow_fails_before_writing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
        let original_len = wal.file.metadata().expect("metadata").len();
        wal.next_sequence = u64::MAX;

        assert!(matches!(
            wal.append(b"must not be written"),
            Err(SafetyWalError::SequenceOverflow { .. })
        ));
        assert_eq!(wal.file.metadata().expect("metadata").len(), original_len);
        assert!(wal.recovered_records().is_empty());
    }

    #[test]
    fn retirement_removes_a_closed_height_log() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, 2, CHAIN, KEY).expect("open WAL");
        wal.append(b"decision").expect("append decision");
        wal.retire().expect("retire finalized WAL");
        assert!(!path.exists());
    }
}
