//! Exact checkpoint durability at the original Kura, without State authority.
//!
//! A receipt records actual synchronized writer bytes and their stable readback.
//! It does not reserve geometry or authorize source execution/State publication.
//! The final publication lease reauthenticates through its already held guards;
//! the standalone reader acquires prune, canonical-chain and sidecar itself.

use super::*;

/// Move-only proof of an exact checkpoint and finality at one original Kura.
///
/// This is an in-memory storage capability, never a wire format or a replacement
/// for the actual State execution/source owner. Reopening the same directory
/// creates another Kura identity and requires a new authenticated receipt.
#[derive(Debug)]
pub(crate) struct KuraWsvCheckpointReceipt {
    finality: KuraV2CommitReceipt,
    checkpoint: WsvCheckpoint,
    checkpoint_metadata: StableSidecarMetadata,
    checkpoint_bytes_hash: Hash,
    // Keep the actual objects alive so inode/file-ID reuse cannot impersonate
    // the original checkpoint or one of its parent directories after deletion.
    written: std::fs::File,
    namespace: BoundProgressNamespace,
    // Retain the Kura and its root lock until every original file handle drops.
    kura: Arc<Kura>,
}

impl KuraWsvCheckpointReceipt {
    /// Borrow the original durable commit receipt authenticated by this writer.
    /// This projection grants neither a new checkpoint nor State permission.
    pub(crate) fn finality_receipt(&self) -> &KuraV2CommitReceipt {
        &self.finality
    }
}

/// Actual synchronized writer and held ancestor namespace, plus stable readback.
/// The shared writer returns these together; no later reopen creates custody.
pub(super) struct DurableWsvCheckpoint {
    pub(super) readback: StableSidecarRead,
    pub(super) written: std::fs::File,
    pub(super) namespace: BoundProgressNamespace,
}

impl Kura {
    /// Persist the exact retained State checkpoint under already durable finality.
    ///
    /// No caller callback, availability flag or supplied authority constructor
    /// participates. The original Kura verifies the complete finality/retained
    /// wire association and every receipt field before touching checkpoint bytes.
    /// An error grants no receipt; the idempotent writer retains any actual
    /// durable work for an exact retry.
    pub(crate) fn persist_wsv_checkpoint_for_v2_commit(
        self: &Arc<Self>,
        finality: &KuraV2CommitReceipt,
        state_hash: Hash,
    ) -> Result<KuraWsvCheckpointReceipt> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.durable_mutation_authorized()?;
        self.authenticate_checkpoint_finality_under_prune_and_canonical_guards(finality)?;
        let _sidecar = self.sidecar_lock.lock();
        let DurableWsvCheckpoint {
            readback,
            written,
            namespace,
        } = self.write_wsv_checkpoint_under_sidecar_guard(
            finality.height(),
            finality.block_hash(),
            state_hash,
        )?;
        let checkpoint = Self::decode_checkpoint_receipt_readback(&readback)?;
        if checkpoint.height != finality.height()
            || checkpoint.block_hash != finality.block_hash()
            || checkpoint.state_hash != state_hash
        {
            return Err(self
                .checkpoint_receipt_error("writer readback differs from its retained checkpoint"));
        }
        Ok(KuraWsvCheckpointReceipt {
            kura: Arc::clone(self),
            finality: finality.clone(),
            checkpoint,
            checkpoint_metadata: readback.metadata,
            checkpoint_bytes_hash: readback.bytes_hash,
            written,
            namespace,
        })
    }

    /// Rejoin a retained receipt to exact currently present storage and finality.
    ///
    /// This does not mutate disk and supplies no publication lease; bounded
    /// derived body/finality caches can be populated. The State aggregate
    /// still owes its final exact canonical-boundary fence before publishing.
    /// Do not call while holding `canonical_publication_lease`.
    pub(crate) fn reauthenticate_wsv_checkpoint_receipt(
        &self,
        receipt: &KuraWsvCheckpointReceipt,
        finality: &V2FinalityArtifact,
        state_hash: Hash,
    ) -> Result<()> {
        let _prune = self.prune_lock.lock();
        let _canonical = self.canonical_chain_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        self.reauthenticate_checkpoint_under_publication_guards(receipt, finality, state_hash)
    }

    /// Caller retains the original prune, canonical and sidecar guards.
    /// This performs bounded reads/verification, including existing derived caches.
    pub(super) fn reauthenticate_checkpoint_under_publication_guards(
        &self,
        receipt: &KuraWsvCheckpointReceipt,
        finality: &V2FinalityArtifact,
        state_hash: Hash,
    ) -> Result<()> {
        if !receipt.kura.instance_identity().matches(self)
            || !checkpoint_finality_matches(&receipt.finality, finality)
            || receipt.checkpoint.state_hash != state_hash
            || receipt.checkpoint.height != finality.height
            || receipt.checkpoint.block_hash != finality.block_hash
        {
            return Err(self.checkpoint_receipt_error(
                "checkpoint receipt differs from its original Kura, finality or State hash",
            ));
        }
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let opened = secure_file_metadata::from_file(&receipt.written).map_err(|error| {
            Error::IO(error, receipt.checkpoint_metadata.canonical_path.clone())
        })?;
        if !Self::sidecar_file_metadata_unchanged(&receipt.checkpoint_metadata.file, &opened)
            || !self.bound_progress_namespace_unchanged(&receipt.namespace)
        {
            return Err(self.checkpoint_receipt_error(
                "checkpoint writer or its held ancestor namespace changed",
            ));
        }
        self.authenticate_checkpoint_finality_under_prune_and_canonical_guards(&receipt.finality)?;
        let path = self.wsv_checkpoint_path(receipt.checkpoint.height);
        let directory = self.wsv_checkpoint_dir();
        let readback = self
            .read_regular_sidecar_snapshot(&path, &directory, MAX_WSV_CHECKPOINT_BYTES)?
            .ok_or_else(|| {
                self.checkpoint_receipt_error("retained checkpoint is no longer present")
            })?;
        if readback.bytes_hash != receipt.checkpoint_bytes_hash
            || !Self::stable_sidecar_file_binding_unchanged(
                &receipt.checkpoint_metadata,
                &readback.metadata,
            )
            || Self::decode_checkpoint_receipt_readback(&readback)? != receipt.checkpoint
            || !self.bound_progress_namespace_unchanged(&receipt.namespace)
        {
            return Err(self.checkpoint_receipt_error(
                "retained checkpoint bytes or original namespace changed",
            ));
        }
        Ok(())
    }

    fn authenticate_checkpoint_finality_under_prune_and_canonical_guards(
        &self,
        receipt: &KuraV2CommitReceipt,
    ) -> Result<()> {
        // Reject zero/max/out-of-range heights before deriving a sidecar path or
        // reading/allocating any height-indexed material. No saturating fallback.
        let height = receipt.height();
        let durable_count = u64::try_from(self.exact_durable_blocks_count()?)?;
        if height == 0 || height > durable_count {
            return Err(self.checkpoint_receipt_error(
                "checkpoint receipt height is outside the exact durable chain",
            ));
        }
        let Some((header, artifact, _)) =
            self.v2_finality_artifact_with_archive_under_prune_and_canonical_guards(height)?
        else {
            return Err(
                self.checkpoint_receipt_error("checkpoint has no exact durable v2 finality")
            );
        };
        if header.hash() != receipt.block_hash() || !checkpoint_finality_matches(receipt, &artifact)
        {
            return Err(self.checkpoint_receipt_error(
                "checkpoint receipt differs from exact durable finality",
            ));
        }
        Ok(())
    }

    /// Decode exactly the canonical bytes returned by the bound checkpoint reader.
    pub(super) fn decode_checkpoint_receipt_readback(
        readback: &StableSidecarRead,
    ) -> Result<WsvCheckpoint> {
        let checkpoint = WsvCheckpoint::decode_all(&mut readback.bytes.as_slice())
            .map_err(Error::NoritoFrame)?;
        if checkpoint.encode() != readback.bytes {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "checkpoint readback is not canonical Norito",
                ),
                readback.metadata.canonical_path.clone(),
            ));
        }
        Ok(checkpoint)
    }

    fn checkpoint_receipt_error(&self, message: &'static str) -> Error {
        Error::IO(
            std::io::Error::new(ErrorKind::InvalidData, message),
            self.wsv_checkpoint_dir(),
        )
    }
}

fn checkpoint_finality_matches(
    receipt: &KuraV2CommitReceipt,
    artifact: &V2FinalityArtifact,
) -> bool {
    receipt.height() == artifact.height
        && receipt.block_hash() == artifact.block_hash
        && receipt.context_id() == artifact.context_id()
        && receipt.subject() == artifact.subject
        && receipt.certificate() == artifact.commit_qc.as_ref()
        && receipt.artifact_hash() == HashOf::new(artifact)
}

#[cfg(test)]
thread_local! {
    static CORRUPT_CHECKPOINT_READBACK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Fail after the real checkpoint durability barriers, before receipt readback.
#[cfg(test)]
pub(super) fn corrupt_next_checkpoint_readback_for_test() {
    CORRUPT_CHECKPOINT_READBACK.set(true);
}

#[cfg(test)]
pub(super) fn inject_checkpoint_readback_corruption_for_test(path: &Path) -> Result<()> {
    if CORRUPT_CHECKPOINT_READBACK.replace(false) {
        std::fs::write(path, b"injected checkpoint readback corruption")
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
    }
    Ok(())
}
