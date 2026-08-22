fn hot_budget_bytes(key_size_bytes: usize, value_size_bytes: usize) -> u64 {
    let key_size_bytes = u64::try_from(key_size_bytes).unwrap_or(u64::MAX);
    let value_size_bytes = u64::try_from(value_size_bytes).unwrap_or(u64::MAX);
    key_size_bytes.saturating_add(value_size_bytes)
}

const fn hot_budget_has_capacity(retained_bytes: u64, entry_bytes: u64, max_bytes: u64) -> bool {
    max_bytes == 0 || entry_bytes <= max_bytes.saturating_sub(retained_bytes)
}

impl EntryScore {
    fn hot_budget_bytes(&self, meta: &EntryMetadata) -> u64 {
        hot_budget_bytes(self.key_encoded.len(), meta.value_size_bytes)
    }
}

impl TieredManifestEntry {
    /// Returns the deterministic measured value footprint for the entry.
    #[must_use]
    pub fn value_size_bytes(&self) -> usize {
        self.value_size_bytes
    }

    /// Returns the canonical encoded-key plus measured-value budget weight.
    #[cfg(any(test, feature = "telemetry"))]
    pub(crate) fn hot_budget_bytes(&self) -> u64 {
        hot_budget_bytes(self.key_payload.len(), self.value_size_bytes)
    }
}
