//! Validated, optional online finalized-load publication service settings.
use super::*;

/// Owner-admitted private custody material. Debug output never reveals keys or encoded bytes.
#[derive(Clone)]
pub struct KagemushaLoadAuthorizerCustody {
    /// Canonical bounded private Norito keyring, decoded and role-checked at daemon startup.
    pub keyring: zeroize::Zeroizing<Vec<u8>>,
    /// Ordinary ledger submitter. Normal account permissions and transaction admission apply.
    pub submitter: KeyPair,
}
impl fmt::Debug for KagemushaLoadAuthorizerCustody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KagemushaLoadAuthorizerCustody")
            .finish_non_exhaustive()
    }
}

/// Node-local service limits. These do not change protocol validity or ledger permissions.
#[derive(Clone, Debug)]
pub struct KagemushaLoadAuthorizer {
    /// Absent by default; present only when both configured private files were admitted.
    pub custody: Option<KagemushaLoadAuthorizerCustody>,
    /// Delay between bounded source reads, including unavailable-source retries.
    pub poll_interval: Duration,
    /// Maximum issuance identities prepared per tick.
    pub page_size: usize,
    /// Finite original CertifiedChain and decode capacity for each tick.
    pub finality_limits: iroha_data_model::sumeragi::finality::NativeFinalityLimits,
    /// Ordinary submitted transaction expiration, independent of permanent voucher validity.
    pub transaction_ttl: Duration,
    /// Explicit per-component maximum online fees paid only by the configured submitter.
    /// Empty limits authorize no charge; ordinary fee admission rejects an uncovered fee.
    pub charge_limits: Vec<iroha_data_model::transaction::FeeChargeLimit>,
}
impl Default for KagemushaLoadAuthorizer {
    fn default() -> Self {
        use defaults::kagemusha_load_authorizer as d;
        Self {
            custody: None,
            poll_interval: Duration::from_millis(d::POLL_INTERVAL_MS),
            page_size: d::PAGE_SIZE,
            finality_limits: iroha_data_model::sumeragi::finality::NativeFinalityLimits {
                block_bytes: d::BLOCK_BYTES,
                journal_bytes: d::JOURNAL_BYTES,
                block_count: d::BLOCK_COUNT,
                allocated_bytes: d::ALLOCATED_BYTES,
            },
            transaction_ttl: Duration::from_millis(d::TRANSACTION_TTL_MS),
            charge_limits: Vec::new(),
        }
    }
}
