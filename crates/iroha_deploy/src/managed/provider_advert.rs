//! Managed original provider-advert publication; transport acknowledgement is not eligibility.

use super::native_operation::{invalid, now_ms, require_deadline};
use super::{
    PreparedLocalnet, Result,
    service_authority::{ProviderPurpose, ServiceAuthority},
};
use std::time::{Duration, Instant};

/// Bounded publication outcome from the original management endpoints.
///
/// This is a transport report only. Consumers must use native authenticated discovery and
/// runtime qualification before treating the provider as usable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManagedProviderAdvertisementReport {
    /// Original deterministic refresh slot sent to all selected peers.
    pub issued_at: u64,
    /// Finite expiry, never later than the original admission interval.
    pub expires_at: u64,
    /// Number of selected management endpoints that acknowledged the submitted advert.
    pub acknowledged_peers: usize,
}

/// Exclusive publisher of one generation's exact original provider advert material.
pub struct ManagedProviderAdvertisement {
    authority: ServiceAuthority,
}

impl ManagedProviderAdvertisement {
    /// Authenticate the original signed profile and retain its fixed publication lock.
    /// # Errors
    /// Refuses changed original material, invalid private custody or an already-held lock.
    pub fn open(
        prepared: &PreparedLocalnet,
        provider: iroha_data_model::sorafs::capacity::ProviderId,
    ) -> Result<Self> {
        Ok(Self {
            authority: ServiceAuthority::open_provider(
                prepared,
                provider,
                ProviderPurpose::ProviderAdvertisement,
            )?,
        })
    }

    /// Sign the deterministic current refresh slot and publish it once per original peer.
    ///
    /// An exact retry in the same slot produces identical signed bytes. Native replay and
    /// admission checks remain owned by Torii. Clock rollback never invents a larger issue
    /// time, and refresh cannot extend the retained admission or certificate lifetime.
    /// # Errors
    /// Refuses expired/deviating original material, an elapsed deadline or total unavailability.
    /// Partial acknowledgements are reported without claiming native readiness or unanimity.
    pub fn publish(&self, deadline: Instant) -> Result<ManagedProviderAdvertisementReport> {
        require_deadline(deadline)?;
        self.authority.validate_profile()?;
        let advert = self
            .authority
            .prepared
            .provider_advert(self.authority.provider_id()?, now_ms()? / 1_000)?;
        self.publish_original(&advert, deadline)
    }

    // A private seam over an already-produced original enables deterministic slot/replay tests.
    // It accepts no endpoint or signer input and performs no transaction dispatch.
    fn publish_original(
        &self,
        advert: &sorafs_manifest::provider_advert::ProviderAdvertV1,
        deadline: Instant,
    ) -> Result<ManagedProviderAdvertisementReport> {
        require_deadline(deadline)?;
        self.authority.validate_profile()?;
        let mut acknowledged_peers = 0;
        for (index, (_, client)) in self.authority.peers.iter().enumerate() {
            let Some(peer_deadline) =
                peer_deadline(Instant::now(), deadline, self.authority.peers.len() - index)
            else {
                break;
            };
            if client
                .with_request_deadline(peer_deadline)
                .post_sorafs_provider_advert(advert)
                .is_ok_and(|response| response.status() == iroha::http::StatusCode::OK)
            {
                acknowledged_peers += 1;
            }
        }
        self.authority.validate_profile()?;
        if acknowledged_peers == 0 {
            return Err(invalid(
                "original provider advert was not acknowledged by selected peers",
            ));
        }
        Ok(ManagedProviderAdvertisementReport {
            issued_at: advert.issued_at,
            expires_at: advert.expires_at,
            acknowledged_peers,
        })
    }
}

// Allocate every still-unattempted original peer a share of the remaining deadline. A stalled
// early peer cannot consume later peers' entire budget. Fast failures release their unused share.
fn peer_deadline(now: Instant, deadline: Instant, peers_remaining: usize) -> Option<Instant> {
    let remaining = deadline.checked_duration_since(now)?;
    let share = remaining.checked_div(u32::try_from(peers_remaining).ok()?)?;
    if share.is_zero() {
        return None;
    }
    now.checked_add(share.min(Duration::from_secs(5)))
        .map(|limit| limit.min(deadline))
}

#[cfg(test)]
mod tests;
