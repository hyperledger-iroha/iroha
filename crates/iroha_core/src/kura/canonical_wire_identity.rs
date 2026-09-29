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
    fn canonical_proposal_wire_hash(block: &SignedBlock) -> Result<Hash> {
        block
            .canonical_proposal_wire_hash()
            .map_err(Error::NoritoFrame)
    }
    fn ensure_existing_block_wire_matches(
        &self,
        block: &SignedBlock,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
    ) -> Result<()> {
        // Caller retains the original canonical publication fences. Never select
        // a cached decoded graph or an obsolete sidecar to repair missing bytes.
        let (wire_len, wire_hash) = Self::canonical_block_wire_identity(block)?;
        let mut store = self.block_store.lock();
        let position = height
            .checked_sub(1)
            .ok_or(Error::CanonicalBlockWireMismatch { height })?;
        let count = store.read_exact_durable_index_count()?;
        if height > count
            || Self::read_durable_hash_at_height(&mut store, height)? != Some(canonical_hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let slot = store.read_block_index(position)?;
        if slot.length != wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let bytes = if slot.is_evicted() {
            store
                .read_optional_da_cache(height)?
                .ok_or(Error::CanonicalBlockWireMismatch { height })?
        } else {
            let mut bytes = vec![0; usize::try_from(wire_len)?];
            store.read_block_data(slot.start, &mut bytes)?;
            bytes
        };
        let confirmed = store.read_block_index(position)?;
        if bytes.len() as u64 != wire_len
            || Hash::new(&bytes) != wire_hash
            || confirmed.start != slot.start
            || confirmed.length != slot.length
            || store.read_exact_durable_index_count()? != count
            || Self::read_durable_hash_at_height(&mut store, height)? != Some(canonical_hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        Ok(())
    }
}
