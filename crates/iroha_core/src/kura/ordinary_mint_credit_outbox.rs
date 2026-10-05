// Separate immutable ordinary Mint credit storage; legacy/OEM outbox is never reinterpreted.
impl Kura {
    fn ordinary_mint_credit_directory(&self) -> PathBuf {
        self.active_blocks_dir
            .lock()
            .join(KAGEMUSHA_ORDINARY_MINT_OUTBOX_DIR_NAME)
    }
    fn ordinary_mint_credit_path(&self, operation: [u8; 32]) -> PathBuf {
        self.ordinary_mint_credit_directory()
            .join(format!("{}.norito", hex::encode(operation)))
    }
    fn read_ordinary_mint_credit_snapshot(
        &self,
        path: &Path,
    ) -> Result<
        Option<(
            iroha_data_model::kagemusha::KagemushaOrdinaryFinalizedMintCreditOriginalV1,
            StableSidecarRead<Vec<u8>>,
        )>,
    > {
        let directory = self.ordinary_mint_credit_directory();
        let Some(snapshot) = self.read_regular_sidecar_snapshot(
            path,
            &directory,
            MAX_KAGEMUSHA_ORDINARY_MINT_OUTBOX_BYTES,
        )?
        else {
            return Ok(None);
        };
        let value = iroha_data_model::kagemusha::KagemushaOrdinaryFinalizedMintCreditOriginalV1::decode_canonical_exact(&snapshot.bytes)
            .map_err(Error::KagemushaMintOutbox)?;
        Ok(Some((value, snapshot)))
    }
    /// Retained data only. The actual Node publication reader reauthenticates native finality,
    /// historical Mint113 and both neutral MintAuthority proofs under its installed runtime.
    pub(crate) fn ordinary_mint_credit_original_v1(
        &self,
        operation: [u8; 32],
    ) -> Result<Option<iroha_data_model::kagemusha::KagemushaOrdinaryFinalizedMintCreditOriginalV1>>
    {
        if operation == [0; 32] {
            return Err(Error::KagemushaMintOutbox(
                "ordinary credit operation is zero".into(),
            ));
        }
        let path = self.ordinary_mint_credit_path(operation);
        let value = {
            let _guard = self.sidecar_lock.lock();
            self.read_ordinary_mint_credit_snapshot(&path)?
                .map(|(value, _)| value)
        };
        let Some(value) = value else {
            return Ok(None);
        };
        let finalized = iroha_data_model::kagemusha::KagemushaOrdinaryTopUpFinalizedOriginalV1::decode_canonical_exact(
            &value.finalized_source_original).map_err(Error::KagemushaMintOutbox)?;
        if finalized
            .finality
            .reserve_receipt_witness
            .receipt
            .operation_id
            != operation
        {
            return Err(Error::KagemushaMintOutbox(
                "ordinary credit path names another operation".into(),
            ));
        }
        Ok(Some(value))
    }
    /// Persist only a genuine generated credit from the same actual Node source and Kura owner.
    /// No raw result or serialized data can invoke this mutation path.
    pub(crate) fn store_ordinary_mint_credit_original_v1(
        &self,
        source: &crate::smartcontracts::isi::kagemusha::ordinary_mint_publication::KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'_>,
        generated: &crate::zk::kagemusha_v1_recursion::KagemushaGeneratedOrdinaryFinalizedMintCreditV1,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        source
            .recheck_retained_custody()
            .map_err(Error::KagemushaMintOutbox)?;
        if !std::ptr::eq(self, crate::state::StateReadOnly::kura(source.view())) {
            return Err(Error::KagemushaMintOutbox(
                "ordinary credit source belongs to another Kura".into(),
            ));
        }
        let finalized = source
            .finalized()
            .map_err(Error::KagemushaMintOutbox)?
            .canonical_bytes()
            .map_err(Error::KagemushaMintOutbox)?;
        let operation = source
            .record()
            .map_err(Error::KagemushaMintOutbox)?
            .operation_id;
        if generated.finalized_source_original_sha256()
            != <[u8; 32]>::from(<sha2::Sha256 as sha2::Digest>::digest(&finalized))
        {
            return Err(Error::KagemushaMintOutbox(
                "ordinary generated credit changes its full source original".into(),
            ));
        }
        let value = iroha_data_model::kagemusha::KagemushaOrdinaryFinalizedMintCreditOriginalV1 {
            version: 1,
            finalized_source_original: finalized,
            mint_credit_original: generated.credit_original().to_vec(),
        };
        let bytes = value
            .canonical_bytes()
            .map_err(Error::KagemushaMintOutbox)?;
        let directory = self.ordinary_mint_credit_directory();
        let path = self.ordinary_mint_credit_path(operation);
        let _guard = self.sidecar_lock.lock();
        create_dir_all_with_context(&directory)?;
        if let Some(parent) = directory.parent() {
            sync_dir(parent).map_err(|e| Error::IO(e, parent.to_path_buf()))?;
        }
        if let Some((existing, _)) = self.read_ordinary_mint_credit_snapshot(&path)? {
            if existing != value {
                return Err(Error::KagemushaMintOutbox(
                    "ordinary credit operation already owns different full originals".into(),
                ));
            }
            source
                .recheck_retained_custody()
                .map_err(Error::KagemushaMintOutbox)?;
            return Ok(());
        }
        let resources = self
            .begin_total_disk_usage_mutation()
            .with_resource_paths(vec![path.clone()]);
        if !self.write_atomic_synced_noclobber(&path, &bytes)? {
            let Some((existing, _)) = self.read_ordinary_mint_credit_snapshot(&path)? else {
                return Err(Error::KagemushaMintOutbox(
                    "ordinary credit missing after no-clobber publication".into(),
                ));
            };
            if existing != value {
                return Err(Error::KagemushaMintOutbox(
                    "ordinary credit no-clobber race changed original".into(),
                ));
            }
        }
        let Some((loaded, physical)) = self.read_ordinary_mint_credit_snapshot(&path)? else {
            return Err(Error::KagemushaMintOutbox(
                "ordinary credit missing after fsync".into(),
            ));
        };
        if loaded != value || physical.bytes != bytes || physical.bytes_hash != Hash::new(&bytes) {
            return Err(Error::KagemushaMintOutbox(
                "ordinary credit fsynced original changed during readback".into(),
            ));
        }
        resources.finish_resources_before_disk_rescan();
        // An error here cannot trigger replacement; the background owner freezes and cold
        // restart authenticates this exact immutable original before any proof production.
        source
            .recheck_retained_custody()
            .map_err(Error::KagemushaMintOutbox)
    }
}

// This cursor is a recovery hint only. Its decoder grants no checkpoint or epoch authority:
// the Node owner compares it to actual native execution and independently verifies the loaded
// recursive checkpoint under installed genesis before resuming. Restoring an older hint only
// replays already immutable checkpoint rows; a future or forged hint is rejected by that owner.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kura::KagemushaOrdinaryMintProgressV1")]
struct KagemushaOrdinaryMintProgressV1 {
    version: u16,
    release_id: [u8; 32],
    authorization: iroha_data_model::sumeragi::epoch::ValidatorEpochAuthorizationV1,
}
impl Kura {
    fn ordinary_mint_progress_path(&self, release: [u8; 32]) -> PathBuf {
        self.active_blocks_dir
            .lock()
            .join(KAGEMUSHA_ORDINARY_MINT_PROGRESS_DIR_NAME)
            .join(format!("{}.norito", hex::encode(release)))
    }
    fn read_ordinary_mint_progress(
        &self,
        path: &Path,
    ) -> Result<Option<KagemushaOrdinaryMintProgressV1>> {
        let directory = self
            .active_blocks_dir
            .lock()
            .join(KAGEMUSHA_ORDINARY_MINT_PROGRESS_DIR_NAME);
        let Some(snapshot) = self.read_regular_sidecar_snapshot(
            path,
            &directory,
            MAX_KAGEMUSHA_ORDINARY_MINT_PROGRESS_BYTES,
        )?
        else {
            return Ok(None);
        };
        let mut cursor = snapshot.bytes.as_slice();
        let value =
            KagemushaOrdinaryMintProgressV1::decode_all(&mut cursor).map_err(Error::NoritoFrame)?;
        if value.version != 1 || value.release_id == [0; 32] || value.encode() != snapshot.bytes {
            return Err(Error::KagemushaMintOutbox(
                "ordinary Mint progress hint is malformed".into(),
            ));
        }
        value
            .authorization
            .validate()
            .map_err(|e| Error::KagemushaMintOutbox(e.to_string()))?;
        Ok(Some(value))
    }
    pub(crate) fn ordinary_mint_checkpoint_progress_v1(
        &self,
        release: [u8; 32],
    ) -> Result<Option<iroha_data_model::sumeragi::epoch::ValidatorEpochAuthorizationV1>> {
        if release == [0; 32] {
            return Err(Error::KagemushaMintOutbox(
                "ordinary progress release is zero".into(),
            ));
        }
        let _guard = self.sidecar_lock.lock();
        let value = self.read_ordinary_mint_progress(&self.ordinary_mint_progress_path(release))?;
        if value.as_ref().is_some_and(|v| v.release_id != release) {
            return Err(Error::KagemushaMintOutbox(
                "ordinary progress path differs".into(),
            ));
        }
        Ok(value.map(|v| v.authorization))
    }
    pub(crate) fn store_ordinary_mint_checkpoint_progress_v1(
        &self,
        release: [u8; 32],
        authorization: &iroha_data_model::sumeragi::epoch::ValidatorEpochAuthorizationV1,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        authorization
            .validate()
            .map_err(|e| Error::KagemushaMintOutbox(e.to_string()))?;
        let value = KagemushaOrdinaryMintProgressV1 {
            version: 1,
            release_id: release,
            authorization: *authorization,
        };
        if release == [0; 32] {
            return Err(Error::KagemushaMintOutbox(
                "ordinary progress release is zero".into(),
            ));
        }
        let bytes = value.encode();
        if bytes.len() > MAX_KAGEMUSHA_ORDINARY_MINT_PROGRESS_BYTES {
            return Err(Error::KagemushaMintOutboxTooLarge {
                actual: bytes.len(),
                max: MAX_KAGEMUSHA_ORDINARY_MINT_PROGRESS_BYTES,
            });
        }
        let path = self.ordinary_mint_progress_path(release);
        let directory = path.parent().ok_or_else(|| {
            Error::KagemushaMintOutbox("ordinary progress directory absent".into())
        })?;
        let _guard = self.sidecar_lock.lock();
        create_dir_all_with_context(directory)?;
        if let Some(parent) = directory.parent() {
            sync_dir(parent).map_err(|e| Error::IO(e, parent.to_path_buf()))?;
        }
        if let Some(existing) = self.read_ordinary_mint_progress(&path)? {
            if existing.release_id != release
                || existing.authorization.network_id != authorization.network_id
            {
                return Err(Error::KagemushaMintOutbox(
                    "ordinary progress scope changes".into(),
                ));
            }
            if existing.authorization.epoch > authorization.epoch {
                return Ok(());
            }
            if existing.authorization.epoch == authorization.epoch {
                return if existing == value {
                    Ok(())
                } else {
                    Err(Error::KagemushaMintOutbox(
                        "ordinary progress same epoch changes".into(),
                    ))
                };
            }
        }
        let resources = self
            .begin_total_disk_usage_mutation()
            .with_resource_paths(vec![path.clone()]);
        self.write_atomic_synced_replace(&path, &bytes)?;
        if self.read_ordinary_mint_progress(&path)?.as_ref() != Some(&value) {
            return Err(Error::KagemushaMintOutbox(
                "ordinary progress fsynced bytes changed".into(),
            ));
        }
        resources.finish_resources_before_disk_rescan();
        Ok(())
    }
}
