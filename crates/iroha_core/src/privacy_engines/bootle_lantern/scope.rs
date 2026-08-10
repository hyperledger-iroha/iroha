//! Reusable credential-scope binding for the concrete Falcon-512 profile.

use iroha_data_model::{
    NetworkId,
    privacy::{
        BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BootleLanternIssuerPolicyLifecycleV1,
        BootleLanternIssuerPolicyV1, PrivacyBootleLanternIssuerPolicyDigestV1,
        PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyPolicyIdV1, PrivacyStatementContextV1,
        PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
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
/// Domain for the reusable credential-scope identity digest.
pub(crate) const BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.credential-scope-digest.v1";
const CONCRETE_PROFILE_ID_V1: &[u8] = b"lazer-falcon512-concrete-specialization";
const PROTOCOL_ID_V1: &[u8] = b"iroha-bootle-lantern-anoncred-v1";
const SCOPE_VERSION_V1: [u8; 2] = 1_u16.to_be_bytes();
/// Largest multiple of 12,289 below 2^16 used by scope hash rejection.
pub const BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1: u16 = 61_445;
/// Maximum 16-bit proposals consumed for one scope coefficient.
pub const BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1: u32 = 4_096;
/// Canonical reusable-scope schema owned by the implementation that absorbs it.
pub const BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1: &[u8] = b"scope-xof:SHAKE256-framed-u32be-uniform-mod12289-accept<61445-max4096-per-coefficient|included:protocol+concrete-profile+version+network-id+canonical-genesis-hash+parameter-id+parameter-digest+verifier-digest+statement-schema-digest+engine-manifest-digest+issuer-id+policy-id+epoch+policy-record-digest+issuer-parameter-id+issuer-parameter-digest|excluded:action-index+transaction-intent-digest|rotation:every-included-field-invalidates-existing-credential";

/// Reusable governed scope permanently signed into one credential.
///
/// Presentation-specific action index and transaction intent are deliberately
/// absent. Every other reusable chain/governance artifact from the statement
/// context is included.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootleLanternCredentialScopeV1 {
    network_id: NetworkId,
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
        if canonical_genesis_hash == [0; 32] {
            return Err(CredentialScopeErrorV1::ZeroBinding("genesis_hash"));
        }
        if context.network_id.as_bytes() != &canonical_genesis_hash {
            return Err(CredentialScopeErrorV1::NetworkGenesisMismatch);
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
            network_id: context.network_id,
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
                *coefficient = bounded_scope_coefficient_v1(
                    BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1,
                    || {
                        let mut bytes = [0_u8; 2];
                        reader.read(&mut bytes);
                        u16::from_be_bytes(bytes)
                    },
                )
                .ok_or(CredentialScopeErrorV1::SamplingExhausted)?;
            }
            *polynomial = ApplicationPolynomialV1::new(coefficients)
                .map_err(|_| CredentialScopeErrorV1::InternalInvariant)?;
        }
        Ok(output)
    }

    pub(crate) fn digest(&self) -> Result<[u8; 32], CredentialScopeErrorV1> {
        let mut state = Shake256::default();
        absorb_frame(&mut state, BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1)?;
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
            (&b"network_id"[..], self.network_id.as_bytes()),
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

fn bounded_scope_coefficient_v1<F>(max_attempts: u32, mut next_candidate: F) -> Option<u16>
where
    F: FnMut() -> u16,
{
    for _ in 0..max_attempts {
        let candidate = next_candidate();
        if candidate < BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1 {
            return Some(candidate % APPLICATION_MODULUS_V1);
        }
    }
    None
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
    /// The typed network identity and explicit genesis hash differ.
    #[error("Bootle/Lantern credential scope network id differs from its genesis hash")]
    NetworkGenesisMismatch,
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

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::privacy::{
        BootleLanternAllowedAttributeValuesV1, BootleLanternIssuerPublicMatrixV1,
        BootleLanternPolynomialV1, PrivacyTransactionIntentDigestV1,
    };
    use sha2::{Digest as _, Sha256};

    use super::*;

    const fn raw(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn network_id(bytes: [u8; 32]) -> NetworkId {
        NetworkId::from_genesis_hash(
            HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::prehashed(bytes),
            ),
        )
    }

    fn kat_scope() -> BootleLanternCredentialScopeV1 {
        BootleLanternCredentialScopeV1 {
            network_id: network_id(raw(2)),
            genesis_hash: raw(2),
            parameter_id: PrivacyParameterIdV1::new(raw(3)),
            parameter_digest: PrivacyParameterDigestV1::new(raw(4)),
            verifier_digest: PrivacyVerifierDigestV1::new(raw(5)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(6)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(7)),
            issuer_id: PrivacyIssuerIdV1::new(raw(8)),
            policy_id: PrivacyPolicyIdV1::new(raw(9)),
            policy_epoch: 10,
            policy_record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new(raw(11)),
            issuer_parameter_id: PrivacyParameterIdV1::new(raw(12)),
            issuer_parameter_digest: PrivacyParameterDigestV1::new(raw(13)),
        }
    }

    fn context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            network_id: network_id(raw(20)),
            action_index: 3,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(raw(1)),
            parameter_id: PrivacyParameterIdV1::new(raw(2)),
            parameter_digest: PrivacyParameterDigestV1::new(raw(3)),
            verifier_digest: PrivacyVerifierDigestV1::new(raw(4)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(5)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(6)),
        }
    }

    fn active_policy() -> BootleLanternIssuerPolicyV1 {
        let first_column = core::array::from_fn(|row| BootleLanternPolynomialV1 {
            coefficients: (0..64)
                .map(|coefficient| {
                    u16::try_from(row * 64 + coefficient + 1).expect("fixture coefficient")
                })
                .collect(),
        });
        let issuer_public_matrix =
            BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(&first_column)
                .expect("canonical dense multiplication matrix");
        let mut policy = BootleLanternIssuerPolicyV1 {
            issuer_id: PrivacyIssuerIdV1::new(raw(7)),
            policy_id: PrivacyPolicyIdV1::new(raw(8)),
            epoch: 1,
            lifecycle: BootleLanternIssuerPolicyLifecycleV1::Active,
            issuer_parameter_id: PrivacyParameterIdV1::new(raw(9)),
            issuer_parameter_digest: PrivacyParameterDigestV1::new(raw(10)),
            issuer_public_matrix,
            required_disclosure_bitmap: 0,
            allowed_values: (0..BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1)
                .map(|_| BootleLanternAllowedAttributeValuesV1 { values: Vec::new() })
                .collect(),
            record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
        };
        policy.issuer_parameter_digest = policy
            .computed_issuer_parameter_digest()
            .expect("issuer parameter digest");
        policy.record_digest = policy.computed_record_digest().expect("policy digest");
        policy.validate().expect("valid fixture policy");
        policy
    }

    fn application_term_digest(scope: &BootleLanternCredentialScopeV1) -> Vec<u8> {
        let term = scope.application_term().expect("scope expansion");
        let mut encoded = Vec::with_capacity(2 * BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1 * 64);
        for polynomial in term {
            for coefficient in polynomial.coefficients() {
                encoded.extend_from_slice(&coefficient.to_le_bytes());
            }
        }
        Sha256::digest(encoded).to_vec()
    }

    #[test]
    fn scope_uniform_rejection_boundaries_and_cap_are_exact() {
        assert_eq!(
            BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1,
            APPLICATION_MODULUS_V1 * (u16::MAX / APPLICATION_MODULUS_V1)
        );
        assert_eq!(
            bounded_scope_coefficient_v1(1, || 61_444),
            Some(APPLICATION_MODULUS_V1 - 1)
        );
        assert_eq!(bounded_scope_coefficient_v1(1, || 61_445), None);
        assert_eq!(bounded_scope_coefficient_v1(0, || 0), None);

        let mut proposals = 0_u32;
        assert_eq!(
            bounded_scope_coefficient_v1(BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1, || {
                proposals += 1;
                61_445
            },),
            None
        );
        assert_eq!(proposals, BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1);

        proposals = 0;
        assert_eq!(
            bounded_scope_coefficient_v1(BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1, || {
                proposals += 1;
                if proposals == BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1 {
                    61_444
                } else {
                    61_445
                }
            },),
            Some(APPLICATION_MODULUS_V1 - 1)
        );
        assert_eq!(proposals, BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1);
    }

    #[test]
    fn scope_digest_and_application_term_match_independent_kat() {
        let scope = kat_scope();
        assert_eq!(
            scope.digest().expect("scope digest"),
            hex::decode("31d0e4e8d38bdb1c70bfa20d832d694924023922026df83c92164e4b38d40709")
                .expect("hex")
                .as_slice()
        );
        assert_eq!(
            &scope.application_term().expect("scope term")[0].coefficients()[..16],
            &[
                3_936, 11_740, 11_923, 4_008, 8_590, 8_443, 9_761, 10_082, 1_401, 10_900, 11_799,
                7_699, 4_506, 2_834, 4_670, 4_468,
            ]
        );
        assert_eq!(
            application_term_digest(&scope),
            hex::decode("a95e7b11b0d368acfdc669610dce1b8d63530fac6dd9fb7c46215cfd0e108f50")
                .expect("hex")
        );
    }

    #[test]
    fn every_included_scope_field_changes_digest_and_application_term() {
        let scope = kat_scope();
        let expected_digest = scope.digest().expect("base digest");
        let expected_term = scope.application_term().expect("base term");
        let mut mutations = Vec::new();
        macro_rules! mutate {
            ($field:ident, $value:expr) => {{
                let mut candidate = scope.clone();
                candidate.$field = $value;
                mutations.push((stringify!($field), candidate));
            }};
        }
        mutate!(network_id, network_id(raw(22)));
        mutate!(genesis_hash, raw(22));
        mutate!(parameter_id, PrivacyParameterIdV1::new(raw(23)));
        mutate!(parameter_digest, PrivacyParameterDigestV1::new(raw(24)));
        mutate!(verifier_digest, PrivacyVerifierDigestV1::new(raw(25)));
        mutate!(
            statement_schema_digest,
            PrivacyStatementSchemaDigestV1::new(raw(26))
        );
        mutate!(
            engine_manifest_digest,
            PrivacyEngineManifestDigestV1::new(raw(27))
        );
        mutate!(issuer_id, PrivacyIssuerIdV1::new(raw(28)));
        mutate!(policy_id, PrivacyPolicyIdV1::new(raw(29)));
        mutate!(policy_epoch, 30);
        mutate!(
            policy_record_digest,
            PrivacyBootleLanternIssuerPolicyDigestV1::new(raw(31))
        );
        mutate!(issuer_parameter_id, PrivacyParameterIdV1::new(raw(32)));
        mutate!(
            issuer_parameter_digest,
            PrivacyParameterDigestV1::new(raw(33))
        );

        for (field, mutation) in mutations {
            assert_ne!(
                mutation.digest().expect("mutated digest"),
                expected_digest,
                "{field} was not digest-bound"
            );
            assert_ne!(
                mutation.application_term().expect("mutated term"),
                expected_term,
                "{field} was not algebraically bound"
            );
        }
    }

    #[test]
    fn action_index_and_transaction_intent_are_presentation_only() {
        let policy = active_policy();
        let base_context = context();
        let base =
            BootleLanternCredentialScopeV1::new(&base_context, raw(20), &policy).expect("scope");
        let mut changed_context = base_context;
        changed_context.action_index = 4_000;
        changed_context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new(raw(21));
        let changed =
            BootleLanternCredentialScopeV1::new(&changed_context, raw(20), &policy).expect("scope");

        assert_eq!(changed, base);
        assert_eq!(changed.digest(), base.digest());
        assert_eq!(changed.application_term(), base.application_term());
    }

    #[test]
    fn revoked_policy_and_zero_reusable_bindings_fail_closed() {
        let context = context();
        let policy = active_policy();
        assert!(matches!(
            BootleLanternCredentialScopeV1::new(&context, [0; 32], &policy),
            Err(CredentialScopeErrorV1::ZeroBinding("genesis_hash"))
        ));

        let mut revoked = policy.clone();
        revoked.lifecycle = BootleLanternIssuerPolicyLifecycleV1::Revoked;
        revoked.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        revoked.record_digest = revoked.computed_record_digest().expect("revoked digest");
        assert!(matches!(
            BootleLanternCredentialScopeV1::new(&context, raw(20), &revoked),
            Err(CredentialScopeErrorV1::InvalidPolicy)
        ));

        let mut zero_issuer = policy.clone();
        zero_issuer.issuer_id = PrivacyIssuerIdV1::new([0; 32]);
        assert!(matches!(
            BootleLanternCredentialScopeV1::new(&context, raw(20), &zero_issuer),
            Err(CredentialScopeErrorV1::InvalidPolicy)
        ));

        let mut zero_contexts = Vec::new();
        macro_rules! zero_context {
            ($field:ident, $value:expr) => {{
                let mut candidate = context.clone();
                candidate.$field = $value;
                zero_contexts.push(candidate);
            }};
        }
        zero_context!(parameter_id, PrivacyParameterIdV1::new([0; 32]));
        zero_context!(parameter_digest, PrivacyParameterDigestV1::new([0; 32]));
        zero_context!(verifier_digest, PrivacyVerifierDigestV1::new([0; 32]));
        zero_context!(
            statement_schema_digest,
            PrivacyStatementSchemaDigestV1::new([0; 32])
        );
        zero_context!(
            engine_manifest_digest,
            PrivacyEngineManifestDigestV1::new([0; 32])
        );
        for zero_context in zero_contexts {
            assert!(matches!(
                BootleLanternCredentialScopeV1::new(&zero_context, raw(20), &policy),
                Err(CredentialScopeErrorV1::ZeroBinding(_))
            ));
        }
    }
}
