impl Kura {
    fn pending_queue_plan_admission_inventory_paths_unlocked(&self) -> Result<Vec<PathBuf>> {
        let (_, merge_bytes) = self.pending_merge_entry_paths_unlocked()?;
        let (paths, admission_bytes) = self.pending_queue_plan_admission_paths_unlocked()?;
        if self
            .pending_control_sidecar_limits
            .combined_bytes_within_limit(merge_bytes, admission_bytes)
        {
            Ok(paths)
        } else {
            Err(Self::invalid_pending_queue_plan_admission_error(
                self.store_root.clone(),
                "pending merge and QueuePlan admission sidecars exceed their shared hard byte limit",
            ))
        }
    }

    /// Resolve one exact pending QueuePlan admission certificate by its byte hash.
    pub fn pending_queue_plan_admission_certificate(&self, hash: Hash) -> Result<Option<Vec<u8>>> {
        self.ensure_prune_recovery_not_required()?;
        let certificate = {
            let _guard = self.sidecar_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            #[cfg(test)]
            self.pending_queue_plan_admission_exact_reads
                .fetch_add(1, Ordering::Relaxed);
            self.read_pending_queue_plan_admission_path(
                &self.pending_queue_plan_admission_path(hash),
                Some(hash),
            )?
            .map(|(_, bytes)| bytes)
        };
        self.ensure_prune_recovery_not_required()?;
        Ok(certificate)
    }

    pub(crate) fn pending_queue_plan_admission_hash_inventory(&self) -> Result<HashSet<Hash>> {
        self.ensure_prune_recovery_not_required()?;
        let hashes = {
            let _guard = self.sidecar_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            self.pending_queue_plan_admission_inventory_paths_unlocked()?
                .into_iter()
                .map(|path| {
                    let hash_text = path.file_stem().and_then(std::ffi::OsStr::to_str).expect(
                        "validated QueuePlan admission path must retain its canonical hash stem",
                    );
                    let bytes: [u8; Hash::LENGTH] = hex::decode(hash_text)
                        .expect("validated QueuePlan admission hash must remain hex")
                        .try_into()
                        .expect("validated QueuePlan admission hash must retain its length");
                    Hash::prehashed(bytes)
                })
                .collect()
        };
        self.ensure_prune_recovery_not_required()?;
        Ok(hashes)
    }

    /// Fingerprint-bound QueuePlan admission capacity used by bounded callers.
    #[must_use]
    pub(crate) const fn pending_queue_plan_admission_capacity(&self) -> usize {
        self.pending_control_sidecar_limits.queue_plan_admissions
    }
}
