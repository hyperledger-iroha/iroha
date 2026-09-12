//! Durable prepare/commit/replay ownership for independent hardware journal operations.
//!
//! The device performs the atomic transaction and owns its retry index. This host WAL retains
//! exact public intent before dispatch and exact authenticated certificate before exposure.
//! Restoring the complete wallet still requires the device's freshly selected checkpoint.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use norito::{Decode, Encode};

use super::private_journal::{PrivateJournal, PrivateJournalFormat};
use crate::zk::kagemusha_v1_recursion::{
    KAGEMUSHA_HARDWARE_TRANSACTION_MAX_BYTES_V1, KagemushaHardwareTransactionV1,
    KagemushaHardwareTransactionVerifierV1,
};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "hardware-transactions.norito.wal",
    magic: b"IKGHTW1\0",
    hash_domain: b"iroha:kagemusha:v1:hardware-transaction-journal\0",
    maximum_payload_bytes: (KAGEMUSHA_HARDWARE_TRANSACTION_MAX_BYTES_V1 * 2) as u64,
};

/// Untrusted I/O to the governed applet's atomic journal operation.
/// Implementations never receive a signing key and cannot bypass certificate verification.
pub trait KagemushaHardwareTransactionTransportV1: Send + Sync {
    /// Commit this immutable request once, or return its original byte-identical certificate.
    /// Reuse of the same ID with another transaction must fail inside the device. An uncertain
    /// result must retain the original transaction; it cannot cancel or reset a predecessor.
    fn commit_or_recover(
        &self,
        request_id: [u8; 32],
        transaction: &KagemushaHardwareTransactionV1,
    ) -> Result<Vec<u8>, String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::hardware_transaction_journal::Record")]
enum Record {
    Initialize {
        binding: [u8; 32],
    },
    Prepare {
        request_id: [u8; 32],
        transaction: KagemushaHardwareTransactionV1,
    },
    Complete {
        request_id: [u8; 32],
        certificate: Vec<u8>,
    },
}

struct Entry {
    transaction: KagemushaHardwareTransactionV1,
    certificate: Option<Vec<u8>>,
}

/// Descriptor-locked public intent and certificate WAL for one authenticated hardware lane.
/// History has no count or age eviction. Disk exhaustion is an I/O failure before exposure.
pub struct KagemushaHardwareTransactionJournalV1 {
    wal: PrivateJournal,
    verifier: KagemushaHardwareTransactionVerifierV1,
    transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    entries: BTreeMap<[u8; 32], Entry>,
}

impl KagemushaHardwareTransactionJournalV1 {
    /// Create a new exclusive WAL; existing paths are never overwritten or reset.
    pub fn create_new(
        path: &Path,
        verifier: KagemushaHardwareTransactionVerifierV1,
        transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    ) -> Result<Self, String> {
        let binding = verifier.storage_binding()?;
        let mut owner = Self {
            wal: PrivateJournal::create_new(path, FORMAT).map_err(error)?,
            verifier,
            transport,
            entries: BTreeMap::new(),
        };
        owner.append(&Record::Initialize { binding })?;
        Ok(owner)
    }

    /// Replay and reauthenticate all retained certificates. A missing or conflicting prefix
    /// never becomes an empty wallet, and a prepared operation resumes the original device ID.
    pub fn open_existing(
        path: &Path,
        verifier: KagemushaHardwareTransactionVerifierV1,
        transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    ) -> Result<Self, String> {
        let binding = verifier.storage_binding()?;
        let mut owner = Self {
            wal: PrivateJournal::open_existing(path, FORMAT).map_err(error)?,
            verifier,
            transport,
            entries: BTreeMap::new(),
        };
        let mut initialized = false;
        while let Some((sequence, bytes)) = owner.wal.replay_next().map_err(error)? {
            let maximum = FORMAT.maximum_payload_bytes as usize;
            let record: Record = norito::decode_canonical_with_limits(
                &bytes,
                norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
            )
            .map_err(error)?;
            if norito::encode_canonical(&record).map_err(error)? != bytes {
                return Err("noncanonical hardware journal record".to_owned());
            }
            match record {
                Record::Initialize { binding: actual } if sequence == 0 && actual == binding => {
                    initialized = true;
                }
                Record::Prepare {
                    request_id,
                    transaction,
                } if initialized => {
                    validate_intent(request_id, &transaction)?;
                    if request_id == [0; 32] || owner.entries.contains_key(&request_id) {
                        return Err("conflicting hardware journal intent".to_owned());
                    }
                    owner.entries.insert(
                        request_id,
                        Entry {
                            transaction,
                            certificate: None,
                        },
                    );
                }
                Record::Complete {
                    request_id,
                    certificate,
                } if initialized => {
                    let entry = owner
                        .entries
                        .get_mut(&request_id)
                        .ok_or_else(|| "hardware completion has no intent".to_owned())?;
                    if entry.certificate.is_some() {
                        return Err("duplicate hardware completion".to_owned());
                    }
                    owner.verifier.verify_for_request(
                        request_id,
                        &entry.transaction,
                        &certificate,
                    )?;
                    entry.certificate = Some(certificate);
                }
                _ => return Err("invalid hardware journal initialization".to_owned()),
            }
        }
        if !initialized {
            return Err("empty hardware journal".to_owned());
        }
        Ok(owner)
    }

    /// Persist intent, execute/recover the one hardware transaction, verify it and fsync the
    /// original certificate before returning. Exact duplicate calls return the retained bytes.
    pub fn commit_or_recover(
        &mut self,
        request_id: [u8; 32],
        transaction: KagemushaHardwareTransactionV1,
    ) -> Result<Vec<u8>, String> {
        validate_intent(request_id, &transaction)?;
        // A previously prepared intent does not permit dispatch through a poisoned/replaced WAL.
        self.wal.recovery_prefix().map_err(error)?;
        if let Some(entry) = self.entries.get(&request_id) {
            if entry.transaction != transaction {
                return Err("conflicting hardware request identity".to_owned());
            }
            if let Some(certificate) = &entry.certificate {
                return Ok(certificate.clone());
            }
        } else {
            self.append(&Record::Prepare {
                request_id,
                transaction: transaction.clone(),
            })?;
            self.entries.insert(
                request_id,
                Entry {
                    transaction: transaction.clone(),
                    certificate: None,
                },
            );
        }
        let certificate = self.transport.commit_or_recover(request_id, &transaction)?;
        self.verifier
            .verify_for_request(request_id, &transaction, &certificate)?;
        self.append(&Record::Complete {
            request_id,
            certificate: certificate.clone(),
        })?;
        self.entries
            .get_mut(&request_id)
            .ok_or_else(|| "hardware completion lost its original intent".to_owned())?
            .certificate = Some(certificate.clone());
        Ok(certificate)
    }

    /// Exact durable prefix for inclusion in hardware-selected recovery metadata.
    pub fn recovery_prefix(&self) -> Result<super::KagemushaRecoveryJournalPrefixV1, String> {
        self.wal.recovery_prefix().map_err(error)
    }

    fn append(&mut self, record: &Record) -> Result<(), String> {
        self.wal
            .append(&norito::encode_canonical(record).map_err(error)?)
            .map_err(error)
    }
}

fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}

fn validate_intent(
    id: [u8; 32],
    transaction: &KagemushaHardwareTransactionV1,
) -> Result<(), String> {
    if id == [0; 32] {
        return Err("zero hardware request identity".to_owned());
    }
    match transaction {
        KagemushaHardwareTransactionV1::CurrentCheckpoint { .. } => {
            return Err(
                "fresh observations cannot be persisted as retryable transactions".to_owned(),
            );
        }
        KagemushaHardwareTransactionV1::RecoveryCheckpoint(value) if value.operation_id != id => {
            return Err("checkpoint request identity mismatch".to_owned());
        }
        _ => (),
    }
    transaction.validate()
}
