impl Kura {
    fn retained_block_record_dir_for(blocks_dir: &Path) -> PathBuf {
        blocks_dir.join(RETAINED_BLOCKS_DIR_NAME)
    }

    fn retained_block_record_path_for(blocks_dir: &Path, height: u64) -> PathBuf {
        Self::retained_block_record_dir_for(blocks_dir).join(format!("{height:020}.norito"))
    }

    fn retained_block_rewrite_staging_dir_for(blocks_dir: &Path) -> PathBuf {
        blocks_dir.join(RETAINED_BLOCK_REWRITE_STAGING_DIR_NAME)
    }

    fn retained_block_rewrite_staging_path_for(blocks_dir: &Path, height: u64) -> PathBuf {
        Self::retained_block_rewrite_staging_dir_for(blocks_dir)
            .join(format!("{height:020}.norito"))
    }

    #[cfg(test)]
    fn retained_block_record_dir(&self) -> PathBuf {
        Self::retained_block_record_dir_for(&self.active_blocks_dir.lock())
    }

    #[cfg(test)]
    fn retained_block_record_path(&self, height: u64) -> PathBuf {
        Self::retained_block_record_path_for(&self.active_blocks_dir.lock(), height)
    }

    fn canonical_height_sidecar_heights_for(
        store_root: &Path,
        directory: &Path,
        label: &'static str,
        max_canonical_entries: u64,
    ) -> Result<Vec<u64>> {
        if Self::canonical_sidecar_directory_for(store_root, directory)?.is_none() {
            return Ok(Vec::new());
        }
        let entries = std::fs::read_dir(directory)
            .map_err(|error| Error::IO(error, directory.to_path_buf()))?;
        let mut heights = Vec::new();
        let mut entries_seen = 0_u64;
        let mut canonical_entries_seen = 0_u64;
        const TRANSIENT_ENTRY_SLACK: u64 = 32;
        for entry in entries {
            let entry = entry.map_err(|error| Error::IO(error, directory.to_path_buf()))?;
            let path = entry.path();
            entries_seen = entries_seen.saturating_add(1);
            if entries_seen > max_canonical_entries.saturating_add(TRANSIENT_ENTRY_SLACK) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} inventory exceeds the durable-chain bound"),
                    ),
                    directory.to_path_buf(),
                ));
            }
            let file_name = entry.file_name();
            let name = file_name.to_str().ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} directory contains a non-UTF-8 entry"),
                    ),
                    path.clone(),
                )
            })?;
            if name.starts_with(".kura-sidecar-") {
                continue;
            }
            if let Some(stem) = name.strip_suffix(".norito.tmp")
                && stem.len() == 20
                && stem.as_bytes().iter().all(u8::is_ascii_digit)
            {
                continue;
            }
            let Some(stem) = name.strip_suffix(".norito") else {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} directory contains an unknown entry"),
                    ),
                    path,
                ));
            };
            if stem.len() != 20 || !stem.as_bytes().iter().all(u8::is_ascii_digit) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} name is not a canonical 20-digit height"),
                    ),
                    path,
                ));
            }
            let height = stem.parse::<u64>().map_err(|_| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} height exceeds the supported range"),
                    ),
                    path.clone(),
                )
            })?;
            if height == 0 {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} height must be non-zero"),
                    ),
                    path,
                ));
            }
            canonical_entries_seen = canonical_entries_seen.saturating_add(1);
            if canonical_entries_seen > max_canonical_entries.saturating_add(TRANSIENT_ENTRY_SLACK)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "{label} count exceeds the durable-chain bound plus recovery slack"
                        ),
                    ),
                    directory.to_path_buf(),
                ));
            }
            Self::regular_sidecar_metadata_for(store_root, &path, directory)?.ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{label} disappeared during inventory validation"),
                    ),
                    path.clone(),
                )
            })?;
            heights.push(height);
        }
        heights.sort_unstable();
        Ok(heights)
    }

    fn retained_block_record_heights_for(
        store_root: &Path,
        blocks_dir: &Path,
        max_canonical_entries: u64,
    ) -> Result<Vec<u64>> {
        Self::canonical_height_sidecar_heights_for(
            store_root,
            &Self::retained_block_record_dir_for(blocks_dir),
            "retained block sidecar",
            max_canonical_entries,
        )
    }

    fn invalid_retained_sccp_archive(height: u64, reason: impl Into<String>) -> Error {
        Error::InvalidRetainedSccpArchive {
            height,
            reason: reason.into(),
        }
    }

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

    fn validate_v2_finality_wire_bindings(
        height: u64,
        artifact: &V2FinalityArtifact,
        proposal_wire_hash: Hash,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Result<()> {
        if artifact.subject.payload_hash != proposal_wire_hash {
            return Err(Error::V2FinalityPayloadHashMismatch { height });
        }
        if artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_len
            != executed_block_wire_len
        {
            return Err(Error::V2FinalityExecutedBlockWireLengthMismatch { height });
        }
        if artifact
            .commit_qc
            .execution_commitment
            .executed_block_wire_hash
            != executed_block_wire_hash
        {
            return Err(Error::V2FinalityExecutedBlockWireHashMismatch { height });
        }
        Ok(())
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

    fn retained_sccp_archive_from_block(
        block: &SignedBlock,
    ) -> Result<Vec<KuraRetainedSccpMessage>> {
        let height = block.header().height().get();
        crate::bridge::validate_sccp_commitment_root_for_signed_block(block).map_err(|error| {
            Self::invalid_retained_sccp_archive(
                height,
                format!("committed block SCCP validation failed: {error:?}"),
            )
        })?;
        let messages = crate::bridge::collect_sccp_messages_from_signed_block(block);
        let max =
            usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)?;
        if messages.len() > max {
            return Err(Self::invalid_retained_sccp_archive(
                height,
                format!(
                    "archive contains {} messages; maximum is {max}",
                    messages.len()
                ),
            ));
        }
        let mut archive = Vec::new();
        archive.try_reserve_exact(messages.len())?;
        for (index, message) in messages.into_iter().enumerate() {
            let commitment_index = u32::try_from(index)?;
            let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&message.payload)
                .map_err(|_| {
                    Self::invalid_retained_sccp_archive(
                        height,
                        format!("message {commitment_index} cannot be canonically encoded"),
                    )
                })?;
            if payload_bytes.is_empty()
                || payload_bytes.len()
                    > iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1
            {
                return Err(Self::invalid_retained_sccp_archive(
                    height,
                    format!("message {commitment_index} exceeds the canonical payload bound"),
                ));
            }
            archive.push(KuraRetainedSccpMessage {
                commitment_index,
                context: message.context,
                payload_bytes,
            });
        }
        Ok(archive)
    }

    fn validate_retained_sccp_archive(
        record: &KuraRetainedBlockRecord,
    ) -> Result<Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>> {
        let height = record.height;
        let max =
            usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)?;
        if record.sccp_archive.len() > max {
            return Err(Self::invalid_retained_sccp_archive(
                height,
                format!(
                    "archive contains {} messages; maximum is {max}",
                    record.sccp_archive.len()
                ),
            ));
        }
        let mut projections = Vec::new();
        projections.try_reserve_exact(record.sccp_archive.len())?;
        let mut seen = BTreeSet::new();
        for (index, archived) in record.sccp_archive.iter().enumerate() {
            let expected_index = u32::try_from(index)?;
            if archived.commitment_index != expected_index {
                return Err(Self::invalid_retained_sccp_archive(
                    height,
                    format!(
                        "archive is not dense: expected index {expected_index}, found {}",
                        archived.commitment_index
                    ),
                ));
            }
            let validated = crate::bridge::validate_recorded_sccp_message_payload_bytes(
                archived.context,
                &archived.payload_bytes,
            )
            .map_err(|error| {
                Self::invalid_retained_sccp_archive(
                    height,
                    format!("message {expected_index} is invalid: {error:?}"),
                )
            })?;
            let canonical =
                iroha_sccp::canonical_sccp_payload_bytes(&validated.payload).map_err(|_| {
                    Self::invalid_retained_sccp_archive(
                        height,
                        format!("message {expected_index} cannot be canonically re-encoded"),
                    )
                })?;
            if canonical != archived.payload_bytes {
                return Err(Self::invalid_retained_sccp_archive(
                    height,
                    format!("message {expected_index} uses noncanonical payload bytes"),
                ));
            }
            if !seen.insert(validated.key) {
                return Err(Self::invalid_retained_sccp_archive(
                    height,
                    format!("message {expected_index} repeats an outbound replay key"),
                ));
            }
            projections.push(crate::bridge::ValidatedSccpOutboundMessageProjectionV1 {
                commitment_index: expected_index,
                context: validated.context,
                payload: validated.payload,
                commitment: validated.commitment,
            });
        }
        let commitments = projections
            .iter()
            .map(|projection| projection.commitment.clone())
            .collect::<Vec<_>>();
        let reconstructed = iroha_sccp::commitment_merkle_root(&commitments);
        if reconstructed != record.block_header.sccp_commitment_root() {
            return Err(Self::invalid_retained_sccp_archive(
                height,
                "archive commitment root differs from the retained canonical header",
            ));
        }
        Ok(projections)
    }

    fn decode_retained_block_record_at(
        &self,
        path: &Path,
        directory: &Path,
    ) -> Result<Option<KuraRetainedBlockRecord>> {
        Ok(self
            .decode_retained_block_record_with_identity_at(path, directory)?
            .map(|(record, _)| record))
    }

    fn decode_retained_block_record_with_identity_at(
        &self,
        path: &Path,
        directory: &Path,
    ) -> Result<Option<(KuraRetainedBlockRecord, StableSidecarRead)>> {
        self.record_startup_replay_historical_payload_read();
        let Some(snapshot) =
            self.read_regular_sidecar_snapshot(path, directory, MAX_RETAINED_BLOCK_RECORD_BYTES)?
        else {
            return Ok(None);
        };
        let mut cursor = snapshot.bytes.as_slice();
        let record =
            KuraRetainedBlockRecord::decode_all(&mut cursor).map_err(Error::NoritoFrame)?;
        if record.encode() != snapshot.bytes {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "Kura retained block record is not canonically encoded",
                ),
                path.to_path_buf(),
            ));
        }
        Ok(Some((record, snapshot)))
    }

    fn validate_retained_block_record_at(
        path: &Path,
        expected_height: u64,
        canonical_hash: HashOf<BlockHeader>,
        record: &KuraRetainedBlockRecord,
    ) -> Result<Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>> {
        if record.format_version != RETAINED_BLOCK_RECORD_VERSION {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "unsupported Kura retained block record version",
                ),
                path.to_path_buf(),
            ));
        }
        if record.height != expected_height || record.block_header.height().get() != expected_height
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "retained block height does not match its canonical file name",
                ),
                path.to_path_buf(),
            ));
        }
        let actual_hash = record.block_header.hash();
        if record.block_hash != actual_hash {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "retained block hash field does not match its header",
                ),
                path.to_path_buf(),
            ));
        }
        if actual_hash != canonical_hash {
            return Err(Error::BlockHeightConflict {
                height: expected_height,
                expected: canonical_hash,
                actual: actual_hash,
            });
        }
        if record.executed_block_wire_len == 0
            || record.executed_block_wire_len > STRICT_INIT_MAX_BLOCK_BYTES
        {
            return Err(Error::CorruptedBlockLength {
                length: record.executed_block_wire_len,
                limit: STRICT_INIT_MAX_BLOCK_BYTES,
            });
        }
        let encoded_len = record.encode().len();
        if encoded_len > MAX_RETAINED_BLOCK_RECORD_BYTES {
            return Err(Error::RetainedBlockRecordTooLarge {
                actual: encoded_len,
                max: MAX_RETAINED_BLOCK_RECORD_BYTES,
            });
        }
        Self::validate_retained_sccp_archive(record)
    }

    fn retained_block_record_at(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
    ) -> Result<
        Option<(
            BlockHeader,
            Hash,
            u64,
            Hash,
            Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>,
        )>,
    > {
        self.retained_block_record_at_inner(blocks_dir, height, canonical_hash, true)
    }

    fn retained_block_record_at_without_live_body(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
    ) -> Result<
        Option<(
            BlockHeader,
            Hash,
            u64,
            Hash,
            Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>,
        )>,
    > {
        self.retained_block_record_at_inner(blocks_dir, height, canonical_hash, false)
    }

    fn retained_block_record_at_with_identity(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
        validate_live_body: bool,
    ) -> Result<
        Option<(
            (
                BlockHeader,
                Hash,
                u64,
                Hash,
                Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>,
            ),
            StableSidecarRead,
        )>,
    > {
        self.retained_block_record_at_inner_with_identity(
            blocks_dir,
            height,
            canonical_hash,
            validate_live_body,
        )
    }

    fn retained_block_record_at_inner(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
        validate_live_body: bool,
    ) -> Result<
        Option<(
            BlockHeader,
            Hash,
            u64,
            Hash,
            Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>,
        )>,
    > {
        Ok(self
            .retained_block_record_at_inner_with_identity(
                blocks_dir,
                height,
                canonical_hash,
                validate_live_body,
            )?
            .map(|(record, _)| record))
    }

    fn retained_block_record_at_inner_with_identity(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
        validate_live_body: bool,
    ) -> Result<
        Option<(
            (
                BlockHeader,
                Hash,
                u64,
                Hash,
                Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>,
            ),
            StableSidecarRead,
        )>,
    > {
        // Callers that expose this result outside rewrite/recovery internals hold
        // `canonical_chain_lock`, so a recoverable rewrite stage can never appear as a transient
        // missing canonical record to proof-serving or state-validation readers.
        let directory = Self::retained_block_record_dir_for(blocks_dir);
        let path = Self::retained_block_record_path_for(blocks_dir, height);
        let Some((record, read_identity)) =
            self.decode_retained_block_record_with_identity_at(&path, &directory)?
        else {
            return Ok(None);
        };
        let archive =
            Self::validate_retained_block_record_at(&path, height, canonical_hash, &record)?;
        if validate_live_body
            && let Some(block_height) = NonZeroUsize::new(usize::try_from(height)?)
            && let Some(block) = self.get_block(block_height)
        {
            let (executed_block_wire_len, executed_block_wire_hash) =
                Self::canonical_block_wire_identity(block.as_ref())?;
            if block.header() != record.block_header
                || Self::canonical_proposal_wire_hash(block.as_ref())? != record.proposal_wire_hash
                || executed_block_wire_len != record.executed_block_wire_len
                || executed_block_wire_hash != record.executed_block_wire_hash
            {
                return Err(Error::ConflictingRetainedBlockRecord { height });
            }
        }
        Ok(Some((
            (
                record.block_header,
                record.proposal_wire_hash,
                record.executed_block_wire_len,
                record.executed_block_wire_hash,
                archive,
            ),
            read_identity,
        )))
    }

    fn prepare_retained_block_record(
        blocks_dir: &Path,
        canonical_hash: HashOf<BlockHeader>,
        block: &SignedBlock,
    ) -> Result<KuraRetainedBlockRecord> {
        let height = block.header().height().get();
        if block.hash() != canonical_hash {
            return Err(Error::BlockHeightConflict {
                height,
                expected: canonical_hash,
                actual: block.hash(),
            });
        }
        let path = Self::retained_block_record_path_for(blocks_dir, height);
        let (executed_block_wire_len, executed_block_wire_hash) =
            Self::canonical_block_wire_identity(block)?;
        let record = KuraRetainedBlockRecord::new(
            block.header(),
            Self::canonical_proposal_wire_hash(block)?,
            executed_block_wire_len,
            executed_block_wire_hash,
            Self::retained_sccp_archive_from_block(block)?,
        );
        let _ = Self::validate_retained_block_record_at(&path, height, canonical_hash, &record)?;
        let bytes = record.encode();
        if bytes.len() > MAX_RETAINED_BLOCK_RECORD_BYTES {
            return Err(Error::RetainedBlockRecordTooLarge {
                actual: bytes.len(),
                max: MAX_RETAINED_BLOCK_RECORD_BYTES,
            });
        }
        Ok(record)
    }

    fn persist_prepared_retained_block_record(
        &self,
        blocks_dir: &Path,
        canonical_hash: HashOf<BlockHeader>,
        record: &KuraRetainedBlockRecord,
    ) -> Result<()> {
        let height = record.height;
        let directory = Self::retained_block_record_dir_for(blocks_dir);
        let path = Self::retained_block_record_path_for(blocks_dir, height);
        let _ = Self::validate_retained_block_record_at(&path, height, canonical_hash, record)?;
        let indexed_wire_len = self
            .block_store
            .lock()
            .read_block_index(height.saturating_sub(1))?
            .length;
        if record.executed_block_wire_len != indexed_wire_len {
            return Err(Error::V2FinalityExecutedBlockWireLengthMismatch { height });
        }
        let bytes = record.encode();
        if bytes.len() > MAX_RETAINED_BLOCK_RECORD_BYTES {
            return Err(Error::RetainedBlockRecordTooLarge {
                actual: bytes.len(),
                max: MAX_RETAINED_BLOCK_RECORD_BYTES,
            });
        }

        if let Some(existing) = self.decode_retained_block_record_at(&path, &directory)? {
            let _ =
                Self::validate_retained_block_record_at(&path, height, canonical_hash, &existing)?;
            return if existing == *record {
                Ok(())
            } else {
                Err(Error::ConflictingRetainedBlockRecord { height })
            };
        }

        create_dir_all_with_context(&directory)?;
        if let Some(parent) = directory.parent() {
            sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        }
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        if !self.write_atomic_synced_noclobber(&path, &bytes)? {
            let Some(existing) = self.decode_retained_block_record_at(&path, &directory)? else {
                return Err(Error::ConflictingRetainedBlockRecord { height });
            };
            let _ =
                Self::validate_retained_block_record_at(&path, height, canonical_hash, &existing)?;
            return if existing == *record {
                Ok(())
            } else {
                Err(Error::ConflictingRetainedBlockRecord { height })
            };
        }
        self.add_total_disk_usage_bytes(u64::try_from(bytes.len())?);

        let Some(persisted) = self.decode_retained_block_record_at(&path, &directory)? else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "new retained block sidecar is missing after durable rename",
                ),
                path,
            ));
        };
        let _ = Self::validate_retained_block_record_at(&path, height, canonical_hash, &persisted)?;
        if persisted != *record {
            return Err(Error::ConflictingRetainedBlockRecord { height });
        }
        accounting_mutation.finish();
        Ok(())
    }

    fn persist_retained_block_record(
        &self,
        blocks_dir: &Path,
        canonical_hash: HashOf<BlockHeader>,
        block: &SignedBlock,
    ) -> Result<()> {
        let record = Self::prepare_retained_block_record(blocks_dir, canonical_hash, block)?;
        self.persist_prepared_retained_block_record(blocks_dir, canonical_hash, &record)
    }

    /// Read a bounded, root-authenticated SCCP archive retained independently of the block body.
    #[cfg(test)]
    pub(crate) fn retained_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            BlockHeader,
            Vec<crate::bridge::ValidatedSccpOutboundMessageProjectionV1>,
        )>,
    > {
        self.ensure_canonical_storage_not_poisoned()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let Some(block_height) = NonZeroUsize::new(usize::try_from(height)?) else {
            return Err(Error::MissingRetainedBlockRecord { height });
        };
        let canonical_hash = self
            .get_durable_block_hash(block_height)
            .ok_or(Error::MissingRetainedBlockRecord { height })?;
        let blocks_dir = self.active_blocks_dir.lock().clone();
        if let Some((header, _, _, _, archive)) =
            self.retained_block_record_at(&blocks_dir, height, canonical_hash)?
        {
            return Ok(Some((header, archive)));
        }

        if let Some(block) = self.get_block(block_height) {
            if block.header().sccp_commitment_root().is_none() {
                return Ok(None);
            }
        } else {
            let finality_dir = Self::v2_finality_artifact_dir_for(&blocks_dir);
            let finality_path = Self::v2_finality_artifact_path_for(&blocks_dir, height);
            if let Some((record, _)) =
                self.decode_v2_finality_record_at(&finality_path, &finality_dir)?
            {
                Self::validate_v2_finality_record_at(
                    &finality_path,
                    height,
                    canonical_hash,
                    &record,
                )?;
                if record.block_header.sccp_commitment_root().is_none() {
                    return Ok(None);
                }
            }
        }
        Err(Error::MissingRetainedBlockRecord { height })
    }

    /// Inventory nonempty retained SCCP archives through an exact committed-height boundary.
    ///
    /// Selected records are decoded one at a time, bound to Kura's canonical hash journal, and
    /// fully archive-validated. The result retains only fixed-size summaries, so canonical SCCP
    /// payloads are never accumulated or duplicated across heights. Valid rootless/empty retained
    /// records are deliberately omitted. Retained suffix records above `committed_height` are not
    /// decoded and cannot leak a Kura-ahead-of-WSV suffix into snapshot validation.
    pub(crate) fn retained_nonempty_sccp_archive_inventory_at_or_below(
        &self,
        committed_height: u64,
    ) -> Result<Vec<RetainedSccpArchiveSummary>> {
        self.ensure_canonical_storage_not_poisoned()?;
        if committed_height == 0 {
            return Ok(Vec::new());
        }
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let blocks_dir = self.active_blocks_dir.lock().clone();
        // The directory can legitimately contain an immutable finalized suffix above the WSV
        // boundary selected by snapshot rollback validation. Bound directory enumeration by the
        // durable canonical chain, then decode only the selected prefix below. Using the WSV
        // boundary as the inventory bound would reject that valid suffix before `take_while` can
        // exclude it.
        let durable_height = self.block_store.lock().read_durable_index_count()?;
        let heights =
            Self::retained_block_record_heights_for(&self.store_root, &blocks_dir, durable_height)?;
        if let Some(retained_height) = heights.last().copied()
            && retained_height > durable_height
        {
            return Err(Error::RetainedBlockBeyondDurableChain {
                retained_height,
                durable_height,
            });
        }
        let mut summaries = Vec::new();
        for height in heights
            .into_iter()
            .take_while(|height| *height <= committed_height)
        {
            let block_height = NonZeroUsize::new(usize::try_from(height)?)
                .ok_or(Error::MissingRetainedBlockRecord { height })?;
            let canonical_hash = self
                .get_durable_block_hash(block_height)
                .ok_or(Error::MissingRetainedBlockRecord { height })?;
            let (header, _, _, _, archive) = self
                .retained_block_record_at(&blocks_dir, height, canonical_hash)?
                .ok_or(Error::MissingRetainedBlockRecord { height })?;
            if archive.is_empty() {
                continue;
            }
            summaries.push(RetainedSccpArchiveSummary {
                height,
                block_hash: header.hash(),
                message_count: u32::try_from(archive.len())?,
            });
        }
        Ok(summaries)
    }

    fn validate_retained_block_inventory_on_startup(&self) -> Result<()> {
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let blocks_dir = self.active_blocks_dir.lock().clone();
        let (durable_height, indices, hashes) = {
            let mut block_store = self.block_store.lock();
            let durable_height = usize::try_from(block_store.read_durable_index_count()?)?;
            let mut indices = vec![BlockIndex::default(); durable_height];
            block_store.read_block_indices(0, &mut indices)?;
            let hashes = block_store.read_block_hashes(0, durable_height)?;
            (durable_height, indices, hashes)
        };
        let durable_height_u64 = u64::try_from(durable_height)?;
        let boundary = ExactReplayBoundary {
            count: durable_height_u64,
            hashes: hashes.clone(),
        };
        let canonical_storage = self.canonical_block_store_metadata(&blocks_dir)?;
        let reuse_startup_validation = {
            let inventory = self.v2_startup_finality_verification_inventory.lock();
            inventory.as_ref().is_some_and(|inventory| {
                inventory.boundary == boundary
                    && Self::canonical_block_store_metadata_unchanged(
                        &inventory.canonical_storage,
                        &canonical_storage,
                    )
            })
        };
        let retained_heights = Self::retained_block_record_heights_for(
            &self.store_root,
            &blocks_dir,
            durable_height_u64,
        )?;
        if let Some(retained_height) = retained_heights.last().copied()
            && retained_height > durable_height_u64
        {
            return Err(Error::RetainedBlockBeyondDurableChain {
                retained_height,
                durable_height: durable_height_u64,
            });
        }
        let retained_height_set = retained_heights.iter().copied().collect::<BTreeSet<_>>();
        for height in retained_heights {
            let index = usize::try_from(height.saturating_sub(1))?;
            let canonical_hash = hashes[index];
            let bodyless = self
                .retained_block_record_at_with_identity(&blocks_dir, height, canonical_hash, false)?
                .ok_or(Error::MissingRetainedBlockRecord { height })?;
            if bodyless.0.2 != indices[index].length {
                return Err(Error::V2FinalityExecutedBlockWireLengthMismatch { height });
            }
            if indices[index].is_evicted()
                || !reuse_startup_validation
                || !self.v2_startup_retained_entry_matches(height, &bodyless.1)
            {
                self.retained_block_record_at(&blocks_dir, height, canonical_hash)?
                    .ok_or(Error::MissingRetainedBlockRecord { height })?;
            }
        }
        for (index, block_index) in indices.iter().enumerate() {
            if block_index.is_evicted() && block_index.length > 0 {
                let height = u64::try_from(index)?.saturating_add(1);
                if !retained_height_set.contains(&height) {
                    return Err(Error::MissingRetainedBlockRecord { height });
                }
            }
        }
        let finalized_heights = Self::v2_finality_artifact_heights_for(
            &self.store_root,
            &blocks_dir,
            durable_height_u64,
        )?;
        let finalized_height_set = finalized_heights.iter().copied().collect::<BTreeSet<_>>();
        for (index, block_index) in indices.iter().enumerate() {
            if block_index.is_evicted() && block_index.length > 0 {
                let height = u64::try_from(index)?.saturating_add(1);
                if !finalized_height_set.contains(&height) {
                    return Err(Error::MissingV2FinalityArtifact { height });
                }
            }
        }
        for height in finalized_heights {
            if !retained_height_set.contains(&height) {
                return Err(Error::MissingRetainedBlockRecord { height });
            }
        }
        if reuse_startup_validation {
            let after_boundary = self.exact_replay_boundary()?;
            let after_storage = self.canonical_block_store_metadata(&blocks_dir)?;
            if after_boundary != boundary
                || !Self::canonical_block_store_metadata_unchanged(
                    &canonical_storage,
                    &after_storage,
                )
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "canonical block storage changed during retained startup validation",
                    ),
                    blocks_dir,
                ));
            }
        }
        Ok(())
    }

    fn prune_retained_block_records_from(
        &self,
        blocks_dir: &Path,
        first_removed_height: u64,
    ) -> Result<()> {
        let authority = StartupRecoveryMutationAuthority::Authenticated;
        self.prune_retained_block_records_from_with_authority(
            blocks_dir,
            first_removed_height,
            &authority,
        )
    }

    fn prune_retained_block_records_from_during_snapshot_finalization(
        &self,
        blocks_dir: &Path,
        first_removed_height: u64,
        authority: &SnapshotFinalizationMutationAuthority<'_>,
    ) -> Result<()> {
        let authority = StartupRecoveryMutationAuthority::SnapshotFinalization(authority);
        self.prune_retained_block_records_from_with_authority(
            blocks_dir,
            first_removed_height,
            &authority,
        )
    }

    fn prune_retained_block_records_from_with_authority(
        &self,
        blocks_dir: &Path,
        first_removed_height: u64,
        authority: &StartupRecoveryMutationAuthority<'_>,
    ) -> Result<()> {
        authority.validate_for(self)?;
        let directory = Self::retained_block_record_dir_for(blocks_dir);
        let durable_height = self.block_store.lock().read_durable_index_count()?;
        let heights =
            Self::retained_block_record_heights_for(&self.store_root, blocks_dir, durable_height)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let mut removed = false;
        let mut removed_bytes = 0_u64;
        for height in heights
            .into_iter()
            .filter(|height| *height >= first_removed_height)
        {
            let path = Self::retained_block_record_path_for(blocks_dir, height);
            self.regular_sidecar_metadata(&path, &directory)?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            "retained block sidecar disappeared during canonical prune",
                        ),
                        path.clone(),
                    )
                })?;
            removed_bytes = removed_bytes.saturating_add(Self::file_len_or_zero(&path)?);
            authority.validate_for(self)?;
            std::fs::remove_file(&path).map_err(|error| Error::IO(error, path))?;
            removed = true;
        }
        if removed {
            authority.validate_for(self)?;
            sync_dir(&directory).map_err(|error| Error::IO(error, directory))?;
            self.sub_total_disk_usage_bytes(removed_bytes);
        }
        accounting_mutation.finish();
        Ok(())
    }

    fn staged_retained_block_record(
        &self,
        stage: &StagedRetainedBlockRewrite,
        entry: &StagedRetainedBlockRewriteEntry,
    ) -> Result<KuraRetainedBlockRecord> {
        let directory = Self::retained_block_rewrite_staging_dir_for(&stage.blocks_dir);
        let path = Self::retained_block_rewrite_staging_path_for(&stage.blocks_dir, entry.height);
        let Some(snapshot) =
            self.read_regular_sidecar_snapshot(&path, &directory, MAX_RETAINED_BLOCK_RECORD_BYTES)?
        else {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::NotFound,
                    "staged retained block record disappeared during canonical rewrite",
                ),
                path,
            ));
        };
        if snapshot.bytes_hash != entry.bytes_hash
            || u64::try_from(snapshot.bytes.len())? != entry.bytes_len
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "staged retained block record changed during canonical rewrite",
                ),
                path,
            ));
        }
        let mut input = snapshot.bytes.as_slice();
        let record = KuraRetainedBlockRecord::decode_all(&mut input).map_err(Error::NoritoFrame)?;
        if record.encode() != snapshot.bytes {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "staged retained block record is not canonically encoded",
                ),
                path,
            ));
        }
        let _ = Self::validate_retained_block_record_at(
            &path,
            entry.height,
            entry.block_hash,
            &record,
        )?;
        Ok(record)
    }

    fn stage_retained_block_records_for_rewrite(
        &self,
        blocks_dir: &Path,
        first_rewritten_height: u64,
    ) -> Result<Option<StagedRetainedBlockRewrite>> {
        let retained_directory = Self::retained_block_record_dir_for(blocks_dir);
        let staging_directory = Self::retained_block_rewrite_staging_dir_for(blocks_dir);
        if std::fs::symlink_metadata(&staging_directory).is_ok() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::AlreadyExists,
                    "an unresolved retained-block rewrite stage already exists",
                ),
                staging_directory,
            ));
        }

        let durable_height = self.block_store.lock().read_durable_index_count()?;
        let heights =
            Self::retained_block_record_heights_for(&self.store_root, blocks_dir, durable_height)?;
        let mut entries = Vec::new();
        let mut removed_total_bytes = 0_u64;
        for height in heights
            .into_iter()
            .filter(|height| *height >= first_rewritten_height)
        {
            let block_height = NonZeroUsize::new(usize::try_from(height)?)
                .ok_or(Error::MissingRetainedBlockRecord { height })?;
            let canonical_hash = self
                .get_durable_block_hash(block_height)
                .ok_or(Error::MissingRetainedBlockRecord { height })?;
            let path = Self::retained_block_record_path_for(blocks_dir, height);
            let Some(record) = self.decode_retained_block_record_at(&path, &retained_directory)?
            else {
                return Err(Error::MissingRetainedBlockRecord { height });
            };
            let _ =
                Self::validate_retained_block_record_at(&path, height, canonical_hash, &record)?;
            let bytes = record.encode();
            let bytes_len = u64::try_from(bytes.len())?;
            removed_total_bytes = removed_total_bytes.saturating_add(bytes_len);
            entries.push(StagedRetainedBlockRewriteEntry {
                height,
                block_hash: canonical_hash,
                bytes_hash: Hash::new(&bytes),
                bytes_len,
            });
        }
        if entries.is_empty() {
            return Ok(None);
        }

        create_dir_all_with_context(&staging_directory)?;
        if let Some(parent) = staging_directory.parent() {
            sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        }
        let _ = self
            .canonical_sidecar_directory(&staging_directory)?
            .ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::NotFound,
                        "retained-block rewrite staging directory disappeared",
                    ),
                    staging_directory.clone(),
                )
            })?;

        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let mut moved = 0_usize;
        let move_result = (|| -> Result<()> {
            for entry in &entries {
                let source = Self::retained_block_record_path_for(blocks_dir, entry.height);
                let target =
                    Self::retained_block_rewrite_staging_path_for(blocks_dir, entry.height);
                if std::fs::symlink_metadata(&target).is_ok() {
                    return Err(Error::IO(
                        std::io::Error::new(
                            ErrorKind::AlreadyExists,
                            "retained-block rewrite staging destination already exists",
                        ),
                        target,
                    ));
                }
                std::fs::rename(&source, &target)
                    .map_err(|error| Error::IO(error, source.clone()))?;
                moved = moved.saturating_add(1);
            }
            sync_dir(&retained_directory)
                .map_err(|error| Error::IO(error, retained_directory.clone()))?;
            sync_dir(&staging_directory)
                .map_err(|error| Error::IO(error, staging_directory.clone()))?;
            Ok(())
        })();
        if let Err(error) = move_result {
            for entry in entries.iter().take(moved).rev() {
                let source =
                    Self::retained_block_rewrite_staging_path_for(blocks_dir, entry.height);
                let target = Self::retained_block_record_path_for(blocks_dir, entry.height);
                if let Err(restore_error) = std::fs::rename(&source, &target) {
                    return Err(Error::IO(restore_error, source));
                }
            }
            let _ = sync_dir(&retained_directory);
            let _ = sync_dir(&staging_directory);
            let _ = std::fs::remove_dir(&staging_directory);
            accounting_mutation.finish();
            return Err(error);
        }
        accounting_mutation.finish();
        Ok(Some(StagedRetainedBlockRewrite {
            blocks_dir: blocks_dir.to_path_buf(),
            entries,
            removed_total_bytes,
        }))
    }

    fn reconcile_staged_retained_block_rewrite_after_error(
        &self,
        stage: &StagedRetainedBlockRewrite,
    ) -> Result<()> {
        let retained_directory = Self::retained_block_record_dir_for(&stage.blocks_dir);
        let staging_directory = Self::retained_block_rewrite_staging_dir_for(&stage.blocks_dir);
        let mut restore = Vec::with_capacity(stage.entries.len());
        let mut removed_total_bytes = 0_u64;
        for entry in &stage.entries {
            let record = self.staged_retained_block_record(stage, entry)?;
            let canonical_hash = NonZeroUsize::new(usize::try_from(entry.height)?)
                .and_then(|height| self.get_durable_block_hash(height));
            let destination = Self::retained_block_record_path_for(&stage.blocks_dir, entry.height);
            if canonical_hash == Some(entry.block_hash) {
                if let Some(existing) =
                    self.decode_retained_block_record_at(&destination, &retained_directory)?
                {
                    let _ = Self::validate_retained_block_record_at(
                        &destination,
                        entry.height,
                        entry.block_hash,
                        &existing,
                    )?;
                    if existing != record {
                        return Err(Error::ConflictingRetainedBlockRecord {
                            height: entry.height,
                        });
                    }
                    restore.push(false);
                    removed_total_bytes = removed_total_bytes.saturating_add(entry.bytes_len);
                } else {
                    restore.push(true);
                }
            } else {
                restore.push(false);
                removed_total_bytes = removed_total_bytes.saturating_add(entry.bytes_len);
            }
        }

        let accounting_mutation = self.begin_total_disk_usage_mutation();
        for (entry, restore) in stage.entries.iter().zip(restore) {
            let source =
                Self::retained_block_rewrite_staging_path_for(&stage.blocks_dir, entry.height);
            if restore {
                let destination =
                    Self::retained_block_record_path_for(&stage.blocks_dir, entry.height);
                std::fs::rename(&source, &destination)
                    .map_err(|error| Error::IO(error, source.clone()))?;
            } else {
                std::fs::remove_file(&source).map_err(|error| Error::IO(error, source.clone()))?;
            }
        }
        sync_dir(&retained_directory)
            .map_err(|error| Error::IO(error, retained_directory.clone()))?;
        sync_dir(&staging_directory)
            .map_err(|error| Error::IO(error, staging_directory.clone()))?;
        std::fs::remove_dir(&staging_directory)
            .map_err(|error| Error::IO(error, staging_directory.clone()))?;
        if let Some(parent) = staging_directory.parent() {
            sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        }
        self.sub_total_disk_usage_bytes(removed_total_bytes);
        accounting_mutation.finish();
        Ok(())
    }

    fn discard_staged_retained_block_rewrite(
        &self,
        stage: &StagedRetainedBlockRewrite,
    ) -> Result<()> {
        let staging_directory = Self::retained_block_rewrite_staging_dir_for(&stage.blocks_dir);
        for entry in &stage.entries {
            let _ = self.staged_retained_block_record(stage, entry)?;
        }
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        #[cfg(test)]
        let fail_after = self
            .fail_retained_rewrite_discard_after
            .swap(usize::MAX, Ordering::AcqRel);
        for (_index, entry) in stage.entries.iter().enumerate() {
            let path =
                Self::retained_block_rewrite_staging_path_for(&stage.blocks_dir, entry.height);
            std::fs::remove_file(&path).map_err(|error| Error::IO(error, path))?;
            #[cfg(test)]
            if _index == fail_after {
                return Err(Error::IO(
                    std::io::Error::other("injected retained-rewrite discard failure"),
                    staging_directory,
                ));
            }
        }
        sync_dir(&staging_directory)
            .map_err(|error| Error::IO(error, staging_directory.clone()))?;
        std::fs::remove_dir(&staging_directory)
            .map_err(|error| Error::IO(error, staging_directory.clone()))?;
        if let Some(parent) = staging_directory.parent() {
            sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        }
        self.sub_total_disk_usage_bytes(stage.removed_total_bytes);
        accounting_mutation.finish();
        Ok(())
    }

    fn with_retained_block_records_staged_for_rewrite<T>(
        &self,
        blocks_dir: &Path,
        first_rewritten_height: u64,
        mutation: impl FnOnce() -> Result<T>,
    ) -> Result<RetainedBlockRewritePublication<T>> {
        let Some(stage) =
            self.stage_retained_block_records_for_rewrite(blocks_dir, first_rewritten_height)?
        else {
            return mutation().map(RetainedBlockRewritePublication::Complete);
        };
        match mutation() {
            Ok(output) => match self.discard_staged_retained_block_rewrite(&stage) {
                Ok(()) => Ok(RetainedBlockRewritePublication::Complete(output)),
                Err(discard_error) => {
                    warn!(
                        ?discard_error,
                        "retained-record cleanup failed after canonical rewrite publication; attempting immediate recovery"
                    );
                    match self.recover_retained_block_rewrite_stage_on_startup(blocks_dir) {
                        Ok(()) => {
                            warn!(
                                ?discard_error,
                                "recovered retained-record cleanup after canonical rewrite publication"
                            );
                            Ok(RetainedBlockRewritePublication::Complete(output))
                        }
                        Err(cleanup_error) => {
                            error!(
                                ?discard_error,
                                ?cleanup_error,
                                "canonical rewrite is committed but retained-record cleanup remains deferred"
                            );
                            Ok(
                                RetainedBlockRewritePublication::CommittedWithDeferredCleanup {
                                    cleanup_error,
                                },
                            )
                        }
                    }
                }
            },
            Err(
                error @ (Error::DaBlockRewriteCommitStateUnknown { .. }
                | Error::CanonicalBlockCommittedRecoveryRequired { .. }
                | Error::CanonicalStoragePoisoned),
            ) => {
                // The DA write-ahead stage is the only authority that can decide whether the old
                // or replacement canonical suffix won. Keep the retained-record stage intact so
                // startup can resolve DA first and only then restore/discard retained evidence.
                Err(error)
            }
            Err(error) => {
                self.reconcile_staged_retained_block_rewrite_after_error(&stage)?;
                Err(error)
            }
        }
    }

    fn resolve_retained_block_rewrite_stage_before_canonical_mutation(
        &self,
        blocks_dir: &Path,
    ) -> Result<()> {
        let staging_directory = Self::retained_block_rewrite_staging_dir_for(blocks_dir);
        match std::fs::symlink_metadata(&staging_directory) {
            Ok(_) => self.recover_retained_block_rewrite_stage_on_startup(blocks_dir),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::IO(error, staging_directory)),
        }
    }

    fn recover_retained_block_rewrite_stage_on_startup(&self, blocks_dir: &Path) -> Result<()> {
        let authority = StartupRecoveryMutationAuthority::Authenticated;
        self.recover_retained_block_rewrite_stage_with_authority(blocks_dir, &authority)
    }

    fn recover_retained_block_rewrite_stage_during_snapshot_finalization(
        &self,
        blocks_dir: &Path,
        authority: &SnapshotFinalizationMutationAuthority<'_>,
    ) -> Result<()> {
        let authority = StartupRecoveryMutationAuthority::SnapshotFinalization(authority);
        self.recover_retained_block_rewrite_stage_with_authority(blocks_dir, &authority)
    }

    fn recover_retained_block_rewrite_stage_with_authority(
        &self,
        blocks_dir: &Path,
        authority: &StartupRecoveryMutationAuthority<'_>,
    ) -> Result<()> {
        authority.validate_for(self)?;
        let staging_directory = Self::retained_block_rewrite_staging_dir_for(blocks_dir);
        #[cfg(test)]
        if self
            .fail_next_retained_rewrite_recovery
            .swap(false, Ordering::AcqRel)
        {
            return Err(Error::IO(
                std::io::Error::other("injected retained-rewrite recovery failure"),
                staging_directory,
            ));
        }
        let durable_height = self.block_store.lock().read_durable_index_count()?;
        let heights = Self::canonical_height_sidecar_heights_for(
            &self.store_root,
            &staging_directory,
            "retained block rewrite staging sidecar",
            durable_height,
        )?;
        if heights.is_empty() {
            if staging_directory.exists() {
                authority.validate_for(self)?;
                let _accounting_mutation = self.begin_total_disk_usage_mutation();
                std::fs::remove_dir(&staging_directory)
                    .map_err(|error| Error::IO(error, staging_directory.clone()))?;
                if let Some(parent) = staging_directory.parent() {
                    sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
                }
            }
            return Ok(());
        }
        authority.validate_for(self)?;
        let _accounting_mutation = self.begin_total_disk_usage_mutation();
        let retained_directory = Self::retained_block_record_dir_for(blocks_dir);
        create_dir_all_with_context(&retained_directory)?;
        for height in heights {
            let path = Self::retained_block_rewrite_staging_path_for(blocks_dir, height);
            let Some(record) = self.decode_retained_block_record_at(&path, &staging_directory)?
            else {
                return Err(Error::MissingRetainedBlockRecord { height });
            };
            let _ =
                Self::validate_retained_block_record_at(&path, height, record.block_hash, &record)?;
            let canonical_hash = NonZeroUsize::new(usize::try_from(height)?)
                .and_then(|height| self.get_durable_block_hash(height));
            let destination = Self::retained_block_record_path_for(blocks_dir, height);
            authority.validate_for(self)?;
            if canonical_hash == Some(record.block_hash) {
                if let Some(existing) =
                    self.decode_retained_block_record_at(&destination, &retained_directory)?
                {
                    let _ = Self::validate_retained_block_record_at(
                        &destination,
                        height,
                        record.block_hash,
                        &existing,
                    )?;
                    if existing != record {
                        return Err(Error::ConflictingRetainedBlockRecord { height });
                    }
                    authority.validate_for(self)?;
                    std::fs::remove_file(&path).map_err(|error| Error::IO(error, path.clone()))?;
                } else {
                    authority.validate_for(self)?;
                    std::fs::rename(&path, &destination)
                        .map_err(|error| Error::IO(error, path.clone()))?;
                }
            } else {
                authority.validate_for(self)?;
                std::fs::remove_file(&path).map_err(|error| Error::IO(error, path.clone()))?;
            }
        }
        sync_dir(&retained_directory)
            .map_err(|error| Error::IO(error, retained_directory.clone()))?;
        sync_dir(&staging_directory)
            .map_err(|error| Error::IO(error, staging_directory.clone()))?;
        authority.validate_for(self)?;
        std::fs::remove_dir(&staging_directory)
            .map_err(|error| Error::IO(error, staging_directory.clone()))?;
        if let Some(parent) = staging_directory.parent() {
            sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
        }
        Ok(())
    }

    fn v2_finality_artifact_dir_for(blocks_dir: &Path) -> PathBuf {
        blocks_dir.join(V2_FINALITY_ARTIFACTS_DIR_NAME)
    }

    fn v2_finality_artifact_path_for(blocks_dir: &Path, height: u64) -> PathBuf {
        Self::v2_finality_artifact_dir_for(blocks_dir).join(format!("{height:020}.norito"))
    }

    fn v2_finality_artifact_dir(&self) -> PathBuf {
        Self::v2_finality_artifact_dir_for(&self.active_blocks_dir.lock())
    }

    fn v2_finality_artifact_path(&self, height: u64) -> PathBuf {
        Self::v2_finality_artifact_path_for(&self.active_blocks_dir.lock(), height)
    }

    fn v2_finality_artifact_heights_for(
        store_root: &Path,
        blocks_dir: &Path,
        durable_height_bound: u64,
    ) -> Result<Vec<u64>> {
        let directory = Self::v2_finality_artifact_dir_for(blocks_dir);
        Self::canonical_height_sidecar_heights_for(
            store_root,
            &directory,
            "v2 finality artifact",
            durable_height_bound,
        )
    }

    fn highest_v2_finality_artifact_height_for(
        store_root: &Path,
        blocks_dir: &Path,
        durable_height_bound: u64,
    ) -> Result<Option<u64>> {
        Ok(
            Self::v2_finality_artifact_heights_for(store_root, blocks_dir, durable_height_bound)?
                .last()
                .copied(),
        )
    }

    fn highest_v2_finality_artifact_height(&self, blocks_dir: &Path) -> Result<Option<u64>> {
        let durable_height = self.block_store.lock().read_durable_index_count()?;
        Self::highest_v2_finality_artifact_height_for(&self.store_root, blocks_dir, durable_height)
    }

    fn ensure_v2_finality_allows_rewrite_from(
        &self,
        blocks_dir: &Path,
        rewrite_from_height: u64,
    ) -> Result<()> {
        if let Some(finalized_height) = self.highest_v2_finality_artifact_height(blocks_dir)?
            && finalized_height >= rewrite_from_height
        {
            return Err(Error::FinalizedV2BlockMutation {
                rewrite_from_height,
                finalized_height,
            });
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

    fn deterministic_kura_replica_keepers(
        &self,
        artifact: &V2FinalityArtifact,
    ) -> Vec<(u32, PeerId)> {
        let validator_count = artifact.height_context.roster.len();
        let Ok(quorum_count) = usize::try_from(artifact.height_context.quorum.min_signers) else {
            return Vec::new();
        };
        let Some(fault_bound) = validator_count.checked_sub(quorum_count) else {
            return Vec::new();
        };
        let required = fault_bound
            .saturating_add(1)
            .max(self.eviction_required_replicas.get());
        if required == 0 || required > artifact.commit_qc.signers.len() {
            return Vec::new();
        }

        let finality_artifact_hash = HashOf::new(artifact);
        let mut candidates = Vec::with_capacity(artifact.commit_qc.signers.len());
        for &signer_index in &artifact.commit_qc.signers {
            let Ok(index) = usize::try_from(signer_index) else {
                return Vec::new();
            };
            let Some(signer) = artifact.height_context.roster.get(index) else {
                return Vec::new();
            };
            let score = Hash::new(
                KuraReplicaKeeperScoreV1 {
                    domain: KURA_REPLICA_KEEPER_SELECTION_DOMAIN_V1.to_vec(),
                    chain_id: artifact.height_context.chain_id.clone(),
                    context_id: artifact.context_id(),
                    height: artifact.height,
                    block_hash: artifact.block_hash,
                    finality_artifact_hash,
                    signer_index,
                    signer: signer.validator.clone(),
                }
                .encode(),
            );
            candidates.push((score, signer_index, signer.validator.clone()));
        }
        candidates.sort_by(|left, right| {
            left.0
                .as_ref()
                .cmp(right.0.as_ref())
                .then_with(|| left.1.cmp(&right.1))
        });
        candidates
            .into_iter()
            .take(required)
            .map(|(_, index, peer)| (index, peer))
            .collect()
    }

    /// Return the exact cryptographically verified finality, complete-wire
    /// identity, and deterministic CommitQC keeper set required for eviction.
    fn verified_kura_replica_authority_for_eviction(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
    ) -> Result<Option<VerifiedKuraReplicaAuthority>> {
        let directory = Self::v2_finality_artifact_dir_for(blocks_dir);
        let path = Self::v2_finality_artifact_path_for(blocks_dir, height);
        let Some((record, read_identity)) = self.decode_v2_finality_record_at(&path, &directory)?
        else {
            return Ok(None);
        };
        Self::validate_v2_finality_record_at(&path, height, canonical_hash, &record)?;
        // `get_block` calls this while holding the block-store guard. Validate the immutable
        // retained record directly here; recursively asking `get_block` to compare the live body
        // would deadlock on a cold read of an evicted height. The caller compares any available
        // body/DA bytes with the returned signed complete-wire hash before exposing them.
        let Some((
            retained_header,
            proposal_wire_hash,
            executed_block_wire_len,
            executed_block_wire_hash,
            _,
        )) = self.retained_block_record_at_without_live_body(blocks_dir, height, canonical_hash)?
        else {
            return Err(Error::MissingRetainedBlockRecord { height });
        };
        if retained_header != record.block_header {
            return Err(Error::ConflictingRetainedBlockRecord { height });
        }
        Self::validate_v2_finality_wire_bindings(
            height,
            &record.artifact,
            proposal_wire_hash,
            executed_block_wire_len,
            executed_block_wire_hash,
        )?;
        self.verify_v2_finality_artifact_at(&path, &directory, &record.artifact, &read_identity)?;
        let key = BlockReplicaKey {
            height,
            block_hash: canonical_hash,
            finality_artifact_hash: HashOf::new(&record.artifact),
            executed_block_wire_len,
            executed_block_wire_hash,
        };
        let selected_keepers = self.deterministic_kura_replica_keepers(&record.artifact);
        Ok(Some(VerifiedKuraReplicaAuthority {
            key,
            chain_id: record.artifact.height_context.chain_id.clone(),
            selected_keepers,
        }))
    }

    /// Return the independently signed complete-wire length and hash required before body eviction.
    fn verified_v2_finality_wire_hash_for_eviction(
        &self,
        blocks_dir: &Path,
        height: u64,
        canonical_hash: HashOf<BlockHeader>,
    ) -> Result<Option<(u64, Hash)>> {
        Ok(self
            .verified_kura_replica_authority_for_eviction(blocks_dir, height, canonical_hash)?
            .map(|authority| {
                (
                    authority.key.executed_block_wire_len,
                    authority.key.executed_block_wire_hash,
                )
            }))
    }
}
