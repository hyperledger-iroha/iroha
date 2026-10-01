//! Deliberate local-certificate corruption after genuine native publication, for rejection tests.

use super::*;

impl Kura {
    /// Corrupt a published block's local certificate for fail-closed read tests.
    /// `Some` replaces only QC bytes, preserving its header, result preimage and signed
    /// availability; `None` removes the certificate. Executed bytes, hash journals and native
    /// execution authority remain unchanged. This never publishes or executes another block.
    ///
    /// # Errors
    /// The original block/certificate is missing, publication changes concurrently, or storage I/O
    /// fails. Use only an exclusively owned test fixture, not a running node.
    #[doc(hidden)]
    pub fn corrupt_commit_certificate_for_testing(
        &self,
        height: NonZeroUsize,
        commit_qc: Option<Vec<u8>>,
    ) -> Result<()> {
        self.corrupt_certificate_parts_for_testing(height, |certificate| {
            commit_qc.map(|commit_qc| {
                iroha_data_model::block::CommitCertificate::from_untrusted_parts(
                    certificate.consensus_header().to_vec(),
                    commit_qc,
                    certificate.result_preimage().to_vec(),
                    certificate.availability().to_vec(),
                )
            })
        })
    }

    /// Replace only the local result preimage after genuine native publication.
    /// The original execution, signed header, QC and availability remain unchanged.
    /// This deliberately corrupts certificate storage to exercise exact-result rejection.
    ///
    /// # Errors
    /// Returns an error if the published block/certificate is absent, publication changes
    /// concurrently, or storage I/O fails. Use only an exclusively owned test fixture.
    #[doc(hidden)]
    pub fn corrupt_commit_result_for_testing(
        &self,
        height: NonZeroUsize,
        result_preimage: Vec<u8>,
    ) -> Result<()> {
        self.corrupt_certificate_parts_for_testing(height, |certificate| {
            Some(
                iroha_data_model::block::CommitCertificate::from_untrusted_parts(
                    certificate.consensus_header().to_vec(),
                    certificate.commit_qc().to_vec(),
                    result_preimage,
                    certificate.availability().to_vec(),
                ),
            )
        })
    }

    fn corrupt_certificate_parts_for_testing(
        &self,
        height: NonZeroUsize,
        change: impl FnOnce(
            &iroha_data_model::block::CommitCertificate,
        ) -> Option<iroha_data_model::block::CommitCertificate>,
    ) -> Result<()> {
        let original = self.get_block(height).ok_or(Error::OutOfBoundsBlockRead {
            start_block_height: u64::try_from(height.get())?,
            block_count: self.blocks_count(),
        })?;
        let height_u64 = u64::try_from(height.get())?;
        let certificate = original
            .commit_certificate()
            .ok_or(Error::CanonicalBlockWireMismatch { height: height_u64 })?;
        let changed = original
            .as_ref()
            .clone()
            .with_commit_certificate(change(certificate));
        let wire = changed.encode_wire().map_err(Error::NoritoFrame)?;
        let _prune = self.prune_lock.lock();
        let _canonical = self.canonical_chain_lock.lock();
        let _write = self.lock_block_store_for_write();
        let mut data = self.block_data.lock();
        let index = height.get() - 1;
        let slot = data
            .get_mut(index)
            .ok_or(Error::CanonicalBlockWireMismatch { height: height_u64 })?;
        if slot.0 != original.hash() {
            return Err(Error::CanonicalBlockWireMismatch { height: height_u64 });
        }
        let mut store = self.block_store.lock();
        let start = store.data_file_len()?;
        // Append a separate physical image so corruption at an old height cannot overwrite a
        // neighboring canonical frame. The native hash/count journals are deliberately untouched.
        store.invalidate_data_mmap();
        store.ensure_data_file()?.try_io(|file| {
            file.seek(SeekFrom::Start(start))?;
            file.write_all(&wire)
        })?;
        store.schedule_fsync_after_write()?;
        store.write_block_index(u64::try_from(index)?, start, u64::try_from(wire.len())?)?;
        store.flush_pending_fsync(true)?;
        slot.1 = None;
        Ok(())
    }
}
