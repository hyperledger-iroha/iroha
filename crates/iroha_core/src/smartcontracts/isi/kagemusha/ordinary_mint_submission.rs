//! Complete ordinary Node Mint113 instruction admission before pooled reserve mutation.
//!
//! Actual World purpose selects identity roots and clock selection before offered signatures
//! are interpreted. Historical proof context and fresh effect context are separately verified;
//! neither a decoded FI certificate nor signed read-only FI status is a one-use monetary grant.
use super::ordinary_mint_clock::admit_ordinary_mint_signed_clock_v1;
use super::ordinary_mint_debit_admission::{
    KagemushaWorldOrdinaryMintDebitDecisionV1, admit_ordinary_mint_debit_decision_v1,
};
use super::ordinary_mint_permission::admit_ordinary_mint_issuer_purpose_v1;
use super::*;
use crate::state::{State, WorldReadOnly as _};
use iroha_core_zk::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1;
use iroha_data_model::kagemusha::*;
use iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint;
use std::any::Any;

pub(super) fn admit(
    transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    submission: &KagemushaOrdinaryNodeMintSubmissionV1,
) -> Result<
    (
        KagemushaVerifiedOrdinaryMintAuthorizationV1,
        KagemushaWorldOrdinaryMintDebitDecisionV1,
    ),
    String,
> {
    submission.canonical_bytes()?; // finite whole frame before expensive cryptography
    let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
        &submission.topup_request_original,
    )?;
    let context = &request.authorization.statement.context;
    if authority != &context.lineage.owner.account_id {
        return Err("ordinary Mint transaction authority is not the exact payer".into());
    }
    request.verify_account_signature(&submission.account_consent)?;
    let token = decode_issuer_purpose_original(&submission.issuer_purpose_original)?;
    let purpose = admit_ordinary_mint_issuer_purpose_v1(transaction, &token)?;
    if purpose.release_id() != context.release_id
        || purpose.issuer_policy().runtime != context.lineage.owner.runtime
    {
        return Err("ordinary Mint World purpose runtime/release differs".into());
    }
    let registry = transaction.world.kagemusha_verifier_registry.get();
    let native = transaction.kagemusha_v1_runtime_verifier.as_ref();
    let native_any: &dyn Any = native;
    runtime_matches_governed_registry(native_any, registry)?;
    let native = native_any
        .downcast_ref::<AuthenticatedKagemushaV1RuntimeVerifier>()
        .ok_or("ordinary Mint genuine installed release owner unavailable")?;
    let runtime =
        ordinary_mint_runtime::selected_runtime(native, &submission.topup_request_original)?;
    let release = &runtime.release;
    let signed_decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
        &submission.debit_decision_original,
    )?;
    signed_decision.verify_for_request(
        &request,
        &signed_decision.subject.selection,
        purpose.issuer_policy(),
    )?;
    let preparation_clock = admit_ordinary_mint_signed_clock_v1(
        transaction,
        &purpose,
        &submission.clock_selection_original,
        &submission.preparation_clock_original,
        &submission.preparation_clock_parent_originals,
        &context.clock_context,
    )?;
    let decision_clock = admit_ordinary_mint_signed_clock_v1(
        transaction,
        &purpose,
        &submission.clock_selection_original,
        &submission.decision_clock_original,
        &submission.decision_clock_parent_originals,
        &signed_decision.subject.decision_clock_context,
    )?;
    decision_clock.require_current_execution_cut(
        transaction,
        &signed_decision.subject.decision_clock_context,
        signed_decision.subject.authority_height,
        signed_decision.subject.authority_context_id,
        signed_decision.subject.world_root,
    )?;
    let policy = KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(
        &submission.identity_policy_original,
    )?
    .authenticate(
        purpose.app_identity_authority(),
        signed_decision.subject.decision_clock_context.lower_at_ms,
    )?;
    policy.recheck_at_trusted_time(signed_decision.subject.decision_clock_context.upper_at_ms)?;
    let core = KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(
        &submission.core_enrollment_issuer_policy_original,
    )?;
    let core_issuer = core.authenticate_under_policy(
        &policy,
        core.lane_namespace_id,
        signed_decision.subject.decision_clock_context.lower_at_ms,
    )?;
    core_issuer.recheck_current(
        &policy,
        core.lane_namespace_id,
        signed_decision.subject.decision_clock_context.upper_at_ms,
    )?;
    if core.enrollment_issuer_key == purpose.issuer_policy().issuer_public_key
        || core.app_authority_key == purpose.issuer_policy().issuer_public_key
        || core.derive_enrollment_lane(&purpose.issuer_policy().runtime.fi_id, authority)?
            != context.lineage.owner.lane_id
    {
        return Err("ordinary Mint independent Core/App/FI roles or original lane differ".into());
    }
    let c = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(
        &submission.enrollment_challenge_original,
    )?;
    let credential =
        KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&submission.credential_original)?;
    let original_enrollment = || KagemushaOrdinaryEnrollmentProofOriginalsV1 {
        preparation_original: &submission.enrollment_challenge_original,
        expected_preparation: &c.challenge,
        raw_admission_original: &submission.raw_admission_original,
        platform_original: &submission.platform_attestation_original,
        possession_original: &submission.enrollment_possession_original,
        credential_original: &submission.credential_original,
        selected_key: &credential.subject.app_public_key,
    };
    let historical = policy.authenticate_proof_enrollment_originals(
        original_enrollment(),
        release,
        integrity_originals(submission.preparation_integrity.as_ref()),
        context.clock_context.lower_at_ms,
        context.clock_context.upper_at_ms,
    )?;
    let current = policy.authenticate_proof_enrollment_originals(
        original_enrollment(),
        release,
        integrity_originals(submission.decision_integrity.as_ref()),
        signed_decision.subject.decision_clock_context.lower_at_ms,
        signed_decision.subject.decision_clock_context.upper_at_ms,
    )?;
    // No Native financial possession owner is constructed from this signed certificate. Its
    // complete original equation/scope is authenticated independently for both proof contexts.
    let fi: KagemushaOrdinaryRetailEnrollmentCertificateV1 = decode_exact(
        &submission.financial_enrollment_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    fi.signature
        .verify(
            &purpose.issuer_policy().issuer_public_key,
            &fi.subject.approval_payload()?,
        )
        .map_err(|_| "ordinary Mint full FI signature rejected")?;
    let subject = &fi.subject;
    if subject.owner != context.lineage.owner
        || subject.issuer_policy_id != purpose.issuer_policy().issuer_policy_id
        || subject.issuer_audience != purpose.issuer_policy().issuer_audience
        || subject.issuance.credential.canonical_bytes()? != submission.credential_original
        || subject.ordinary_app_credential_digest != historical.credential().digest()
        || subject.issuance.release_id != release.release_id()
        || subject.issuance.hardware_policy_digest != release.hardware_policy_digest()
        || subject.issuance.core_authorization_key_reference == [0; 32]
        || subject.issued_at_ms < purpose.issuer_policy().valid_from_ms
        || subject.expires_at_ms > purpose.issuer_policy().expires_at_ms
        || subject
            .expires_at_ms
            .checked_sub(subject.issued_at_ms)
            .is_none_or(|n| n == 0 || n > purpose.issuer_policy().maximum_certificate_lifetime_ms)
    {
        return Err("ordinary Mint complete FI scope/original equation differs".into());
    }
    for interval in [
        context.clock_context,
        signed_decision.subject.decision_clock_context,
    ] {
        if interval.lower_at_ms < subject.issued_at_ms
            || interval.upper_at_ms >= subject.expires_at_ms
        {
            return Err("ordinary Mint FI original does not cover both interval endpoints".into());
        }
    }
    let preparation_control = admit_control(
        submission,
        &fi,
        purpose.issuer_policy(),
        historical.credential(),
        historical.selected_integrity_lease(),
        &context.clock_context,
        &submission.preparation_control_original,
    )?;
    if <[u8; 32]>::from(Sha256::digest(&submission.preparation_control_original))
        != context.financial_control_original_sha256
    {
        return Err("ordinary Mint historical FI decision original differs".into());
    }
    let control = admit_control(
        submission,
        &fi,
        purpose.issuer_policy(),
        current.credential(),
        current.selected_integrity_lease(),
        &signed_decision.subject.decision_clock_context,
        &submission.current_control_original,
    )?;
    // The separate Core effect window cannot outlive any actual current FI/C/PI operand.
    // Ledger time checks remain deterministic and are not described as Native elapsed time.
    let current_subject = current.credential().subject();
    let lease_deadline = current
        .selected_integrity_lease()
        .map(|lease| {
            lease
                .subject()
                .expires_at_ms
                .min(lease.subject().binding.refresh_before_ms)
        })
        .unwrap_or(current_subject.expires_at_ms);
    let effect_deadline = [
        control.subject.expires_at_ms,
        fi.subject.expires_at_ms,
        current_subject.expires_at_ms,
        policy.policy().profile.expires_at_ms,
        lease_deadline,
    ]
    .into_iter()
    .min()
    .ok_or("ordinary Mint current effect deadline absent")?;
    let ledger_time = transaction.block_unix_timestamp_ms();
    if signed_decision.subject.expires_at_ms > effect_deadline
        || ledger_time < signed_decision.subject.issued_at_ms
        || ledger_time >= effect_deadline
    {
        return Err(
            "ordinary Mint current effect window outlives an authenticated original".into(),
        );
    }
    let actual_asset = transaction
        .world
        .asset_definition(&context.lineage.owner.runtime.asset)
        .map_err(|e| e.to_string())?;
    if control.subject.authority_height != decision_clock.height()
        || *control.subject.authority_context_id.as_ref() != decision_clock.context_id()
        || *control.subject.world_root.as_ref() != signed_decision.subject.world_root
        || control.subject.world_schema_hash != State::native_world_schema_hash_v1()?
        || control.subject.asset_definition_original_sha256
            != kagemusha_ordinary_current_control_original_sha256_v1(&actual_asset)?
        || control.subject.verifier_registry_original_sha256
            != kagemusha_ordinary_current_control_original_sha256_v1(registry)?
        || control.subject.data_incarnation_digest
            != signed_decision.subject.data_incarnation_digest
        || control.subject.data_policy_epoch != signed_decision.subject.data_policy_epoch
        || control.subject.data_schema_epoch != signed_decision.subject.data_schema_epoch
    {
        return Err(
            "ordinary Mint fresh FI control/current certified World or DATA identity differs"
                .into(),
        );
    }
    let _ = preparation_control; // admitted immutable proof evidence, never a current effect grant
    historical.recheck_originals(&policy)?;
    current.recheck_originals(&policy)?;
    let proof = ordinary_mint_runtime::verify(
        native,
        &submission.topup_request_original,
        historical.credential(),
        historical.selected_integrity_lease(),
        preparation_clock.verified_original(),
    )?;
    // Fresh current FI/PI and exact purpose are separate from hours-long immutable proof work.
    current.recheck_originals(&policy)?;
    purpose.recheck(transaction)?;
    let decision = admit_ordinary_mint_debit_decision_v1(
        transaction,
        authority,
        &request,
        &signed_decision.subject.selection,
        &submission.account_consent,
        &token,
        &signed_decision,
        decision_clock,
    )?;
    Ok((proof, decision))
}

fn integrity_originals(
    value: Option<&KagemushaOrdinaryNodeMintIntegrityOriginalsV1>,
) -> Option<KagemushaOrdinaryIntegrityProofOriginalsV1<'_>> {
    value.map(|v| KagemushaOrdinaryIntegrityProofOriginalsV1 {
        challenge_original: &v.challenge_original,
        lease_original: &v.lease_original,
    })
}

fn admit_control(
    submission: &KagemushaOrdinaryNodeMintSubmissionV1,
    fi: &KagemushaOrdinaryRetailEnrollmentCertificateV1,
    issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    interval: &KagemushaOrdinaryCashClockContextV1,
    raw: &[u8],
) -> Result<KagemushaSignedOrdinaryCurrentControlV1, String> {
    let signed: KagemushaSignedOrdinaryCurrentControlV1 =
        decode_exact(raw, KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1)?;
    signed.verify_for_request(&signed.subject.request, issuer)?;
    let request = &signed.subject.request;
    let s = credential.subject();
    if request.owner != fi.subject.owner
        || request.enrollment_original_sha256
            != <[u8; 32]>::from(Sha256::digest(&submission.financial_enrollment_original))
        || request.credential_original_sha256
            != <[u8; 32]>::from(Sha256::digest(&submission.credential_original))
        || signed.subject.release_id != s.release_id
        || signed.subject.hardware_profile_id != s.hardware_profile_id
        || signed.subject.profile_policy_epoch != s.policy_epoch
        || signed.subject.ordinary_trust_policy_digest != s.trust_policy_digest
        || signed.subject.app_authority_policy_digest != s.app_authority_policy_digest
        || signed.subject.latest_integrity_lease_original.as_deref() != lease.map(|v| v.original())
        || interval.lower_at_ms < signed.subject.issued_at_ms
        || interval.upper_at_ms >= signed.subject.expires_at_ms
    {
        return Err("ordinary Mint complete FI decision/C/PI/original interval differs".into());
    }
    Ok(signed)
}
// Permission tokens own canonical JSON payloads inside the maintained complete Permission
// Norito archive. They are not Norito DTOs; no second binary layout or generic token decoder
// is admitted. The name and exact canonical payload survive the actual World equality check.
fn decode_issuer_purpose_original(raw: &[u8]) -> Result<CanAuthorizeKagemushaOrdinaryMint, String> {
    let original: iroha_data_model::permission::Permission = decode_exact(raw, 32 * 1024)?;
    let token = CanAuthorizeKagemushaOrdinaryMint::try_from(&original)
        .map_err(|_| "ordinary Mint permission name/payload differs")?;
    token.validate_scope()?;
    let canonical: iroha_data_model::permission::Permission = token.clone().into();
    if canonical != original {
        return Err("ordinary Mint permission payload is not the sole canonical token".into());
    }
    Ok(token)
}
fn decode_exact<T: norito::NoritoSerialize>(raw: &[u8], maximum: usize) -> Result<T, String>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if raw.is_empty() || raw.len() > maximum {
        return Err("ordinary Mint original exceeds its independent bound".into());
    }
    let value: T =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|e| e.to_string())?;
    if norito::encode_canonical(&value).map_err(|e| e.to_string())? != raw {
        return Err("ordinary Mint original is not sole canonical data".into());
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn generic_reserve_purpose() -> Vec<u8> {
        let permission: iroha_data_model::permission::Permission =
            iroha_executor_data_model::permission::kagemusha::CanManageKagemushaReserve.into();
        norito::encode_canonical(&permission).expect("public generic reserve permission")
    }

    #[test]
    fn ordinary_mint_rejects_generic_reserve_purpose() {
        assert!(decode_issuer_purpose_original(&generic_reserve_purpose()).is_err());
    }

    #[test]
    fn ordinary_mint_rejects_permission_suffix() {
        let mut original = generic_reserve_purpose();
        decode_exact::<iroha_data_model::permission::Permission>(&original, 32 * 1024)
            .expect("base is a canonical generic permission");
        original.push(0);
        let canonical_error =
            decode_exact::<iroha_data_model::permission::Permission>(&original, 32 * 1024)
                .expect_err("suffix is not sole canonical data");
        assert_eq!(
            decode_issuer_purpose_original(&original)
                .expect_err("canonical refusal precedes purpose conversion"),
            canonical_error
        );
    }

    #[test]
    fn ordinary_mint_issuer_purpose_has_independent_original_bound() {
        for original in [Vec::new(), vec![0; 32 * 1024 + 1]] {
            assert_eq!(
                decode_issuer_purpose_original(&original).expect_err("independent original bound"),
                "ordinary Mint original exceeds its independent bound"
            );
        }
    }
    // Public syntax only: these values are never granted in World or installed into Native.
    // The compressed Ed25519 basepoint is fixed public point data, not a generated keypair.
    fn public_syntactic_mint_purpose() -> CanAuthorizeKagemushaOrdinaryMint {
        use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
        use iroha_data_model::{NetworkId, asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1};
        use iroha_model_base::topology::DataSpaceId;

        let mut public_point = [0x66; 32];
        public_point[0] = 0x58;
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &public_point)
            .expect("fixed public Ed25519 point");
        // This is a syntactic NetworkId for codec coverage, not an authenticated genesis.
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"ordinary-mint-purpose-codec-only-network",
        )));
        let runtime = KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "codec-only-fi".parse().expect("public FI name"),
            ledger_dataspace_id: DataSpaceId::new(1),
            authentication_namespace: "codec-only-auth".parse().expect("public namespace"),
            network_id,
            asset: AssetDefinitionId::from_uuid_bytes([
                0x34, 0x72, 0xb1, 0xc9, 0x46, 0x50, 0x4a, 0x67, 0x81, 0xb1, 0x34, 0xa1, 0x70, 0x24,
                0x5d, 0x29,
            ])
            .expect("public syntactic asset ID"),
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"ordinary-mint-purpose-codec-only-incarnation").as_ref(),
            )
            .expect("public syntactic asset incarnation"),
            scale: 2,
        };
        // Codec-only DATA selectors: no service originals, DATA session or World grant exist.
        let lineage_data_authority = KagemushaOrdinaryLineageDataAuthorityV1 {
            version: 1,
            liability_pool_id: kagemusha_liability_pool_id_v1(
                &runtime.network_id,
                &runtime.asset,
                runtime.asset_incarnation,
            )
            .expect("public syntactic liability pool"),
            service_identity_digest: [0x66; 32],
            data_incarnation_digest: [0x77; 32],
            dataspace: "codec-only-data".into(),
            tenant: "codec-only-tenant".into(),
            principal: "codec-only-principal".into(),
            collection: "codec-only-lineages".into(),
        };
        CanAuthorizeKagemushaOrdinaryMint {
            issuer_policy: KagemushaRetailEnrollmentIssuerPolicyV1 {
                version: 1,
                issuer_policy_id: [0x11; 32],
                issuer_public_key: public_key.clone(),
                issuer_audience: "codec-only-mint".parse().expect("public audience"),
                runtime,
                valid_from_ms: 1,
                expires_at_ms: 100,
                maximum_certificate_lifetime_ms: 99,
            },
            release_id: [0x22; 32],
            app_identity_authority: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
                version: 1,
                authority_set_id: [0x33; 32],
                network_id,
                expected_identity_policy_id: [0x44; 32],
                threshold: 1,
                authorized_signers: vec![public_key],
            },
            clock_selection_original_sha256: [0x55; 32],
            lineage_data_authority,
        }
    }

    #[test]
    fn ordinary_mint_accepts_exact_public_typed_permission_roundtrip() {
        let token = public_syntactic_mint_purpose();
        token.validate_scope().expect("public token shape only");
        let permission: iroha_data_model::permission::Permission = token.clone().into();
        let original = norito::encode_canonical(&permission).expect("canonical public permission");
        assert_eq!(
            decode_exact::<iroha_data_model::permission::Permission>(&original, 32 * 1024)
                .expect("sole canonical public Permission archive"),
            permission
        );
        let decoded = decode_issuer_purpose_original(&original).expect("exact public typed token");
        assert_eq!(decoded, token);
        let restored: iroha_data_model::permission::Permission = decoded.into();
        assert_eq!(restored, permission);
        assert_eq!(
            norito::encode_canonical(&restored).expect("canonical restored permission"),
            original
        );
    }

    #[test]
    fn ordinary_mint_rejects_payload_field_discarded_by_typed_conversion() {
        let token = public_syntactic_mint_purpose();
        token.validate_scope().expect("public token shape only");
        let permission: iroha_data_model::permission::Permission = token.clone().into();
        let object_tail = permission
            .payload()
            .get()
            .strip_prefix('{')
            .expect("typed permission has an object payload");
        // The added field precedes app_identity_authority, keeping the JSON lexically canonical.
        let payload = iroha_primitives::json::Json::from_raw_json(format!(
            "{{\"additional_untrusted_scope\":true,{object_tail}"
        ))
        .expect("canonical public payload with one extra field");
        let expanded =
            iroha_data_model::permission::Permission::new(permission.name.clone(), payload);
        assert_eq!(
            CanAuthorizeKagemushaOrdinaryMint::try_from(&expanded)
                .expect("current typed parser discards the unknown top-level field"),
            token
        );
        let original = norito::encode_canonical(&expanded).expect("canonical expanded permission");
        assert_eq!(
            decode_exact::<iroha_data_model::permission::Permission>(&original, 32 * 1024)
                .expect("extra field survives the sole canonical archive"),
            expanded
        );
        assert_eq!(
            decode_issuer_purpose_original(&original)
                .expect_err("the actual helper rejects typed payload data loss"),
            "ordinary Mint permission payload is not the sole canonical token"
        );
    }
}

#[cfg(test)]
mod permission_original_tests {
    use super::*;
    #[test]
    fn ordinary_mint_purpose_refuses_generic_reserve_permission_and_noncanonical_full_archive() {
        let other: iroha_data_model::permission::Permission =
            iroha_executor_data_model::permission::kagemusha::CanManageKagemushaReserve.into();
        let raw = norito::encode_canonical(&other).unwrap();
        assert!(decode_issuer_purpose_original(&raw).is_err());
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(decode_issuer_purpose_original(&trailing).is_err());
        assert!(decode_issuer_purpose_original(&raw[..raw.len() - 1]).is_err());
        assert!(decode_issuer_purpose_original(&[]).is_err());
    }
}
