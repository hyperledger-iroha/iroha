/// Exact finalized identity of one archived committed view.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct ProviderIngestFinalizedArchiveKeyV1 {
    /// Exact genesis-derived network containing the committed state.
    pub network_id: NetworkId,
    /// One-based finalized block height.
    pub height: u64,
    /// Exact finalized block hash.
    pub block_hash: [u8; 32],
    /// Exact result-bearing block creation time.
    pub finalized_at_unix_ms: u64,
}

impl ProviderIngestFinalizedArchiveKeyV1 {
    /// Construct one validated exact key.
    ///
    /// # Errors
    ///
    /// Rejects an unmarked network identity, zero height/hash/time, or the
    /// reserved maximum timestamp.
    pub fn try_new(
        network_id: NetworkId,
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
    ) -> Result<Self, ProviderIngestFinalizedArchiveErrorV1> {
        let key = Self {
            network_id,
            height,
            block_hash,
            finalized_at_unix_ms,
        };
        key.validate()?;
        Ok(key)
    }

    /// Validate this exact finalized identity.
    ///
    /// # Errors
    ///
    /// Returns a stable key-validation failure.
    pub fn validate(&self) -> Result<(), ProviderIngestFinalizedArchiveErrorV1> {
        if self.network_id.as_bytes()[31] & 1 != 1 {
            return Err(ProviderIngestFinalizedArchiveErrorV1::InvalidKey {
                reason: "network id must be an exact genesis-derived identity",
            });
        }
        if self.height == 0 {
            return Err(ProviderIngestFinalizedArchiveErrorV1::InvalidKey {
                reason: "finalized height must be one-based",
            });
        }
        if self.block_hash == [0; 32] {
            return Err(ProviderIngestFinalizedArchiveErrorV1::InvalidKey {
                reason: "finalized block hash must be non-zero",
            });
        }
        if self.finalized_at_unix_ms == 0 || self.finalized_at_unix_ms == u64::MAX {
            return Err(ProviderIngestFinalizedArchiveErrorV1::InvalidKey {
                reason: "finalized block time must be a canonical non-zero timestamp",
            });
        }
        Ok(())
    }

    fn finalized_anchor(&self) -> ProviderIngestFinalizedAnchorV1 {
        ProviderIngestFinalizedAnchorV1 {
            height: self.height,
            block_hash: self.block_hash,
        }
    }
}
