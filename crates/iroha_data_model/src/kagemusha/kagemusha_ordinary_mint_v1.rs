//! First-release ordinary pre-debit Mint data, separate from OEM credential/key-handle authority.
//!
//! Native selects the actual financial lineage, global predecessor, one-use encryption key,
//! current FI decision and original clock before identifying and encrypting a mint credit.
//! Context -> issuance -> credit ID -> AEAD -> statement -> dedicated approval -> proof is
//! acyclic. No future debit/finality/State proof or captured Bootstrap approval enters the IDs.
//! These are data and equations, never a decoder-to-Native funding or State effect capability.
use super::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KAGEMUSHA_ASSET_SCALE_MAX_V1,
    KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
    KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
    KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, KagemushaAppAttestReleaseMeasurementV1,
    KagemushaAppOperationApprovalEvidenceV1, KagemushaCreditOpeningV1, KagemushaDeviceSignatureV1,
    KagemushaEncryptedCreditAadV1, KagemushaEncryptedCreditEnvelopeV1,
    KagemushaEncryptedCreditPurposeV1, KagemushaHardwarePlatformClassV1,
    KagemushaLifecycleBindingV1, KagemushaMintCreditStatementV1, KagemushaOperationKindV1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryFinancialHeadV1,
    KagemushaOrdinaryFinancialLineageV1, KagemushaVerifiedOrdinaryAppCredentialV1,
    kagemusha_ciphertext_digest_v1, kagemusha_liability_pool_id_v1,
    kagemusha_mint_credit_opening_commitment_v1, kagemusha_ordinary_app_account_binding_v1,
    kagemusha_ordinary_financial_epoch_id_v1, kagemusha_recipient_credential_commitment_v1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
use iroha_crypto::{Algorithm, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Finite complete ordinary pre-debit authorization/request data bound, excluding independently
/// transported full FI/credential/clock originals. The Core owner must authenticate those originals.
pub const KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1: usize = 64 * 1024;
/// Dedicated pre-debit app signing purpose. It is not a generic State or cash terminal approval.
pub const KAGEMUSHA_ORDINARY_MINT_APPROVAL_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-mint-pre-debit-approval\0";

/// Exact pre-ID ordinary recipient and payer scope. First-release top-ups debit the same actual
/// enrolled account that receives the credit. Lineage/head data alone are not a DATA reservation.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintAuthorizationContextV1")]
pub struct KagemushaOrdinaryMintAuthorizationContextV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Actual Native idempotent funding attempt identity, selected before encryption/proof.
    pub operation_id: [u8; 32],
    /// Original independently admitted account/FI/runtime/lane and financial witness commitment.
    pub lineage: KagemushaOrdinaryFinancialLineageV1,
    /// Actual complete PUBLIC predecessor checkpoint selected by the global funding reservation.
    pub predecessor: KagemushaOrdinaryFinancialHeadV1,
    /// Actual independently threshold-admitted proof release.
    pub release_id: [u8; 32],
    /// Actual ordinary suite selected from that release, not an OEM proof family.
    pub suite_id: [u8; 32],
    /// Actual release-pinned ordinary verifying-key set.
    pub vk_digest: [u8; 32],
    /// Complete actual authenticated artifact manifest digest.
    pub artifact_manifest_digest: [u8; 32],
    /// Complete purpose-bound original ordinary C digest, never an OEM credential ID.
    pub recipient_app_credential_digest: [u8; 32],
    /// Actual enabled ordinary app profile; no hardware anti-rollback assertion is made.
    pub app_credential_profile_id: [u8; 32],
    /// Actual governed ordinary profile policy epoch.
    pub policy_epoch: u64,
    /// Positive exact amount in the lineage runtime's authoritative scale.
    pub amount: u128,
    /// Neutral randomized commitment to that same C and its private recipient opening.
    pub recipient_credential_commitment: [u8; 32],
    /// Neutral pre-ID amount/account/key commitment to the actual plaintext credit opening.
    pub credit_commitment: [u8; 32],
    /// Genuine one-use Native X25519 recipient key, held independently of the platform key.
    pub recipient_one_time_key: [u8; 32],
    /// Exact original preparation clock interval projection; full signed observations are separate.
    pub clock_context: KagemushaOrdinaryCashClockContextV1,
    /// SHA256 of the exact separately acknowledged full signed current FI-control original.
    pub financial_control_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryMintAuthorizationContextV1 {
    /// Validate neutral data shape and authoritative pool formula, without admitting FI/DATA/time.
    /// # Errors
    /// Refuses unsupported version, zero selector/amount/epoch, invalid key or runtime scale.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.lineage.validate_shape()?;
        self.predecessor.validate_shape()?;
        self.clock_context.validate_shape()?;
        if self.version != 1
            || self.policy_epoch == 0
            || self.amount == 0
            || self.lineage.owner.runtime.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
        {
            return Err("ordinary mint version/amount/profile/runtime differs".into());
        }
        nonzero(&[
            self.operation_id,
            self.release_id,
            self.suite_id,
            self.vk_digest,
            self.artifact_manifest_digest,
            self.recipient_app_credential_digest,
            self.app_credential_profile_id,
            self.recipient_credential_commitment,
            self.credit_commitment,
            self.financial_control_original_sha256,
        ])?;
        X25519Sha256::decode_public_key(&self.recipient_one_time_key)
            .map_err(|_| "ordinary mint Native X25519 key invalid")?;
        Ok(())
    }
    /// Sole complete canonical context original. It contains no plaintext or financial secret.
    /// # Errors
    /// Refuses shape, unsupported canonical layout or finite original capacity.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded(self)
    }
    /// Complete pre-ID context digest, excluding ciphertext, approval, proof and future finality.
    /// # Errors
    /// Refuses shape or canonical serialization.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        hash_original(
            b"iroha:kagemusha:v1:ordinary-mint-context\0",
            &self.canonical_bytes()?,
        )
    }
    /// Pre-encryption issuance commitment from the same sole complete context.
    /// # Errors
    /// Refuses invalid context or encoding.
    pub fn issuance_commitment(&self) -> Result<[u8; 32], String> {
        let context = self.binding_digest()?;
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-mint-issuance\0");
        h.update(self.operation_id);
        h.update(context);
        Ok(h.finalize().into())
    }
    /// Derive the exact credit ID with the maintained neutral Mint credit-ID kernel. The
    /// original ordinary context digest distinguishes its authority family; no OEM C is fabricated.
    /// # Errors
    /// Refuses invalid context or typed lifecycle/credit preimage encoding.
    pub fn credit_id(&self) -> Result<[u8; 32], String> {
        self.credit_statement([0; 32], 0, [0; 32])?
            .expected_credit_id()
            .map_err(|e| e.to_string())
    }
    /// Same pre-encryption AAD for actual Native encryption/recovery/decryption.
    /// # Errors
    /// Refuses malformed context, credit identity or typed AAD shape.
    pub fn encrypted_credit_aad(&self) -> Result<KagemushaEncryptedCreditAadV1, String> {
        let value = KagemushaEncryptedCreditAadV1 {
            version: 1,
            purpose: KagemushaEncryptedCreditPurposeV1::Mint,
            context_digest: self.binding_digest()?,
            issuance_or_transition_commitment: self.issuance_commitment()?,
            credit_id: self.credit_id()?,
            amount: self.amount,
        };
        value.validate_shape().map_err(|e| e.to_string())?;
        Ok(value)
    }
    /// Join the actual original C; this checks data against a genuinely verified operand and
    /// does not create a Native financial/key/time or mint-debit permission.
    /// # Errors
    /// Refuses another account, credential, lineage, release, profile, epoch or financial witness.
    pub fn validate_against_credential(
        &self,
        c: &KagemushaVerifiedOrdinaryAppCredentialV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        let s = c.subject();
        let owner = &self.lineage.owner;
        if c.digest() != self.recipient_app_credential_digest
            || s.release_id != self.release_id
            || s.suite_id != self.suite_id
            || s.hardware_profile_id != self.app_credential_profile_id
            || s.policy_epoch != self.policy_epoch
            || s.lane_id != owner.lane_id
            || s.network_id != *owner.runtime.network_id.as_bytes()
            || s.account_binding != kagemusha_ordinary_app_account_binding_v1(&owner.account_id)
            || self.lineage.financial_authority_commitment != s.financial_authority_commitment
            || self.lineage.financial_epoch_id != kagemusha_ordinary_financial_epoch_id_v1(s)?
        {
            return Err("ordinary mint original credential/financial lineage differs".into());
        }
        Ok(())
    }
    /// Open the neutral recipient and credit commitments. Actual Native possession of the
    /// one-use key, genuine proof verification and finalized debit remain separate requirements.
    /// # Errors
    /// Refuses another plaintext credit ID/amount/key/account or either secret opening.
    pub fn validate_credit_opening(
        &self,
        opening: &KagemushaCreditOpeningV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        opening
            .validate_shape_against(self.credit_id()?, self.amount)
            .map_err(|e| e.to_string())?;
        let owner = &self.lineage.owner;
        let rt = &owner.runtime;
        if self.recipient_credential_commitment
            != kagemusha_recipient_credential_commitment_v1(
                self.operation_id,
                self.recipient_app_credential_digest,
                opening.recipient_binding_opening,
            )
            .map_err(|e| e.to_string())?
            || self.credit_commitment
                != kagemusha_mint_credit_opening_commitment_v1(
                    &rt.network_id,
                    &rt.asset,
                    rt.asset_incarnation,
                    rt.scale,
                    kagemusha_liability_pool_id_v1(&rt.network_id, &rt.asset, rt.asset_incarnation)
                        .map_err(|e| e.to_string())?,
                    self.amount,
                    &owner.account_id,
                    self.recipient_one_time_key,
                    opening.credit_commitment_opening,
                )
                .map_err(|e| e.to_string())?
        {
            return Err("ordinary mint plaintext commitments differ".into());
        }
        Ok(())
    }
    fn credit_statement(
        &self,
        credit_id: [u8; 32],
        minted_at_ms: u64,
        authorization_digest: [u8; 32],
    ) -> Result<KagemushaMintCreditStatementV1, String> {
        self.validate_shape()?;
        let owner = &self.lineage.owner;
        let rt = &owner.runtime;
        Ok(KagemushaMintCreditStatementV1 {
            version: 1,
            lifecycle: KagemushaLifecycleBindingV1 {
                version: 1,
                protocol_version: 1,
                network_id: rt.network_id,
                suite_id: self.suite_id,
                vk_digest: self.vk_digest,
                release_id: self.release_id,
                asset: rt.asset.clone(),
                asset_incarnation: rt.asset_incarnation,
                scale: rt.scale,
                liability_pool_id: kagemusha_liability_pool_id_v1(
                    &rt.network_id,
                    &rt.asset,
                    rt.asset_incarnation,
                )
                .map_err(|e| e.to_string())?,
                // The common lifecycle slot names the actual release profile. No credential ID,
                // key handle, hardware counter or OEM assurance is supplied by this ordinary family.
                hardware_profile_id: self.app_credential_profile_id,
                policy_epoch: self.policy_epoch,
                operation_kind: KagemushaOperationKindV1::MintFold,
                request_id: [0; 32],
                receiver_lane_commitment: [0; 32],
                credit_id,
                ciphertext_digest: [0; 32],
            },
            recipient_credential_commitment: self.recipient_credential_commitment,
            authorization_context_digest: self.binding_digest()?,
            mint_authorization_digest: authorization_digest,
            amount: self.amount,
            issuance_commitment: self.issuance_commitment()?,
            recipient: owner.account_id.clone(),
            credit_commitment: self.credit_commitment,
            minted_at_ms,
        })
    }
}

/// Exact post-encryption pre-debit statement. Approval/proof and future finality are excluded.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(
    name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintAuthorizationStatementV1"
)]
pub struct KagemushaOrdinaryMintAuthorizationStatementV1 {
    /// Sole version.
    pub version: u16,
    /// Same pre-ID context.
    pub context: KagemushaOrdinaryMintAuthorizationContextV1,
    /// Derived context-bound issuance commitment.
    pub issuance_commitment: [u8; 32],
    /// Derived output credit ID fixed before encryption and proof.
    pub credit_id: [u8; 32],
    /// Sole digest of the complete384-byte actual encrypted credit original.
    pub ciphertext_digest: [u8; 32],
}
impl KagemushaOrdinaryMintAuthorizationStatementV1 {
    /// Data-only exact identifier check.
    /// # Errors
    /// Refuses version/ID/cipher selector changes.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.context.validate_shape()?;
        nonzero(&[self.ciphertext_digest])?;
        if self.version != 1
            || self.issuance_commitment != self.context.issuance_commitment()?
            || self.credit_id != self.context.credit_id()?
        {
            return Err("ordinary mint identifiers differ".into());
        }
        Ok(())
    }
    /// Sole bounded post-encryption statement original.
    /// # Errors
    /// Refuses invalid shape, encoding or capacity.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded(self)
    }
    /// Complete exact statement digest signed by the separate pre-debit app approval.
    /// # Errors
    /// Refuses invalid shape or encoding.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        hash_original(
            b"iroha:kagemusha:v1:ordinary-mint-statement\0",
            &self.canonical_bytes()?,
        )
    }
    /// Join complete real ciphertext data, without a proof or financial authority conversion.
    /// # Errors
    /// Refuses changed bytes, recipient key or malformed complete envelope.
    pub fn validate_encrypted_credit(&self, raw: &[u8]) -> Result<(), String> {
        self.validate_shape()?;
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            raw,
            self.context.recipient_one_time_key,
        )
        .map_err(|e| e.to_string())?;
        if raw.len() != 384 || kagemusha_ciphertext_digest_v1(raw) != self.ciphertext_digest {
            return Err("ordinary mint ciphertext original differs".into());
        }
        Ok(())
    }
}
/// Dedicated bounded Native pre-debit challenge; constructing it grants no invocation permission.
#[derive(
    Debug,
    Clone,
    Copy,
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintApprovalChallengeV1")]
pub struct KagemushaOrdinaryMintApprovalChallengeV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Same actual reserved Native funding operation.
    pub operation_id: [u8; 32],
    /// Fresh actual Native nonce, fsynced before the single platform invocation.
    pub nonce: [u8; 32],
    /// Same complete purpose-bound C digest.
    pub credential_digest: [u8; 32],
    /// Exact full post-encryption pre-debit statement digest.
    pub statement_digest: [u8; 32],
    /// Same original Native-selected preparation clock context digest.
    pub clock_context_digest: [u8; 32],
    /// Same acknowledged complete signed current FI-control original SHA.
    pub financial_control_original_sha256: [u8; 32],
    /// Actual Native inclusive issue time under both signed interval bounds.
    pub issued_at_ms: u64,
    /// Actual exclusive original approval/FI deadline; retries never widen it.
    pub expires_at_ms: u64,
}
impl KagemushaOrdinaryMintApprovalChallengeV1 {
    /// Exact dedicated210-byte payload with domain and LE64 width. It never signs a future State.
    /// # Errors
    /// Refuses another version, missing selector, invalid interval or widened lifetime.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        nonzero(&[
            self.operation_id,
            self.nonce,
            self.credential_digest,
            self.statement_digest,
            self.clock_context_digest,
            self.financial_control_original_sha256,
        ])?;
        if self.version != 1
            || self.issued_at_ms == 0
            || self.issued_at_ms >= self.expires_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("ordinary mint original approval interval differs".into());
        }
        let mut raw = KAGEMUSHA_ORDINARY_MINT_APPROVAL_DOMAIN_V1.to_vec();
        raw.extend(210_u64.to_le_bytes());
        raw.extend(self.version.to_le_bytes());
        for digest in [
            self.operation_id,
            self.nonce,
            self.credential_digest,
            self.statement_digest,
            self.clock_context_digest,
            self.financial_control_original_sha256,
        ] {
            raw.extend(digest);
        }
        raw.extend(self.issued_at_ms.to_le_bytes());
        raw.extend(self.expires_at_ms.to_le_bytes());
        Ok(raw)
    }
    /// Bind exact pre-debit scope, without granting State/debit/Native authority.
    /// # Errors
    /// Refuses substituted statement, C, original FI decision or clock interval.
    pub fn validate_against_statement(
        &self,
        statement: &KagemushaOrdinaryMintAuthorizationStatementV1,
    ) -> Result<(), String> {
        self.canonical_signing_bytes()?;
        statement.validate_shape()?;
        let c = &statement.context;
        if self.operation_id != c.operation_id
            || self.credential_digest != c.recipient_app_credential_digest
            || self.statement_digest != statement.binding_digest()?
            || self.clock_context_digest != c.clock_context.binding_digest()?
            || self.financial_control_original_sha256 != c.financial_control_original_sha256
            || c.clock_context.lower_at_ms < self.issued_at_ms
            || c.clock_context.upper_at_ms >= self.expires_at_ms
        {
            return Err("ordinary mint approval original context differs".into());
        }
        Ok(())
    }
}
/// Actual platform equation original for this dedicated purpose; never a generic cash approval.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintApprovalV1")]
pub struct KagemushaOrdinaryMintApprovalV1 {
    /// Exact original Native funding challenge.
    pub challenge: KagemushaOrdinaryMintApprovalChallengeV1,
    /// Unmodified DER/App Attest evidence under the actual same enrolled platform key.
    pub evidence: KagemushaAppOperationApprovalEvidenceV1,
}
impl KagemushaOrdinaryMintApprovalV1 {
    /// Check the bounded original evidence codec only. This never authenticates its signature.
    /// # Errors
    /// Refuses unsupported challenge shape, malformed Android DER or oversized Apple original.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.challenge.canonical_signing_bytes()?;
        match &self.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der }
                if (8..=72).contains(&signature_der.len()) =>
            {
                KagemushaDeviceSignatureV1::from_der_normalizing_low_s(signature_der)
                    .map_err(|e| e.to_string())?;
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion }
                if !raw_assertion.is_empty()
                    && raw_assertion.len() <= KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1 => {}
            _ => return Err("ordinary mint evidence original bound differs".into()),
        }
        Ok(())
    }
    /// Exact canonical original of the dedicated pre-debit approval.
    /// # Errors
    /// Refuses malformed evidence or unsupported bounded encoding.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded(self)
    }
    /// Domain-bound complete original approval digest; not a cash or Bootstrap approval identity.
    /// # Errors
    /// Refuses malformed evidence or unsupported bounded encoding.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        hash_original(
            b"iroha:kagemusha:v1:ordinary-mint-approval-original\0",
            &self.canonical_bytes()?,
        )
    }

    /// Recheck only the actual original platform equation against genuine independently verified C.
    /// Current FI/PI/time, Native one-call WAL and financial possession remain mandatory elsewhere.
    /// # Errors
    /// Refuses another C, stale independent Apple floor, signature or malformed challenge.
    pub fn authenticate_platform_equation(
        &self,
        c: &KagemushaVerifiedOrdinaryAppCredentialV1,
        independent_apple_counter_floor: Option<u32>,
    ) -> Result<(Option<u32>, Option<KagemushaAppAttestReleaseMeasurementV1>), String> {
        self.validate_shape()?;
        let s = c.subject();
        if self.challenge.credential_digest != c.digest() {
            return Err("ordinary mint signing C differs".into());
        }
        match (s.platform_class, independent_apple_counter_floor) {
            (KagemushaHardwarePlatformClassV1::AndroidKeyMint, None) => (),
            (KagemushaHardwarePlatformClassV1::AppleAppAttest, Some(floor))
                if floor >= s.app_attest_counter_floor =>
            {
                ()
            }
            _ => return Err("ordinary mint original counter floor differs".into()),
        }
        self.evidence.authenticate_signature(
            s.platform_class,
            &s.app_public_key,
            s.app_signing_identity_digest,
            s.app_release_digest,
            independent_apple_counter_floor,
            &self.challenge.canonical_signing_bytes()?,
        )
    }
}
/// Separate ordinary Mint proof data. The relation directly opens the actual issuer table,
/// full ordinary C, dedicated approval and financial/credit secret commitments. This standalone
/// leaf has no deferred recursive equation/audit carrier. Its current proofs and complete empty
/// histories must independently decide under the actual installed ordinary Mint protocols.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintPairedProofV1")]
pub struct KagemushaOrdinaryMintPairedProofV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact independently installed ordinary Mint Eq protocol commitment.
    pub eq_protocol_digest: [u8; 32],
    /// Exact independently installed ordinary Mint Ep protocol commitment.
    pub ep_protocol_digest: [u8; 32],
    /// Exact complete post-encryption statement used by the closed verifier to reconstruct every
    /// expected ordinary Mint public column, including all semantic operands in both parities.
    pub statement_digest: [u8; 32],
    /// Complete exact dedicated approval selected by that same expected public contract. Its
    /// challenge/evidence equations and full original evidence SHA are constrained in both parities.
    pub approval_original_digest: [u8; 32],
    /// Complete current Eq proof.
    pub eq_proof: Vec<u8>,
    /// Complete current Ep proof.
    pub ep_proof: Vec<u8>,
    /// Actual complete compact Eq empty history for this standalone authorization leaf.
    pub eq_history: Vec<u8>,
    /// Actual complete compact Ep empty history for this standalone authorization leaf.
    pub ep_history: Vec<u8>,
}
impl KagemushaOrdinaryMintPairedProofV1 {
    /// Validate data-only distinct roles, exact histories and current proof capacities.
    /// # Errors
    /// Refuses missing/aliased selectors, another statement, malformed history or proof bounds.
    pub fn validate_shape_for_authorization(
        &self,
        expected_statement: [u8; 32],
        expected_approval: [u8; 32],
    ) -> Result<(), String> {
        nonzero(&[
            self.eq_protocol_digest,
            self.ep_protocol_digest,
            self.statement_digest,
            self.approval_original_digest,
        ])?;
        if self.version != 1
            || self.statement_digest != expected_statement
            || self.approval_original_digest != expected_approval
            || self.eq_protocol_digest == self.ep_protocol_digest
            || self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > KAGEMUSHA_CURRENT_PROOFS_MAX_BYTES_V1
            || self.eq_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.ep_history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
            || self.eq_history.iter().all(|b| *b == 0)
            || self.ep_history.iter().all(|b| *b == 0)
            || self.eq_history == self.ep_history
        {
            return Err("ordinary mint proof original roles/statement/width differ".into());
        }
        if norito::encode_canonical(self)
            .map_err(|e| e.to_string())?
            .len()
            > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        {
            return Err("ordinary mint complete proof original exceeds capacity".into());
        }
        Ok(())
    }
}

/// Data-only ordinary pre-debit proof family, consumed by actual installed ordinary Mint keys.
/// Neither a paired proof shape nor an app signature authorizes the real online account debit.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintAuthorizationV1")]
pub struct KagemushaOrdinaryMintAuthorizationV1 {
    /// Sole version.
    pub version: u16,
    /// Exact post-encryption original approved before the actual online debit.
    pub statement: KagemushaOrdinaryMintAuthorizationStatementV1,
    /// Distinct original approval, not purpose1/W2/captured Bootstrap.
    pub approval: KagemushaOrdinaryMintApprovalV1,
    /// Actual both-parity ordinary Mint proof and complete histories. Decoding is not verification.
    pub proof: KagemushaOrdinaryMintPairedProofV1,
}
impl KagemushaOrdinaryMintAuthorizationV1 {
    /// Data-only complete proof original validation.
    /// # Errors
    /// Refuses version, complete signed subject or proof-frame substitution.
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("ordinary mint authorization version differs".into());
        }
        self.approval.validate_shape()?;
        self.approval
            .challenge
            .validate_against_statement(&self.statement)?;
        self.proof.validate_shape_for_authorization(
            self.statement.binding_digest()?,
            self.approval.binding_digest()?,
        )?;
        Ok(())
    }
    /// Sole bounded complete ordinary pre-debit authorization original.
    /// # Errors
    /// Refuses malformed frame, encoding or capacity.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded(self)
    }
    /// Full authorization/proof original digest, excluded from the earlier issuance/credit-ID preimage.
    /// # Errors
    /// Refuses invalid data or encoding.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        hash_original(
            b"iroha:kagemusha:v1:ordinary-mint-authorization\0",
            &self.canonical_bytes()?,
        )
    }
    /// Reconstruct the neutral finalized Mint statement data from actual finalized debit time.
    /// Calling this data helper supplies no ledger finality or MintAuthority proof.
    /// # Errors
    /// Refuses malformed authorization or finalized neutral lifecycle statement shape.
    pub fn finalized_credit_statement(
        &self,
        minted_at_ms: u64,
    ) -> Result<KagemushaMintCreditStatementV1, String> {
        self.validate_shape()?;
        let mut s = self.statement.context.credit_statement(
            self.statement.credit_id,
            minted_at_ms,
            self.binding_digest()?,
        )?;
        s.lifecycle.ciphertext_digest = self.statement.ciphertext_digest;
        s.validate_shape().map_err(|e| e.to_string())?;
        Ok(s)
    }
}
/// Sole first-release ordinary top-up request data. Actual installed proof/FI/clock/DATA policy,
/// genuine account signing and finality are required by Core before moving online funds.
#[derive(
    Debug,
    Clone,
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryTopUpRequestV1")]
pub struct KagemushaOrdinaryTopUpRequestV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Complete dedicated ordinary authorization, not OEM credential/key-handle data.
    pub authorization: KagemushaOrdinaryMintAuthorizationV1,
    /// Complete384-byte actual AEAD original, fixed before app approval/proof.
    pub encrypted_credit: Vec<u8>,
}
impl KagemushaOrdinaryTopUpRequestV1 {
    /// Validate exact neutral request data, without proof/effect authority.
    /// # Errors
    /// Refuses missing actual authorization, changed ciphertext or canonical bounds.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        if self.version != 1 {
            return Err("ordinary top-up request version differs".into());
        }
        self.authorization.validate_shape()?;
        self.authorization
            .statement
            .validate_encrypted_credit(&self.encrypted_credit)?;
        bounded(self)
    }
    /// Exact account-consent message for this complete top-up, distinct from read-only FI/cash CAS.
    /// # Errors
    /// Refuses malformed original request or bounded canonical encoding.
    pub fn account_signing_message(&self) -> Result<Vec<u8>, String> {
        let raw = self.canonical_bytes()?;
        let mut message = b"iroha:kagemusha:v1:ordinary-topup-account-consent\0".to_vec();
        message.extend((raw.len() as u64).to_le_bytes());
        message.extend(raw);
        Ok(message)
    }
    /// Verify the same single-member Ed wallet account supported by actual Native custody.
    /// An account signature still supplies no FI current grant, ordinary proof or debit finality.
    /// # Errors
    /// Refuses account/threshold/weight/key-algorithm drift or another complete consent message.
    pub fn verify_account_signature(&self, signature: &Signature) -> Result<(), String> {
        let policy = self
            .authorization
            .statement
            .context
            .lineage
            .owner
            .account_id
            .multisig_policy()
            .ok_or("ordinary top-up wallet account unsupported")?;
        let member = policy
            .members()
            .first()
            .ok_or("ordinary top-up account signer absent")?;
        if policy.threshold() != 1
            || policy.members().len() != 1
            || member.weight() != 1
            || member.public_key().algorithm() != Algorithm::Ed25519
            || signature.payload().len() != 64
        {
            return Err("ordinary top-up account consent shape differs".into());
        }
        signature
            .verify(member.public_key(), &self.account_signing_message()?)
            .map_err(|_| "ordinary top-up account consent rejected".into())
    }
    /// Decode one sole bounded complete ordinary original, never a Native owner or top-up grant.
    /// # Errors
    /// Refuses noncanonical, trailing, unsupported or malformed data.
    pub fn decode_canonical_exact(raw: &[u8]) -> Result<Self, String> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1 {
            return Err("ordinary top-up original bound differs".into());
        }
        let v: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if v.canonical_bytes() != Ok(raw.to_vec()) {
            return Err("ordinary top-up original differs".into());
        }
        Ok(v)
    }
}
fn nonzero(digests: &[[u8; 32]]) -> Result<(), String> {
    if digests.contains(&[0; 32]) {
        return Err("ordinary mint selector absent".into());
    }
    Ok(())
}
fn bounded<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let raw = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1 {
        return Err("ordinary mint original bound differs".into());
    }
    Ok(raw)
}
fn hash_original(domain: &[u8], raw: &[u8]) -> Result<[u8; 32], String> {
    if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1 {
        return Err("ordinary mint hash original bound differs".into());
    }
    let mut h = Sha256::new();
    h.update(domain);
    h.update((raw.len() as u64).to_le_bytes());
    h.update(raw);
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::KeyGenOption;
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    // Known-public synthetic enrollment and envelope/proof codec data. Only the app equation
    // test below signs and verifies real ECDSA. This fixture supplies no Mint proof/debit authority.
    fn make_context(
        apple: bool,
    ) -> (
        Fixture,
        KagemushaOrdinaryMintAuthorizationContextV1,
        KagemushaCreditOpeningV1,
    ) {
        let fixture = Fixture::with_single_member_wallet(apple, false, [19; 32]);
        let verified = fixture.verify(1000).unwrap();
        let c = verified.app_credential();
        let s = c.subject();
        let (x, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![32; 32]));
        let recipient_key = x.to_bytes();
        let owner = fixture.selection.owner.clone();
        let rt = &owner.runtime;
        let operation_id = [45; 32];
        let amount = 177;
        let opening = KagemushaCreditOpeningV1 {
            version: 1,
            credit_id: [0; 32],
            amount,
            credit_commitment_opening: [50; 32],
            recipient_binding_opening: [51; 32],
            recovery_nonce: [52; 32],
        };
        let mut context = KagemushaOrdinaryMintAuthorizationContextV1 {
            version: 1,
            operation_id,
            lineage: KagemushaOrdinaryFinancialLineageV1 {
                version: 1,
                owner: owner.clone(),
                financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(s).unwrap(),
                financial_authority_commitment: s.financial_authority_commitment,
            },
            predecessor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: [46; 32],
                logical_sequence: (1_u128 << 101) + 7,
                state_original_sha256: [47; 32],
            },
            release_id: s.release_id,
            suite_id: s.suite_id,
            vk_digest: [48; 32],
            artifact_manifest_digest: [49; 32],
            recipient_app_credential_digest: c.digest(),
            app_credential_profile_id: s.hardware_profile_id,
            policy_epoch: s.policy_epoch,
            amount,
            recipient_credential_commitment: kagemusha_recipient_credential_commitment_v1(
                operation_id,
                c.digest(),
                opening.recipient_binding_opening,
            )
            .unwrap(),
            credit_commitment: kagemusha_mint_credit_opening_commitment_v1(
                &rt.network_id,
                &rt.asset,
                rt.asset_incarnation,
                rt.scale,
                kagemusha_liability_pool_id_v1(&rt.network_id, &rt.asset, rt.asset_incarnation)
                    .unwrap(),
                amount,
                &owner.account_id,
                recipient_key,
                opening.credit_commitment_opening,
            )
            .unwrap(),
            recipient_one_time_key: recipient_key,
            clock_context: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: [53; 32],
                signed_observations_original_digest: [54; 32],
                lower_at_ms: 1000,
                upper_at_ms: 1010,
            },
            financial_control_original_sha256: [55; 32],
        };
        context.validate_shape().unwrap();
        context.validate_against_credential(c).unwrap();
        let mut opening = opening;
        opening.credit_id = context.credit_id().unwrap();
        context.validate_credit_opening(&opening).unwrap();
        // Ensure a stable valid context is returned, without claiming these metadata bytes were observed.
        context.version = 1;
        (fixture, context, opening)
    }
    fn make_statement(
        context: KagemushaOrdinaryMintAuthorizationContextV1,
    ) -> (KagemushaOrdinaryMintAuthorizationStatementV1, Vec<u8>) {
        let (x, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![33; 32]));
        let envelope = KagemushaEncryptedCreditEnvelopeV1 {
            version: 1,
            ephemeral_x25519_public_key: x.to_bytes(),
            nonce: [56; 24],
            ciphertext_and_tag: vec![
                57;
                super::super::kagemusha_credit_opening_canonical_len_v1()
                    .unwrap()
                    + 16
            ],
        };
        let raw = envelope
            .canonical_bytes_against_recipient_key(context.recipient_one_time_key)
            .unwrap();
        assert_eq!(raw.len(), 384);
        let s = KagemushaOrdinaryMintAuthorizationStatementV1 {
            version: 1,
            issuance_commitment: context.issuance_commitment().unwrap(),
            credit_id: context.credit_id().unwrap(),
            context,
            ciphertext_digest: kagemusha_ciphertext_digest_v1(&raw),
        };
        s.validate_encrypted_credit(&raw).unwrap();
        (s, raw)
    }
    fn approval(
        statement: &KagemushaOrdinaryMintAuthorizationStatementV1,
    ) -> KagemushaOrdinaryMintApprovalV1 {
        let context = &statement.context;
        let challenge = KagemushaOrdinaryMintApprovalChallengeV1 {
            version: 1,
            operation_id: context.operation_id,
            nonce: [58; 32],
            credential_digest: context.recipient_app_credential_digest,
            statement_digest: statement.binding_digest().unwrap(),
            clock_context_digest: context.clock_context.binding_digest().unwrap(),
            financial_control_original_sha256: context.financial_control_original_sha256,
            issued_at_ms: 1000,
            expires_at_ms: 1100,
        };
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let signature: P256Signature = key.sign(&challenge.canonical_signing_bytes().unwrap());
        KagemushaOrdinaryMintApprovalV1 {
            challenge,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            },
        }
    }
    fn authorization(
        statement: KagemushaOrdinaryMintAuthorizationStatementV1,
    ) -> KagemushaOrdinaryMintAuthorizationV1 {
        let approval = approval(&statement);
        // Explicitly inert proof frame data for codec/selector tests; no proof verifier is called.
        let proof = KagemushaOrdinaryMintPairedProofV1 {
            version: 1,
            eq_protocol_digest: [59; 32],
            ep_protocol_digest: [60; 32],
            statement_digest: statement.binding_digest().unwrap(),
            approval_original_digest: approval.binding_digest().unwrap(),
            eq_proof: vec![63],
            ep_proof: vec![64],
            eq_history: vec![65; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![66; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        };
        KagemushaOrdinaryMintAuthorizationV1 {
            version: 1,
            statement,
            approval,
            proof,
        }
    }
    #[test]
    fn ordinary_mint_identifiers_are_acyclic_and_full_context_is_bound() {
        let (_, context, _) = make_context(false);
        let (mut statement, _) = make_statement(context.clone());
        let issuance = context.issuance_commitment().unwrap();
        let id = context.credit_id().unwrap();
        let digest = statement.binding_digest().unwrap();
        statement.ciphertext_digest[0] ^= 1;
        assert_ne!(statement.binding_digest().unwrap(), digest);
        assert_eq!(statement.context.issuance_commitment().unwrap(), issuance);
        assert_eq!(statement.context.credit_id().unwrap(), id);
        for field in 0..5 {
            let mut changed = context.clone();
            match field {
                0 => changed.predecessor.logical_sequence ^= 1_u128 << 100,
                1 => changed.predecessor.state_original_sha256[0] ^= 1,
                2 => changed.financial_control_original_sha256[0] ^= 1,
                3 => changed.clock_context.upper_at_ms += 1,
                _ => changed.lineage.financial_authority_commitment[0] ^= 1,
            }
            assert_ne!(
                changed.binding_digest().unwrap(),
                context.binding_digest().unwrap()
            );
            assert_ne!(changed.credit_id().unwrap(), id);
        }
    }
    #[test]
    fn ordinary_mint_credit_openings_and_typed_aad_join_actual_identifiers() {
        let (_, context, opening) = make_context(false);
        let aad = context.encrypted_credit_aad().unwrap();
        assert_eq!(aad.purpose, KagemushaEncryptedCreditPurposeV1::Mint);
        assert_eq!(aad.credit_id, opening.credit_id);
        assert_eq!(aad.amount, opening.amount);
        assert_eq!(aad.context_digest, context.binding_digest().unwrap());
        for field in 0..4 {
            let mut changed = opening.clone();
            match field {
                0 => changed.credit_commitment_opening[0] ^= 1,
                1 => changed.recipient_binding_opening[0] ^= 1,
                2 => changed.amount += 1,
                _ => changed.credit_id[0] ^= 1,
            }
            assert!(context.validate_credit_opening(&changed).is_err());
        }
    }
    #[test]
    fn ordinary_mint_dedicated_equation_binds_original_nonce_statement_and_counter_scope() {
        let (fixture, context, _) = make_context(false);
        let (statement, _) = make_statement(context);
        let mut original = approval(&statement);
        let verified = fixture.verify(1000).unwrap();
        original
            .challenge
            .validate_against_statement(&statement)
            .unwrap();
        assert_eq!(
            original
                .authenticate_platform_equation(verified.app_credential(), None)
                .unwrap(),
            (None, None)
        );
        let signing = original.challenge.canonical_signing_bytes().unwrap();
        assert_eq!(
            signing.len(),
            KAGEMUSHA_ORDINARY_MINT_APPROVAL_DOMAIN_V1.len() + 8 + 210
        );
        assert_eq!(
            &signing[KAGEMUSHA_ORDINARY_MINT_APPROVAL_DOMAIN_V1.len()..][..8],
            &210_u64.to_le_bytes()
        );
        assert!(
            original
                .authenticate_platform_equation(verified.app_credential(), Some(0))
                .is_err()
        );
        original.challenge.nonce[0] ^= 1;
        assert!(
            original
                .authenticate_platform_equation(verified.app_credential(), None)
                .is_err()
        );
        let (apple_fixture, apple_context, _) = make_context(true);
        let (apple_statement, _) = make_statement(apple_context);
        let mut apple = approval(&apple_statement);
        apple.evidence = KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
            raw_assertion: vec![1],
        };
        let apple_verified = apple_fixture.verify(1000).unwrap();
        assert!(
            apple
                .authenticate_platform_equation(apple_verified.app_credential(), Some(10))
                .unwrap_err()
                .contains("original counter floor")
        );
    }
    #[test]
    fn ordinary_mint_approval_interval_and_full_original_scope_refuse_substitutions() {
        let (_, context, _) = make_context(false);
        let (statement, _) = make_statement(context);
        let original = approval(&statement);
        for field in 0..7 {
            let mut changed = original.challenge;
            match field {
                0 => changed.operation_id[0] ^= 1,
                1 => changed.credential_digest[0] ^= 1,
                2 => changed.statement_digest[0] ^= 1,
                3 => changed.clock_context_digest[0] ^= 1,
                4 => changed.financial_control_original_sha256[0] ^= 1,
                5 => changed.issued_at_ms = 1001,
                _ => changed.expires_at_ms = 1010,
            }
            assert!(changed.validate_against_statement(&statement).is_err());
        }
        let mut widened = original.challenge;
        widened.expires_at_ms =
            widened.issued_at_ms + KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1 + 1;
        assert!(widened.canonical_signing_bytes().is_err());
    }
    #[test]
    fn ordinary_mint_complete_authorization_binds_approval_and_distinct_proof_histories() {
        let (_, context, _) = make_context(false);
        let (statement, _) = make_statement(context);
        let original = authorization(statement);
        original.validate_shape().unwrap();
        for field in 0..6 {
            let mut changed = original.clone();
            match field {
                0 => changed.approval.challenge.nonce[0] ^= 1,
                1 => changed.proof.approval_original_digest[0] ^= 1,
                2 => changed.proof.statement_digest[0] ^= 1,
                3 => changed.proof.ep_protocol_digest = changed.proof.eq_protocol_digest,
                4 => changed.proof.eq_history.pop().map(|_| ()).unwrap(),
                _ => changed.proof.ep_history = changed.proof.eq_history.clone(),
            }
            assert!(changed.validate_shape().is_err());
        }
        let mut new_blinding = original.clone();
        new_blinding.proof.eq_proof[0] ^= 1;
        assert_eq!(
            new_blinding.statement.context.credit_id().unwrap(),
            original.statement.credit_id
        );
        assert_ne!(
            new_blinding.binding_digest().unwrap(),
            original.binding_digest().unwrap()
        );
    }
    #[test]
    fn ordinary_mint_finalized_statement_keeps_prior_credit_id_and_requires_actual_cipher_time() {
        let (_, context, _) = make_context(false);
        let (statement, _) = make_statement(context);
        let original = authorization(statement);
        let finalized = original.finalized_credit_statement(1400).unwrap();
        assert_eq!(finalized.lifecycle.credit_id, original.statement.credit_id);
        assert_eq!(
            finalized.lifecycle.ciphertext_digest,
            original.statement.ciphertext_digest
        );
        assert_eq!(
            finalized.mint_authorization_digest,
            original.binding_digest().unwrap()
        );
        assert_eq!(finalized.minted_at_ms, 1400);
        assert!(original.finalized_credit_statement(0).is_err());
    }
    #[test]
    fn ordinary_topup_complete_canonical_original_refuses_trailing_and_cipher_substitution() {
        let (_, context, _) = make_context(false);
        let (statement, encrypted_credit) = make_statement(context);
        let request = KagemushaOrdinaryTopUpRequestV1 {
            version: 1,
            authorization: authorization(statement),
            encrypted_credit,
        };
        let raw = request.canonical_bytes().unwrap();
        assert_eq!(
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&raw).unwrap(),
            request
        );
        let mut trailing = raw;
        trailing.push(0);
        assert!(KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&trailing).is_err());
        let mut changed = request;
        changed.encrypted_credit[383] ^= 1;
        assert!(changed.canonical_bytes().is_err());
    }
    #[test]
    fn ordinary_topup_actual_account_signature_binds_entire_original_request() {
        let (_, context, _) = make_context(false);
        let (statement, encrypted_credit) = make_statement(context);
        let request = KagemushaOrdinaryTopUpRequestV1 {
            version: 1,
            authorization: authorization(statement),
            encrypted_credit,
        };
        let wallet = iroha_crypto::KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let signature = Signature::new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        );
        request.verify_account_signature(&signature).unwrap();
        let mut substituted = request;
        substituted.authorization.proof.eq_proof[0] ^= 1;
        assert!(substituted.verify_account_signature(&signature).is_err());
    }
}
