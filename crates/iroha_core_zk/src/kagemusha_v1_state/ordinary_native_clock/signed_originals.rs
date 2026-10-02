//! Complete public signed originals, separately from current Native elapsed-clock authority.
use super::*;
use iroha_data_model::kagemusha::KagemushaOrdinaryCashClockContextV1;

/// Maximum complete original frame. The four original replies retain the existing four-MiB
/// per-node limit; the additional fixed fields and canonical framing have a four-KiB budget.
pub const KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1: usize =
    MAX_FRAME + 4 * 1024;

/// Sole bounded public carrier of the four complete signed clock observations. Decoding this
/// carrier creates no installed root, current clock, elapsed-time reading or monetary grant.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core_zk::ordinary_native_clock::KagemushaOrdinaryNativeSignedClockOriginalV1"
)]
pub struct KagemushaOrdinaryNativeSignedClockOriginalV1 {
    pub(super) version: u16,
    pub(super) installed_selection_digest: [u8; 32],
    pub(super) request_nonce: [u8; 32],
    pub(super) certified_context_id: Hash,
    pub(super) originals: [Vec<u8>; 4],
}
impl KagemushaOrdinaryNativeSignedClockOriginalV1 {
    fn validate_shape(&self) -> Result<()> {
        if self.version != 1
            || self.installed_selection_digest == [0; 32]
            || self.request_nonce == [0; 32]
            || self
                .originals
                .iter()
                .any(|raw| raw.is_empty() || raw.len() > MAX_FRAME / 4)
        {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Decode only the complete sole canonical public carrier. Authentication additionally
    /// requires independently retained installed originals and a real finality verifier.
    /// # Errors
    /// Rejects oversized, malformed, noncanonical, truncated or trailing data.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty()
            || raw.len() > KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
        {
            return Err(Rejected);
        }
        let value: Self = norito::decode_canonical_with_limits(
            raw,
            norito::canonical_decode_limits(
                KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
            ),
        )
        .map_err(|_| Rejected)?;
        value.validate_shape()?;
        if value.canonical_original()? != raw {
            return Err(Rejected);
        }
        Ok(value)
    }
    /// Encode the complete public data carrier, without authenticating its offered fields.
    /// # Errors
    /// Rejects invalid shape or a frame beyond the complete carrier budget.
    pub fn canonical_original(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        let raw = norito::encode_canonical(self).map_err(|_| Rejected)?;
        if raw.is_empty()
            || raw.len() > KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
        {
            return Err(Rejected);
        }
        Ok(raw)
    }
    /// Complete originals in the installed node order; these are public signed data only.
    #[must_use]
    pub fn signed_observations(&self) -> &[Vec<u8>; 4] {
        &self.originals
    }
    /// Original request nonce, without current-read or freshness authority.
    #[must_use]
    pub fn request_nonce(&self) -> [u8; 32] {
        self.request_nonce
    }
    /// Exact complete installed checkpoint/node/policy selection identity.
    #[must_use]
    pub fn installed_selection_digest(&self) -> [u8; 32] {
        self.installed_selection_digest
    }
    /// Context authenticated by every complete signed observation.
    #[must_use]
    pub fn certified_context_id(&self) -> Hash {
        self.certified_context_id
    }
}

/// Closed signature/finality admission of the complete signed originals under an independently
/// held root. It authenticates the signed samples and their retained certified decision, not
/// the sender's measured elapsed interval. Effects require a separately fresh current clock.
pub struct KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1 {
    original: KagemushaOrdinaryNativeSignedClockOriginalV1,
    original_bytes: Vec<u8>,
    median_ms: u64,
    signed_observations_original_digest: [u8; 32],
    maximum_projection_age_ms: u64,
}
impl KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1 {
    /// Full admitted public original; no private financial or software elapsed-clock state.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original_bytes
    }
    /// Join a retained data context to these exact original samples. This checks consistency
    /// and the governed finite projection window; it does not authenticate sender elapsed time.
    /// # Errors
    /// Rejects foreign nonce/digest, invalid interval or an out-of-window projection.
    pub fn recheck_cash_context(
        &self,
        context: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<()> {
        context.validate_shape().map_err(|_| Rejected)?;
        let end = self
            .median_ms
            .checked_add(self.maximum_projection_age_ms)
            .ok_or(Rejected)?;
        if context.request_nonce != self.original.request_nonce
            || context.signed_observations_original_digest
                != self.signed_observations_original_digest
            || context.lower_at_ms < self.median_ms
            || context.upper_at_ms < context.lower_at_ms
            || context.upper_at_ms >= end
        {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Height shared by the four signature/finality-admitted original observations.
    /// This lends their historical certified prefix, without a current clock or FI loan.
    pub(crate) fn certified_height(&self) -> Result<u64> {
        let mut height = None;
        for original in &self.original.originals {
            let reply: SumeragiFinalityAttestation = norito::decode_canonical_with_limits(
                original,
                norito::canonical_decode_limits(MAX_FRAME / 4),
            )
            .map_err(|_| Rejected)?;
            let current = reply.body.finality_proof.height();
            if current == 0 || height.is_some_and(|prior| prior != current) {
                return Err(Rejected);
            }
            height = Some(current);
        }
        height.ok_or(Rejected)
    }

    /// Complete sample selector shared with the Native context's maintained purpose-bound digest.
    #[must_use]
    pub fn signed_observations_original_digest(&self) -> [u8; 32] {
        self.signed_observations_original_digest
    }
}

/// Authenticate the sole original under independently installed node/policy/checkpoint custody
/// and a finality prefix already admitted by the caller's actual Native owner. The offered
/// original cannot replace either authority, and this function lends no current elapsed clock.
/// # Errors
/// Rejects foreign selection, nonce, signature, node/build/configuration or certified decision.
pub fn verify_ordinary_native_signed_clock_original_v1(
    selected: &KagemushaOrdinaryNativeClockOriginalsV1,
    verifier: &SumeragiFinalityVerifier,
    raw: &[u8],
) -> Result<KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1> {
    let original = KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(raw)?;
    if original.installed_selection_digest != selected.digest {
        return Err(Rejected);
    }
    let (median_ms, context) = verify_signed_observations(
        selected,
        verifier,
        original.request_nonce,
        &original.originals,
        None,
    )?;
    if context != original.certified_context_id {
        return Err(Rejected);
    }
    let digest = signed_observation_digest(original.request_nonce, context, &original.originals)?;
    Ok(KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1 {
        original,
        original_bytes: raw.to_vec(),
        median_ms,
        signed_observations_original_digest: digest,
        maximum_projection_age_ms: selected.policy.maximum_projection_age_ms,
    })
}

/// Private complete-original loan obtained only by scanning or borrowing this owner's actual
/// acknowledged clock WAL. It cannot be constructed from a transported original or decoded pins.
pub(crate) struct KagemushaRetainedOrdinaryNativeClockOriginalsV1 {
    owner: Arc<()>,
    selected: Arc<KagemushaOrdinaryNativeClockOriginalsV1>,
    observation: Arc<Observation>,
    original: Vec<u8>,
    context: KagemushaOrdinaryCashClockContextV1,
}
impl KagemushaRetainedOrdinaryNativeClockOriginalsV1 {
    pub(crate) fn canonical_original(&self) -> &[u8] {
        &self.original
    }
    /// Independently verify this private acknowledged WAL loan under its actual installed owner.
    /// Offered samples cannot construct this loan or select its checkpoint/node authority.
    pub(crate) fn verified_original(
        &self,
        owner: &KagemushaOrdinaryNativeClockOwnerV1,
    ) -> Result<KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1> {
        self.recheck(owner)?;
        let verified = verify_ordinary_native_signed_clock_original_v1(
            &owner.selected,
            &owner.verifier,
            &self.original,
        )?;
        verified.recheck_cash_context(&self.context)?;
        self.recheck(owner)?;
        Ok(verified)
    }
    pub(crate) fn recheck(&self, owner: &KagemushaOrdinaryNativeClockOwnerV1) -> Result<()> {
        owner.recheck()?;
        if !Arc::ptr_eq(&self.owner, &owner.identity)
            || !Arc::ptr_eq(&self.selected, &owner.selected)
            || !owner.consumed_nonces.contains(&self.observation.nonce)
        {
            return Err(Rejected);
        }
        let verified = verify_ordinary_native_signed_clock_original_v1(
            &owner.selected,
            &owner.verifier,
            &self.original,
        )?;
        verified.recheck_cash_context(&self.context)?;
        owner.recheck()
    }
}
impl KagemushaOrdinaryNativeClockOwnerV1 {
    /// Authenticate received historical samples under this actual installed clock root/prefix.
    /// Offered samples cannot enter the clock WAL, renew elapsed time or construct a retained
    /// Native nonce loan. This lends only independently checked signed/finality original data.
    pub(crate) fn authenticate_received_historical_signed_original(
        &self,
        raw: &[u8],
    ) -> Result<KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1> {
        self.recheck()?;
        let verifier = self.retained_finality_verifier_for_original_custody()?;
        let verified =
            verify_ordinary_native_signed_clock_original_v1(&self.selected, &verifier, raw)?;
        verified.certified_height()?;
        self.recheck()?;
        Ok(verified)
    }

    /// Borrow historical signed data from the authentic private WAL without lending a previous
    /// boot's current time. Only actual Native retained cash records supply the context here.
    pub(crate) fn retained_cash_clock_originals(
        &self,
        context: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<KagemushaRetainedOrdinaryNativeClockOriginalsV1> {
        self.recheck()?;
        context.validate_shape().map_err(|_| Rejected)?;
        let matching = |observation: &Observation| -> Result<bool> {
            Ok(observation.nonce == context.request_nonce
                && signed_observation_digest(
                    observation.nonce,
                    observation.certified_context_id,
                    &observation.originals,
                )? == context.signed_observations_original_digest)
        };
        let observation = if let Some(latest) = &self.latest_observation {
            if matching(latest)? {
                Some(latest.clone())
            } else {
                None
            }
        } else {
            None
        };
        let observation = if let Some(observation) = observation {
            observation
        } else {
            let mut cursor = self.journal.replay_cursor().map_err(|_| Custody)?;
            let mut found = None;
            let mut rows = 0usize;
            while let Some((_, raw)) = self
                .journal
                .read_cursor_next(&mut cursor)
                .map_err(|_| Custody)?
            {
                rows = rows.checked_add(1).ok_or(Rejected)?;
                if rows > MAX_ROWS {
                    return Err(Rejected);
                }
                if let Record::Observation(value) = decode(&raw)? {
                    if matching(&value)? {
                        if found.is_some() {
                            return Err(Rejected);
                        }
                        found = Some(Arc::new(*value));
                    }
                }
            }
            if rows != self.rows {
                return Err(Custody);
            }
            found.ok_or(Rejected)?
        };
        let frame = KagemushaOrdinaryNativeSignedClockOriginalV1 {
            version: 1,
            installed_selection_digest: self.selected.digest,
            request_nonce: observation.nonce,
            certified_context_id: observation.certified_context_id,
            originals: observation.originals.clone(),
        };
        let loan = KagemushaRetainedOrdinaryNativeClockOriginalsV1 {
            owner: self.identity.clone(),
            selected: self.selected.clone(),
            observation,
            original: frame.canonical_original()?,
            context: context.clone(),
        };
        loan.recheck(self)?;
        Ok(loan)
    }
}
