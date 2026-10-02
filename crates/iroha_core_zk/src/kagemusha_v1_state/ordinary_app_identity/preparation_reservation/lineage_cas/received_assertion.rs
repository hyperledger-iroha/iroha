//! Independent receiver admission of an immutable purpose-governed Core DATA commit assertion.
//! The delegated signature asserts durable global CAS. Finality authenticates only its selected
//! historical ledger cut; no Taira Merkle membership for DATA is claimed.
use super::*;

/// Complete first-release receipt envelope bound: full historical finality16MiB, DATA128KiB,
/// signed Core result32KiB and bounded canonical framing. It creates no Native authority.
pub const KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1: usize =
    16 * 1024 * 1024 + 192 * 1024;

/// Data-only canonical public envelope. A decoded envelope is never an authenticated receipt.
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryReceivedCommitOriginalV1")]
pub struct KagemushaOrdinaryReceivedLineageCommitOriginalV1 {
    version: u16,
    signed_result_original: Vec<u8>,
    data_record_original: Vec<u8>,
    finality_proof_original: Vec<u8>,
}
impl KagemushaOrdinaryReceivedLineageCommitOriginalV1 {
    /// Encode complete original public components; this performs no issuer/finality admission.
    pub fn from_public_originals(
        signed: Vec<u8>,
        data: Vec<u8>,
        finality: Vec<u8>,
    ) -> Result<Self> {
        let value = Self {
            version: 1,
            signed_result_original: signed,
            data_record_original: data,
            finality_proof_original: finality,
        };
        value.require_data()?;
        Ok(value)
    }
    fn require_data(&self) -> Result<()> {
        if self.version != 1
            || self.data_record_original.is_empty()
            || self.data_record_original.len() > MAX_DATA_RECORD
        {
            return Err(Rejected);
        }
        let signed: KagemushaSignedOrdinaryLineageResultV1 = decode(
            &self.signed_result_original,
            KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
        )?;
        if !matches!(
            signed.subject.request.operation,
            KagemushaOrdinaryLineageRequestOperationV1::Commit(_)
        ) || signed.subject.data_record_original_sha256
            != <[u8; 32]>::from(Sha256::digest(&self.data_record_original))
        {
            return Err(Rejected);
        }
        decode::<iroha_data_model::sumeragi_finality::SumeragiFinalityProof>(
            &self.finality_proof_original,
            MAX_FINALITY_ORIGINAL,
        )?;
        Ok(())
    }
    /// Sole bounded canonical envelope; bytes alone establish no signature or global effect.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.require_data()?;
        let raw = norito::encode_canonical(self).map_err(|_| Rejected)?;
        if raw.len() > KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1 {
            return Err(Rejected);
        }
        Ok(raw)
    }
    /// Strict data decoder, never a Native owner constructor.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        let value: Self = decode(
            raw,
            KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1,
        )?;
        if value.canonical_bytes()? != raw {
            return Err(Rejected);
        }
        Ok(value)
    }
}

/// Distinct receiver-side immutable assertion authenticated under its own installed issuer
/// purpose/runtime and real clock/finality custody. No sender loan or decoder constructs it.
pub struct KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'a> {
    owner: &'a KagemushaOrdinaryLineageCasOwnerV1,
    financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
    original: KagemushaOrdinaryReceivedLineageCommitOriginalV1,
    signed: KagemushaSignedOrdinaryLineageResultV1,
}
impl KagemushaOrdinaryLineageCasOwnerV1 {
    /// Receiver-only creation after genuine fresh same-owner FI control. This is historical
    /// assertion custody; actual Receive effects still require a separately fresh loan and
    /// one-use receiver key/inbox plus complete Receive State proof and global serialization.
    pub(crate) fn authenticate_received_commit_assertion<'a>(
        &'a self,
        financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        original: &[u8],
    ) -> Result<KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'a>> {
        self.recheck_live(financial, current)?;
        let original = KagemushaOrdinaryReceivedLineageCommitOriginalV1::decode_original(original)?;
        let signed = self.verify_received_assertion_original(&original)?;
        // The old committed DATA assertion must belong to the actual current DATA incarnation
        // and policy/schema. A stale revision is immutable history, never today's live grant.
        let control: KagemushaSignedOrdinaryCurrentControlV1 = decode(
            current.original()?,
            KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        )?;
        let subject = &signed.subject;
        if control.subject.data_incarnation_digest != subject.data_incarnation_digest
            || control.subject.data_policy_epoch != subject.data_policy_epoch
            || control.subject.data_schema_epoch != subject.data_schema_epoch
            || control.subject.data_revision < subject.data_revision
            || control.subject.authority_height < subject.authority_height
        {
            return Err(Rejected);
        }
        self.recheck_live(financial, current)?;
        Ok(
            KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1 {
                owner: self,
                financial,
                original,
                signed,
            },
        )
    }
    fn verify_received_assertion_original(
        &self,
        original: &KagemushaOrdinaryReceivedLineageCommitOriginalV1,
    ) -> Result<KagemushaSignedOrdinaryLineageResultV1> {
        self.recheck_journal()?;
        original.require_data()?;
        self.policy
            .validate_for_issuer(&self.selected.issuer)
            .map_err(|_| Rejected)?;
        let signed: KagemushaSignedOrdinaryLineageResultV1 = decode(
            &original.signed_result_original,
            KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
        )?;
        signed
            .verify_for_request(&signed.subject.request, &self.policy.issuer_public_key)
            .map_err(|_| Rejected)?;
        if signed.subject.cas_policy_digest != self.policy.digest().map_err(|_| Rejected)?
            || signed.subject.release_id != self.selected.governed.release().release_id()
            || signed.subject.request.issuer_policy_digest != self.policy.issuer_policy_digest
            || signed.subject.request.operation.lineage().owner.runtime != self.policy.runtime
        {
            return Err(Rejected);
        }
        // Same installed finality prefix, but no sender-owned WAL/pending request is assumed.
        self.verify_retained_finality(&signed.subject, &original.finality_proof_original)?;
        Ok(signed)
    }
}
impl KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'_> {
    /// Historical custody check only. It does not renew FI, time or permit inbox/State effects.
    pub fn recheck_historical(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.owner.recheck_historical(financial)?;
        self.owner.recheck_historical(self.financial)?;
        if self
            .owner
            .verify_received_assertion_original(&self.original)?
            != self.signed
        {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Exact immutable committed selection under the independently installed purpose.
    pub fn commit(&self) -> Result<&KagemushaOrdinaryLineageCommitV1> {
        self.recheck_historical(self.financial)?;
        match &self.signed.subject.request.operation {
            KagemushaOrdinaryLineageRequestOperationV1::Commit(value) => Ok(value),
            _ => Err(Rejected),
        }
    }
    /// Exact complete original Core signature asserting DATA CAS, without a DATA membership claim.
    pub fn original(&self) -> Result<&[u8]> {
        self.recheck_historical(self.financial)?;
        Ok(&self.original.signed_result_original)
    }
    /// Actual independently installed sender FI issuer policy, never an offered decoder key.
    /// Only this installed runtime scope is supported by this capability.
    pub fn issuer_policy(&self) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        self.recheck_historical(self.financial)?;
        Ok(&self.owner.selected.issuer)
    }
    /// Exact public envelope, retained without duplicating sender Native ownership.
    pub fn transport_original(&self) -> Result<Vec<u8>> {
        self.recheck_historical(self.financial)?;
        self.original.canonical_bytes()
    }
}
impl KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1<'_> {
    /// Produce the complete receiver envelope from this same acknowledged sender WAL original.
    /// The projection cannot reconstruct the sender holder or authorize receiver effects.
    pub(crate) fn receiver_original(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<u8>> {
        let actual = self.owner.acknowledged(self.request_sha256, financial)?;
        if !matches!(
            actual.request.operation,
            KagemushaOrdinaryLineageRequestOperationV1::Commit(_)
        ) {
            return Err(Rejected);
        }
        KagemushaOrdinaryReceivedLineageCommitOriginalV1::from_public_originals(
            actual.originals.signed_result.clone(),
            actual.originals.data_record.clone(),
            actual.originals.finality_original.clone(),
        )?
        .canonical_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn received_assertion_decoder_rejects_empty_oversized_and_noncanonical_before_authority() {
        assert!(KagemushaOrdinaryReceivedLineageCommitOriginalV1::decode_original(&[]).is_err());
        let oversized = vec![0; KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1 + 1];
        assert!(
            KagemushaOrdinaryReceivedLineageCommitOriginalV1::decode_original(&oversized).is_err()
        );
        let invalid = KagemushaOrdinaryReceivedLineageCommitOriginalV1 {
            version: 2,
            signed_result_original: vec![1],
            data_record_original: vec![1],
            finality_proof_original: vec![1],
        };
        assert!(invalid.canonical_bytes().is_err());
        assert!(
            KagemushaOrdinaryReceivedLineageCommitOriginalV1::from_public_originals(
                vec![1],
                vec![1],
                vec![1],
            )
            .is_err()
        );
    }
}
