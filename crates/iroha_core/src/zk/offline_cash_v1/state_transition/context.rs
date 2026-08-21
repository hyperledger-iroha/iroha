//! Canonical private-state context.

use super::*;

/// Public context shared by every private state of one asset.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OfflineCashStateContextV1 {
    pub(super) release_id: Digest,
    pub(super) network_id: NetworkId,
    pub(super) asset: AssetDefinitionId,
    pub(super) scale: u32,
    pub(super) digest: Digest,
}

impl fmt::Debug for OfflineCashStateContextV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashStateContextV1")
            .field("release_id", &self.release_id)
            .field("network_id", &self.network_id)
            .field("asset", &self.asset)
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

impl OfflineCashStateContextV1 {
    /// Construct a canonical private-state context.
    pub(crate) fn new(
        release_id: Digest,
        network_id: NetworkId,
        asset: AssetDefinitionId,
        scale: u32,
    ) -> Result<Self, StateTransitionErrorV1> {
        if release_id == [0; 32] || scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
            return Err(StateTransitionErrorV1::InvalidContext);
        }
        let network_bytes = norito::encode_canonical(&network_id)
            .map_err(|_| StateTransitionErrorV1::InvalidContext)?;
        let asset_bytes =
            norito::encode_canonical(&asset).map_err(|_| StateTransitionErrorV1::InvalidContext)?;
        let scale_bytes = scale.to_le_bytes();
        let digest = digest_framed(
            CONTEXT_DOMAIN,
            &[&release_id, &network_bytes, &asset_bytes, &scale_bytes],
        );
        Ok(Self {
            release_id,
            network_id,
            asset,
            scale,
            digest,
        })
    }

    pub(super) fn matches_request(&self, request: &OfflineCashPaymentRequestV1) -> bool {
        request.release_id == self.release_id
            && request.network_id == self.network_id
            && request.asset == self.asset
            && request.scale == self.scale
    }

    pub(crate) fn matches_statement(&self, statement: &OfflineCashTransferStatementV1) -> bool {
        statement.release_id == self.release_id
            && statement.network_id == self.network_id
            && statement.asset == self.asset
            && statement.scale == self.scale
    }

    pub(crate) const fn digest(&self) -> Digest {
        self.digest
    }
}
