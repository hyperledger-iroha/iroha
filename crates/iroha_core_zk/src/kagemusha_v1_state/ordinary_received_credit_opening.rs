//! Closed Native receiver request custody and authenticated credit opening.
//!
//! The Main WAL owns the captured request/key. This child never consumes a key or funds a
//! balance: those effects require the later whole Receive proof and acknowledged global head.

use super::*;
use crate::kagemusha_v1_crypto::open_kagemusha_credit_v1;
use crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryReceivedCashOutputV1;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1;
use iroha_data_model::kagemusha::{
    KagemushaCreditOpeningV1, KagemushaEncryptedCreditEnvelopeV1, KagemushaOrdinaryPaymentOutputV1,
    KagemushaRetailEnrollmentIssuerPolicyV1, kagemusha_ciphertext_digest_v1,
    kagemusha_peer_credit_opening_commitment_v1,
};

/// Borrow of the same Main-owned, fsynced receiver request; no decoder or raw key creates it.
/// The exact WAL prefix and current FI custody are rechecked on each use.
pub(crate) struct KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

/// Read-only historical borrow of the same actual Main-retained request and nonexported key.
/// It rechecks original journal/captured FI/C/PI custody and lends no current FI/time authority.
pub(crate) struct KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

pub(super) fn from_main_historical(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
) -> Result<KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'_>, KagemushaStateErrorV1> {
    let loan = KagemushaHistoricalOrdinaryReceiverRequestCustodyV1 {
        owner,
        request_id,
        prefix: owner.prefix,
    };
    loan.recheck_historical_custody()?;
    Ok(loan)
}

impl<'owner> KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'owner> {
    /// Same actual installed receiver FI policy, borrowed only through this retained original
    /// request and financial custody. It supplies no current authorization or offered policy.
    pub(crate) fn issuer_policy(
        &self,
    ) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1, KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        let policy = self
            .owner
            .publication
            .cash_financial()
            .retained_proof_issuer_policy()
            .map_err(material)?;
        self.recheck_historical_custody()?;
        Ok(policy)
    }

    fn retained(&self) -> Result<&super::super::RetainedReceiverRequest, KagemushaStateErrorV1> {
        self.owner
            .retained_receiver_requests
            .get(&self.request_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    pub(crate) fn recheck_historical_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.owner.recovery_failed
            || self.owner.recovery_catalog.is_some()
            || self.owner.prefix != self.prefix
            || self.owner.journal.recovery_prefix().map_err(storage)? != self.prefix
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.journal.check_owned().map_err(storage)?;
        self.owner.publication.recheck_historical_cash_custody()?;
        self.owner.recheck_lineage_retained_custody()?;
        let retained = self.retained()?;
        if retained.captured.reservation().request_id() != self.request_id
            || !self.owner.used_operations.contains(&self.request_id)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.require_receiver_request_control(
            retained.financial_control,
            retained.captured.reservation(),
        )?;
        retained
            .captured
            .recheck_historical_sources(self.owner, retained.lease.as_deref())?;
        self.owner
            .publication
            .cash_financial()
            .recheck_historical_proof_custody()
            .map_err(material)?;
        self.owner.journal.check_owned().map_err(storage)
    }
    pub(crate) fn request_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        Ok(self.retained()?.captured.original())
    }
    pub(crate) fn enrollment(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, KagemushaStateErrorV1>
    {
        self.recheck_historical_custody()?;
        Ok(self.owner.publication.cash_financial().enrollment())
    }
    pub(crate) fn previous_app_attest_counter(&self) -> Result<Option<u32>, KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        Ok(self
            .retained()?
            .captured
            .reservation
            .previous_app_attest_counter)
    }
    /// Exact PI selected when this same Main request was durably captured. This historical
    /// loan never replaces it with the current incoming approval's PI or extends its window.
    pub(crate) fn selected_integrity_lease(
        &self,
    ) -> Result<Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>, KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        Ok(self.retained()?.lease.as_deref())
    }
    /// Original request creation context, retained by Main before the platform fence.
    /// This context is public DATA; the signed-clock visitor below supplies authentic custody.
    pub(crate) fn request_clock_context(
        &self,
    ) -> Result<KagemushaOrdinaryCashClockContextV1, KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        Ok(self.retained()?.captured.reservation.clock())
    }
    /// Actual request-signature CaptureAck context, unchanged by later PI/counter refresh.
    pub(crate) fn signature_admission_clock_context(
        &self,
    ) -> Result<KagemushaOrdinaryCashClockContextV1, KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        Ok(self.retained()?.captured.signature_admission_clock)
    }
    /// Lend both complete genuine signed clock cuts under this same Main RequestCapture.
    /// Historical clock proof custody never lends a current elapsed clock or financial effect.
    pub(crate) fn with_retained_verified_signed_clock_originals(
        &self,
        visitor: &mut dyn for<'clock> FnMut(
            [&'clock KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 2],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        let financial = self.owner.publication.cash_financial();
        let request_context = self.request_clock_context()?;
        let admission_context = self.signature_admission_clock_context()?;
        let request = financial
            .verified_retained_cash_clock_originals(&request_context)
            .map_err(material)?;
        let admission = financial
            .verified_retained_cash_clock_originals(&admission_context)
            .map_err(material)?;
        request
            .recheck_cash_context(&request_context)
            .map_err(material)?;
        admission
            .recheck_cash_context(&admission_context)
            .map_err(material)?;
        visitor([&request, &admission])?;
        self.recheck_historical_custody()
    }
    pub(crate) fn financial_owner(
        &self,
    ) -> Result<&KagemushaOrdinaryEnrolledFinancialOwnerV1, KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        Ok(self.owner.publication.cash_financial())
    }
    pub(crate) fn open_received<'loan, 'assertion>(
        &'loan self,
        admitted: &'loan KagemushaVerifiedOrdinaryReceivedCashOutputV1,
        assertion: &'loan KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<
            'assertion,
        >,
    ) -> Result<
        KagemushaHistoricalOrdinaryReceivedCreditOpeningV1<'loan, 'owner, 'assertion>,
        KagemushaStateErrorV1,
    > {
        self.require_admitted_output(admitted, assertion)?;
        let captured = &self.retained()?.captured;
        let opening = open_selected_original_data(
            &captured.reservation.private_key.0,
            captured.original(),
            admitted.output(),
            admitted.encrypted_credit(),
            admitted.preparation_clock(),
        )?;
        let loan = KagemushaHistoricalOrdinaryReceivedCreditOpeningV1 {
            request_custody: self,
            admitted,
            assertion,
            opening,
        };
        loan.recheck_historical_custody()?;
        Ok(loan)
    }
    fn require_admitted_output(
        &self,
        admitted: &KagemushaVerifiedOrdinaryReceivedCashOutputV1,
        assertion: &KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'_>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        assertion
            .recheck_historical(self.financial_owner()?)
            .map_err(material)?;
        let commit = assertion.commit().map_err(material)?;
        let transport = assertion.transport_original().map_err(material)?;
        if admitted.request_original() != self.request_original()?
            || admitted.receiver_credential_digest() != self.enrollment()?.app_credential().digest()
            || admitted.received_assertion_original_sha256()
                != <DigestV1>::from(Sha256::digest(&transport))
            || admitted.commit() != commit
            || commit.outgoing_original_sha256
                != <DigestV1>::from(Sha256::digest(admitted.outgoing_original()))
            || admitted.output().encrypted_credit_digest
                != kagemusha_ciphertext_digest_v1(admitted.encrypted_credit())
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_historical_custody()
    }
}

/// Distinct historical cryptographic opening. No current-funding API accepts this loan.
pub(crate) struct KagemushaHistoricalOrdinaryReceivedCreditOpeningV1<'loan, 'owner, 'assertion> {
    request_custody: &'loan KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'owner>,
    admitted: &'loan KagemushaVerifiedOrdinaryReceivedCashOutputV1,
    assertion: &'loan KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'assertion>,
    opening: PrivateReceivedCreditOpening,
}
impl KagemushaHistoricalOrdinaryReceivedCreditOpeningV1<'_, '_, '_> {
    pub(crate) fn recheck_historical_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.request_custody
            .require_admitted_output(self.admitted, self.assertion)?;
        require_opening_commitment(
            &self.opening.0,
            self.request_custody.request_original()?,
            self.admitted.output(),
        )?;
        self.request_custody.recheck_historical_custody()
    }
    pub(crate) fn with_borrowed_credit_opening(
        &self,
        consume: &mut dyn for<'secret> FnMut(
            &'secret KagemushaCreditOpeningV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_historical_custody()?;
        let result = consume(&self.opening.0);
        self.recheck_historical_custody()?;
        result
    }
}

// Only the private Main receiver-request module forwards this constructor.
pub(super) fn from_main(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
) -> Result<KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'_>, KagemushaStateErrorV1> {
    let loan = KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1 {
        owner,
        request_id,
        prefix: owner.prefix,
    };
    loan.recheck_current_custody()?;
    Ok(loan)
}

impl<'owner> KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'owner> {
    fn retained(&self) -> Result<&super::super::RetainedReceiverRequest, KagemushaStateErrorV1> {
        self.owner
            .retained_receiver_requests
            .get(&self.request_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }

    /// Require the actual current FI owner, full captured sources and unchanged private prefix.
    pub(crate) fn recheck_current_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.require_current_financial_control()?;
        if self.owner.prefix != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let retained = self.retained()?;
        if retained.captured.reservation().request_id() != self.request_id {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.require_receiver_request_control(
            retained.financial_control,
            retained.captured.reservation(),
        )?;
        retained
            .captured
            .recheck_historical_sources(self.owner, retained.lease.as_deref())?;
        self.owner.require_current_financial_control()
    }

    /// Complete exact signed request retained after the irreversible platform fence and fsync.
    pub(crate) fn request_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck_current_custody()?;
        Ok(self.retained()?.captured.original())
    }

    /// Independently admitted same receiver enrollment; never an offered credential decoder.
    pub(crate) fn enrollment(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, KagemushaStateErrorV1>
    {
        self.recheck_current_custody()?;
        Ok(self.owner.publication.cash_financial().enrollment())
    }

    /// Original request-signing floor, distinct from the global counter advanced by capture.
    pub(crate) fn previous_app_attest_counter(&self) -> Result<Option<u32>, KagemushaStateErrorV1> {
        self.recheck_current_custody()?;
        Ok(self
            .retained()?
            .captured
            .reservation
            .previous_app_attest_counter)
    }

    /// Actual financial owner borrow under the identical current request custody checks.
    pub(crate) fn financial_owner(
        &self,
    ) -> Result<&KagemushaOrdinaryEnrolledFinancialOwnerV1, KagemushaStateErrorV1> {
        self.recheck_current_custody()?;
        Ok(self.owner.publication.cash_financial())
    }

    /// Open only the exact output admitted by the real compact Wrapper and independent Core
    /// received Commit assertion. No software callback or decoded transport grants decryption.
    /// The key remains retained for crash recovery and the later acknowledged Receive effect.
    pub(crate) fn open_received<'loan, 'assertion>(
        &'loan self,
        admitted: &'loan KagemushaVerifiedOrdinaryReceivedCashOutputV1,
        assertion: &'loan KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<
            'assertion,
        >,
    ) -> Result<
        KagemushaAuthenticatedOrdinaryReceivedCreditOpeningV1<'loan, 'owner, 'assertion>,
        KagemushaStateErrorV1,
    > {
        self.require_admitted_output(admitted, assertion)?;
        let captured = &self.retained()?.captured;
        let opening = open_selected_original_data(
            &captured.reservation.private_key.0,
            captured.original(),
            admitted.output(),
            admitted.encrypted_credit(),
            admitted.preparation_clock(),
        )?;
        let loan = KagemushaAuthenticatedOrdinaryReceivedCreditOpeningV1 {
            request_custody: self,
            admitted,
            assertion,
            opening,
        };
        loan.recheck_current_custody()?;
        Ok(loan)
    }

    fn require_admitted_output(
        &self,
        admitted: &KagemushaVerifiedOrdinaryReceivedCashOutputV1,
        assertion: &KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'_>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_custody()?;
        assertion
            .recheck_historical(self.financial_owner()?)
            .map_err(material)?;
        let commit = assertion.commit().map_err(material)?;
        let transport_original = assertion.transport_original().map_err(material)?;
        let captured = &self.retained()?.captured;
        if admitted.request_original() != captured.original()
            || admitted.receiver_credential_digest() != self.enrollment()?.app_credential().digest()
            || admitted.received_assertion_original_sha256()
                != <DigestV1>::from(Sha256::digest(&transport_original))
            || admitted.commit() != commit
            || commit.outgoing_original_sha256
                != <DigestV1>::from(Sha256::digest(admitted.outgoing_original()))
            || admitted.output().encrypted_credit_digest
                != kagemusha_ciphertext_digest_v1(admitted.encrypted_credit())
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_current_custody()
    }
}

// The maintained AEAD helper returns an owning opening. Wrap it immediately, before any
// subsequent semantic comparison can fail, so all three secrets clear on every error/drop.
struct PrivateReceivedCreditOpening(KagemushaCreditOpeningV1);
impl Drop for PrivateReceivedCreditOpening {
    fn drop(&mut self) {
        self.0.credit_commitment_opening.zeroize();
        self.0.recipient_binding_opening.zeroize();
        self.0.recovery_nonce.zeroize();
    }
}

/// Closed opening borrowed only by Native Receive witnesses, with no key/plaintext exporter.
/// This capability creates no balance, replay insertion, key retirement or global head effect.
pub(crate) struct KagemushaAuthenticatedOrdinaryReceivedCreditOpeningV1<'loan, 'owner, 'assertion> {
    request_custody: &'loan KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'owner>,
    admitted: &'loan KagemushaVerifiedOrdinaryReceivedCashOutputV1,
    assertion: &'loan KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'assertion>,
    opening: PrivateReceivedCreditOpening,
}

impl KagemushaAuthenticatedOrdinaryReceivedCreditOpeningV1<'_, '_, '_> {
    /// Recheck the same Main prefix, actual current FI and independently held receipt originals.
    pub(crate) fn recheck_current_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.request_custody
            .require_admitted_output(self.admitted, self.assertion)?;
        require_opening_commitment(
            &self.opening.0,
            self.request_custody.request_original()?,
            self.admitted.output(),
        )?;
        self.request_custody.recheck_current_custody()
    }
}

// Private data check used only after closed receipt/proof admission. Test specimens call this
// directly to check actual cryptography; they do not construct a Native owner or funding grant.
fn open_selected_original_data(
    private_key: &[u8; 32],
    request_original: &[u8],
    output: &KagemushaOrdinaryPaymentOutputV1,
    encrypted_credit: &[u8],
    preparation_clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<PrivateReceivedCreditOpening, KagemushaStateErrorV1> {
    let request = KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(request_original)
        .map_err(material)?;
    if output.encrypted_credit_digest != kagemusha_ciphertext_digest_v1(encrypted_credit) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let aad = output
        .encrypted_credit_aad_against(&request, preparation_clock)
        .map_err(material)?;
    let envelope =
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            encrypted_credit,
            request.body.recipient_encryption_key,
        )
        .map_err(material)?;
    let opening = PrivateReceivedCreditOpening(
        open_kagemusha_credit_v1(
            &envelope,
            &aad,
            request.body.recipient_encryption_key,
            private_key,
        )
        .map_err(material)?,
    );
    require_opening_commitment(&opening.0, request_original, output)?;
    Ok(opening)
}

fn require_opening_commitment(
    opening: &KagemushaCreditOpeningV1,
    request_original: &[u8],
    output: &KagemushaOrdinaryPaymentOutputV1,
) -> Result<(), KagemushaStateErrorV1> {
    let request = KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(request_original)
        .map_err(material)?;
    let request_digest = request.canonical_original_digest().map_err(material)?;
    if output.request_digest != request_digest
        || output.amount != request.body.amount
        || opening.amount != output.amount
        || opening.credit_id != output.credit_id
        || kagemusha_peer_credit_opening_commitment_v1(
            request_digest,
            request.body.recipient_encryption_key,
            opening.amount,
            opening.credit_commitment_opening,
            opening.recipient_binding_opening,
            opening.recovery_nonce,
        )
        .map_err(material)?
            != output.ciphertext_commitment
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

#[cfg(test)]
#[path = "ordinary_received_credit_opening_tests.rs"]
mod tests;
