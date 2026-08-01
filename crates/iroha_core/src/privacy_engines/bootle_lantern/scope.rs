//! Reusable credential-scope binding for the concrete Falcon-512 profile.

use iroha_data_model::{
    ChainId,
    privacy::{
        BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BootleLanternIssuerPolicyLifecycleV1,
        BootleLanternIssuerPolicyV1, PRIVACY_MAX_CHAIN_ID_BYTES_V1,
        PrivacyBootleLanternIssuerPolicyDigestV1, PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1,
        PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyIdV1,
        PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
    },
};
use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use thiserror::Error;

use super::{params::APPLICATION_MODULUS_V1, ring::ApplicationPolynomialV1};

/// Domain for the algebraic credential-scope term.
pub const BOOTLE_LANTERN_CREDENTIAL_SCOPE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.lazer-falcon512-credential-scope.v1";
const CONCRETE_PROFILE_ID_V1: &[u8] = b"lazer-falcon512-concrete-specialization";
const PROTOCOL_ID_V1: &[u8] = b"iroha-bootle-lantern-anoncred-v1";
const SCOPE_VERSION_V1: [u8; 2] = 1_u16.to_be_bytes();
/// Largest multiple of 12,289 below 2^16 used by scope hash rejection.
pub const BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1: u16 = 61_445;
/// Maximum 16-bit proposals consumed for one scope coefficient.
pub const BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1: u32 = 4_096;

/// Reusable governed scope permanently signed into one credential.
///
/// Presentation-specific action index and transaction intent are deliberately
/// absent. Every other reusable chain/governance artifact from the statement
/// context is included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootleLanternCredentialScopeV1 {
    chain_id: ChainId,
    genesis_hash: [u8; 32],
    parameter_id: PrivacyParameterIdV1,
    parameter_digest: PrivacyParameterDigestV1,
    verifier_digest: PrivacyVerifierDigestV1,
    statement_schema_digest: PrivacyStatementSchemaDigestV1,
    engine_manifest_digest: PrivacyEngineManifestDigestV1,
    issuer_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    policy_epoch: u64,
    policy_record_digest: PrivacyBootleLanternIssuerPolicyDigestV1,
    issuer_parameter_id: PrivacyParameterIdV1,
    issuer_parameter_digest: PrivacyParameterDigestV1,
}

impl BootleLanternCredentialScopeV1 {
    /// Select the reusable scope from a statement-context template and one
    /// exact active policy record.
    ///
    /// `action_index` and `transaction_intent_digest` in `context` are ignored
    /// because a credential is reusable across presentations.
    ///
    /// # Errors
    ///
    /// Rejects malformed or zero reusable bindings, an invalid policy, or a
    /// policy/context identity mismatch.
    pub fn new(
        context: &PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &BootleLanternIssuerPolicyV1,
    ) -> Result<Self, CredentialScopeErrorV1> {
        policy
            .validate()
            .map_err(|_| CredentialScopeErrorV1::InvalidPolicy)?;
        if policy.lifecycle != BootleLanternIssuerPolicyLifecycleV1::Active {
            return Err(CredentialScopeErrorV1::InvalidPolicy);
        }
        let chain_id_length = context.chain_id.as_str().len();
        if chain_id_length == 0
            || u32::try_from(chain_id_length)
                .ok()
                .is_none_or(|length| length > PRIVACY_MAX_CHAIN_ID_BYTES_V1)
        {
            return Err(CredentialScopeErrorV1::InvalidChainId);
        }
        if canonical_genesis_hash == [0; 32] {
            return Err(CredentialScopeErrorV1::ZeroBinding("genesis_hash"));
        }
        for (field, is_zero) in [
            ("parameter_id", context.parameter_id.is_zero()),
            ("parameter_digest", context.parameter_digest.is_zero()),
            ("verifier_digest", context.verifier_digest.is_zero()),
            (
                "statement_schema_digest",
                context.statement_schema_digest.is_zero(),
            ),
            (
                "engine_manifest_digest",
                context.engine_manifest_digest.is_zero(),
            ),
        ] {
            if is_zero {
                return Err(CredentialScopeErrorV1::ZeroBinding(field));
            }
        }
        Ok(Self {
            chain_id: context.chain_id.clone(),
            genesis_hash: canonical_genesis_hash,
            parameter_id: context.parameter_id,
            parameter_digest: context.parameter_digest,
            verifier_digest: context.verifier_digest,
            statement_schema_digest: context.statement_schema_digest,
            engine_manifest_digest: context.engine_manifest_digest,
            issuer_id: policy.issuer_id,
            policy_id: policy.policy_id,
            policy_epoch: policy.epoch,
            policy_record_digest: policy.record_digest,
            issuer_parameter_id: policy.issuer_parameter_id,
            issuer_parameter_digest: policy.issuer_parameter_digest,
        })
    }

    /// Return whether this scope is exactly selected by a later presentation.
    #[must_use]
    pub fn matches(
        &self,
        context: &PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &BootleLanternIssuerPolicyV1,
    ) -> bool {
        Self::new(context, canonical_genesis_hash, policy).is_ok_and(|candidate| candidate == *self)
    }

    pub(crate) fn application_term(
        &self,
    ) -> Result<[ApplicationPolynomialV1; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1], CredentialScopeErrorV1>
    {
        let mut state = Shake256::default();
        absorb_frame(&mut state, BOOTLE_LANTERN_CREDENTIAL_SCOPE_DOMAIN_V1)?;
        self.absorb_fields(&mut state)?;
        let mut reader = state.finalize_xof();
        let mut output = [ApplicationPolynomialV1::ZERO; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1];
        for polynomial in &mut output {
            let mut coefficients = [0_u16; 64];
            for coefficient in &mut coefficients {
                let mut accepted = None;
                for _ in 0..BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1 {
                    let mut bytes = [0_u8; 2];
                    reader.read(&mut bytes);
                    let candidate = u16::from_be_bytes(bytes);
                    if candidate < BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1 {
                        accepted = Some(candidate % APPLICATION_MODULUS_V1);
                        break;
                    }
                }
                *coefficient = accepted.ok_or(CredentialScopeErrorV1::SamplingExhausted)?;
            }
            *polynomial = ApplicationPolynomialV1::new(coefficients)
                .map_err(|_| CredentialScopeErrorV1::InternalInvariant)?;
        }
        Ok(output)
    }

    pub(crate) fn digest(&self) -> Result<[u8; 32], CredentialScopeErrorV1> {
        let mut state = Shake256::default();
        absorb_frame(
            &mut state,
            b"iroha.privacy.bootle-lantern.credential-scope-digest.v1",
        )?;
        self.absorb_fields(&mut state)?;
        let mut reader = state.finalize_xof();
        let mut digest = [0_u8; 32];
        reader.read(&mut digest);
        if digest == [0; 32] {
            return Err(CredentialScopeErrorV1::InternalInvariant);
        }
        Ok(digest)
    }

    fn absorb_fields(&self, state: &mut Shake256) -> Result<(), CredentialScopeErrorV1> {
        let epoch = self.policy_epoch.to_be_bytes();
        for (label, value) in [
            (&b"protocol"[..], PROTOCOL_ID_V1),
            (&b"profile"[..], CONCRETE_PROFILE_ID_V1),
            (&b"version"[..], &SCOPE_VERSION_V1),
            (&b"chain_id"[..], self.chain_id.as_str().as_bytes()),
            (&b"genesis_hash"[..], &self.genesis_hash),
            (&b"parameter_id"[..], self.parameter_id.as_bytes()),
            (&b"parameter_digest"[..], self.parameter_digest.as_bytes()),
            (&b"verifier_digest"[..], self.verifier_digest.as_bytes()),
            (
                &b"statement_schema_digest"[..],
                self.statement_schema_digest.as_bytes(),
            ),
            (
                &b"engine_manifest_digest"[..],
                self.engine_manifest_digest.as_bytes(),
            ),
            (&b"issuer_id"[..], self.issuer_id.as_bytes()),
            (&b"policy_id"[..], self.policy_id.as_bytes()),
            (&b"policy_epoch"[..], &epoch),
            (
                &b"policy_record_digest"[..],
                self.policy_record_digest.as_bytes(),
            ),
            (
                &b"issuer_parameter_id"[..],
                self.issuer_parameter_id.as_bytes(),
            ),
            (
                &b"issuer_parameter_digest"[..],
                self.issuer_parameter_digest.as_bytes(),
            ),
        ] {
            absorb_frame(state, label)?;
            absorb_frame(state, value)?;
        }
        Ok(())
    }
}

fn absorb_frame(state: &mut Shake256, value: &[u8]) -> Result<(), CredentialScopeErrorV1> {
    let length = u32::try_from(value.len()).map_err(|_| CredentialScopeErrorV1::FieldTooLarge)?;
    state.update(&length.to_be_bytes());
    state.update(value);
    Ok(())
}

/// Failure while selecting or expanding one reusable credential scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CredentialScopeErrorV1 {
    /// The issuer policy is malformed, revoked, or self-inconsistent.
    #[error("Bootle/Lantern credential scope selected an invalid issuer policy")]
    InvalidPolicy,
    /// The chain identifier is empty or exceeds its fixed public cap.
    #[error("Bootle/Lantern credential scope chain id is invalid")]
    InvalidChainId,
    /// A reusable governed binding is zero.
    #[error("Bootle/Lantern credential scope field is zero: {0}")]
    ZeroBinding(&'static str),
    /// One framed public field exceeded the canonical `u32` length domain.
    #[error("Bootle/Lantern credential scope field is too large")]
    FieldTooLarge,
    /// Uniform rejection sampling exhausted its fixed public cap.
    #[error("Bootle/Lantern credential scope expansion exhausted its work budget")]
    SamplingExhausted,
    /// A closed arithmetic invariant failed.
    #[error("Bootle/Lantern credential scope internal invariant failed")]
    InternalInvariant,
}
