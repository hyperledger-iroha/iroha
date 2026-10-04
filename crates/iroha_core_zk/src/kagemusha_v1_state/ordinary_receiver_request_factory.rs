//! Closed Native ordinary receiver request key data, for the Main cash WAL.
//!
//! This candidate child uses only the actual current Native owner and the sole model request
//! encoder. Main must fsync Reserve before lending the signing message, fence before any OS
//! invocation, and fsync Capture before exposing the complete signed request. Decoded originals
//! grant no request custody or financial authority. No ReceiveFold or Mint balance is created.

use super::*;
use iroha_crypto::kagemusha::kagemusha_x25519_public_key_v1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_MAX_BYTES_V1, KAGEMUSHA_REQUEST_MAX_TTL_MS_V1,
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaOrdinaryCashClockContextV1,
    KagemushaOrdinaryPaymentRequestBodyV1, KagemushaOrdinaryPaymentRequestV1,
    KagemushaVerifiedOrdinaryAppCredentialV1, KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
    kagemusha_asset_identity_digest_v1,
};
use rand_core_06::OsRng;
use zeroize::{Zeroize as _, Zeroizing};

#[path = "ordinary_received_credit_opening.rs"]
mod received_credit;
pub(crate) use received_credit::{
    KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1,
    KagemushaHistoricalOrdinaryReceiverRequestCustodyV1,
};

// Only Main's retained capture map and real current owner enter this private constructor.
pub(super) fn loan_main_request(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
) -> Result<KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'_>, KagemushaStateErrorV1> {
    received_credit::from_main(owner, request_id)
}

// Only actual Main replay/read-only witness selection forwards the historical constructor.
pub(super) fn loan_main_request_historical(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
) -> Result<KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'_>, KagemushaStateErrorV1> {
    received_credit::from_main_historical(owner, request_id)
}

/// Private data-only one-use key reservation. The sole constructor requires the actual owner.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryReceiverRequestOriginalsV1")]
pub(super) struct ReceiverRequestOriginals {
    publication_originals: [DigestV1; 8],
    predecessor: DigestV1,
    private_key: ReceiverPrivateKey,
    body: KagemushaOrdinaryPaymentRequestBodyV1,
    fi_original: Vec<u8>,
    credential_original: Vec<u8>,
    possession_original: Vec<u8>,
    integrity_lease_original: Option<Vec<u8>>,
    previous_app_attest_counter: Option<u32>,
}

// A partially decoded or cloned secret owns its clearing behavior before later fields decode.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryReceiverPrivateKeyV1")]
struct ReceiverPrivateKey([u8; 32]);
impl Drop for ReceiverPrivateKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl core::fmt::Debug for ReceiverRequestOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReceiverRequestOriginals")
            .field("request_id", &self.body.request_id)
            .finish_non_exhaustive()
    }
}

/// Original data only. Main retains the predecessor/key and enforces the irreversible fence.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCapturedReceiverRequestV1")]
pub(super) struct CapturedReceiverRequestOriginals {
    reservation: ReceiverRequestOriginals,
    signed_request_original: Vec<u8>,
    signature_admission_clock: KagemushaOrdinaryCashClockContextV1,
    accepted_app_attest_counter: Option<u32>,
}

impl ReceiverRequestOriginals {
    /// Called only by Main with no conflicting pending monetary/request operation. Native chooses
    /// the key, request identity and clock. Amount is user intent, never a claimed funding grant.
    /// The result is private WAL data; Main must fsync it before invoking signing_message().
    pub(super) fn create(
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        amount: u128,
    ) -> Result<Self, KagemushaStateErrorV1> {
        owner.require_current_financial_control()?;
        let financial = owner.publication.cash_financial();
        let enrollment = financial.enrollment();
        let credential = enrollment.app_credential();
        let subject = credential.subject();
        let state = &owner.state;
        let clock = financial.current_cash_clock_context().map_err(material)?;
        let expires_at_ms = clock
            .lower_at_ms
            .checked_add(KAGEMUSHA_REQUEST_MAX_TTL_MS_V1)
            .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
            .min(owner.credential_floor()?.approval_valid_until_ms());
        clock
            .validate_within_original_window(clock.lower_at_ms, expires_at_ms)
            .map_err(material)?;
        let mut entropy = Zeroizing::new([0_u8; 64]);
        OsRng.try_fill_bytes(entropy.as_mut()).map_err(material)?;
        let private_key = ReceiverPrivateKey(entropy[..32].try_into().map_err(material)?);
        let public_key = kagemusha_x25519_public_key_v1(&private_key.0).map_err(material)?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-receiver-native-request\0");
        hash.update(owner.prefix.head);
        hash.update(owner.prefix.sequence.to_le_bytes());
        hash.update(state.state_commitment);
        hash.update(credential.digest());
        hash.update(clock.binding_digest().map_err(material)?);
        hash.update(public_key);
        hash.update(&entropy[32..]);
        let body = KagemushaOrdinaryPaymentRequestBodyV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            release_id: state.release_id,
            network_id: *state.lane.network_id.as_bytes(),
            normalized_asset_id: kagemusha_asset_identity_digest_v1(&state.lane.asset)
                .map_err(material)?,
            asset_incarnation: *state.asset_incarnation.as_bytes(),
            scale: state.lane.scale,
            reserve_pool_id: state.liability_pool_id,
            recipient_account_binding: subject.account_binding,
            amount,
            recipient_encryption_key: public_key,
            recipient_credential_digest: credential.digest(),
            recipient_lane_id: subject.lane_id,
            request_id: hash.finalize().into(),
            clock_context: clock,
            issued_at_ms: clock.lower_at_ms,
            expires_at_ms,
        };
        let this = Self {
            publication_originals: owner.publication.original_commitments()?,
            predecessor: state.state_commitment,
            private_key,
            body,
            fi_original: enrollment
                .certificate()
                .canonical_bytes()
                .map_err(material)?,
            credential_original: credential.original().to_vec(),
            possession_original: enrollment.possession().original().to_vec(),
            integrity_lease_original: financial
                .retained_integrity_lease()
                .map(|lease| lease.original().to_vec()),
            previous_app_attest_counter: owner.counter_floor,
        };
        this.recheck_live_source(owner, financial.retained_integrity_lease().map(Arc::as_ref))?;
        Ok(this)
    }

    pub(super) fn request_id(&self) -> DigestV1 {
        self.body.request_id
    }
    pub(super) fn amount(&self) -> u128 {
        self.body.amount
    }
    pub(super) fn clock(&self) -> KagemushaOrdinaryCashClockContextV1 {
        self.body.clock_context
    }
    pub(super) fn original_counter_floor(&self) -> Option<u32> {
        self.previous_app_attest_counter
    }
    pub(super) fn lease_original(&self) -> Option<&[u8]> {
        self.integrity_lease_original.as_deref()
    }
    pub(super) fn credential_original(&self) -> &[u8] {
        &self.credential_original
    }
    pub(super) fn capacity_charge_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        let bytes = Zeroizing::new(norito::encode_canonical(self).map_err(material)?);
        // Charge complete key/source data, maximum model request and retained capture framing.
        u64::try_from(bytes.len())
            .map_err(material)?
            .checked_add((KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_MAX_BYTES_V1 + 256) as u64)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    }

    /// Historical source join at the actual Main WAL position. Original clocks here are private
    /// authenticated WAL data sampled by create(), never caller-supplied clock authority.
    pub(super) fn recheck_at_replay_position(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.predecessor != owner.state.state_commitment
            || self.previous_app_attest_counter != owner.counter_floor
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_original_sources(owner, lease)
    }

    fn recheck_original_sources(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        let enrollment = owner.publication.cash_financial().enrollment();
        let credential = enrollment.app_credential();
        let c = credential.subject();
        let state = &owner.state;
        if self.publication_originals != owner.publication.historical_original_commitments()?
            || self.predecessor == [0; 32]
            || self.fi_original
                != enrollment
                    .certificate()
                    .canonical_bytes()
                    .map_err(material)?
            || self.credential_original != credential.original()
            || self.possession_original != enrollment.possession().original()
            || self.integrity_lease_original.as_deref() != lease.map(|held| held.original())
            || self.body.recipient_encryption_key
                != kagemusha_x25519_public_key_v1(&self.private_key.0).map_err(material)?
            || self.body.release_id != state.release_id
            || self.body.network_id != *state.lane.network_id.as_bytes()
            || self.body.normalized_asset_id
                != kagemusha_asset_identity_digest_v1(&state.lane.asset).map_err(material)?
            || self.body.asset_incarnation != *state.asset_incarnation.as_bytes()
            || self.body.scale != state.lane.scale
            || self.body.reserve_pool_id != state.liability_pool_id
            || self.body.recipient_account_binding != c.account_binding
            || self.body.recipient_credential_digest != credential.digest()
            || self.body.recipient_lane_id != c.lane_id
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.body.validate_shape().map_err(material)?;
        self.require_enrollment_at(enrollment, lease, &self.body.clock_context)
    }

    fn require_enrollment_at(
        &self,
        enrollment: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        clock
            .validate_within_original_window(self.body.issued_at_ms, self.body.expires_at_ms)
            .map_err(material)?;
        for now in [clock.lower_at_ms, clock.upper_at_ms] {
            match lease {
                Some(held) => enrollment.recheck_with_integrity_lease(held, now),
                None => enrollment.recheck_at_trusted_time(now),
            }
            .map_err(material)?;
        }
        Ok(())
    }

    pub(super) fn recheck_live_source(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.require_current_financial_control()?;
        self.recheck_at_replay_position(owner, lease)?;
        let clock = owner
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        self.require_enrollment_at(
            owner.publication.cash_financial().enrollment(),
            lease,
            &clock,
        )
    }

    /// Main must have durably retained Reserve and must not yet have fenced OS dispatch.
    pub(super) fn signing_message(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_live_source(owner, lease)?;
        self.body.canonical_signing_bytes().map_err(material)
    }

    /// Pure data capture following the Main fence. Main fsyncs this complete result before
    /// exposing it or advancing the same global Apple counter. No raw private key is lent.
    pub(super) fn capture_original(
        self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        original: &[u8],
    ) -> Result<CapturedReceiverRequestOriginals, KagemushaStateErrorV1> {
        self.recheck_live_source(owner, lease)?;
        let accepted_app_attest_counter = authenticate_selected_request_data(
            &self.body,
            original,
            owner
                .publication
                .cash_financial()
                .enrollment()
                .app_credential(),
            self.previous_app_attest_counter,
        )?;
        let clock = owner
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        self.require_enrollment_at(
            owner.publication.cash_financial().enrollment(),
            lease,
            &clock,
        )?;
        if clock.lower_at_ms < self.body.clock_context.lower_at_ms
            || clock.upper_at_ms < self.body.clock_context.upper_at_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        owner.require_current_financial_control()?;
        Ok(CapturedReceiverRequestOriginals {
            reservation: self,
            signed_request_original: original.to_vec(),
            signature_admission_clock: clock,
            accepted_app_attest_counter,
        })
    }
}

impl core::fmt::Debug for CapturedReceiverRequestOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CapturedReceiverRequestOriginals")
            .field("request_id", &self.reservation.request_id())
            .finish_non_exhaustive()
    }
}
impl CapturedReceiverRequestOriginals {
    pub(super) fn reservation(&self) -> &ReceiverRequestOriginals {
        &self.reservation
    }
    pub(super) fn original(&self) -> &[u8] {
        &self.signed_request_original
    }
    pub(super) fn accepted_counter(&self) -> Option<u32> {
        self.accepted_app_attest_counter
    }
    pub(super) fn recheck_historical_sources(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.reservation.recheck_original_sources(owner, lease)?;
        self.recheck_capture_data(owner.publication.cash_financial().enrollment(), lease)
    }
    // This source/signature/time comparison lends no Native owner, fsync or funding grant.
    fn recheck_capture_data(
        &self,
        enrollment: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        let clock = &self.signature_admission_clock;
        if clock.lower_at_ms < self.reservation.clock().lower_at_ms
            || clock.upper_at_ms < self.reservation.clock().upper_at_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.reservation
            .require_enrollment_at(enrollment, lease, clock)?;
        if authenticate_selected_request_data(
            &self.reservation.body,
            &self.signed_request_original,
            enrollment.app_credential(),
            self.reservation.previous_app_attest_counter,
        )? != self.accepted_app_attest_counter
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

// Pure exact-source/signature join, not a request/clock/financial custody constructor.
fn authenticate_selected_request_data(
    body: &KagemushaOrdinaryPaymentRequestBodyV1,
    original: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    original_floor: Option<u32>,
) -> Result<Option<u32>, KagemushaStateErrorV1> {
    let request =
        KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(original).map_err(material)?;
    if request.body != *body {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    request
        .authenticate_receiver_signature(credential, original_floor)
        .map(|(counter, _)| counter)
        .map_err(material)
}

#[cfg(test)]
#[path = "ordinary_receiver_request_factory_tests.rs"]
mod tests;
