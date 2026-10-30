//! Caller-key authenticated progress for one original pending native payment.
//! Only the native observer publishes evidence; decoded checkpoints are never public authority.

use std::{fs::File, path::Path, sync::Arc};

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

/// Maximum plaintext custody: three native checkpoints and exact original-payment metadata.
/// Each native checkpoint retains at most three decisions, never an arbitrary history archive.
pub const MAX_NATIVE_PAYMENT_FINALITY_BYTES: usize = 3 * MAX_FINALITY_CHECKPOINT_BYTES + 4096;

const RECORD_NAME: &str = "native-payment.finality";
const LOCK_NAME: &str = "native-payment.finality.lock";
const AEAD_OVERHEAD: usize = 28;

/// Original local refusal from authenticated pending-payment custody.
#[derive(Debug, thiserror::Error)]
pub enum NativePaymentFinalityStorageError {
    /// Caller key, exact original binding, immutable success or record shape differs.
    #[error("native payment finality custody binding mismatch")]
    Binding,
    /// The explicit record, native checkpoint or physical custody allowance is exhausted.
    #[error("native payment finality custody capacity exhausted")]
    Capacity,
    /// Another original publisher or reader holds the same native record lock.
    #[error("native payment finality custody is busy")]
    Busy,
    /// The authenticated record changed before compare-and-publish.
    #[error("native payment finality custody changed")]
    Conflict,
    /// Original filesystem failure, including an uncertain publication acknowledgement.
    #[error("native payment finality custody I/O unavailable")]
    Io(#[source] std::io::Error),
    /// Original secure nonce or local encryption failure.
    #[error("native payment finality encryption unavailable")]
    Encryption(#[source] iroha_crypto::encryption::Error),
    /// Original canonical codec refusal; no alternate decoder is attempted.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Original bounded native checkpoint decoder refusal.
    #[error(transparent)]
    Finality(#[from] FinalityReadError),
    /// Original native checkpoint encoding or structural refusal.
    #[error(transparent)]
    Execution(#[from] FinalityError),
}
use NativePaymentFinalityStorageError as Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentFinalityBusinessLocatorV1")]
pub(super) struct BusinessLocator {
    pub height: u64,
    pub block_hash: Digest32,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentFinalityLookupV1")]
pub(super) enum FinalityLookup {
    Business(BusinessLocator),
    Carrier {
        identity: TransactionIdentityV1,
        height: u64,
    },
}
impl FinalityLookup {
    pub(super) fn height(&self) -> u64 {
        match self {
            Self::Business(locator) => locator.height,
            Self::Carrier { height, .. } => *height,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PendingFinalityPrefix {
    pub lookup: FinalityLookup,
    pub checkpoint: SumeragiFinalityCheckpoint,
}

#[derive(Clone, Debug)]
pub(super) struct PendingCarrierPin {
    pub identity: TransactionIdentityV1,
    pub checkpoint: SumeragiFinalityCheckpoint,
    // Present only after exact native rejected-execution verification. A peer locator,
    // pipeline status or checkpoint framing check cannot populate this field.
    pub replay: Option<BusinessLocator>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingBusinessPin {
    pub identity: TransactionIdentityV1,
    pub checkpoint: SumeragiFinalityCheckpoint,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PendingFinalityRecord {
    pub prefix: Option<PendingFinalityPrefix>,
    pub carrier: Option<PendingCarrierPin>,
    pub business: Option<PendingBusinessPin>,
}

// Each native frame has its own advertised layout and bounded model decoder. The outer
// decoder never allocates unchecked checkpoint graphs on behalf of three independent slots.
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentStoredFinalityPrefixV1")]
struct StoredPrefix {
    lookup: FinalityLookup,
    checkpoint: Vec<u8>,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentStoredCarrierPinV1")]
struct StoredCarrier {
    identity: TransactionIdentityV1,
    checkpoint: Vec<u8>,
    replay: Option<BusinessLocator>,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentStoredBusinessPinV1")]
struct StoredBusiness {
    identity: TransactionIdentityV1,
    checkpoint: Vec<u8>,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentPendingFinalityRecordV1")]
struct StoredRecord {
    prefix: Option<StoredPrefix>,
    carrier: Option<StoredCarrier>,
    business: Option<StoredBusiness>,
}
impl StoredRecord {
    fn into_record(self) -> Result<PendingFinalityRecord, Error> {
        Ok(PendingFinalityRecord {
            prefix: self
                .prefix
                .map(|value| {
                    Ok::<_, Error>(PendingFinalityPrefix {
                        lookup: value.lookup,
                        checkpoint: SumeragiFinalityCheckpoint::decode_canonical(
                            &value.checkpoint,
                        )?,
                    })
                })
                .transpose()?,
            carrier: self
                .carrier
                .map(|value| {
                    Ok::<_, Error>(PendingCarrierPin {
                        identity: value.identity,
                        checkpoint: SumeragiFinalityCheckpoint::decode_canonical(
                            &value.checkpoint,
                        )?,
                        replay: value.replay,
                    })
                })
                .transpose()?,
            business: self
                .business
                .map(|value| {
                    Ok::<_, Error>(PendingBusinessPin {
                        identity: value.identity,
                        checkpoint: SumeragiFinalityCheckpoint::decode_canonical(
                            &value.checkpoint,
                        )?,
                    })
                })
                .transpose()?,
        })
    }
}
impl PendingFinalityRecord {
    fn checkpoints(&self) -> impl Iterator<Item = &SumeragiFinalityCheckpoint> {
        self.prefix
            .iter()
            .map(|entry| &entry.checkpoint)
            .chain(self.carrier.iter().map(|entry| &entry.checkpoint))
            .chain(self.business.iter().map(|entry| &entry.checkpoint))
    }
}
fn encode_record(
    record: &PendingFinalityRecord,
    maximum: usize,
) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut checkpoint_bytes = 0_usize;
    for checkpoint in record.checkpoints() {
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
        carrier: record
            .carrier
            .as_ref()
            .map(|value| {
                Ok::<_, Error>(StoredCarrier {
                    identity: value.identity,
                    checkpoint: value.checkpoint.encode_canonical()?,
                    replay: value.replay,
                })
            })
            .transpose()?,
        business: record
            .business
            .as_ref()
            .map(|value| {
                Ok::<_, Error>(StoredBusiness {
                    identity: value.identity,
                    checkpoint: value.checkpoint.encode_canonical()?,
                })
            })
            .transpose()?,
    };
    if norito::canonical_frame_len(&stored)? > maximum {
        return Err(Error::Capacity);
    }
    Ok(Zeroizing::new(norito::encode_canonical(&stored)?))
}

pub(super) struct LoadedPendingFinality {
    pub record: PendingFinalityRecord,
    pub digest: Option<Digest32>,
}

#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::client::NativePaymentPendingFinalityBindingV1")]
struct Binding {
    workflow: Digest32,
    request: Digest32,
    network: NetworkId,
    chain: String,
    instance: Digest32,
    scope: SumeragiRootScope,
    initial_epoch: Digest32,
}

/// Caller-key encrypted custody for one original native payment's pending finality.
///
/// Use a dedicated directory for each original payment and retain its key across restarts.
/// Opening creates only the private directory. The operation binds its exact request and
/// independently selected Global genesis; callers cannot install trusted checkpoints.
/// Physical custody is one active encrypted record plus one bounded atomic staging record
/// and an empty lock: at most `2 * (maximum_total_bytes + 28)` bytes. Interrupted staging
/// is reconciled only under the original exclusive native lock. Key loss and filesystem
/// rollback are not repaired by this software custody owner.
pub struct FileNativePaymentFinality {
    directory: PrivateDirectory,
    cipher: SymmetricEncryptor<ChaCha20Poly1305>,
    maximum: usize,
}
impl FileNativePaymentFinality {
    /// Open or create an owner-private directory with a caller-retained key and record cap.
    ///
    /// The cap covers the plaintext containing all three fixed slots. This local allowance
    /// never changes transaction validity, proof verification rules or the proof-read budget.
    /// # Errors
    /// Zero key, zero/excessive cap, unsafe native custody or original filesystem failure.
    pub fn open(
        path: impl AsRef<Path>,
        key: &[u8; 32],
        maximum_total_bytes: usize,
    ) -> Result<Self, Error> {
        if key.iter().all(|byte| *byte == 0) {
            return Err(Error::Binding);
        }
        if !(1..=MAX_NATIVE_PAYMENT_FINALITY_BYTES).contains(&maximum_total_bytes) {
            return Err(Error::Capacity);
        }
        Ok(Self {
            directory: PrivateDirectory::open_or_create(path).map_err(Error::Io)?,
            cipher: SymmetricEncryptor::new_with_key(key).map_err(|_| Error::Binding)?,
            maximum: maximum_total_bytes,
        })
    }

    pub(super) fn bind(
        self: &Arc<Self>,
        workflow: WorkflowId,
        request: Digest32,
        verifier: &SumeragiFinalityVerifier,
    ) -> Result<BoundNativePaymentFinality, Error> {
        if verifier.root_scope() != SumeragiRootScope::Global {
            return Err(Error::Binding);
        }
        let initial_epoch = norito::encode_canonical(verifier.initial_epoch())?;
        Ok(BoundNativePaymentFinality {
            owner: self.clone(),
            binding: Binding {
                workflow: *workflow.as_bytes(),
                request,
                network: verifier.initial_epoch().network_id,
                chain: verifier.chain_id().to_owned(),
                instance: verifier.instance().0,
                scope: verifier.root_scope(),
                initial_epoch: domain_digest(
                    b"iroha:native-payment:initial-epoch:v1\0",
                    &[&initial_epoch],
                ),
            },
        })
    }

    fn maximum_envelope(&self) -> Result<usize, Error> {
        self.maximum
            .checked_add(AEAD_OVERHEAD)
            .ok_or(Error::Capacity)
    }

    fn writer(&self) -> Result<File, Error> {
        let lock = self.directory.open_lock(LOCK_NAME).map_err(Error::Io)?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(Error::Io(error)),
        }
        self.directory
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        self.inventory()?;
        self.directory
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        Ok(lock)
    }

    // Only called with the original exclusive lock. Validate the complete finite inventory
    // before the native owner removes its one unpublished staging file.
    fn inventory(&self) -> Result<(), Error> {
        let maximum = self.maximum_envelope()?;
        let mut required = vec![LOCK_NAME];
        for name in self.directory.entries(3).map_err(Error::Io)? {
            let name = name.to_str().ok_or(Error::Binding)?;
            match name {
                LOCK_NAME => {
                    if self
                        .directory
                        .open_read(name)
                        .map_err(Error::Io)?
                        .metadata()
                        .map_err(Error::Io)?
                        .len()
                        != 0
                    {
                        return Err(Error::Binding);
                    }
                }
                RECORD_NAME => {
                    if self
                        .directory
                        .open_read(name)
                        .map_err(Error::Io)?
                        .metadata()
                        .map_err(Error::Io)?
                        .len()
                        > maximum as u64
                    {
                        return Err(Error::Capacity);
                    }
                    required.push(RECORD_NAME);
                }
                candidate if candidate.starts_with(".iroha-fs-") => (),
                _ => return Err(Error::Binding),
            }
        }
        self.directory
            .reconcile_atomic_staging(&required, 2, 1, maximum)
            .map_err(Error::Io)?;
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct BoundNativePaymentFinality {
    owner: Arc<FileNativePaymentFinality>,
    binding: Binding,
}
impl BoundNativePaymentFinality {
    fn aad(&self) -> Result<Vec<u8>, Error> {
        Ok(norito::encode_canonical(&(
            "iroha:native-payment:pending-finality:v1".to_owned(),
            self.binding.clone(),
        ))?)
    }

    fn validate(&self, record: &PendingFinalityRecord) -> Result<(), Error> {
        if let Some(prefix) = &record.prefix
            && (prefix.lookup.height() < 2
                || prefix.checkpoint.height() == 0
                || prefix.checkpoint.height() > prefix.lookup.height())
        {
            return Err(Error::Binding);
        }
        if record.carrier.as_ref().is_some_and(|pin| {
            pin.checkpoint.height() < 2 || pin.replay.is_some_and(|locator| locator.height < 2)
        }) || record
            .business
            .as_ref()
            .is_some_and(|pin| pin.checkpoint.height() < 2)
        {
            return Err(Error::Binding);
        }
        if record.checkpoints().any(|checkpoint| {
            checkpoint.network_id() != self.binding.network
                || checkpoint.chain_id() != self.binding.chain
        }) {
            return Err(Error::Binding);
        }
        Ok(())
    }

    pub(super) fn load(&self) -> Result<LoadedPendingFinality, Error> {
        let names = self.owner.directory.entries(3).map_err(Error::Io)?;
        if names.is_empty() {
            return Ok(LoadedPendingFinality {
                record: PendingFinalityRecord::default(),
                digest: None,
            });
        }
        if !names.iter().any(|name| name == LOCK_NAME) {
            return Err(Error::Binding);
        }
        let lock = self.owner.writer()?;
        let loaded = self.load_locked()?;
        self.owner
            .directory
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        Ok(loaded)
    }

    fn load_locked(&self) -> Result<LoadedPendingFinality, Error> {
        let Some(envelope) = self
            .owner
            .directory
            .read_optional(RECORD_NAME, self.owner.maximum_envelope()?)
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

    /// Internal observer only: all slots must first pass native import, exact root/epoch
    /// comparison and payment-effect verification. Carrier replacement follows the original
    /// workflow's selected identity; a saved rejection alone never grants signing authority.
    /// The caller retains its original observation lease through publication and outcome use.
    pub(super) fn compare_and_publish(
        &self,
        expected: Option<&Digest32>,
        record: &PendingFinalityRecord,
    ) -> Result<Digest32, Error> {
        self.validate(record)?;
        let plaintext = encode_record(record, self.owner.maximum)?;
        let lock = self.owner.writer()?;
        let current = self.load_locked()?;
        if current.digest.as_ref() != expected {
            return Err(Error::Conflict);
        }
        if let Some(original) = current.record.business {
            let Some(candidate) = &record.business else {
                return Err(Error::Binding);
            };
            if original.identity != candidate.identity
                || original.checkpoint != candidate.checkpoint
            {
                return Err(Error::Binding);
            }
        }
        let ciphertext = self
            .owner
            .cipher
            .encrypt_easy(self.aad()?.as_slice(), plaintext.as_slice())
            .map_err(Error::Encryption)?;
        self.owner
            .directory
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        self.owner
            .directory
            .write_atomic(
                RECORD_NAME,
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
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        Ok(record_digest(&plaintext))
    }
}

fn record_digest(bytes: &[u8]) -> Digest32 {
    domain_digest(
        b"iroha:native-payment:pending-finality-record:v1\0",
        &[bytes],
    )
}

#[cfg(test)]
mod tests;
