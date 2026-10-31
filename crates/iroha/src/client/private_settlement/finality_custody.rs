//! Caller-key authenticated custody of one pending settlement's bounded finality progress.
//! Only the native observer can publish checkpoints; peers and public callers cannot install them.

use std::{path::Path, sync::Arc};

use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    sumeragi_finality::{
        FinalityError, FinalityReadError, MAX_FINALITY_CHECKPOINT_BYTES,
        SumeragiFinalityCheckpoint, SumeragiFinalityVerifier,
    },
};
use iroha_fs::{PrivateDirectory, PublishMode};
use norito::{Decode, Encode};
use zeroize::Zeroizing;

use crate::workflow::{Digest32, TransactionIdentityV1, WorkflowId, domain_digest};

/// Hard local custody bound: three native checkpoints and their exact request/identity metadata.
/// Each checkpoint retains only its native bounded prefix, never arbitrary State or payment history.
pub const MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES: usize = 3 * MAX_FINALITY_CHECKPOINT_BYTES + 4096;

/// Original refusal from authenticated pending-finality custody.
#[derive(Debug, thiserror::Error)]
pub enum PrivateSettlementFinalityStorageError {
    /// Caller key, exact request/root binding or stored shape differs from the selected owner.
    #[error("private settlement finality custody binding mismatch")]
    Binding,
    /// The explicit local record or native checkpoint bound is exhausted.
    #[error("private settlement finality custody capacity exhausted")]
    Capacity,
    /// Another original publisher holds the same record's native lock.
    #[error("private settlement finality custody is busy")]
    Busy,
    /// The original record changed before compare-and-publish.
    #[error("private settlement finality custody changed")]
    Conflict,
    /// Original filesystem failure, including an uncertain atomic publication acknowledgement.
    #[error("private settlement finality custody I/O unavailable")]
    Io(#[source] std::io::Error),
    /// Original local encryption or secure-nonce failure before publication.
    #[error("private settlement finality encryption unavailable")]
    Encryption(#[source] iroha_crypto::encryption::Error),
    /// Original canonical codec refusal; no alternate decoder is attempted.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Original checkpoint decoding or native bounds refusal.
    #[error(transparent)]
    Finality(#[from] FinalityReadError),
    /// Original native checkpoint encoding or structural refusal.
    #[error(transparent)]
    Execution(#[from] FinalityError),
}
use PrivateSettlementFinalityStorageError as Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementFinalityPhaseV1")]
pub enum FinalityPhase {
    Register,
    Finalize,
    Abort,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementFinalityLookupV1")]
pub struct FinalityLookup {
    pub phase: FinalityPhase,
    pub identity: Option<TransactionIdentityV1>,
    pub height: u64,
}

#[derive(Clone, Debug)]
pub struct PendingFinalityPrefix {
    pub lookup: FinalityLookup,
    pub checkpoint: SumeragiFinalityCheckpoint,
}

#[derive(Clone, Debug)]
pub struct PendingFinalityPin {
    pub identity: TransactionIdentityV1,
    pub checkpoint: SumeragiFinalityCheckpoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementFinalityTerminalKindV1")]
pub enum FinalityTerminalKind {
    Settled,
    Aborted,
}
impl FinalityTerminalKind {
    pub(crate) const fn phase(self) -> FinalityPhase {
        match self {
            Self::Settled => FinalityPhase::Finalize,
            Self::Aborted => FinalityPhase::Abort,
        }
    }
}
#[derive(Clone, Debug)]
pub struct PendingFinalityTerminal {
    pub kind: FinalityTerminalKind,
    pub pin: PendingFinalityPin,
}

#[derive(Clone, Debug, Default)]
pub struct PendingFinalityRecord {
    pub prefix: Option<PendingFinalityPrefix>,
    pub register: Option<PendingFinalityPin>,
    pub terminal: Option<PendingFinalityTerminal>,
}

// Each native checkpoint retains its own advertised canonical frame and decoder bounds.
// The outer fixed-slot record never asks a generic decoder to allocate three unchecked
// checkpoint graphs before applying the model's admission contract.
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementStoredFinalityPrefixV1")]
struct StoredPrefix {
    lookup: FinalityLookup,
    checkpoint: Vec<u8>,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementStoredFinalityPinV1")]
struct StoredPin {
    identity: TransactionIdentityV1,
    checkpoint: Vec<u8>,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementStoredFinalityTerminalV1")]
struct StoredTerminal {
    kind: FinalityTerminalKind,
    pin: StoredPin,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementPendingFinalityRecordV1")]
struct StoredRecord {
    prefix: Option<StoredPrefix>,
    register: Option<StoredPin>,
    terminal: Option<StoredTerminal>,
}
impl StoredRecord {
    fn into_record(self) -> Result<PendingFinalityRecord, Error> {
        fn pin(stored: StoredPin) -> Result<PendingFinalityPin, Error> {
            Ok(PendingFinalityPin {
                identity: stored.identity,
                checkpoint: SumeragiFinalityCheckpoint::decode_canonical(&stored.checkpoint)?,
            })
        }
        Ok(PendingFinalityRecord {
            prefix: self
                .prefix
                .map(|stored| {
                    Ok::<_, Error>(PendingFinalityPrefix {
                        lookup: stored.lookup,
                        checkpoint: SumeragiFinalityCheckpoint::decode_canonical(
                            &stored.checkpoint,
                        )?,
                    })
                })
                .transpose()?,
            register: self.register.map(pin).transpose()?,
            terminal: self
                .terminal
                .map(|stored| {
                    Ok::<_, Error>(PendingFinalityTerminal {
                        kind: stored.kind,
                        pin: pin(stored.pin)?,
                    })
                })
                .transpose()?,
        })
    }
}
fn encode_record(
    record: &PendingFinalityRecord,
    maximum: usize,
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut checkpoint_bytes = 0_usize;
    for checkpoint in record
        .prefix
        .iter()
        .map(|entry| &entry.checkpoint)
        .chain(record.register.iter().map(|entry| &entry.checkpoint))
        .chain(record.terminal.iter().map(|entry| &entry.pin.checkpoint))
    {
        let length = norito::canonical_frame_len(checkpoint)?;
        if length > MAX_FINALITY_CHECKPOINT_BYTES {
            return Err(Error::Capacity);
        }
        checkpoint_bytes = checkpoint_bytes
            .checked_add(length)
            .ok_or(Error::Capacity)?;
        if checkpoint_bytes > maximum {
            return Err(Error::Capacity);
        }
    }
    fn pin(value: &PendingFinalityPin) -> Result<StoredPin, Error> {
        Ok(StoredPin {
            identity: value.identity,
            checkpoint: value.checkpoint.encode_canonical()?,
        })
    }
    let stored = StoredRecord {
        prefix: record
            .prefix
            .as_ref()
            .map(|value| {
                Ok::<_, Error>(StoredPrefix {
                    lookup: value.lookup.clone(),
                    checkpoint: value.checkpoint.encode_canonical()?,
                })
            })
            .transpose()?,
        register: record.register.as_ref().map(pin).transpose()?,
        terminal: record
            .terminal
            .as_ref()
            .map(|value| {
                Ok::<_, Error>(StoredTerminal {
                    kind: value.kind,
                    pin: pin(&value.pin)?,
                })
            })
            .transpose()?,
    };
    if norito::canonical_frame_len(&stored)? > maximum {
        return Err(Error::Capacity);
    }
    Ok(Zeroizing::new(norito::encode_canonical(&stored)?))
}

pub struct LoadedPendingFinality {
    pub record: PendingFinalityRecord,
    pub digest: Option<Digest32>,
}

#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::PrivateSettlementPendingFinalityBindingV1")]
struct Binding {
    workflow: Digest32,
    request: Digest32,
    network: NetworkId,
    chain: String,
    instance: [u8; 32],
    scope: SumeragiRootScope,
}

/// Caller-key encrypted file owner for pending prepared-settlement finality.
///
/// Opening this capability creates no checkpoint or lock file. The operation binds its exact
/// prepared request, workflow and independently selected Global genesis internally. Its native
/// observer is the sole publisher; no public method accepts a checkpoint as trusted input.
/// The caller retains this key across restart. Filesystem/media rollback and key loss are not
/// repaired by this local software-custody owner.
pub struct FilePrivateSettlementFinality {
    directory: PrivateDirectory,
    cipher: SymmetricEncryptor<ChaCha20Poly1305>,
    maximum: usize,
}
impl FilePrivateSettlementFinality {
    /// Open or create an owner-private directory with an explicit caller-retained key and cap.
    ///
    /// The cap covers the complete plaintext record containing all three fixed slots. It is a
    /// local resource limit, never a consensus or transaction validity setting.
    /// # Errors
    /// Zero key, zero/excessive cap, unsafe custody, or original filesystem failure.
    pub fn open(
        path: impl AsRef<Path>,
        key: &[u8; 32],
        maximum_total_bytes: usize,
    ) -> Result<Self, Error> {
        if key.iter().all(|byte| *byte == 0) {
            return Err(Error::Binding);
        }
        if !(1..=MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES).contains(&maximum_total_bytes) {
            return Err(Error::Capacity);
        }
        Ok(Self {
            directory: PrivateDirectory::open_or_create(path).map_err(Error::Io)?,
            cipher: SymmetricEncryptor::new_with_key(key).map_err(|_| Error::Binding)?,
            maximum: maximum_total_bytes,
        })
    }

    /// Retain the same directory and key under a smaller explicit total-record allowance.
    ///
    /// One-call preparation uses this to charge pending settlement custody against the original
    /// aggregate artifact budget after reloading its immutable proof/context originals.
    /// No file is created, removed or overwritten by this operation.
    /// # Errors
    /// Zero or widened allowance, or changed native directory custody.
    pub fn with_maximum_bytes(&self, maximum_total_bytes: usize) -> Result<Self, Error> {
        if maximum_total_bytes == 0 || maximum_total_bytes > self.maximum {
            return Err(Error::Capacity);
        }
        Ok(Self {
            directory: self.directory.retain().map_err(Error::Io)?,
            cipher: self.cipher.clone(),
            maximum: maximum_total_bytes,
        })
    }

    pub(crate) fn bind(
        self: &Arc<Self>,
        workflow: WorkflowId,
        request: Digest32,
        verifier: &SumeragiFinalityVerifier,
    ) -> Result<BoundPrivateSettlementFinality, Error> {
        if verifier.root_scope() != SumeragiRootScope::Global {
            return Err(Error::Binding);
        }
        Ok(BoundPrivateSettlementFinality {
            owner: self.clone(),
            binding: Binding {
                workflow: *workflow.as_bytes(),
                request,
                network: verifier.initial_epoch().network_id,
                chain: verifier.chain_id().to_owned(),
                instance: verifier.instance().0,
                scope: verifier.root_scope(),
            },
        })
    }
}

#[derive(Clone)]
pub struct BoundPrivateSettlementFinality {
    owner: Arc<FilePrivateSettlementFinality>,
    binding: Binding,
}
impl BoundPrivateSettlementFinality {
    fn name(&self) -> String {
        format!("{}.settlement-finality", hex::encode(self.binding.workflow))
    }
    fn lock_name(&self) -> String {
        format!(
            "{}.settlement-finality.lock",
            hex::encode(self.binding.workflow)
        )
    }
    fn aad(&self) -> Result<Vec<u8>, Error> {
        Ok(norito::encode_canonical(&(
            "iroha:private-settlement:pending-finality:v1".to_owned(),
            self.binding.clone(),
        ))?)
    }
    fn validate(&self, record: &PendingFinalityRecord) -> Result<(), Error> {
        if let Some(prefix) = &record.prefix
            && (prefix.lookup.height < 2
                || prefix.checkpoint.height() == 0
                || prefix.checkpoint.height() > prefix.lookup.height
                || (prefix.lookup.phase == FinalityPhase::Register
                    && prefix.lookup.identity.is_none()))
        {
            return Err(Error::Binding);
        }
        if record
            .register
            .iter()
            .chain(record.terminal.iter().map(|terminal| &terminal.pin))
            .any(|pin| pin.checkpoint.height() < 2)
        {
            return Err(Error::Binding);
        }
        for checkpoint in record
            .prefix
            .iter()
            .map(|entry| &entry.checkpoint)
            .chain(record.register.iter().map(|entry| &entry.checkpoint))
            .chain(record.terminal.iter().map(|entry| &entry.pin.checkpoint))
        {
            if checkpoint.network_id() != self.binding.network
                || checkpoint.chain_id() != self.binding.chain
            {
                return Err(Error::Binding);
            }
        }
        Ok(())
    }

    pub(crate) fn load(&self) -> Result<LoadedPendingFinality, Error> {
        let limit = self.owner.maximum.checked_add(28).ok_or(Error::Capacity)?;
        let Some(envelope) = self
            .owner
            .directory
            .read_optional(self.name(), limit)
            .map_err(Error::Io)?
        else {
            return Ok(LoadedPendingFinality {
                record: PendingFinalityRecord::default(),
                digest: None,
            });
        };
        let plaintext = Zeroizing::new(
            self.owner
                .cipher
                .decrypt_easy(self.aad()?.as_slice(), envelope.as_slice())
                .map_err(|_| Error::Binding)?,
        );
        if plaintext.len() > self.owner.maximum {
            return Err(Error::Capacity);
        }
        let stored: StoredRecord = norito::decode_canonical_with_limits(
            &plaintext,
            norito::canonical_decode_limits(plaintext.len()),
        )?;
        let record = stored.into_record()?;
        self.validate(&record)?;
        Ok(LoadedPendingFinality {
            record,
            digest: Some(record_digest(&plaintext)),
        })
    }

    /// Native observer only: every supplied checkpoint must have been exported from its
    /// independently authenticated verifier, with exact phase/effect checks before pinning.
    /// The caller's original operation runner lease remains held across this whole call.
    pub(crate) fn compare_and_publish(
        &self,
        expected: Option<&Digest32>,
        record: &PendingFinalityRecord,
    ) -> Result<Digest32, Error> {
        self.validate(record)?;
        let plaintext = encode_record(record, self.owner.maximum)?;
        let lock = self
            .owner
            .directory
            .open_lock(self.lock_name())
            .map_err(Error::Io)?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(Error::Io(error)),
        }
        self.owner
            .directory
            .require_same_lock(self.lock_name(), &lock)
            .map_err(Error::Io)?;
        // Read and authenticate the exact original before CAS. Corruption and foreign
        // ciphertext never authorize replacement, even when the caller expects absence.
        let current = self.load()?;
        if current.digest.as_ref() != expected {
            return Err(Error::Conflict);
        }
        let ciphertext = self
            .owner
            .cipher
            .encrypt_easy(self.aad()?.as_slice(), plaintext.as_slice())
            .map_err(Error::Encryption)?;
        self.owner
            .directory
            .require_same_lock(self.lock_name(), &lock)
            .map_err(Error::Io)?;
        self.owner
            .directory
            .write_atomic(
                self.name(),
                &ciphertext,
                if current.digest.is_some() {
                    PublishMode::Replace
                } else {
                    PublishMode::CreateNew
                },
            )
            .map_err(Error::Io)?;
        self.owner
            .directory
            .require_same_lock(self.lock_name(), &lock)
            .map_err(Error::Io)?;
        Ok(record_digest(&plaintext))
    }
}

fn record_digest(bytes: &[u8]) -> Digest32 {
    domain_digest(
        b"iroha:private-settlement:pending-finality-record:v1\0",
        &[bytes],
    )
}

#[cfg(test)]
mod tests;
