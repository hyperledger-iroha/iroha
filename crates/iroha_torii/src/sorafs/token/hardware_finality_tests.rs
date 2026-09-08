//! The real Core boundary never upgrades empty storage or a hash-cache claim into finality.
use super::{
    StreamTokenIssuerError,
    hardware_finality::{CoreFinalityV1, HardwareFinalityV1},
};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{BlockHashes, State, World},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::BlockHeader;
use sorafs_manifest::signer::custody::SignerCustodyAnchorV1;
use std::{num::NonZeroUsize, sync::Arc};

#[test]
fn actual_core_finality_requires_durable_certified_history_beyond_public_cache_claims() {
    let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x74; 32]));
    let anchor = SignerCustodyAnchorV1 {
        height: 1,
        block_hash: *hash.as_ref(),
        state_digest: [0x85; 32],
    };
    for cache_only in [false, true] {
        let kura = Kura::blank_kura_for_testing();
        let mut state =
            State::new_for_testing(World::new(), kura.clone(), LiveQueryStore::start_test());
        if cache_only {
            state.block_hashes = BlockHashes::new(vec![hash]);
        }
        assert_eq!(state.block_hashes.view().len(), usize::from(cache_only));
        assert!(
            kura.get_durable_block_hash(NonZeroUsize::new(1).unwrap())
                .is_none()
        );
        let guard = CoreFinalityV1::new(Arc::new(state));
        assert!(matches!(
            guard.capture(anchor),
            Err(StreamTokenIssuerError::HardwareFinalityUnavailable)
        ));
    }
}
