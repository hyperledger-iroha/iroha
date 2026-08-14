impl Kura {
    /// Read a WSV checkpoint while the caller holds `sidecar_lock`.
    fn wsv_checkpoint_under_sidecar_guard(&self, height: u64) -> Result<Option<WsvCheckpoint>> {
        self.ensure_prune_recovery_not_required()?;
        self.record_startup_replay_historical_payload_read();
        let path = self.wsv_checkpoint_path(height);
        let checkpoint = Self::decode_wsv_checkpoint_at(&path)?;
        let Some(checkpoint) = checkpoint else {
            self.ensure_prune_recovery_not_required()?;
            return Ok(None);
        };
        if checkpoint.height != height {
            return Err(Error::NoritoFrame(norito::core::Error::Message(format!(
                "WSV checkpoint height mismatch: expected {height}, got {}",
                checkpoint.height
            ))));
        }
        let Some(block_height) = NonZeroUsize::new(usize::try_from(height)?) else {
            return Err(Error::NoritoFrame(norito::core::Error::Message(
                "WSV checkpoint height must be non-zero".into(),
            )));
        };
        let durable_hash = self.get_durable_block_hash(block_height);
        self.ensure_prune_recovery_not_required()?;
        let Some(durable_hash) = durable_hash else {
            return Err(Error::BlockHeightGap {
                expected_next_height: u64::try_from(self.exact_durable_blocks_count()?)?
                    .saturating_add(1),
                actual_height: height,
            });
        };
        if checkpoint.block_hash != durable_hash {
            return Err(Error::BlockHeightConflict {
                height,
                expected: durable_hash,
                actual: checkpoint.block_hash,
            });
        }
        self.ensure_prune_recovery_not_required()?;
        Ok(Some(checkpoint))
    }
    fn decode_wsv_checkpoint_at(path: &Path) -> Result<Option<WsvCheckpoint>> {
        let Some(bytes) = Self::read_bounded_replay_sidecar_at(path, MAX_WSV_CHECKPOINT_BYTES)?
        else {
            return Ok(None);
        };
        let mut cursor = bytes.as_slice();
        WsvCheckpoint::decode_all(&mut cursor)
            .map(Some)
            .map_err(Error::NoritoFrame)
    }
    fn read_bounded_replay_sidecar_at(path: &Path, byte_limit: usize) -> Result<Option<Vec<u8>>> {
        let expected_directory = path.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "replay sidecar path has no parent directory",
                ),
                path.to_path_buf(),
            )
        })?;
        let blocks_root = expected_directory.parent().ok_or_else(|| {
            Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "replay sidecar directory has no Kura blocks root",
                ),
                expected_directory.to_path_buf(),
            )
        })?;
        Self::read_regular_sidecar_bytes_for(blocks_root, path, expected_directory, byte_limit)
    }
    fn ensure_sidecar_encoding_within_limit(
        path: &Path,
        kind: &str,
        bytes: &[u8],
        byte_limit: usize,
    ) -> Result<()> {
        if bytes.len() > byte_limit {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{kind} exceeds its {byte_limit}-byte hard limit"),
                ),
                path.to_path_buf(),
            ));
        }
        Ok(())
    }
}
