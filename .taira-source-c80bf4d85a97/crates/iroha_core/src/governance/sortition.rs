//! VRF sortition helper for governance bodies (seeding + ranked alternates).

use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{ChainId, account::AccountId};

/// VRF draw result: ranked winners plus alternates (descending output; ties by account id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    #[doc = "Selected members in descending VRF output order."]
    pub members: Vec<AccountId>,
    #[doc = "Backup candidates to replace members that decline or are ineligible."]
    pub alternates: Vec<AccountId>,
}

/// Compute deterministic sortition seed.
pub fn compute_seed(chain_id: &ChainId, epoch: u64, beacon: &[u8; 32], domain: &[u8]) -> [u8; 64] {
    let mut hasher = Blake2b512::new();
    hasher.update(domain);
    hasher.update(chain_id.as_str().as_bytes());
    hasher.update(epoch.to_be_bytes());
    hasher.update(beacon);
    let digest = hasher.finalize();
    let mut out = [0u8; 64];
    out.copy_from_slice(&digest);
    out
}

/// Build VRF input = domain || seed || `encode(account_id)`.
pub fn build_input(domain: &[u8], seed: &[u8; 64], account_id: &AccountId) -> Vec<u8> {
    use iroha_data_model::Encode;
    let account_bytes = Encode::encode(account_id);
    let mut buf = Vec::with_capacity(domain.len() + seed.len() + account_bytes.len());
    buf.extend_from_slice(domain);
    buf.extend_from_slice(seed);
    buf.extend_from_slice(&account_bytes);
    buf
}
