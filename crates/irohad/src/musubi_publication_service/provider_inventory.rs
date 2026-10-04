//! Exact-three provider inventory reads under original endpoint selection and current native state.
use super::MusubiPublicationPrivateServiceContextV1;
use iroha_core::{
    query::provider_attestation_inventory::{
        ProviderAttestationInventoryReadErrorV1, authorize_provider_attestation_inventory_read_v1,
    },
    state::{State, StateReadOnly},
};
use iroha_data_model::{
    NetworkId, account::AccountId, musubi::MusubiProviderBundleAttestationKeyV1,
    sorafs::capacity::ProviderId,
};
use sorafs_node::{
    MusubiProviderAttestationInventoryItemV1, MusubiProviderAttestationInventoryScopeV1,
    MusubiProviderAttestationInventoryV1,
};
use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// A closed read failure; missing/partial inventory never becomes publication success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MusubiPublicationProviderInventoryReadErrorV1 {
    /// Original scope, endpoint or native authority differs.
    #[error("provider attestation inventory selection rejected")]
    Rejected,
    /// Original provider inventory/history/resources are unavailable within the deadline.
    #[error("provider attestation inventory unavailable")]
    Unavailable,
}
use MusubiPublicationProviderInventoryReadErrorV1::{Rejected, Unavailable};

/// Read-only operation owner. Endpoint/provider selection is intent, never admission or authority.
/// Each response must pass the same native completion/manager join before becoming an inventory item.
/// This owner has no transaction, registry, Queue, fallback-provider or journal-open surface.
pub struct MusubiPublicationProviderInventoryReaderV1 {
    state: Arc<State>,
    network: NetworkId,
    manager: AccountId,
    providers: [(ProviderId, iroha::client::Client); 3],
}
impl MusubiPublicationProviderInventoryReaderV1 {
    /// Bind three exact original endpoints to the daemon's State and the original manager client.
    /// The generated installer supplies original profile endpoints; responses cannot replace them.
    /// HTTPS is required except for explicitly selected numeric loopback management origins.
    /// # Errors
    /// Rejects mismatched network/chain, duplicate/zero providers, unsafe origins or signing context.
    pub fn new(
        context: &MusubiPublicationPrivateServiceContextV1,
        manager: &iroha::config::Config,
        originals: [(ProviderId, reqwest::Url); 3],
    ) -> Result<Self, MusubiPublicationProviderInventoryReadErrorV1> {
        let state = context.state();
        if manager.network_id != context.network_id() || state.view().chain_id() != &manager.chain {
            return Err(Rejected);
        }
        validate_originals(&originals)?;
        let build = |(provider, endpoint)| -> Result<
            (ProviderId, iroha::client::Client),
            MusubiPublicationProviderInventoryReadErrorV1,
        > {
            let mut selected = manager.clone();
            selected.torii_api_url = endpoint;
            let client = iroha::client::Client::builder(selected)
                .build()
                .map_err(|_| Rejected)?;
            client.account_client().map_err(|_| Rejected)?;
            Ok((provider, client))
        };
        let [first, second, third] = originals;
        Ok(Self {
            state,
            network: context.network_id(),
            manager: manager.account.clone(),
            providers: [build(first)?, build(second)?, build(third)?],
        })
    }

    /// Read all three originally selected providers; a missing item refuses the whole result.
    /// Already acquired items are not a quorum or a registry membership proof. Retrying reads
    /// uses these same providers and does not submit or replace any original attestation.
    /// # Errors
    /// Refuses a foreign scope, changed native authority, deadline or any incomplete provider read.
    pub fn read_exact_three(
        &self,
        scope: &MusubiProviderAttestationInventoryScopeV1,
        deadline: Instant,
    ) -> Result<MusubiProviderAttestationInventoryV1, MusubiPublicationProviderInventoryReadErrorV1>
    {
        // Every native proof, response decode and retained result shares this finite allowance.
        // Nested SDK/Core scopes preserve a tighter caller budget instead of replenishing it.
        norito::with_decode_limits_scope(
            norito::DecodeLimits::new(
                iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_BYTES,
                iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_BYTES,
                64 * 1024 * 1024,
                256 * 1024 * 1024,
                128,
            ),
            || self.read_originals(scope, deadline),
        )
    }
    fn read_originals(
        &self,
        scope: &MusubiProviderAttestationInventoryScopeV1,
        deadline: Instant,
    ) -> Result<MusubiProviderAttestationInventoryV1, MusubiPublicationProviderInventoryReadErrorV1>
    {
        scope.validate().map_err(|_| Rejected)?;
        if scope.network_id != self.network {
            return Err(Rejected);
        }
        let mut items = Vec::new();
        norito::core::reserve_decode_allocation(
            3 * std::mem::size_of::<MusubiProviderAttestationInventoryItemV1>(),
        )
        .map_err(|_| Unavailable)?;
        items.try_reserve_exact(3).map_err(|_| Unavailable)?;
        for (provider, client) in &self.providers {
            check_deadline(deadline)?;
            let key = MusubiProviderBundleAttestationKeyV1 {
                archive_id: scope.archive_id,
                replication_order: scope.replication_order,
                provider_id: *provider,
            };
            self.authorize(key, None, deadline)?;
            let attestation = client
                .with_request_deadline(deadline)
                .get_sorafs_provider_attestation(key)
                .map_err(|_| Unavailable)?
                .ok_or(Unavailable)?;
            self.authorize(key, Some(&attestation), deadline)?;
            items.push(
                MusubiProviderAttestationInventoryItemV1::new(attestation).map_err(|_| Rejected)?,
            );
        }
        // Rejoin every original against one final captured cut; a later provider response
        // cannot make earlier authority checks from different views into a combined fact.
        let view = self.state.view();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Unavailable)?
            .as_secs();
        for item in &items {
            check_deadline(deadline)?;
            authorize_provider_attestation_inventory_read_v1(
                &view,
                &self.manager,
                item.key(),
                Some(item.attestation()),
                now,
            )
            .map_err(map_authorization)?;
        }
        check_deadline(deadline)?;
        MusubiProviderAttestationInventoryV1::new(scope.clone(), items).map_err(|_| Rejected)
    }
    fn authorize(
        &self,
        key: MusubiProviderBundleAttestationKeyV1,
        attestation: Option<
            &iroha_data_model::musubi::MusubiProviderBundleVerificationAttestationV1,
        >,
        deadline: Instant,
    ) -> Result<(), MusubiPublicationProviderInventoryReadErrorV1> {
        check_deadline(deadline)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Unavailable)?
            .as_secs();
        authorize_provider_attestation_inventory_read_v1(
            &self.state.view(),
            &self.manager,
            key,
            attestation,
            now,
        )
        .map_err(map_authorization)?;
        check_deadline(deadline)
    }
}
fn map_authorization(
    error: ProviderAttestationInventoryReadErrorV1,
) -> MusubiPublicationProviderInventoryReadErrorV1 {
    match error {
        ProviderAttestationInventoryReadErrorV1::Rejected => Rejected,
        ProviderAttestationInventoryReadErrorV1::Unavailable => Unavailable,
    }
}
fn check_deadline(deadline: Instant) -> Result<(), MusubiPublicationProviderInventoryReadErrorV1> {
    if Instant::now() >= deadline {
        Err(Unavailable)
    } else {
        Ok(())
    }
}
fn validate_originals(
    originals: &[(ProviderId, reqwest::Url); 3],
) -> Result<(), MusubiPublicationProviderInventoryReadErrorV1> {
    for (index, (provider, endpoint)) in originals.iter().enumerate() {
        if provider.as_bytes() == &[0; 32]
            || originals[..index]
                .iter()
                .any(|(prior, _)| prior == provider)
            || endpoint.as_str().len() > 2048
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
            || endpoint.host_str().is_none()
            || endpoint.port() == Some(0)
            || !(endpoint.scheme() == "https"
                || endpoint.scheme() == "http"
                    && matches!(endpoint.host_str(), Some("127.0.0.1" | "[::1]" | "::1")))
        {
            return Err(Rejected);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn originals() -> [(ProviderId, reqwest::Url); 3] {
        std::array::from_fn(|index| {
            (
                ProviderId::new([index as u8 + 1; 32]),
                format!("http://127.0.0.1:{}/", 8180 + index)
                    .parse()
                    .unwrap(),
            )
        })
    }
    #[test]
    fn inventory_selection_retains_exact_three_and_original_management_ports() {
        let endpoints = originals();
        assert_eq!(validate_originals(&endpoints), Ok(()));
        assert_eq!(endpoints[2].1.port(), Some(8182));
        let mut remote = endpoints.clone();
        remote[1].1 = "https://provider.example:9443/".parse().unwrap();
        assert_eq!(validate_originals(&remote), Ok(()));
        let mut duplicate = endpoints;
        duplicate[2].0 = duplicate[0].0;
        assert_eq!(validate_originals(&duplicate), Err(Rejected));
    }
    #[test]
    fn inventory_selection_refuses_unsafe_origins_and_zero_provider_before_clients() {
        for address in [
            "http://provider.example/",
            "http://localhost/",
            "http://10.0.0.1/",
            "https://provider.example:0/",
            "https://user:secret@provider.example/",
            "https://provider.example/other",
            "https://provider.example/?q=1",
            "https://provider.example/#f",
        ] {
            let mut endpoints = originals();
            endpoints[0].1 = address.parse().unwrap();
            assert_eq!(validate_originals(&endpoints), Err(Rejected), "{address}");
        }
        let mut endpoints = originals();
        endpoints[0].0 = ProviderId::new([0; 32]);
        assert_eq!(validate_originals(&endpoints), Err(Rejected));
        assert_eq!(check_deadline(Instant::now()), Err(Unavailable));
    }
}
