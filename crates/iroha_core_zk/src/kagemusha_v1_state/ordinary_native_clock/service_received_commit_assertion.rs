//! Service receiver admission of the exact immutable Core-signed DATA Commit assertion.
//! The Core signature asserts durable global exclusion. Retained ledger finality authenticates
//! its historical policy cut, never DATA Merkle membership or a current receiver money grant.
use super::service_incoming_reservation_assertion::require_data_record;
use super::*;
use crate::kagemusha_v1_state::{
    KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1,
    KagemushaOrdinaryReceivedLineageCommitOriginalV1,
};
use iroha_data_model::kagemusha::*;

/// Closed immutable received assertion from the independently installed service clock, release
/// and signing purpose. No sender Financial/key borrower, decoded receipt or callback makes it.
/// The shipping Core factory supplies its descriptor-backed originals and actual current World
/// DATA selection; clients cannot supply or replace these roots at that entry point.
pub struct KagemushaAuthenticatedOrdinaryServiceReceivedLineageCommitAssertionV1<'a> {
    clock: &'a KagemushaOrdinaryNativeClockOwnerV1,
    release: &'a KagemushaAuthenticatedReleaseV1,
    issuer: &'a KagemushaRetailEnrollmentIssuerPolicyV1,
    policy: &'a KagemushaOrdinaryLineageIssuerPolicyV1,
    envelope: KagemushaOrdinaryReceivedLineageCommitOriginalV1,
    transport_original: Vec<u8>,
    signed: KagemushaSignedOrdinaryLineageResultV1,
}
impl KagemushaOrdinaryNativeClockOwnerV1 {
    /// Authenticate immutable sender history under this actual clock's installed validator prefix.
    /// The actual Core caller must hold independently installed release/issuer/purpose originals
    /// and prove their full DATA selection in current World. Offered receipt fields select none.
    /// No Native Financial owner, elapsed reading or receiver plaintext is lent.
    /// # Errors
    /// Refuses changed purpose, release, signature, account consent, DATA original or finality.
    pub fn authenticate_service_received_lineage_commit_assertion<'a>(
        &'a self,
        release: &'a KagemushaAuthenticatedReleaseV1,
        issuer: &'a KagemushaRetailEnrollmentIssuerPolicyV1,
        policy: &'a KagemushaOrdinaryLineageIssuerPolicyV1,
        transport_original: &[u8],
    ) -> Result<KagemushaAuthenticatedOrdinaryServiceReceivedLineageCommitAssertionV1<'a>> {
        self.recheck()?;
        if transport_original.is_empty()
            || transport_original.len() > KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1
        {
            return Err(Rejected);
        }
        let envelope =
            KagemushaOrdinaryReceivedLineageCommitOriginalV1::decode_original(transport_original)
                .map_err(|_| Rejected)?;
        let raw = envelope.signed_result_original().map_err(|_| Rejected)?;
        let signed: KagemushaSignedOrdinaryLineageResultV1 =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|_| Rejected)?;
        let value = KagemushaAuthenticatedOrdinaryServiceReceivedLineageCommitAssertionV1 {
            clock: self,
            release,
            issuer,
            policy,
            envelope,
            transport_original: transport_original.to_vec(),
            signed,
        };
        value.recheck_retained_custody()?;
        Ok(value)
    }
}
impl KagemushaAuthenticatedOrdinaryServiceReceivedLineageCommitAssertionV1<'_> {
    pub(super) fn retained_release_for_received_source(
        &self,
    ) -> Result<&KagemushaAuthenticatedReleaseV1> {
        self.recheck_retained_custody()?;
        Ok(self.release)
    }

    // Borrow only the exact actual installed Clock WAL/prefix that admitted this assertion.
    // Sibling source consumers may reauthenticate complete old originals, never current time.
    pub(super) fn retained_clock_for_received_source(
        &self,
    ) -> Result<&KagemushaOrdinaryNativeClockOwnerV1> {
        self.recheck_retained_custody()?;
        Ok(self.clock)
    }
    /// Exact installed-purpose committed selection; immutable history creates no receiver funds.
    /// # Errors
    /// Refuses changed retained signature, prefix or installation identity.
    pub fn commit(&self) -> Result<&KagemushaOrdinaryLineageCommitV1> {
        self.recheck_retained_custody()?;
        match &self.signed.subject.request.operation {
            KagemushaOrdinaryLineageRequestOperationV1::Commit(value) => Ok(value),
            _ => Err(Rejected),
        }
    }
    /// Complete signed Core result original, distinct from the public receipt transport.
    /// # Errors
    /// Refuses changed retained custody.
    pub fn original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        self.envelope.signed_result_original().map_err(|_| Rejected)
    }
    /// Exact full canonical receipt transport, including DATA original and historical finality.
    /// # Errors
    /// Refuses changed retained custody.
    pub fn transport_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.transport_original)
    }
    /// Same descriptor-backed installed issuer policy, never a key chosen by the sender archive.
    /// # Errors
    /// Refuses changed retained custody.
    pub fn issuer_policy(&self) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        self.recheck_retained_custody()?;
        Ok(self.issuer)
    }
    /// Full historical signature/finality custody only. Old issuance timestamps and sender
    /// observations never become current Core/receiver time, FI, PI, pending DATA or funds.
    /// # Errors
    /// Refuses any changed full original, purpose, release, account signature or retained prefix.
    pub fn recheck_retained_custody(&self) -> Result<()> {
        self.clock.recheck()?;
        self.policy
            .validate_for_issuer(self.issuer)
            .map_err(|_| Rejected)?;
        let subject = &self.signed.subject;
        let request = &subject.request;
        let KagemushaOrdinaryLineageRequestOperationV1::Commit(commit) = &request.operation else {
            return Err(Rejected);
        };
        commit.validate_shape().map_err(|_| Rejected)?;
        if self.envelope.canonical_bytes().map_err(|_| Rejected)? != self.transport_original
            || self.signed.canonical_bytes().map_err(|_| Rejected)?
                != self
                    .envelope
                    .signed_result_original()
                    .map_err(|_| Rejected)?
            || self.clock.network_id()? != self.policy.runtime.network_id
            || self.release.network_id() != self.policy.runtime.network_id
            || subject.release_id != self.release.release_id()
            || request.operation.lineage().owner.runtime != self.policy.runtime
            || request.issuer_policy_digest != self.policy.issuer_policy_digest
            || subject.cas_policy_digest != self.policy.digest().map_err(|_| Rejected)?
            || subject.data_incarnation_digest != self.policy.data_authority.data_incarnation_digest
            || subject.issued_at_ms < self.issuer.valid_from_ms
            || subject.issued_at_ms >= self.issuer.expires_at_ms
            || subject.data_record_original_sha256
                != <[u8; 32]>::from(Sha256::digest(
                    self.envelope.data_record_original().map_err(|_| Rejected)?,
                ))
        {
            return Err(Rejected);
        }
        self.signed
            .verify_for_request(request, &self.policy.issuer_public_key)
            .map_err(|_| Rejected)?;
        let data = self.envelope.data_record_original().map_err(|_| Rejected)?;
        let proof_digest = received_data_proof_digest(data)?;
        require_data_record(data, request, proof_digest)?;
        let raw = self
            .envelope
            .finality_proof_original()
            .map_err(|_| Rejected)?;
        let proof: SumeragiFinalityProof =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|_| Rejected)?;
        let block = self
            .clock
            .retained_finality_verifier_for_original_custody()?
            .verify_retained_decision(&proof)
            .map_err(|_| Rejected)?;
        if block.height() != subject.authority_height
            || block.context_id().as_ref() != &subject.authority_context_id
            || block.execution().world_state_root.as_ref() != &subject.authority_world_root
        {
            return Err(Rejected);
        }
        self.clock.recheck()
    }
}
// The whole DATA preimage is bound by the admitted Core signature. The selected proof digest
// remains data here; the independent compact Wrapper verifier must still decide actual proofs.
fn received_data_proof_digest(raw: &[u8]) -> Result<[u8; 32]> {
    if raw.is_empty() || raw.len() > 128 * 1024 {
        return Err(Rejected);
    }
    let value: norito::json::Value = norito::json::from_slice(raw).map_err(|_| Rejected)?;
    let s = value
        .as_object()
        .ok_or(Rejected)?
        .get("proof_bundle_original_sha256")
        .and_then(norito::json::Value::as_str)
        .ok_or(Rejected)?;
    let bytes: [u8; 32] = hex::decode(s)
        .map_err(|_| Rejected)?
        .try_into()
        .map_err(|_| Rejected)?;
    if bytes == [0; 32] || s != hex::encode(bytes) {
        return Err(Rejected);
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn received_data_digest_is_complete_canonical_nonzero_selector_only() {
        assert!(received_data_proof_digest(&[]).is_err());
        assert!(received_data_proof_digest(b"{}").is_err());
        for invalid in ["00".repeat(32), "AA".repeat(32), "a".repeat(63)] {
            let raw =
                norito::json::to_vec(&norito::json!({"proof_bundle_original_sha256": invalid}))
                    .unwrap();
            assert!(received_data_proof_digest(&raw).is_err());
        }
        let digest = [7; 32];
        let raw = norito::json::to_vec(
            &norito::json!({"proof_bundle_original_sha256": (hex::encode(digest))}),
        )
        .unwrap();
        assert_eq!(received_data_proof_digest(&raw).unwrap(), digest);
        assert!(received_data_proof_digest(&vec![b' '; 128 * 1024 + 1]).is_err());
        // This codec result is not an assertion capability or proof admission.
    }
}
