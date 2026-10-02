//! Independently purpose-governed immutable Core IncomingReserve DATA assertion.
//! Its signature asserts durable DATA CAS; ledger finality authenticates the policy cut only.
//! Historical proof custody creates no live pending-head, clock, FI, debit or State grant.
use super::*;
use crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryIncomingReservationProofV1;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model::kagemusha::*;

const MAX_DATA_RECORD: usize = 128 * 1024;
const MAX_FINALITY_ORIGINAL: usize = 16 * 1024 * 1024;

/// Closed service assertion under the same retained source, installed issuer purpose and actual
/// validator prefix. A decoder, user Financial owner, Boolean or callback cannot create it.
/// The installing service must select the policy from its independently admitted signed runtime
/// inventory. The actual Core effect separately reholds the exact exclusive DATA pending row.
pub struct KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1<'a> {
    source: RetainedIncomingSource<'a>,
    policy: &'a KagemushaOrdinaryLineageIssuerPolicyV1,
    proof: &'a KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    signed: KagemushaSignedOrdinaryLineageResultV1,
    signed_original: Vec<u8>,
    data_record_original: Vec<u8>,
    finality_original: Vec<u8>,
    request_original_sha256: [u8; 32],
}
// Both alternatives are actual retained service capabilities. No raw source archive or
// Native Financial/key borrower can populate this private union.
enum RetainedIncomingSource<'a> {
    Mint(&'a KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'a>),
    Receive(&'a KagemushaAuthenticatedOrdinaryServiceReceivedSourceV1<'a>),
}
impl RetainedIncomingSource<'_> {
    fn recheck(&self) -> Result<()> {
        match self {
            Self::Mint(s) => s.recheck_retained_custody(),
            Self::Receive(s) => s.recheck_retained_custody(),
        }
    }
    fn issuer_policy(&self) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        match self {
            Self::Mint(s) => s.issuer_policy(),
            Self::Receive(s) => s.issuer_policy(),
        }
    }
    fn release(&self) -> Result<&KagemushaAuthenticatedReleaseV1> {
        match self {
            Self::Mint(s) => s.retained_release_for_service_assertion(),
            Self::Receive(s) => s.retained_release_for_service_assertion(),
        }
    }
    fn clock(&self) -> Result<&KagemushaOrdinaryNativeClockOwnerV1> {
        match self {
            Self::Mint(s) => s.retained_clock_for_service_assertion(),
            Self::Receive(s) => s.retained_clock_for_service_assertion(),
        }
    }
    fn require_reservation(&self, expected: &KagemushaOrdinaryIncomingReservationV1) -> Result<()> {
        self.recheck()?;
        expected.validate_shape().map_err(|_| Rejected)?;
        match self {
            Self::Mint(s) => {
                let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                    s.authorization()?.request_original(),
                )
                .map_err(|_| Rejected)?;
                expected
                    .selection
                    .validate_against_topup(&request)
                    .map_err(|_| Rejected)?;
                if expected.finalized_source_original_sha256
                    != <[u8; 32]>::from(Sha256::digest(s.finalized_original()?))
                    || expected.source_semantic_digest != s.source_semantic_digest()?
                    || expected.selection.credit_id != s.credit_statement()?.lifecycle.credit_id
                {
                    return Err(Rejected);
                }
            }
            Self::Receive(s) => {
                let output = s.received_output()?;
                let KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
                    sender_commit_transport_original_sha256,
                    sender_outgoing_original_sha256,
                    recipient_request_original_digest,
                    encrypted_credit_original_sha256,
                } = expected.selection.source
                else {
                    return Err(Rejected);
                };
                let transport_sha =
                    <[u8; 32]>::from(Sha256::digest(s.received_assertion_transport_original()?));
                let outgoing_sha = <[u8; 32]>::from(Sha256::digest(output.outgoing_original()));
                let fi: KagemushaOrdinaryRetailEnrollmentCertificateV1 = decode_exact(
                    s.financial_enrollment_original()?,
                    KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
                )?;
                if sender_commit_transport_original_sha256 != transport_sha
                    || sender_outgoing_original_sha256 != outgoing_sha
                    || recipient_request_original_digest != output.output().request_digest
                    || encrypted_credit_original_sha256
                        != <[u8; 32]>::from(Sha256::digest(output.encrypted_credit()))
                    || expected.finalized_source_original_sha256 != transport_sha
                    || expected.source_proof_original_sha256 != outgoing_sha
                    || expected.source_semantic_digest != output.source_semantic_digest()
                    || expected.selection.lineage.owner != fi.subject.owner
                    || expected.selection.credit_id != output.output().credit_id
                    || expected.selection.amount != output.output().amount
                    || expected.selection.scale != output.request().body.scale
                    || expected.selection.recipient_app_credential_digest
                        != output.request().body.recipient_credential_digest
                {
                    return Err(Rejected);
                }
            }
        }
        self.recheck()
    }
}
impl KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'_> {
    /// Authenticate the exact immutable signed ReserveIncoming under this genuine source owner.
    /// `policy` must be the service's independently installed inventory original, never a field
    /// offered by the proof/HTTP client. This historical assertion cannot hold or advance DATA.
    /// # Errors
    /// Refuses purpose/scope drift, another proof/source, account signature, DATA or finality.
    pub fn authenticate_incoming_reservation_assertion<'a>(
        &'a self,
        policy: &'a KagemushaOrdinaryLineageIssuerPolicyV1,
        proof: &'a KagemushaVerifiedOrdinaryIncomingReservationProofV1,
        signed_original: &[u8],
        data_record_original: &[u8],
        finality_original: &[u8],
    ) -> Result<KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1<'a>> {
        authenticate_assertion(
            RetainedIncomingSource::Mint(self),
            policy,
            proof,
            signed_original,
            data_record_original,
            finality_original,
        )
    }
}
impl KagemushaAuthenticatedOrdinaryServiceReceivedSourceV1<'_> {
    /// Authenticate immutable IncomingReserve from this actual installed Receive source.
    /// The complete signed/DATA/finality originals cannot create a live pending-head grant.
    /// # Errors
    /// Refuses any changed source selector, genuine proof, signature or retained prefix.
    pub fn authenticate_incoming_reservation_assertion<'a>(
        &'a self,
        policy: &'a KagemushaOrdinaryLineageIssuerPolicyV1,
        proof: &'a KagemushaVerifiedOrdinaryIncomingReservationProofV1,
        signed_original: &[u8],
        data_record_original: &[u8],
        finality_original: &[u8],
    ) -> Result<KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1<'a>> {
        authenticate_assertion(
            RetainedIncomingSource::Receive(self),
            policy,
            proof,
            signed_original,
            data_record_original,
            finality_original,
        )
    }
}
fn authenticate_assertion<'a>(
    source: RetainedIncomingSource<'a>,
    policy: &'a KagemushaOrdinaryLineageIssuerPolicyV1,
    proof: &'a KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    signed_original: &[u8],
    data_record_original: &[u8],
    finality_original: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1<'a>> {
    source.recheck()?;
    let signed: KagemushaSignedOrdinaryLineageResultV1 = decode_exact(
        signed_original,
        KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
    )?;
    let request_original_sha256 = Sha256::digest(
        signed
            .subject
            .request
            .canonical_bytes()
            .map_err(|_| Rejected)?,
    )
    .into();
    let value = KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1 {
        source,
        policy,
        proof,
        signed,
        signed_original: bounded_copy(
            signed_original,
            KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
        )?,
        data_record_original: bounded_copy(data_record_original, MAX_DATA_RECORD)?,
        finality_original: bounded_copy(finality_original, MAX_FINALITY_ORIGINAL)?,
        request_original_sha256,
    };
    value.recheck_retained_custody()?;
    Ok(value)
}
impl KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1<'_> {
    /// Exact closed genuine finalized-source/State reservation, never an offered selector.
    /// # Errors
    /// Refuses changed installed/source/original custody.
    pub fn reservation(&self) -> Result<&KagemushaOrdinaryIncomingReservationV1> {
        self.recheck_retained_custody()?;
        Ok(self.proof.reservation())
    }
    /// Complete original signed Core assertion, distinct from the DATA record or ledger proof.
    /// # Errors
    /// Refuses changed installed/source/original custody.
    pub fn original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.signed_original)
    }
    /// Raw SHA256 of the complete unchanged canonical account-signed CAS request.
    /// # Errors
    /// Refuses changed installed/source/original custody.
    pub fn request_original_sha256(&self) -> Result<[u8; 32]> {
        self.recheck_retained_custody()?;
        Ok(self.request_original_sha256)
    }
    /// Genuine same installed FI issuer policy, selected by the source's retained service owner.
    /// # Errors
    /// Refuses changed installed/source/original custody.
    pub fn issuer_policy(&self) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        self.recheck_retained_custody()?;
        self.source.issuer_policy()
    }
    /// Immutable proof custody only. It does not renew current FI/PI/clock or certify that this
    /// pending row is still current. Actual Commit effects recheck the held live DATA row.
    /// # Errors
    /// Refuses exact original, signature, policy, source, protocol or historical-cut changes.
    pub fn recheck_retained_custody(&self) -> Result<()> {
        self.source.recheck()?;
        let issuer = self.source.issuer_policy()?;
        self.policy
            .validate_for_issuer(issuer)
            .map_err(|_| Rejected)?;
        let expected = self.proof.reservation();
        let request = &self.signed.subject.request;
        self.source.require_reservation(expected)?;
        let s = &self.signed.subject;
        if request.operation
            != KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(
                expected.clone(),
            ))
            || request.issuer_policy_digest != self.policy.issuer_policy_digest
            || request.operation.lineage().owner.runtime != self.policy.runtime
            || self.proof.release_id() != self.source.release()?.release_id()
            || s.release_id != self.source.release()?.release_id()
            || s.data_incarnation_digest != self.policy.data_authority.data_incarnation_digest
            || s.cas_policy_digest != self.policy.digest().map_err(|_| Rejected)?
            || self.proof.original_sha256()
                != <[u8; 32]>::from(Sha256::digest(self.proof.original()))
            || s.data_record_original_sha256
                != <[u8; 32]>::from(Sha256::digest(&self.data_record_original))
            || self.signed.canonical_bytes().map_err(|_| Rejected)? != self.signed_original
            || self.request_original_sha256
                != <[u8; 32]>::from(Sha256::digest(
                    request.canonical_bytes().map_err(|_| Rejected)?,
                ))
            || s.issued_at_ms < issuer.valid_from_ms
            || s.issued_at_ms >= issuer.expires_at_ms
        {
            return Err(Rejected);
        }
        self.signed
            .verify_for_request(request, &self.policy.issuer_public_key)
            .map_err(|_| Rejected)?;
        require_data_record(
            &self.data_record_original,
            request,
            self.proof.original_sha256(),
        )?;
        let proof: SumeragiFinalityProof =
            decode_exact(&self.finality_original, MAX_FINALITY_ORIGINAL)?;
        let verifier = self
            .source
            .clock()?
            .retained_finality_verifier_for_original_custody()?;
        let block = verifier
            .verify_retained_decision(&proof)
            .map_err(|_| Rejected)?;
        if block.height() != s.authority_height
            || block.context_id().as_ref() != &s.authority_context_id
            || block.execution().world_state_root.as_ref() != &s.authority_world_root
        {
            return Err(Rejected);
        }
        self.source.recheck()
    }
}
// Core's existing immutable DATA document is strictly decoded as the exact maintained four
// fields. The signed whole-original SHA authenticates every original byte, not JSON reencoding.
pub(super) fn require_data_record(
    raw: &[u8],
    request: &KagemushaOrdinaryLineageRequestV1,
    proof_sha256: [u8; 32],
) -> Result<()> {
    let value: norito::json::Value = norito::json::from_slice(raw).map_err(|_| Rejected)?;
    let fields = value.as_object().ok_or(Rejected)?;
    if fields.len() != 4 {
        return Err(Rejected);
    }
    let string = |key: &str| {
        fields
            .get(key)
            .and_then(norito::json::Value::as_str)
            .ok_or(Rejected)
    };
    if string("schema")? != "iroha.kagemusha.ordinary-lineage-data-result.v1" {
        return Err(Rejected);
    }
    let request_raw = canonical_base64(
        string("canonical_request_base64")?,
        KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
    )?;
    let signature_raw = canonical_base64(string("account_signature_base64")?, 64)?;
    let digest = string("proof_bundle_original_sha256")?;
    if signature_raw.len() != 64
        || request_raw != request.canonical_bytes().map_err(|_| Rejected)?
        || digest != hex::encode(proof_sha256)
    {
        return Err(Rejected);
    }
    request
        .verify_account_signature(&iroha_crypto::Signature::from_bytes(&signature_raw))
        .map_err(|_| Rejected)
}
fn canonical_base64(value: &str, maximum: usize) -> Result<Vec<u8>> {
    let encoded_max = maximum
        .checked_add(2)
        .and_then(|v| v.checked_div(3))
        .and_then(|v| v.checked_mul(4))
        .ok_or(Rejected)?;
    if value.is_empty() || value.len() > encoded_max {
        return Err(Rejected);
    }
    let raw = STANDARD.decode(value).map_err(|_| Rejected)?;
    if raw.is_empty() || raw.len() > maximum || STANDARD.encode(&raw) != value {
        return Err(Rejected);
    }
    Ok(raw)
}
fn bounded_copy(raw: &[u8], maximum: usize) -> Result<Vec<u8>> {
    if raw.is_empty() || raw.len() > maximum {
        return Err(Rejected);
    }
    Ok(raw.to_vec())
}
fn decode_exact<T: norito::NoritoSerialize>(raw: &[u8], maximum: usize) -> Result<T>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    bounded_copy(raw, maximum)?;
    let value: T =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|_| Rejected)?;
    if norito::encode_canonical(&value).map_err(|_| Rejected)? != raw {
        return Err(Rejected);
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_reserve_originals_reject_empty_oversized_and_noncanonical_carriers() {
        assert!(bounded_copy(&[], MAX_DATA_RECORD).is_err());
        assert!(bounded_copy(&vec![1; MAX_DATA_RECORD + 1], MAX_DATA_RECORD).is_err());
        assert!(canonical_base64("AQ", 1).is_err());
        assert!(canonical_base64("AQI=", 1).is_err());
        assert_eq!(canonical_base64("AQ==", 1).unwrap(), vec![1]);
        assert!(
            decode_exact::<KagemushaSignedOrdinaryLineageResultV1>(
                &[1],
                KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1
            )
            .is_err()
        );
    }
}
#[cfg(test)]
mod data_record_tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    // Public codec/signature fixture only. No real State proof, pending DATA or source cap is made.
    fn request() -> KagemushaOrdinaryLineageRequestV1 {
        let fixture =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let mint = fixture.request;
        let context = &mint.authorization.statement.context;
        let selection = KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: context.lineage.clone(),
            operation_id: context.operation_id,
            predecessor: context.predecessor.clone(),
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256: Sha256::digest(mint.canonical_bytes().unwrap())
                    .into(),
            },
            credit_id: mint.authorization.statement.credit_id,
            amount: context.amount,
            scale: context.lineage.owner.runtime.scale,
            recipient_app_credential_digest: context.recipient_app_credential_digest,
            financial_control_original_sha256: context.financial_control_original_sha256,
            clock_context_digest: context.clock_context.binding_digest().unwrap(),
        };
        KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: [61; 32],
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &fixture.enrollment_fixture.issuer_policy,
            )
            .unwrap(),
            operation: KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(
                KagemushaOrdinaryIncomingReservationV1 {
                    selection,
                    finalized_source_original_sha256: [62; 32],
                    source_proof_original_sha256: [63; 32],
                    source_semantic_digest: [64; 32],
                },
            )),
        }
    }
    #[test]
    fn service_reserve_data_record_requires_exact_account_consent_and_whole_admitted_proof_sha() {
        let request = request();
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let signature = Signature::new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        );
        let proof = [65; 32];
        let mut value = norito::json!({
            "schema": "iroha.kagemusha.ordinary-lineage-data-result.v1",
            "canonical_request_base64": (STANDARD.encode(request.canonical_bytes().unwrap())),
            "account_signature_base64": (STANDARD.encode(signature.payload())),
            "proof_bundle_original_sha256": (hex::encode(proof)),
        });
        let raw = norito::json::to_vec(&value).unwrap();
        require_data_record(&raw, &request, proof).unwrap();
        assert!(require_data_record(&raw, &request, [66; 32]).is_err());
        value.as_object_mut().unwrap().insert(
            "account_signature_base64".into(),
            norito::json::Value::String(STANDARD.encode([0; 64])),
        );
        assert!(
            require_data_record(&norito::json::to_vec(&value).unwrap(), &request, proof).is_err()
        );
        value.as_object_mut().unwrap().insert(
            "account_signature_base64".into(),
            norito::json::Value::String(STANDARD.encode(signature.payload())),
        );
        value.as_object_mut().unwrap().insert(
            "offered_root".into(),
            norito::json::Value::String("unauthenticated".into()),
        );
        assert!(
            require_data_record(&norito::json::to_vec(&value).unwrap(), &request, proof).is_err()
        );
    }
}
