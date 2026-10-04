// Exact native frame admission and reads; metadata alone never authenticates execution.
/// One descriptor-owner-bound durable slot, still untrusted until native verification.
/// Capturing this receipt reads only bounded index/hash metadata, never the frame body.
pub(crate) struct NativeFrameRead<'kura> {
    kura: &'kura Kura,
    height: u64,
    hash: HashOf<BlockHeader>,
    wire_len: u64,
}
impl NativeFrameRead<'_> {
    /// Exact occupied slot length to admit before any body allocation or I/O.
    pub(crate) const fn wire_len(&self) -> u64 {
        self.wire_len
    }

    /// Load only this admitted slot, rechecking all durable metadata under storage guards.
    /// The returned bytes are untrusted and cannot authorize outputs by themselves.
    pub(crate) fn read(self, admitted_wire_len: u64) -> Result<Option<Vec<u8>>> {
        let Self {
            kura,
            height,
            hash,
            wire_len,
        } = self;
        if admitted_wire_len != wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let position = height - 1;
        let _prune = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()?;
        let _canonical = kura.canonical_chain_lock.lock();
        kura.ensure_canonical_storage_not_poisoned()?;
        let mut store = kura.block_store.lock();
        let count = store.read_exact_durable_index_count()?;
        if height > count {
            return Ok(None);
        }
        if Kura::read_durable_hash_at_height(&mut store, height)? != Some(hash) {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let slot = store.read_block_index(position)?;
        if slot.length != wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let length = usize::try_from(wire_len)?;
        let bytes = if slot.is_evicted() {
            // The bounded sidecar owner charges its one raw buffer before allocation.
            let Some(bytes) = Kura::read_regular_sidecar_bytes_for(
                &store.path_to_blockchain,
                &store.da_block_path(height),
                &store.da_blocks_dir,
                length,
            )?
            else {
                return Ok(None);
            };
            bytes
        } else {
            // Inline storage owns its buffer here; subsequent native decoding retains
            // this inherited cumulative scope and accounts for its separate graph.
            norito::core::reserve_decode_allocation(length).map_err(Error::NoritoFrame)?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length).map_err(|_| {
                Error::NoritoFrame(norito::Error::AllocationFailed { bytes: wire_len })
            })?;
            bytes.resize(length, 0);
            store.read_block_data(slot.start, &mut bytes)?;
            bytes
        };
        let confirmed = store.read_block_index(position)?;
        if u64::try_from(bytes.len())? != wire_len
            || store.read_exact_durable_index_count()? != count
            || confirmed.start != slot.start
            || confirmed.length != slot.length
            || Kura::read_durable_hash_at_height(&mut store, height)? != Some(hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        Ok(Some(bytes))
    }
}
impl Kura {
    /// Capture an exact native slot length without decoding or accepting any local certificate.
    /// No sidecar finality format, cache fallback, repair or directory creation is consulted.
    pub(crate) fn native_frame_read(
        &self,
        height: u64,
        hash: HashOf<BlockHeader>,
    ) -> Result<Option<NativeFrameRead<'_>>> {
        let position = height
            .checked_sub(1)
            .ok_or(Error::CanonicalBlockWireMismatch { height })?;
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let mut store = self.block_store.lock();
        if height > store.read_exact_durable_index_count()? {
            return Ok(None);
        }
        if Self::read_durable_hash_at_height(&mut store, height)? != Some(hash) {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let wire_len = store.read_block_index(position)?.length;
        if wire_len == 0 || wire_len > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Error::CorruptedBlockLength {
                length: wire_len,
                limit: STRICT_INIT_MAX_BLOCK_BYTES,
            });
        }
        Ok(Some(NativeFrameRead {
            kura: self,
            height,
            hash,
            wire_len,
        }))
    }

    /// Read the exact frame of an already authenticated original execution graph.
    /// The caller admits its full stored length first. No second decoded graph is constructed.
    pub(crate) fn read_authenticated_execution_wire(
        &self,
        authority: &crate::sumeragi::certified_chain::AuthenticatedExecutionBlock,
        admitted_wire_len: u64,
    ) -> Result<Option<(iroha_data_model::block::SharedSignedBlock, Vec<u8>)>> {
        let expected = authority.block();
        let height = expected.header().height().get();
        let (wire_len, wire_hash) = expected.canonical_wire_identity()?;
        if wire_len == 0 || wire_len != admitted_wire_len || wire_len > STRICT_INIT_MAX_BLOCK_BYTES
        {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        let Some(source) = self.native_frame_read(height, expected.hash())? else {
            return Ok(None);
        };
        let Some(bytes) = source.read(admitted_wire_len)? else {
            return Ok(None);
        };
        if Hash::new(&bytes) != wire_hash {
            return Err(Error::CanonicalBlockWireMismatch { height });
        }
        Ok(Some((expected.clone(), bytes)))
    }
}

#[cfg(test)]
mod native_execution_read_tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::{
            certified_chain::CertifiedChain,
            test_chain::{CertifiedTestChain, TestChainConfig},
        },
    };

    #[test]
    fn native_original_frame_admission_preserves_cumulative_allocation_on_both_stores() {
        for evicted in [false, true] {
            let mut chain =
                CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
            chain.commit(Vec::new());
            let original = chain.committed(2);
            let wire = original.block().encode_wire().unwrap();
            let length = wire.len();
            if evicted {
                let mut store = chain.kura().block_store.lock();
                store.write_da_block_bytes(2, &wire).unwrap();
                store
                    .write_block_index(1, EVICTED_BLOCK_START, length as u64)
                    .unwrap();
                store.publish_commit_marker(2).unwrap();
            }
            let read = |admitted| -> Result<Option<Vec<u8>>> {
                chain
                    .kura()
                    .native_frame_read(2, original.block_hash())?
                    .ok_or(Error::CanonicalBlockWireMismatch { height: 2 })?
                    .read(admitted)
            };
            let unspent = chain
                .kura()
                .native_frame_read(2, original.block_hash())
                .unwrap()
                .unwrap();
            let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, 64);
            norito::with_decode_limits_scope(limits, || {
                assert!(matches!(
                    unspent.read(length as u64 - 1),
                    Err(Error::CanonicalBlockWireMismatch { height: 2 })
                ));
                norito::core::reserve_decode_allocation(1).unwrap();
            });
            // Include the real repeated durable-marker decodes; source metadata is not free.
            // Probe cumulative consumption without guessing those current codec allocations.
            const PROBE_LIMIT: usize = 16 * 1024 * 1024;
            let limits =
                norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, PROBE_LIMIT, 64);
            let exact = norito::with_decode_limits_scope(limits, || {
                assert_eq!(read(length as u64).unwrap().unwrap(), wire);
                let error = norito::core::reserve_decode_allocation(PROBE_LIMIT).unwrap_err();
                let norito::Error::TotalAllocationExceeded { attempted, limit } = error else {
                    panic!("original quota refusal")
                };
                usize::try_from(attempted - limit).unwrap()
            });
            assert!(
                exact >= length,
                "the original frame buffer must be charged before reading"
            );
            let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, exact, 64);
            norito::with_decode_limits_scope(limits, || {
                assert_eq!(read(length as u64).unwrap().unwrap(), wire);
                assert!(matches!(
                    read(length as u64),
                    Err(Error::NoritoFrame(
                        norito::Error::TotalAllocationExceeded { .. }
                    ))
                ));
            });
            let limits =
                norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, exact - 1, 64);
            norito::with_decode_limits_scope(limits, || {
                assert!(matches!(
                    read(length as u64),
                    Err(Error::NoritoFrame(
                        norito::Error::TotalAllocationExceeded { .. }
                    ))
                ));
            });
            assert_eq!(read(length as u64).unwrap().unwrap(), wire);
        }
    }

    #[test]
    fn native_read_requires_exact_prepaid_length_and_retains_original_graph() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        let view = chain.state().view();
        let source = CertifiedChain::new(&view).unwrap();
        for height in 1..=2 {
            let receipt = source.authenticated_execution(height).unwrap();
            let (length, _) = receipt.block().canonical_wire_identity().unwrap();
            let pointer = receipt.block().clone();
            for wrong in [0, length - 1, length + 1] {
                assert!(
                    chain
                        .kura()
                        .read_authenticated_execution_wire(&receipt, wrong)
                        .is_err()
                );
            }
            let (original, bytes) = chain
                .kura()
                .read_authenticated_execution_wire(&receipt, length)
                .unwrap()
                .unwrap();
            assert!(iroha_data_model::block::SharedSignedBlock::ptr_eq(
                &original, &pointer
            ));
            assert_eq!(bytes, original.encode_wire().unwrap());
            assert!(!chain.kura().store_root().join("v2_finality").exists());
        }
    }

    #[test]
    fn changed_occupied_native_frame_fails_without_repair_or_cache_substitution() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        let view = chain.state().view();
        let receipt = CertifiedChain::new(&view)
            .unwrap()
            .authenticated_execution(2)
            .unwrap();
        let bytes = receipt.block().encode_wire().unwrap();
        let length = bytes.len() as u64;
        let mut changed = bytes.clone();
        *changed.last_mut().unwrap() ^= 1;
        let start = {
            let mut store = chain.kura().block_store.lock();
            let slot = store.read_block_index(1).unwrap();
            assert_eq!(slot.length, length);
            store.write_block_data(slot.start, &changed).unwrap();
            slot.start
        };
        assert!(matches!(
            chain
                .kura()
                .read_authenticated_execution_wire(&receipt, length),
            Err(Error::CanonicalBlockWireMismatch { height: 2 })
        ));
        let mut store = chain.kura().block_store.lock();
        let mut observed = vec![0; changed.len()];
        store.read_block_data(start, &mut observed).unwrap();
        assert_eq!(
            observed, changed,
            "the reader cannot repair occupied evidence"
        );
        store.write_block_data(start, &bytes).unwrap();
        drop(store);
        let (original, _) = chain
            .kura()
            .read_authenticated_execution_wire(&receipt, length)
            .unwrap()
            .unwrap();
        assert!(iroha_data_model::block::SharedSignedBlock::ptr_eq(
            &original,
            receipt.block()
        ));
    }
    #[test]
    fn native_metadata_is_untrusted_and_cannot_expand_an_admitted_read() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        let original = chain.committed(2);
        let source = chain
            .kura()
            .native_frame_read(2, original.block_hash())
            .unwrap()
            .unwrap();
        let wire_len = source.wire_len();
        assert!(source.read(wire_len - 1).is_err());
        let source = chain
            .kura()
            .native_frame_read(2, original.block_hash())
            .unwrap()
            .unwrap();
        let wire = source.read(wire_len).unwrap().unwrap();
        assert_eq!(wire, original.block().encode_wire().unwrap());
        assert!(
            chain
                .kura()
                .native_frame_read(0, original.block_hash())
                .is_err()
        );
    }
}

#[cfg(test)]
impl Kura {
    /// Corrupt the stored native frame without changing its hash/index or cached original.
    pub(crate) fn corrupt_native_frame_for_test(&self, height: NonZeroUsize) {
        let mut store = self.block_store.lock();
        let slot = store
            .read_block_index(u64::try_from(height.get() - 1).unwrap())
            .unwrap();
        assert!(!slot.is_evicted());
        let mut bytes = vec![0; usize::try_from(slot.length).unwrap()];
        store.read_block_data(slot.start, &mut bytes).unwrap();
        *bytes.last_mut().expect("native frame is nonempty") ^= 1;
        store.write_block_data(slot.start, &bytes).unwrap();
    }
}
