// Consensus reads distinguish a proved absence from unreadable durable evidence.
#[derive(Clone, Copy)]
enum CanonicalBlockReadAuthority<'a, 'k> {
    Published,
    PublishedBounded {
        hash: &'a HashOf<BlockHeader>,
        wire_len: u64,
    },
    #[cfg_attr(not(test), allow(dead_code, reason = "TODO: wire native consensus owner"))]
    Apply(&'a crate::block::VerifiedV2FinalityArtifact),
    Startup(&'a V2StartupFinalityVerificationSession<'k>),
}
impl Kura {




    /// Read an authenticated finalized body without consulting or publishing caches.
    ///
    /// An uncommitted height, authenticated imported prefix, or authenticated
    /// evicted body without a local replica is absent. Invalid occupied storage
    /// is an error. An occupied append whose finality is not yet published is
    /// an explicit `MissingV2FinalityArtifact` error, never finalized evidence.
    /// The reader never repairs or synchronizes storage.
    pub(crate) fn read_block_body(&self, height: NonZeroUsize) -> Result<Option<Arc<SignedBlock>>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_guard = self.canonical_chain_lock.lock();
        self.read_block_body_under_prune_and_canonical_guards(height)
    }

    /// Read the exact finalized body whose durable length the caller already charged.
    ///
    /// Recheck the hash and length under the canonical storage guards before
    /// allocating body bytes. A changed slot cannot consume an earlier, smaller
    /// reservation. Missing and corrupt bodies retain the ordinary read errors.
    pub(crate) fn read_block_body_with_wire_bound(
        &self,
        height: NonZeroUsize,
        hash: HashOf<BlockHeader>,
        wire_len: u64,
    ) -> Result<Option<Arc<SignedBlock>>> {
        self.read_block_body_and_wire_with_wire_bound(height, hash, wire_len)
            .map(|body| body.map(|(block, _)| block))
    }

    /// Return the original authenticated wire with its decoded body after size admission.
    ///
    /// Both values come from the same guarded exact-finality read. Proof transport
    /// uses these bytes directly, avoiding a second complete carrier allocation or
    /// a re-encoding which could differ from the QC-authenticated wire identity.
    pub(crate) fn read_block_body_and_wire_with_wire_bound(
        &self,
        height: NonZeroUsize,
        hash: HashOf<BlockHeader>,
        wire_len: u64,
    ) -> Result<Option<(Arc<SignedBlock>, Vec<u8>)>> {
        let _prune = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical = self.canonical_chain_lock.lock();
        self.read_block_body_and_wire_with_authority_under_guards(
            height,
            CanonicalBlockReadAuthority::PublishedBounded {
                hash: &hash,
                wire_len,
            },
        )
    }


    fn read_block_body_under_prune_and_canonical_guards(
        &self,
        height: NonZeroUsize,
    ) -> Result<Option<Arc<SignedBlock>>> {
        self.read_block_body_with_authority_under_guards(
            height,
            CanonicalBlockReadAuthority::Published,
        )
    }

    fn read_block_body_with_authority_under_guards(
        &self,
        height: NonZeroUsize,
        authority: CanonicalBlockReadAuthority<'_, '_>,
    ) -> Result<Option<Arc<SignedBlock>>> {
        self.read_block_body_and_wire_with_authority_under_guards(height, authority)
            .map(|body| body.map(|(block, _)| block))
    }

    fn read_block_body_and_wire_with_authority_under_guards(
        &self,
        height: NonZeroUsize,
        authority: CanonicalBlockReadAuthority<'_, '_>,
    ) -> Result<Option<(Arc<SignedBlock>, Vec<u8>)>> {
        if let CanonicalBlockReadAuthority::Startup(startup) = authority
            && !std::ptr::eq(self, startup.kura)
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "startup canonical body authority belongs to another Kura",
            ));
        }
        self.ensure_prune_recovery_not_required()?;
        self.ensure_canonical_storage_not_poisoned()?;
        let height_u64 = u64::try_from(height.get())?;
        let position = height_u64 - 1;
        let mut store = self.block_store.lock();
        let count = store.read_exact_durable_index_count()?;
        if height_u64 > count {
            return Ok(None);
        }
        if self.is_hard_fork_hash_only_block(height.get() - 1) {
            self.ensure_snapshot_bootstrap_authenticated()?;
            return Ok(None);
        }
        let hash = Self::read_durable_hash_at_height(&mut store, height_u64)?
            .ok_or(Error::HashesFileHeightMismatch)?;
        let parent = if position == 0 {
            None
        } else {
            Some(
                Self::read_durable_hash_at_height(&mut store, position)?
                    .ok_or(Error::HashesFileHeightMismatch)?,
            )
        };
        let slot = store.read_block_index(position)?;
        if slot.length == 0 || slot.length > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Error::CorruptedBlockLength {
                length: slot.length,
                limit: STRICT_INIT_MAX_BLOCK_BYTES,
            });
        }
        let artifact = match authority {
            CanonicalBlockReadAuthority::Published
            | CanonicalBlockReadAuthority::PublishedBounded { .. } => None,
            CanonicalBlockReadAuthority::Apply(authority) => Some(authority.artifact()),
            CanonicalBlockReadAuthority::Startup(startup) => {
                Some(startup.canonical_tip_finality_for_read(
                    self,
                    height_u64,
                    &store.path_to_blockchain,
                )?)
            }
        };
        // A startup session has already authenticated these exact files and
        // excludes canonical writers. Recheck its retained identities instead
        // of decoding the same historical finality and retained record again.
        let published_wire = if matches!(authority, CanonicalBlockReadAuthority::Startup(_)) {
            None
        } else {
            self.verified_v2_finality_wire_hash_for_eviction(
                &store.path_to_blockchain,
                height_u64,
                hash,
            )?
        };
        let (wire_len, wire_hash) = if let Some(artifact) = artifact {
            if artifact.height != height_u64
                || artifact.block_hash != hash
                || artifact.subject.parent_block_hash != parent
            {
                return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
            }
            let commitment = &artifact.commit_qc.execution_commitment;
            let expected = (
                commitment.executed_block_wire_len,
                commitment.executed_block_wire_hash,
            );
            if let Some(published_wire) = published_wire {
                let directory = Self::v2_finality_artifact_dir_for(&store.path_to_blockchain);
                let path =
                    Self::v2_finality_artifact_path_for(&store.path_to_blockchain, height_u64);
                let (record, _) = self
                    .decode_v2_finality_record_at(&path, &directory)?
                    .ok_or(Error::MissingV2FinalityArtifact { height: height_u64 })?;
                if published_wire != expected || record.artifact != *artifact {
                    return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
                }
            }
            (expected.0, Some(expected.1))
        } else if let Some((wire_len, wire_hash)) = published_wire {
            (wire_len, Some(wire_hash))
        } else {
            // A Sumeragi block has no v2 finality sidecar: the block store verified its commit
            // certificate before writing the frame, and the decoded header's hash and parent
            // are checked below. TODO(WP8c): Kura keeps no v2 finality.
            (slot.length, None)
        };
        if slot.length != wire_len {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        if let CanonicalBlockReadAuthority::PublishedBounded {
            hash: expected_hash,
            wire_len: charged_wire_len,
        } = authority
            && (hash != *expected_hash || wire_len != charged_wire_len)
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let bytes = if slot.is_evicted() {
            let Some(bytes) = Self::read_regular_sidecar_bytes_for(
                &store.path_to_blockchain,
                &store.da_block_path(height_u64),
                &store.da_blocks_dir,
                usize::try_from(wire_len)?,
            )?
            else {
                if let CanonicalBlockReadAuthority::Startup(startup) = authority {
                    startup.canonical_tip_finality_for_read(
                        self,
                        height_u64,
                        &store.path_to_blockchain,
                    )?;
                }
                return Ok(None);
            };
            bytes
        } else {
            let mut bytes = vec![0; usize::try_from(slot.length)?];
            store.read_block_data(slot.start, &mut bytes)?;
            bytes
        };
        if u64::try_from(bytes.len())? != wire_len
            || wire_hash.is_some_and(|wire_hash| Hash::new(&bytes) != wire_hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let block = decode_framed_signed_block(&bytes)?;
        if block.hash() != hash
            || block.header().height().get() != height_u64
            || block.header().prev_block_hash() != parent
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        if let Some(artifact) = artifact
            && block.canonical_proposal_wire_hash()? != artifact.subject.payload_hash
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let confirmed_slot = store.read_block_index(position)?;
        if store.read_exact_durable_index_count()? != count
            || confirmed_slot.start != slot.start
            || confirmed_slot.length != slot.length
            || Self::read_durable_hash_at_height(&mut store, height_u64)? != Some(hash)
        {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        if let CanonicalBlockReadAuthority::Startup(startup) = authority {
            startup.canonical_tip_finality_for_read(self, height_u64, &store.path_to_blockchain)?;
        }
        Ok(Some((Arc::new(block), bytes)))
    }



















}
