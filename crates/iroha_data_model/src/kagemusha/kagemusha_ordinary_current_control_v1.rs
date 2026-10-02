//! Purpose-bound current FI control originals. These data carriers grant no Native loan.
//! Native must independently retain the FI/C, actual clock and current certified World source.
use super::*;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, Hash, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
/// Sole finite current-control duration; neither issuer nor app can extend this protocol ceiling.
pub const KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_LIFETIME_MS_V1: u64 = 10_000;
/// Finite complete signed-control original ceiling, excluding the separately verified World cut.
pub const KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1: usize = 32 * 1024;
/// Exact sole account-request signing domain.
pub const KAGEMUSHA_ORDINARY_CURRENT_CONTROL_REQUEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-current-fi-control-request\0";
/// Exact sole delegated-issuer signing domain.
pub const KAGEMUSHA_ORDINARY_CURRENT_CONTROL_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-current-fi-control\0";
fn reject<T>() -> Result<T, String> {
    Err("ordinary current FI control original rejected".into())
}
fn encoded<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    norito::encode_canonical(value).map_err(|e| e.to_string())
}
/// Fresh same-owner request selected and retained by Native before transport.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCurrentControlRequestV1")]
pub struct KagemushaOrdinaryCurrentControlRequestV1 {
    /// Exactly one, without fallback decoding.
    pub version: u16,
    /// Fresh actual Native-generated nonce; replay history belongs to Native and DATA owners.
    pub request_nonce: [u8; 32],
    /// Exact original canonical wallet/runtime/lane; decoding supplies no owner.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// SHA256 of the complete exact retained FI certificate original.
    pub enrollment_original_sha256: [u8; 32],
    /// SHA256 of the complete exact retained app credential original.
    pub credential_original_sha256: [u8; 32],
    /// Original independently installed selected issuer-policy digest.
    pub issuer_policy_digest: [u8; 32],
}
impl KagemushaOrdinaryCurrentControlRequestV1 {
    /// Enforce bounded exact request shape, without creating account or FI authority.
    /// # Errors
    /// Refuses missing nonce/digests, malformed owner or unsupported wallet controller.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.owner.enrollment_id().map_err(|e| e.to_string())?;
        let p = self
            .owner
            .account_id
            .multisig_policy()
            .ok_or("current control W unsupported")?;
        if self.version != 1
            || self.request_nonce == [0; 32]
            || self.enrollment_original_sha256 == [0; 32]
            || self.credential_original_sha256 == [0; 32]
            || self.issuer_policy_digest == [0; 32]
            || p.threshold() != 1
            || p.members().len() != 1
            || p.members()[0].weight() != 1
            || p.members()[0].public_key().algorithm() != Algorithm::Ed25519
        {
            return reject();
        }
        Ok(())
    }
    /// Sole complete canonical request original.
    /// # Errors
    /// Refuses shape or a request beyond 8192 bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let raw = encoded(self)?;
        if raw.len() > 8192 {
            return reject();
        }
        Ok(raw)
    }
    /// Exact account consent subject. It is unrelated to E20/C20 possession or money approval.
    /// # Errors
    /// Refuses an invalid canonical request.
    pub fn account_signing_message(&self) -> Result<Vec<u8>, String> {
        let mut m = KAGEMUSHA_ORDINARY_CURRENT_CONTROL_REQUEST_DOMAIN_V1.to_vec();
        m.extend(self.canonical_bytes()?);
        Ok(m)
    }
    /// Verify the request under the sole original W member; this still establishes no live KYC.
    /// # Errors
    /// Refuses a changed subject or non-genuine Ed signature.
    pub fn verify_account_signature(&self, signature: &Signature) -> Result<(), String> {
        self.validate_shape()?;
        let policy = self
            .owner
            .account_id
            .multisig_policy()
            .ok_or("current control W unsupported")?;
        let member = policy
            .members()
            .first()
            .ok_or("current control W unsupported")?;
        if signature.payload().len() != 64 {
            return reject();
        }
        signature
            .verify(member.public_key(), &self.account_signing_message()?)
            .map_err(|e| e.to_string())
    }
}
/// Small issuer assertion only after actual current Native owner/DATA CAS and release verification.
/// Full World originals remain a separate bounded data input verified by Native at intake.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCurrentControlSubjectV1")]
pub struct KagemushaOrdinaryCurrentControlSubjectV1 {
    /// Exact original nonce/account/runtime/FI/C request, not reconstructed caller pins.
    pub request: KagemushaOrdinaryCurrentControlRequestV1,
    /// Exact current selected release identity.
    pub release_id: [u8; 32],
    /// Exact original credential profile.
    pub hardware_profile_id: [u8; 32],
    /// Current governed enabled-profile policy epoch, not a financial sequence.
    pub profile_policy_epoch: u64,
    /// Independently governed ordinary trust policy digest.
    pub ordinary_trust_policy_digest: [u8; 32],
    /// Independently governed app-authority policy digest.
    pub app_authority_policy_digest: [u8; 32],
    /// Actual certified authority original height.
    pub authority_height: u64,
    /// Exact actual certified execution context.
    pub authority_context_id: Hash,
    /// Complete World root to which all typed authority originals must belong.
    pub world_root: Hash,
    /// Independently installed compiled complete World schema.
    pub world_schema_hash: Hash,
    /// Canonical full selected asset-definition original SHA256.
    pub asset_definition_original_sha256: [u8; 32],
    /// Canonical full current governed verifier/revocation registry SHA256.
    pub verifier_registry_original_sha256: [u8; 32],
    /// Purpose-bound actual Native DATA incarnation identity; no database authority is imported.
    pub data_incarnation_digest: [u8; 32],
    /// Original linearizable DATA revision fenced in this response's durable CAS.
    pub data_revision: u64,
    /// Original DATA policy epoch, separate from the governed profile epoch.
    pub data_policy_epoch: u64,
    /// Original DATA schema epoch.
    pub data_schema_epoch: u64,
    /// Full latest current PI lease original selected in the same DATA CAS, or explicit governed None.
    pub latest_integrity_lease_original: Option<Vec<u8>>,
    /// Inclusive actual current DATA leader time, checked independently by Native clock lower.
    pub issued_at_ms: u64,
    /// Exclusive finite expiry, checked independently by Native clock upper.
    pub expires_at_ms: u64,
}
impl KagemushaOrdinaryCurrentControlSubjectV1 {
    /// Validate protocol shape only; actual issuer/World/native custody remain external.
    /// # Errors
    /// Refuses missing scope, unsupported durations, oversized PI or invalid coordinates.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.request.validate_shape()?;
        if self.release_id == [0; 32]
            || self.hardware_profile_id == [0; 32]
            || self.profile_policy_epoch == 0
            || self.ordinary_trust_policy_digest == [0; 32]
            || self.app_authority_policy_digest == [0; 32]
            || self.authority_height < 2
            || self.asset_definition_original_sha256 == [0; 32]
            || self.verifier_registry_original_sha256 == [0; 32]
            || self.data_incarnation_digest == [0; 32]
            || self.data_revision == 0
            || self.data_policy_epoch == 0
            || self.data_schema_epoch == 0
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_LIFETIME_MS_V1
            || self
                .latest_integrity_lease_original
                .as_ref()
                .is_some_and(|r| r.is_empty() || r.len() > 4096)
        {
            return reject();
        }
        Ok(())
    }
    /// Sole issuer signing subject, with an explicit domain and the complete canonical body.
    /// # Errors
    /// Refuses malformed scope or canonical encoding.
    pub fn issuer_signing_message(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let mut m = KAGEMUSHA_ORDINARY_CURRENT_CONTROL_DOMAIN_V1.to_vec();
        m.extend(encoded(self)?);
        Ok(m)
    }
}
/// Complete delegated Ed signature original. Decoding and verifying it creates no Native loan.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaSignedOrdinaryCurrentControlV1")]
pub struct KagemushaSignedOrdinaryCurrentControlV1 {
    /// Complete signed original subject.
    pub subject: KagemushaOrdinaryCurrentControlSubjectV1,
    /// Genuine purpose-bound signature under independently retained issuer policy.
    pub signature: Signature,
}
impl KagemushaSignedOrdinaryCurrentControlV1 {
    /// Sole complete canonical signed current-control original.
    /// # Errors
    /// Refuses malformed shape, unsupported encoding or finite original bounds.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.validate_shape()?;
        if self.signature.payload().len() != 64 {
            return reject();
        }
        let raw = encoded(self)?;
        if raw.len() > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1 {
            return reject();
        }
        Ok(raw)
    }
    /// Check exact selected request and delegated Ed signature. Native must separately verify
    /// current World release/revocation membership and the real clock/FI/C/latest PI custody.
    /// # Errors
    /// Refuses another request/policy/runtime/key or a forged original.
    pub fn verify_for_request(
        &self,
        request: &KagemushaOrdinaryCurrentControlRequestV1,
        issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<(), String> {
        self.canonical_bytes()?;
        issuer.validate().map_err(|e| e.to_string())?;
        if &self.subject.request != request
            || issuer.runtime != request.owner.runtime
            || self.subject.issued_at_ms < issuer.valid_from_ms
            || self.subject.expires_at_ms > issuer.expires_at_ms
            || issuer.issuer_public_key.algorithm() != Algorithm::Ed25519
            || kagemusha_ordinary_retail_issuer_policy_digest_v1(issuer)?
                != request.issuer_policy_digest
        {
            return reject();
        }
        self.signature
            .verify(
                &issuer.issuer_public_key,
                &self.subject.issuer_signing_message()?,
            )
            .map_err(|e| e.to_string())
    }
}
/// SHA256 over one complete sole canonical typed original, without importing authority.
/// # Errors
/// Refuses serialization failure.
pub fn kagemusha_ordinary_current_control_original_sha256_v1<T: norito::NoritoSerialize>(
    original: &T,
) -> Result<[u8; 32], String> {
    Ok(Sha256::digest(encoded(original)?).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{AccountId, MultisigMember, MultisigPolicy};
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::KeyPair;
    fn wallet_fixture() -> Fixture {
        Fixture::with_single_member_wallet(false, false, [19; 32])
    }
    fn original(f: &Fixture) -> KagemushaSignedOrdinaryCurrentControlV1 {
        let request = KagemushaOrdinaryCurrentControlRequestV1 {
            version: 1,
            request_nonce: [8; 32],
            owner: f.selection.owner.clone(),
            enrollment_original_sha256: Sha256::digest(f.certificate.canonical_bytes().unwrap())
                .into(),
            credential_original_sha256: Sha256::digest(
                f.selection.issuance.credential.canonical_bytes().unwrap(),
            )
            .into(),
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &f.issuer_policy,
            )
            .unwrap(),
        };
        let profile = f.selection.preparation.challenge.hardware_profile_id;
        let subject = KagemushaOrdinaryCurrentControlSubjectV1 {
            request,
            release_id: f.release.release_id(),
            hardware_profile_id: profile,
            profile_policy_epoch: f.release.enabled_profile(profile).unwrap().policy_epoch,
            ordinary_trust_policy_digest: f.trust.canonical_digest().unwrap(),
            app_authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            authority_height: 2,
            authority_context_id: Hash::new(b"explicit synthetic current context"),
            world_root: Hash::new(b"explicit synthetic complete World root"),
            world_schema_hash: Hash::new(b"independently pinned synthetic schema"),
            asset_definition_original_sha256: [10; 32],
            verifier_registry_original_sha256: [11; 32],
            data_incarnation_digest: [12; 32],
            data_revision: 3,
            data_policy_epoch: 4,
            data_schema_epoch: 5,
            latest_integrity_lease_original: None,
            issued_at_ms: 300,
            expires_at_ms: 800,
        };
        let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        assert_eq!(issuer.public_key(), &f.issuer_policy.issuer_public_key);
        KagemushaSignedOrdinaryCurrentControlV1 {
            signature: Signature::try_new(
                issuer.private_key(),
                &subject.issuer_signing_message().unwrap(),
            )
            .unwrap(),
            subject,
        }
    }
    #[test]
    fn current_control_original_roundtrips_and_authenticates_actual_wallet_member() {
        let f = wallet_fixture();
        let signed = original(&f);
        let request = &signed.subject.request;
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let signature = Signature::try_new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        )
        .unwrap();
        request.verify_account_signature(&signature).unwrap();
        signed
            .verify_for_request(request, &f.issuer_policy)
            .unwrap();
        let raw = signed.canonical_bytes().unwrap();
        let recovered: KagemushaSignedOrdinaryCurrentControlV1 =
            norito::decode_canonical(&raw).unwrap();
        assert_eq!(recovered, signed);
        assert_eq!(recovered.canonical_bytes().unwrap(), raw);
        assert_eq!(
            norito::json::from_str::<KagemushaSignedOrdinaryCurrentControlV1>(
                &norito::json::to_json(&signed).unwrap()
            )
            .unwrap(),
            signed
        );
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(
            norito::decode_canonical::<KagemushaSignedOrdinaryCurrentControlV1>(&trailing).is_err()
        );
        assert!(
            norito::decode_canonical::<KagemushaSignedOrdinaryCurrentControlV1>(
                &raw[..raw.len() - 1]
            )
            .is_err()
        );
        let mut renamed = raw.clone();
        renamed[6..22].copy_from_slice(&norito::schema::identity::frame_hash::<u8>());
        assert!(
            norito::decode_canonical::<KagemushaSignedOrdinaryCurrentControlV1>(&renamed).is_err()
        );
    }
    #[test]
    fn current_control_signatures_bind_nonce_full_fi_and_each_current_scope() {
        let f = wallet_fixture();
        let signed = original(&f);
        for change in 0..18 {
            let mut other = signed.clone();
            match change {
                0 => other.subject.request.request_nonce[0] ^= 1,
                1 => other.subject.request.enrollment_original_sha256[0] ^= 1,
                2 => other.subject.request.credential_original_sha256[0] ^= 1,
                3 => other.subject.release_id[0] ^= 1,
                4 => other.subject.hardware_profile_id[0] ^= 1,
                5 => other.subject.profile_policy_epoch += 1,
                6 => other.subject.ordinary_trust_policy_digest[0] ^= 1,
                7 => other.subject.app_authority_policy_digest[0] ^= 1,
                8 => other.subject.authority_height += 1,
                9 => other.subject.authority_context_id = Hash::new(b"foreign context"),
                10 => other.subject.world_root = Hash::new(b"foreign World"),
                11 => other.subject.world_schema_hash = Hash::new(b"foreign schema"),
                12 => other.subject.asset_definition_original_sha256[0] ^= 1,
                13 => other.subject.verifier_registry_original_sha256[0] ^= 1,
                14 => other.subject.data_incarnation_digest[0] ^= 1,
                15 => other.subject.data_revision += 1,
                16 => other.subject.latest_integrity_lease_original = Some(vec![1; 32]),
                _ => other.subject.expires_at_ms += 1,
            }
            assert!(
                other
                    .verify_for_request(&signed.subject.request, &f.issuer_policy)
                    .is_err(),
                "field {change}"
            );
        }
        let other = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
        let wrong = Signature::try_new(
            other.private_key(),
            &signed.subject.request.account_signing_message().unwrap(),
        )
        .unwrap();
        assert!(
            signed
                .subject
                .request
                .verify_account_signature(&wrong)
                .is_err()
        );
        let app_authority = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        assert_eq!(app_authority.public_key(), &f.app_authority.authority_key);
        let mut wrong_role = signed.clone();
        wrong_role.signature = Signature::try_new(
            app_authority.private_key(),
            &wrong_role.subject.issuer_signing_message().unwrap(),
        )
        .unwrap();
        assert!(
            wrong_role
                .verify_for_request(&signed.subject.request, &f.issuer_policy)
                .is_err()
        );
        let mut policy = f.issuer_policy.clone();
        policy.issuer_public_key = other.public_key().clone();
        assert!(
            signed
                .verify_for_request(&signed.subject.request, &policy)
                .is_err()
        );
    }
    #[test]
    fn current_control_refuses_multi_party_wallet_and_unbounded_or_empty_originals() {
        let f = wallet_fixture();
        let signed = original(&f);
        let a = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let b = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        let mut request = signed.subject.request.clone();
        request.owner.account_id = AccountId::new_multisig(
            MultisigPolicy::new(
                2,
                vec![
                    MultisigMember::new(a.public_key().clone(), 1).unwrap(),
                    MultisigMember::new(b.public_key().clone(), 1).unwrap(),
                ],
            )
            .unwrap(),
        );
        assert!(request.validate_shape().is_err());
        for change in 0..5 {
            let mut subject = signed.subject.clone();
            match change {
                0 => subject.expires_at_ms = subject.issued_at_ms,
                1 => subject.expires_at_ms = subject.issued_at_ms + 10_001,
                2 => subject.latest_integrity_lease_original = Some(vec![]),
                3 => subject.latest_integrity_lease_original = Some(vec![1; 4097]),
                _ => subject.request.request_nonce = [0; 32],
            }
            assert!(subject.validate_shape().is_err());
        }
    }
    #[test]
    fn current_control_original_digest_binds_complete_canonical_signed_frame() {
        let fixture = wallet_fixture();
        let signed = original(&fixture);
        signed
            .verify_for_request(&signed.subject.request, &fixture.issuer_policy)
            .unwrap();
        let canonical = signed.canonical_bytes().unwrap();
        let expected: [u8; 32] = Sha256::digest(&canonical).into();
        assert_eq!(
            kagemusha_ordinary_current_control_original_sha256_v1(&signed).unwrap(),
            expected
        );
        let recovered: KagemushaSignedOrdinaryCurrentControlV1 =
            norito::decode_canonical(&canonical).unwrap();
        assert_eq!(recovered, signed);
        assert_eq!(
            kagemusha_ordinary_current_control_original_sha256_v1(&recovered).unwrap(),
            expected
        );
        let mut substituted = signed.clone();
        substituted.subject.data_revision += 1;
        assert_ne!(
            kagemusha_ordinary_current_control_original_sha256_v1(&substituted).unwrap(),
            expected
        );
        assert!(
            substituted
                .verify_for_request(&signed.subject.request, &fixture.issuer_policy)
                .is_err()
        );
    }
}
