//! Crate-wide fixtures shared by unit tests.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, block::BlockHeader};

/// Build a deterministic exact network identity for protocol fixtures.
pub(crate) fn synthetic_network_id(seed: &str) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        seed.as_bytes(),
    )))
}

mod tests {
    use super::synthetic_network_id;

    #[test]
    fn synthetic_network_id_is_deterministic_per_seed() {
        assert_eq!(synthetic_network_id("a"), synthetic_network_id("a"));
        assert_ne!(synthetic_network_id("a"), synthetic_network_id("b"));
    }
}
