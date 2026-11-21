//! Source-selected originals for nonmonetary setup and native direct time exchanges.
//!
//! These indexes grant no monetary authority. Requests are made available to transport only
//! after their exact signed bytes are durably selected. Clock tokens are
//! native, single-use and intentionally not serializable: restart requires a new exchange.

use rand::rand_core::TryRng as _;

use super::{preparation_custody::IssuedRequestCustodyV1, *};
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletFsV1, KagemushaWalletPlatformV1, KagemushaWalletUnavailableV1,
};

const ANCHOR_CUSTODY_MAX_BYTES: usize = KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1 + 1024;

/// Native single-use request observation. There is no decoder or foreign constructor.
#[allow(
    missing_copy_implementations,
    reason = "direct time observation custody is consumed exactly once"
)]
pub struct DirectTimeExchangeV1 {
    scheme: [u8; 32],
    wallet: [u8; 32],
    nonce: [u8; 32],
    sent: KagemushaWalletMonotonicReadingV1,
    maximum_response_ms: u64,
}

impl DirectTimeExchangeV1 {
    /// The only value sent to the issuer; local clock custody never crosses the boundary.
    #[must_use]
    pub const fn nonce(&self) -> [u8; 32] {
        self.nonce
    }

    pub(crate) fn start(
        scheme: [u8; 32],
        wallet: [u8; 32],
        maximum_response_ms: u64,
        read_time: impl FnOnce() -> Result<KagemushaWalletMonotonicReadingV1, Error>,
    ) -> Result<Self, Error> {
        let unavailable = || {
            Error::Provider(ProviderError::Unavailable(
                KagemushaWalletUnavailableV1::Platform(0),
            ))
        };
        let mut nonce = [0; 32];
        for _ in 0..128 {
            rand::rngs::SysRng
                .try_fill_bytes(&mut nonce)
                .map_err(|_| unavailable())?;
            if nonce != [0; 32] {
                let sent = read_time()?;
                if sent.boot_id == [0; 32] {
                    return Err(unavailable());
                }
                return Ok(Self {
                    scheme,
                    wallet,
                    nonce,
                    sent,
                    maximum_response_ms,
                });
            }
        }
        Err(unavailable())
    }

    pub(crate) fn complete_for_wallet(
        self,
        anchor: KagemushaWalletTimeAnchorV1,
        certificate: &KagemushaWalletSignerCertificateV1,
        scheme: &KagemushaWalletSchemeV1,
        scope: ([u8; 32], [u8; 32]),
        maximum_response_ms: u64,
        received: KagemushaWalletMonotonicReadingV1,
    ) -> Result<KagemushaWalletAnchoredTimeV1, Error> {
        valid(anchor.verify(scheme, certificate))?;
        if self.scheme != scheme.scheme_id()
            || (self.scheme, self.wallet) != scope
            || anchor.body.wallet_id != self.wallet
            || anchor.body.nonce != self.nonce
            || self.sent.boot_id != received.boot_id
        {
            return Err(Error::Invalid("direct time exchange binding"));
        }
        valid(KagemushaWalletAnchoredTimeV1::new(
            anchor,
            received.boot_id,
            self.sent.monotonic_ms,
            received.monotonic_ms,
            self.maximum_response_ms.min(maximum_response_ms),
        ))
    }
}

pub(super) fn retain_request<A: ObjectWorkspace>(
    store: &mut A,
    index: IndexRoot,
    request: &KagemushaWalletRequestV1,
) -> Result<IndexRoot, Error> {
    // The receiver retains exactly the open Request it issued; every payer completes it.
    valid(request.validate_open())?;
    let bytes = archive::encode(request)?;
    if bytes.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
        return Err(Error::Invalid("issued Request size"));
    }
    let record = IssuedRequestCustodyV1 {
        request: store.stage_object(&bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?,
    };
    let value = archive::encode(&record)?;
    let key = request.request_digest();
    if let Some(previous) = index.get(store, &key)? {
        if previous != value {
            return Err(Error::WitnessLost("changed issued Request custody"));
        }
        return Ok(index);
    }
    index.set(store, key, &value)
}

pub(crate) fn retain_anchor<A: ObjectWorkspace>(
    store: &mut A,
    index: IndexRoot,
    anchored: &KagemushaWalletAnchoredTimeV1,
) -> Result<IndexRoot, Error> {
    let address = store.stage_object(&archive::encode(anchored)?, ANCHOR_CUSTODY_MAX_BYTES)?;
    let key = anchored.anchor.time_anchor_digest();
    if let Some(previous) = index.get(store, &key)? {
        if previous != address {
            return Err(Error::WitnessLost("changed direct time exchange"));
        }
        return Ok(index);
    }
    index.set(store, key, &address)
}

/// Read the native observations selected by the authenticated archive root.
pub(crate) fn read_direct_anchor(
    store: &mut impl ObjectWorkspace,
    index: IndexRoot,
    digest: &[u8; 32],
    scheme: &[u8; 32],
    wallet: &[u8; 32],
    maximum_response_ms: u64,
) -> Result<KagemushaWalletAnchoredTimeV1, Error> {
    let address = index
        .get(store, digest)?
        .ok_or(Error::WitnessLost("direct time exchange"))?;
    let address = address
        .try_into()
        .map_err(|_| Error::WitnessLost("direct time address"))?;
    let bytes = store.read_staged_object(&address, ANCHOR_CUSTODY_MAX_BYTES)?;
    let anchored: KagemushaWalletAnchoredTimeV1 = archive::decode(&bytes)?;
    anchored
        .validate(maximum_response_ms)
        .map_err(|_| Error::WitnessLost("direct anchor observation"))?;
    if anchored.anchor.time_anchor_digest() != *digest
        || anchored.anchor.body.scheme_id != *scheme
        || anchored.anchor.body.wallet_id != *wallet
    {
        return Err(Error::WitnessLost("direct anchor binding"));
    }
    Ok(anchored)
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, N: NativeProofs>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, N>
{
    /// Capture a fresh nonce and request reading from the exclusive native provider.
    ///
    /// # Errors
    /// Missing selected head or originals, unavailable entropy, native clock or storage.
    pub fn begin_direct_time_exchange(&mut self) -> Result<DirectTimeExchangeV1, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let step = self.indexed_step(&manifest, manifest.indexed.ok_or(Error::NoHead)?)?;
        self.source_custody(&manifest, &step)?;
        DirectTimeExchangeV1::start(
            self.scheme_id,
            self.wallet_id,
            step.capsule()
                .body()
                .successor_state
                .core
                .time_anchor_max_response_ms,
            || self.custody.observations().time().map_err(Error::Provider),
        )
    }

    /// Authenticate and durably retain a direct response before ordinary `TimeAnchor` Refresh.
    /// Refresh alone cannot create this native observation. A lost token requires reanchoring.
    ///
    /// # Errors
    /// Invalid issuer, nonce, wallet, boot or response interval; lost selected witnesses;
    /// or unavailable native clock or durable storage. The token is consumed on every call.
    pub fn finish_direct_time_exchange(
        &mut self,
        exchange: DirectTimeExchangeV1,
        anchor: KagemushaWalletTimeAnchorV1,
        certificate: &KagemushaWalletSignerCertificateV1,
    ) -> Result<(), Error> {
        // Record arrival before storage and signature work can add latency to this sample.
        let received = self.custody.observations().time()?;
        let (root, mut manifest) = self.sync_manifest()?;
        let step = self.indexed_step(&manifest, manifest.indexed.ok_or(Error::NoHead)?)?;
        self.source_custody(&manifest, &step)?;
        let (scheme, _) = self.proofs.ledger_scope()?;
        let anchored = exchange.complete_for_wallet(
            anchor,
            certificate,
            &scheme,
            (
                step.capsule().body().successor_state.core.scheme_id,
                step.capsule().body().successor_state.core.wallet_id,
            ),
            step.capsule()
                .body()
                .successor_state
                .core
                .time_anchor_max_response_ms,
            received,
        )?;
        let anchors = retain_anchor(&mut self.archive, manifest.direct_anchors, &anchored)?;
        if anchors != manifest.direct_anchors {
            manifest.direct_anchors = anchors;
            self.publish_manifest(root, &manifest)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
