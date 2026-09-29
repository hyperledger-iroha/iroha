impl Kura {
    fn canonical_block_wire_hash(block: &SignedBlock) -> Result<Hash> {
        Self::canonical_block_wire_identity(block).map(|(_, hash)| hash)
    }
    fn canonical_block_wire_identity(block: &SignedBlock) -> Result<(u64, Hash)> {
        let wire = block.encode_wire().map_err(Error::NoritoFrame)?;
        let len = u64::try_from(wire.len())?;
        if len == 0 || len > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Error::CorruptedBlockLength {
                length: len,
                limit: STRICT_INIT_MAX_BLOCK_BYTES,
            });
        }
        Ok((len, Hash::new(&wire)))
    }
    fn ensure_existing_block_wire_matches(
        &self,
        block: &SignedBlock,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        self.ensure_durable_block_at_height(height, canonical_hash)?;
        let (incoming_wire_len, incoming_wire_hash) = Self::canonical_block_wire_identity(block)?;
        let blocks_dir = self.active_blocks_dir.lock().clone();
        let durable_index = self
            .block_store
            .lock()
            .read_block_index(height.saturating_sub(1))?;
        if durable_index.length != incoming_wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        if durable_index.is_evicted() {
            // An evicted index has no independently readable canonical complete wire. This is also
            // true for authenticated hash-only snapshot entries: their header hash is canonical,
            // but it does not select one SignedBlock envelope. Never let an unsigned retained
            // record fill that gap; existing-body admission requires signed complete-wire finality
            // for every evicted shape.
            let (signed_wire_len, signed_wire_hash) = self
                .verified_v2_finality_wire_hash_for_eviction(&blocks_dir, height, canonical_hash)?
                .ok_or(Error::MissingV2FinalityArtifact { height })?;
            if incoming_wire_len != durable_index.length
                || incoming_wire_len != signed_wire_len
                || incoming_wire_hash != signed_wire_hash
            {
                return Err(Error::CanonicalBlockWireMismatch { height });
            }
            return Ok(());
        }
        if let Some((retained_header, _, retained_wire_len, retained_wire_hash, _)) =
            self.retained_block_record_at(&blocks_dir, height, canonical_hash)?
        {
            if retained_header != block.header()
                || retained_wire_len != durable_index.length
                || retained_wire_len != incoming_wire_len
                || retained_wire_hash != incoming_wire_hash
            {
                return Err(Error::CanonicalBlockWireMismatch { height });
            }
            return Ok(());
        }
        let block_height = NonZeroUsize::new(usize::try_from(height)?)
            .ok_or(Error::CanonicalBlockWireMismatch { height })?;
        let canonical_block = self
            .get_block_without_merge_sidecar(block_height)
            .ok_or(Error::CanonicalBlockWireMismatch { height })?;
        if canonical_block.header() != block.header()
            || Self::canonical_block_wire_hash(canonical_block.as_ref())? != incoming_wire_hash
        {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        Ok(())
    }
    fn ensure_replay_metadata_allows_top_replacement_while_sidecars_locked(
        &self,
        blocks_dir: &Path,
        height: u64,
    ) -> Result<()> {
        // The caller holds `sidecar_lock` across this preflight (and, for the
        // final check, canonical marker publication). Read the two sidecars
        // directly: the public accessors acquire the same non-reentrant mutex.
        self.ensure_prune_recovery_not_required()?;
        let checkpoint_path = Self::wsv_checkpoint_path_for(blocks_dir, height);
        if let Some(checkpoint) = Self::decode_wsv_checkpoint_at(&checkpoint_path)? {
            if checkpoint.height != height {
                return Err(Error::NoritoFrame(norito::core::Error::Message(format!(
                    "WSV checkpoint height mismatch: expected {height}, got {}",
                    checkpoint.height
                ))));
            }
            self.ensure_durable_block_at_height(height, checkpoint.block_hash)?;
            return Err(Error::CommittedBlockReplacementForbidden { height });
        }
        let manifest_path = Self::commit_manifest_path_for(blocks_dir, height);
        if let Some(manifest) = Self::decode_commit_manifest_at(&manifest_path)? {
            if manifest.height != height {
                return Err(Error::NoritoFrame(norito::core::Error::Message(format!(
                    "commit manifest height mismatch: expected {height}, got {}",
                    manifest.height
                ))));
            }
            self.ensure_durable_block_at_height(height, manifest.block_hash)?;
            return Err(Error::CommittedBlockReplacementForbidden { height });
        }
        Ok(())
    }
}
