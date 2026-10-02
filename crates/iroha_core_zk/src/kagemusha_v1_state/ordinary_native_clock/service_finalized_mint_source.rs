//! Service-only immutable Mint source under the actual retained Native validator prefix.
//! Source proof custody lends no user secret, elapsed time, current FI, DATA head or funds.
use super::*;
use crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1;
use iroha_data_model::kagemusha::*;

/// Closed historical source for stateless incoming proofs; no decoder, Clone or user financial
/// owner conversion. The issuing service retains its independently installed release/issuer
/// originals, and the genuine original Node execution certifies the prior reserve debit.
pub struct KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'a> {
    clock: &'a KagemushaOrdinaryNativeClockOwnerV1,
    release: &'a KagemushaAuthenticatedReleaseV1,
    issuer: &'a KagemushaRetailEnrollmentIssuerPolicyV1,
    authorization: KagemushaVerifiedOrdinaryMintAuthorizationV1,
    finalized: KagemushaOrdinaryTopUpFinalizedOriginalV1,
    finalized_original: Vec<u8>,
    financial_enrollment_original: Vec<u8>,
    preparation_control_original: Vec<u8>,
    credit_statement: KagemushaMintCreditStatementV1,
}
impl KagemushaOrdinaryNativeClockOwnerV1 {
    /// Authenticate full historical debit data under this actual installed clock/prefix.
    /// `release`/`issuer` MUST originate in the service's held independent runtime installation
    /// after actual World-purpose admission. No response, JNI input or decoded source selects
    /// these Native roots. Original Node finality authenticates the applied reserve receipt,
    /// whose purpose-bound intent binds BOTH the complete request and original signed decision.
    /// Current service World/FI/PI/clock and exclusive DATA remain separate effect gates.
    /// # Errors
    /// Refuses changed custody, original/policy/window drift, absent Mint113 or actual finality.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate_service_finalized_mint_source<'a>(
        &'a self,
        release: &'a KagemushaAuthenticatedReleaseV1,
        issuer: &'a KagemushaRetailEnrollmentIssuerPolicyV1,
        authorization: KagemushaVerifiedOrdinaryMintAuthorizationV1,
        original: &[u8],
        financial_enrollment_original: &[u8],
        preparation_control_original: &[u8],
    ) -> Result<KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'a>> {
        self.recheck()?;
        let finalized = KagemushaOrdinaryTopUpFinalizedOriginalV1::decode_canonical_exact(original)
            .map_err(|_| Rejected)?;
        let credit_statement = authorization
            .authorization()
            .finalized_credit_statement(
                finalized
                    .finality
                    .reserve_receipt_witness
                    .receipt
                    .committed_at_ms,
            )
            .map_err(|_| Rejected)?;
        let value = KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1 {
            clock: self,
            release,
            issuer,
            authorization,
            finalized,
            finalized_original: original.to_vec(),
            financial_enrollment_original: bounded_copy(
                financial_enrollment_original,
                KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
            )?,
            preparation_control_original: bounded_copy(
                preparation_control_original,
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
            )?,
            credit_statement,
        };
        value.recheck_retained_custody()?;
        Ok(value)
    }
}
impl KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'_> {
    // These private loans keep the new assertion with this exact actual source/prefix/release.
    // Public policy data or another clock cannot substitute for this custody.
    pub(super) fn retained_clock_for_service_assertion(
        &self,
    ) -> Result<&KagemushaOrdinaryNativeClockOwnerV1> {
        self.recheck_retained_custody()?;
        Ok(self.clock)
    }
    pub(super) fn retained_release_for_service_assertion(
        &self,
    ) -> Result<&KagemushaAuthenticatedReleaseV1> {
        self.recheck_retained_custody()?;
        Ok(self.release)
    }

    /// Whole finalized source including actual original reserve/finality membership.
    /// # Errors
    /// Refuses changed held custody or full originals.
    pub fn finalized_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.finalized_original)
    }
    /// Genuine same whole request admission; no new proof or financial owner is created.
    /// # Errors
    /// Refuses changed held custody or full originals.
    pub fn authorization(&self) -> Result<&KagemushaVerifiedOrdinaryMintAuthorizationV1> {
        self.recheck_retained_custody()?;
        Ok(&self.authorization)
    }
    /// Actual finalized neutral statement; no incoming State grant.
    /// # Errors
    /// Refuses changed held custody or full originals.
    pub fn credit_statement(&self) -> Result<&KagemushaMintCreditStatementV1> {
        self.recheck_retained_custody()?;
        Ok(&self.credit_statement)
    }
    /// Original preparation FI-control, never replaced by the service's fresh decision.
    /// # Errors
    /// Refuses changed held custody or full originals.
    pub fn preparation_financial_control_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.preparation_control_original)
    }
    /// Complete original FI certificate whose exact SHA is in the preparation control.
    /// # Errors
    /// Refuses changed held custody or full originals.
    pub fn financial_enrollment_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.financial_enrollment_original)
    }
    /// Actual held same-lineage issuer policy; never selected by incoming request fields.
    /// # Errors
    /// Refuses changed held custody or full originals.
    pub fn issuer_policy(&self) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        self.recheck_retained_custody()?;
        Ok(self.issuer)
    }
    /// Same finalized neutral semantic selector, without any current effect authority.
    /// # Errors
    /// Refuses changed held custody or encoding.
    pub fn source_semantic_digest(&self) -> Result<[u8; 32]> {
        self.recheck_retained_custody()?;
        self.credit_statement
            .canonical_digest()
            .map_err(|_| Rejected)
    }
    /// Recheck the immutable source under this same protected owner. It cannot advance the
    /// prefix, renew elapsed time/current FI or authorize exclusive incoming DATA/funds.
    /// # Errors
    /// Refuses changed complete originals, signatures, intervals, release or finality membership.
    pub fn recheck_retained_custody(&self) -> Result<()> {
        self.clock.recheck()?;
        self.issuer.validate().map_err(|_| Rejected)?;
        let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
            self.authorization.request_original(),
        )
        .map_err(|_| Rejected)?;
        let context = &request.authorization.statement.context;
        let credential = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
            self.authorization.credential_original(),
        )
        .map_err(|_| Rejected)?;
        let decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &self.finalized.issuer_decision_original,
        )
        .map_err(|_| Rejected)?;
        decision
            .verify_for_request(&request, &decision.subject.selection, self.issuer)
            .map_err(|_| Rejected)?;
        let fi: KagemushaOrdinaryRetailEnrollmentCertificateV1 = decode_exact(
            &self.financial_enrollment_original,
            KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
        )?;
        let control: KagemushaSignedOrdinaryCurrentControlV1 = decode_exact(
            &self.preparation_control_original,
            KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        )?;
        require_preparation_financial_originals(
            &request,
            &credential,
            self.issuer,
            &fi,
            &self.financial_enrollment_original,
            &control,
            &self.preparation_control_original,
            self.authorization.selected_integrity_original(),
        )?;
        if self.finalized.request_original != self.authorization.request_original()
            || self.authorization.request_original_sha256()
                != <[u8; 32]>::from(Sha256::digest(self.authorization.request_original()))
            || self.authorization.authorization() != &request.authorization
            || self.authorization.authorization_original_digest()
                != request
                    .authorization
                    .binding_digest()
                    .map_err(|_| Rejected)?
            || context.release_id != self.release.release_id()
            || context.artifact_manifest_digest != self.release.manifest_digest()
            || fi.subject.issuance.hardware_policy_digest != self.release.hardware_policy_digest()
            || decision.subject.decision_clock_context.lower_at_ms < fi.subject.issued_at_ms
            || decision.subject.decision_clock_context.upper_at_ms >= fi.subject.expires_at_ms
            || context.lineage.owner.runtime.network_id != self.release.network_id()
            || context.lineage.owner.runtime != self.issuer.runtime
            || context.lineage.owner.runtime.network_id != self.clock.network_id()?
            || self.finalized.canonical_bytes().map_err(|_| Rejected)? != self.finalized_original
            || self.credit_statement
                != request
                    .authorization
                    .finalized_credit_statement(
                        self.finalized
                            .finality
                            .reserve_receipt_witness
                            .receipt
                            .committed_at_ms,
                    )
                    .map_err(|_| Rejected)?
        {
            return Err(Rejected);
        }
        let original = self
            .clock
            .authenticate_received_historical_signed_original(
                self.authorization.preparation_clock_original(),
            )?;
        original.recheck_cash_context(&context.clock_context)?;
        let verifier = self
            .clock
            .retained_finality_verifier_for_original_custody()?;
        self.finalized
            .finality
            .validate_retained_with_verifier(&context.lineage.owner.runtime.network_id, &verifier)
            .map_err(|_| Rejected)?;
        self.clock.recheck()
    }
}
#[allow(clippy::too_many_arguments)]
fn require_preparation_financial_originals(
    request: &KagemushaOrdinaryTopUpRequestV1,
    credential: &KagemushaOrdinaryAppCredentialV1,
    issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    fi: &KagemushaOrdinaryRetailEnrollmentCertificateV1,
    fi_raw: &[u8],
    control: &KagemushaSignedOrdinaryCurrentControlV1,
    control_raw: &[u8],
    selected_integrity_original: Option<&[u8]>,
) -> Result<()> {
    let context = &request.authorization.statement.context;
    let c = &credential.subject;
    fi.signature
        .verify(
            &issuer.issuer_public_key,
            &fi.subject.approval_payload().map_err(|_| Rejected)?,
        )
        .map_err(|_| Rejected)?;
    control
        .verify_for_request(&control.subject.request, issuer)
        .map_err(|_| Rejected)?;
    let f = &fi.subject;
    let decision = &control.subject;
    if f.owner != context.lineage.owner
        || f.issuer_policy_id != issuer.issuer_policy_id
        || f.issuer_audience != issuer.issuer_audience
        || f.issued_at_ms < issuer.valid_from_ms
        || f.expires_at_ms > issuer.expires_at_ms
        || f.expires_at_ms
            .checked_sub(f.issued_at_ms)
            .is_none_or(|life| life == 0 || life > issuer.maximum_certificate_lifetime_ms)
        || f.issuance
            .credential
            .canonical_bytes()
            .map_err(|_| Rejected)?
            != credential.canonical_bytes().map_err(|_| Rejected)?
        || f.ordinary_app_credential_digest != context.recipient_app_credential_digest
        || f.issuance.release_id != context.release_id
        || context.lineage.financial_epoch_id
            != kagemusha_ordinary_financial_epoch_id_v1(c).map_err(|_| Rejected)?
        || context.lineage.financial_authority_commitment != c.financial_authority_commitment
        || context.financial_control_original_sha256
            != <[u8; 32]>::from(Sha256::digest(control_raw))
        || decision.request.owner != context.lineage.owner
        || decision.request.enrollment_original_sha256 != <[u8; 32]>::from(Sha256::digest(fi_raw))
        || decision.request.credential_original_sha256
            != <[u8; 32]>::from(Sha256::digest(
                credential.canonical_bytes().map_err(|_| Rejected)?,
            ))
        || decision.release_id != c.release_id
        || decision.hardware_profile_id != c.hardware_profile_id
        || decision.profile_policy_epoch != c.policy_epoch
        || decision.ordinary_trust_policy_digest != c.trust_policy_digest
        || decision.app_authority_policy_digest != c.app_authority_policy_digest
        || decision.latest_integrity_lease_original.as_deref() != selected_integrity_original
        || context.clock_context.lower_at_ms < f.issued_at_ms
        || context.clock_context.upper_at_ms >= f.expires_at_ms
        || context.clock_context.lower_at_ms < decision.issued_at_ms
        || context.clock_context.upper_at_ms >= decision.expires_at_ms
    {
        return Err(Rejected);
    }
    Ok(())
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
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    fn preparation_control(
        f: &iroha_data_model::testing::ordinary_mint::KagemushaOrdinaryMintCodecFixtureV1,
    ) -> KagemushaSignedOrdinaryCurrentControlV1 {
        let credential = &f.enrollment_fixture.selection.issuance.credential;
        let c = &credential.subject;
        let fi = f.enrollment_fixture.certificate.canonical_bytes().unwrap();
        let subject = KagemushaOrdinaryCurrentControlSubjectV1 {
            request: KagemushaOrdinaryCurrentControlRequestV1 {
                version: 1,
                request_nonce: [71; 32],
                owner: f.enrollment_fixture.selection.owner.clone(),
                enrollment_original_sha256: Sha256::digest(&fi).into(),
                credential_original_sha256: Sha256::digest(credential.canonical_bytes().unwrap())
                    .into(),
                issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
                    &f.enrollment_fixture.issuer_policy,
                )
                .unwrap(),
            },
            release_id: c.release_id,
            hardware_profile_id: c.hardware_profile_id,
            profile_policy_epoch: c.policy_epoch,
            ordinary_trust_policy_digest: c.trust_policy_digest,
            app_authority_policy_digest: c.app_authority_policy_digest,
            authority_height: 2,
            authority_context_id: Hash::new(b"inert original context"),
            world_root: Hash::new(b"inert original world"),
            world_schema_hash: Hash::new(b"inert schema"),
            asset_definition_original_sha256: [72; 32],
            verifier_registry_original_sha256: [73; 32],
            data_incarnation_digest: [74; 32],
            data_revision: 1,
            data_policy_epoch: 1,
            data_schema_epoch: 1,
            latest_integrity_lease_original: None,
            issued_at_ms: 1000,
            expires_at_ms: 1100,
        };
        let key = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        KagemushaSignedOrdinaryCurrentControlV1 {
            signature: Signature::new(
                key.private_key(),
                &subject.issuer_signing_message().unwrap(),
            ),
            subject,
        }
    }
    #[test]
    fn historical_full_fi_control_equations_do_not_grant_service_source_or_renew_time() {
        // Only canonical test data/signature equations are exercised. No accepted Mint proof,
        // service prefix, finalized receipt or closed source is constructed by this fixture.
        let initial =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let mut control = preparation_control(&initial);
        for mode in 0..3 {
            if mode == 1 {
                control.subject.issued_at_ms = 1001;
                let key = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
                control.signature = Signature::new(
                    key.private_key(),
                    &control.subject.issuer_signing_message().unwrap(),
                );
            } else if mode == 2 {
                control.subject.issued_at_ms = 1000;
                let wrong_role = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
                control.signature = Signature::new(
                    wrong_role.private_key(),
                    &control.subject.issuer_signing_message().unwrap(),
                );
            }
            let control_raw = control.canonical_bytes().unwrap();
            let fixture = iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(&control_raw).unwrap();
            let fi = &fixture.enrollment_fixture.certificate;
            let fi_raw = fi.canonical_bytes().unwrap();
            let credential = &fixture.enrollment_fixture.selection.issuance.credential;
            let check = require_preparation_financial_originals(
                &fixture.request,
                credential,
                &fixture.enrollment_fixture.issuer_policy,
                fi,
                &fi_raw,
                &control,
                &control_raw,
                None,
            );
            if mode == 0 {
                check.unwrap();
                let mut raw = fi_raw.clone();
                raw.push(0);
                assert!(
                    require_preparation_financial_originals(
                        &fixture.request,
                        credential,
                        &fixture.enrollment_fixture.issuer_policy,
                        fi,
                        &raw,
                        &control,
                        &control_raw,
                        None
                    )
                    .is_err()
                );
                let mut raw = control_raw.clone();
                raw.push(0);
                assert!(
                    require_preparation_financial_originals(
                        &fixture.request,
                        credential,
                        &fixture.enrollment_fixture.issuer_policy,
                        fi,
                        &fi_raw,
                        &control,
                        &raw,
                        None
                    )
                    .is_err()
                );
                assert!(
                    require_preparation_financial_originals(
                        &fixture.request,
                        credential,
                        &fixture.enrollment_fixture.issuer_policy,
                        fi,
                        &fi_raw,
                        &control,
                        &control_raw,
                        Some(b"different selected PI original")
                    )
                    .is_err()
                );
            } else {
                assert!(check.is_err());
            }
        }
    }
    #[test]
    fn historical_preparation_control_refuses_separate_app_and_core_signers() {
        // Real public signature equations only: no Native source, current FI or finality owner.
        let initial =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let correct = preparation_control(&initial);
        let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        assert_eq!(
            issuer.public_key(),
            &initial.enrollment_fixture.issuer_policy.issuer_public_key
        );
        for seed in [64, 61, 63] {
            let mut control = correct.clone();
            let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            control.signature = Signature::new(
                key.private_key(),
                &control.subject.issuer_signing_message().unwrap(),
            );
            let control_raw = control.canonical_bytes().unwrap();
            let fixture = iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(&control_raw).unwrap();
            let fi = &fixture.enrollment_fixture.certificate;
            let fi_raw = fi.canonical_bytes().unwrap();
            let credential = &fixture.enrollment_fixture.selection.issuance.credential;
            let result = require_preparation_financial_originals(
                &fixture.request,
                credential,
                &fixture.enrollment_fixture.issuer_policy,
                fi,
                &fi_raw,
                &control,
                &control_raw,
                None,
            );
            if seed == 64 {
                result.unwrap();
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn service_original_decoders_are_bounded_exact_data_without_a_cap_constructor() {
        let f =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let raw = f.enrollment_fixture.certificate.canonical_bytes().unwrap();
        let value: KagemushaOrdinaryRetailEnrollmentCertificateV1 =
            decode_exact(&raw, KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1).unwrap();
        assert_eq!(value, f.enrollment_fixture.certificate);
        assert!(
            decode_exact::<KagemushaOrdinaryRetailEnrollmentCertificateV1>(&raw, raw.len() - 1)
                .is_err()
        );
        assert!(
            decode_exact::<KagemushaOrdinaryRetailEnrollmentCertificateV1>(&[], raw.len()).is_err()
        );
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(
            decode_exact::<KagemushaOrdinaryRetailEnrollmentCertificateV1>(
                &trailing,
                trailing.len()
            )
            .is_err()
        );
    }
}
