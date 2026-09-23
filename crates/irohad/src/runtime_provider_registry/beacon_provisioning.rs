//! Exact public-catalog updates paired with prepared beacon credential inventories.

use super::*;

impl IrohaRuntimeProviderBindingsV1 {
    /// Preserve every public role while advancing only the beacon inventory qualification.
    ///
    /// A joining node may start a one-role catalog. This constructs deployment artifacts only;
    /// publishing or using the catalog does not activate a consensus beacon session.
    ///
    /// # Errors
    /// Rejects substituted chain/network/handle, stale revisions or a noncanonical catalog.
    pub fn with_prepared_beacon_inventory_v1(
        retained: Option<&Self>,
        chain_id: &str,
        network_id: NetworkId,
        handle: &str,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> Result<Self, IrohaRuntimeProviderCatalogErrorV1> {
        let rejected = IrohaRuntimeProviderCatalogErrorV1::InvalidBinding;
        let binding = IrohaRuntimeProviderBindingV1::try_new(
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
            handle,
            Some(revision),
            Some(policy_digest),
        )
        .map_err(|_| rejected)?;
        let mut catalog = retained.cloned().unwrap_or_else(|| Self {
            chain_id: chain_id.to_owned(),
            network_id,
            bindings: Vec::new(),
        });
        if catalog.chain_id != chain_id || catalog.network_id != network_id {
            return Err(rejected);
        }
        if let Some(existing) = catalog
            .bindings
            .iter_mut()
            .find(|entry| entry.slot == IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner)
        {
            if existing.handle != handle || existing.revision.is_none_or(|old| revision <= old) {
                return Err(rejected);
            }
            *existing = binding;
        } else {
            catalog.bindings.push(binding);
            catalog.bindings.sort_unstable_by_key(|entry| entry.slot);
        }
        Self::load_canonical_v1(&catalog.export_canonical_v1()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_beacon_catalog_binds_revision_network_and_handle() {
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"catalog-network"),
        ));
        let first = IrohaRuntimeProviderBindingsV1::with_prepared_beacon_inventory_v1(
            None,
            "beacon-chain",
            network,
            "software://beacon/main",
            1,
            [1; 32],
        )
        .unwrap();
        let next = IrohaRuntimeProviderBindingsV1::with_prepared_beacon_inventory_v1(
            Some(&first),
            "beacon-chain",
            network,
            "software://beacon/main",
            2,
            [2; 32],
        )
        .unwrap();
        assert_eq!(next.iter().next().unwrap().revision(), Some(2));
        assert_eq!(first.iter().next().unwrap().revision(), Some(1));
        for (chain, handle, revision) in [
            ("other-chain", "software://beacon/main", 2),
            ("beacon-chain", "software://beacon/other", 2),
            ("beacon-chain", "software://beacon/main", 1),
        ] {
            assert!(
                IrohaRuntimeProviderBindingsV1::with_prepared_beacon_inventory_v1(
                    Some(&first),
                    chain,
                    network,
                    handle,
                    revision,
                    [2; 32],
                )
                .is_err()
            );
        }
    }
}
