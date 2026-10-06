//! Bounded authenticated publication queue. Only unpaid signing work is indexed; permanent
//! issuance history remains authoritative and is never copied into a worker backlog.
use super::{storage, *};
use mv::storage::StorageReadOnly as _;

pub(super) const KIND: u8 = 13;
pub(super) const CAP: usize = 256;
/// Largest page of pending identities one source read may return.
pub const MAX_PENDING_PAGE: usize = 256;

/// Original load identities retained atomically with an unsigned issuance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kagemusha_wallet_v1::PendingPublication")]
pub struct PendingPublication {
    /// Exact wallet incarnation.
    pub wallet: Digest,
    /// Payer-selected immutable issuance retry identity.
    pub request: Digest,
}
impl PendingPublication {
    /// Stable bounded-page cursor. This value carries no signing or completion authority.
    #[must_use]
    pub fn cursor(self) -> Digest {
        *iroha_crypto::Hash::new_from_chunks(&[
            b"iroha:kagemusha:pending-publication:v1\0",
            &self.wallet,
            &self.request,
        ])
        .as_ref()
    }
    pub(super) fn key(self, scheme: Digest, certificate: Digest) -> LedgerKey {
        storage::entry_key(KIND, scheme, certificate, self.cursor())
    }
    pub(super) fn from_issuance(issuance: &Issuance) -> Self {
        Self {
            wallet: issuance.command.wallet,
            request: issuance.command.request_id,
        }
    }
}

impl FinalizedLedger<'_, '_> {
    /// Read at most one finite page for a historical certificate from this verified cut.
    /// `after` is only a scheduling cursor, never an authority or completion assertion.
    /// An empty result permits wrapping to the start on the next tick.
    ///
    /// # Errors
    /// Rejects zero identities, invalid page capacity, corrupt/mixed indexes, or unavailable
    /// original issuance. Every index is checked against its same-generation unsigned source.
    pub fn pending_publications(
        &self,
        scheme: Digest,
        certificate: Digest,
        after: Option<Digest>,
        maximum: usize,
    ) -> Result<Vec<PendingPublication>> {
        use std::ops::Bound::{Excluded, Included};
        if scheme == [0; 32] || certificate == [0; 32] || !(1..=MAX_PENDING_PAGE).contains(&maximum)
        {
            return Err(Error::Binding);
        }
        let start = after.map_or_else(
            || Included(storage::entry_key(KIND, scheme, certificate, [0; 32])),
            |entry| Excluded(storage::entry_key(KIND, scheme, certificate, entry)),
        );
        let end = Included(storage::entry_key(KIND, scheme, certificate, [255; 32]));
        norito::core::with_decode_limits_scope(self.decode_limits, || {
            let mut page = Vec::with_capacity(maximum);
            for (key, bytes) in self
                .view
                .world
                .kagemusha_wallet_ledger
                .range((start, end))
                .take(maximum)
            {
                storage::validate_row(key, bytes)?;
                let pending: PendingPublication = storage::decode(bytes, CAP)?;
                let issuance = self.read_issuance(
                    &scheme,
                    &pending.wallet,
                    &pending.request,
                    self.record_bytes,
                )?;
                if issuance.body.authorizer_certificate != certificate || issuance.voucher.is_some()
                {
                    return Err(Error::Binding);
                }
                page.push(pending);
            }
            Ok(page)
        })
    }
}
