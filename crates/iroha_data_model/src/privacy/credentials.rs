/// Closed Anonymous PGC anonymity-set sizes in the first release.
pub const ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1: [u32; 3] = [16, 32, 64];
/// Maximum Anonymous PGC anonymity-set size in the first release.
pub const ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1: u32 = 64;
/// Maximum Anonymous PGC intended recipients in the first release.
pub const ANONYMOUS_PGC_MAX_RECIPIENTS_V1: u32 = 8;
/// Hard maximum `VeRange` aggregation count in the first release.
pub const VERANGE_HARD_MAX_AGGREGATION_COUNT_V1: u32 = 64;
/// Effective `VeRange` aggregation ceiling under the Taira global commitment cap.
pub const VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1: u32 =
    if VERANGE_HARD_MAX_AGGREGATION_COUNT_V1 < TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1 {
        VERANGE_HARD_MAX_AGGREGATION_COUNT_V1
    } else {
        TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
    };
/// Maximum ordered anchors in one first-release ZK-AMS batch settlement.
pub const ZK_AMS_MAX_BATCH_SIZE_V1: u32 = 8;
/// Closed admitted seed-key ring sizes in the first release.
pub const ZK_AMS_RING_SIZES_V1: [u32; 3] = [16, 32, 64];
/// Maximum admitted seed-key ring size in the first release.
pub const ZK_AMS_MAX_RING_SIZE_V1: u32 = 64;
/// Exact polynomial count in one first-release Jindo batched univariate opening.
pub const IROHA_JINDO_MAX_POLYNOMIALS_V1: u32 = 4;
/// Exact canonical byte width of one Jindo coefficient-field element.
pub const IROHA_JINDO_FIELD_ELEMENT_BYTES_V1: usize = 32;
/// Canonical little-endian modulus of the first-release Jindo coefficient field.
///
/// The field is `F_p` for `p = 3611623616^8 + 1`. Keeping the wire-order modulus
/// beside the public field-element width gives data-model validation and the
/// native arithmetic engine one authoritative boundary constant.
pub const IROHA_JINDO_FIELD_MODULUS_LE_V1: [u8; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa1, 0xf9, 0x0e, 0x57, 0x64, 0x77, 0xbe, 0x54, 0xe8, 0x17,
    0xec, 0xae, 0x55, 0x03, 0x13, 0x70, 0xde, 0xc1, 0x7c, 0x27, 0x71, 0xb8, 0x69, 0x09, 0x00, 0x40,
];
/// Exact fixed-profile Jindo outer-commitment rank.
pub const IROHA_JINDO_OUTER_COMMITMENT_RANK_V1: usize = 3;
/// Exact fixed-profile Jindo application-ring degree.
pub const IROHA_JINDO_RING_DEGREE_V1: usize = 1024;
/// Exact signed coefficient width in the public rounded commitment wire.
pub const IROHA_JINDO_COMMITMENT_COEFFICIENT_BYTES_V1: usize = 4;
/// Exact canonical byte width of one fixed-profile Jindo lattice commitment.
pub const IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1: usize = IROHA_JINDO_OUTER_COMMITMENT_RANK_V1
    * IROHA_JINDO_RING_DEGREE_V1
    * IROHA_JINDO_COMMITMENT_COEFFICIENT_BYTES_V1;
/// Minimum canonical rounded outer-commitment coefficient.
///
/// This is the arithmetic-floor quotient of the smallest balanced residue
/// modulo the fixed 71-bit outer modulus by `2^48`.
pub const IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1: i32 = -4_194_303;
/// Maximum canonical rounded outer-commitment coefficient.
///
/// The one-value asymmetry relative to the minimum is required by the odd
/// outer modulus and arithmetic-floor rounding; accepting a wider symmetric
/// interval would admit encodings the commitment algorithm cannot produce.
pub const IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1: i32 = 4_194_302;
/// Exact direct 64-bit attribute count in the Bootle/Lantern credential profile.
pub const BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1: usize = 8;
/// Exact byte width of one direct Bootle/Lantern attribute.
pub const BOOTLE_LANTERN_ATTRIBUTE_BYTES_V1: usize = 8;
/// Degree of every polynomial in the Bootle/Lantern application ring.
pub const BOOTLE_LANTERN_RING_DEGREE_V1: usize = 64;
/// Application-ring modulus used by the fixed Bootle/Lantern profile.
pub const BOOTLE_LANTERN_APPLICATION_MODULUS_V1: u16 = 12_289;
/// Rows and columns in the issuer's canonical public matrix `B`.
pub const BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1: usize = 8;
/// Maximum selectively disclosed attributes in one Bootle/Lantern statement.
pub const BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1: u32 = 8;
/// Maximum governed allowed public values for one required attribute.
pub const BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1: u32 = 32;
/// Maximum authoritative Bootle/Lantern issuer-policy lineages in committed state.
pub const BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1: usize = 4_096;
/// Maximum immutable Vega issuer revisions retained across all lineages.
pub const VEGA_MAX_ISSUER_RECORDS_V1: usize = 4_096;
/// Maximum immutable revisions retained for one Vega issuer lineage.
pub const VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1: usize = 64;
/// Canonical origin epoch for a Vega issuer-key/policy lineage.
pub const VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1: u64 = 1;
/// Maximum admitted X.509 chain depth, including the leaf certificate.
pub const ZK_X509_MAX_CHAIN_DEPTH_V1: u8 = 3;
/// Minimum admitted X.509 chain depth, including leaf and terminal root.
pub const ZK_X509_MIN_CHAIN_DEPTH_V1: u8 = 2;
/// Maximum accepted lag from signed CRL `thisUpdate` to the presentation-window end.
pub const ZK_X509_MAX_CRL_AGE_SECONDS_V1: u64 = 300;
/// Maximum public presentation window covered by one X.509 proof.
///
/// The proof establishes certificate and signed-CRL validity for the complete
/// window; consensus then checks the unpredictable inclusion timestamp lies
/// inside it. Keeping this equal to the CRL-age ceiling preserves the strict
/// five-minute freshness profile.
pub const ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1: u64 = ZK_X509_MAX_CRL_AGE_SECONDS_V1;
/// Maximum DER bytes for one X.509 certificate in the canonical proof topology.
///
/// This is deliberately identical to the native witness codec, RFC 5280 AIR,
/// and fixed-capacity SHA-256 schedule. The first release has no larger
/// API-only tier that could be admitted but not proved.
pub const ZK_X509_MAX_CERTIFICATE_BYTES_V1: u32 = 4 * 1024;
/// Maximum combined DER bytes for an admitted X.509 chain.
pub const ZK_X509_MAX_CHAIN_BYTES_V1: u32 = ZK_X509_MAX_CERTIFICATE_BYTES_V1 * 3;
/// Closed number of selectively disclosable X.509 subject attributes.
pub const ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1: usize = 4;
/// Closed number of extended-key-usage purposes in the first-release profile.
pub const ZK_X509_MAX_EXTENDED_KEY_USAGES_V1: usize = 3;
/// Maximum immutable trust-anchor revisions retained across all lineages.
pub const ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1: usize = 4_096;
/// Maximum immutable certificate-policy revisions retained across all lineages.
pub const ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1: usize = 4_096;
/// Maximum current issuer-scoped signed-CRL lineages in world state.
///
/// Unlike rare trust-anchor and policy governance, CRLs rotate frequently.
/// Consensus therefore stores one self-chained current record per policy
/// lineage instead of retaining an eventually terminal fixed revision count.
pub const ZK_X509_MAX_CRL_LINEAGES_V1: usize = 4_096;
/// Maximum immutable revisions retained for one trust-anchor or policy lineage.
pub const ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1: usize = 64;
/// Canonical origin epoch for an X.509 trust-anchor or certificate-policy lineage.
pub const ZK_X509_INITIAL_RECORD_EPOCH_V1: u64 = 1;
/// Maximum Orchard actions in one first-release bundle.
pub const ORCHARD_MAX_ACTIONS_V1: u32 = 2;
/// Exact Orchard V3 encrypted-note ciphertext width.
pub const ORCHARD_ENCRYPTED_NOTE_BYTES_V1: usize = 580;
/// Exact Orchard V3 outgoing ciphertext width.
pub const ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1: usize = 80;
/// Largest public Orchard value balance representable by the pinned native API.
pub const ORCHARD_MAX_VALUE_BALANCE_V1: u128 = i64::MAX as u128;
/// Maximum FCMP++ consumed outputs in one first-release transfer.
pub const FCMP_MAX_INPUTS_V1: u32 = 2;
/// Maximum FCMP++ new outputs in one first-release transfer.
pub const FCMP_MAX_OUTPUTS_V1: u32 = 4;
/// Maximum native IVM private-note inputs in one first-release action.
pub const IVM_PRIVATE_NOTE_MAX_INPUTS_V1: u32 = 2;
/// Maximum native IVM private-note outputs in one first-release action.
pub const IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1: u32 = 2;
/// Maximum PQ-MASP inputs in one first-release action.
pub const PQ_MASP_MAX_INPUTS_V1: u32 = 2;
/// Maximum PQ-MASP outputs in one first-release action.
pub const PQ_MASP_MAX_OUTPUTS_V1: u32 = 2;
/// Maximum genesis commitments in one typed proof-managed pool bootstrap.
pub const PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1: usize = 4_096;
/// Maximum UTF-8 byte length admitted for a privacy transcript chain id.
pub const PRIVACY_MAX_CHAIN_ID_BYTES_V1: u32 = crate::id::MAX_CHAIN_ID_BYTES as u32;

/// Explicit chain and governed-artifact binding shared by every statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyStatementContextV1 {
    /// Exact chain identifier.
    pub chain_id: ChainId,
    /// Zero-based privacy action index within the transaction.
    pub action_index: u32,
    /// Digest of the canonical transaction projection with derived privacy
    /// digests zeroed and the typed proof payload empty.
    pub transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    /// Exact governed parameter-set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the governed parameter set.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier artifact.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of this protocol's public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the pinned experimental native engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
}

impl PrivacyStatementContextV1 {
    /// Validate transcript context and non-zero governed artifact bindings.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyStatementValidationError`] for an invalid chain id,
    /// action index, or fixed artifact binding.
    pub fn validate(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyStatementValidationError> {
        let chain_id_bytes = u32::try_from(self.chain_id.as_str().len())
            .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
        if chain_id_bytes == 0 || chain_id_bytes > PRIVACY_MAX_CHAIN_ID_BYTES_V1 {
            return Err(PrivacyStatementValidationError::InvalidChainIdLength {
                bytes: chain_id_bytes,
                max: PRIVACY_MAX_CHAIN_ID_BYTES_V1,
            });
        }
        if self.action_index >= limits.max_actions_per_transaction {
            return Err(PrivacyStatementValidationError::ActionIndexOutOfBounds {
                index: self.action_index,
                max_actions: limits.max_actions_per_transaction,
            });
        }
        if self.transaction_intent_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroTransactionIntentDigest);
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEngineManifestDigest);
        }
        Ok(())
    }
}

/// Exact chain, genesis, action, and governed-artifact binding shared by native engines.
///
/// The binding owns every consensus-selected byte that a native proof
/// transcript must commit. It is constructed from a validated
/// [`PrivacyStatementContextV1`] plus the trusted chain genesis hash; there is
/// no optional field, default, alias, or legacy wire shape.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.native-consensus-binding.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyNativeConsensusBindingV1 {
    /// Exact chain identifier.
    pub chain_id: ChainId,
    /// Trusted committed genesis-block hash for this chain.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Zero-based privacy action index within the transaction.
    pub action_index: u32,
    /// Digest of the canonical transaction projection selected by the action.
    pub transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    /// Exact governed parameter-set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the governed parameter set.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier artifact.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of this protocol's public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the pinned native engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
}

impl PrivacyNativeConsensusBindingV1 {
    /// Construct the sole canonical native consensus binding for a statement context.
    ///
    /// # Errors
    ///
    /// Rejects invalid consensus limits, an invalid statement context, or the
    /// reserved all-zero genesis hash.
    pub fn new(
        context: &PrivacyStatementContextV1,
        genesis_hash: [u8; 32],
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<Self, PrivacyNativeConsensusBindingValidationErrorV1> {
        let binding = Self {
            chain_id: context.chain_id.clone(),
            genesis_hash,
            action_index: context.action_index,
            transaction_intent_digest: context.transaction_intent_digest,
            parameter_id: context.parameter_id,
            parameter_digest: context.parameter_digest,
            verifier_digest: context.verifier_digest,
            statement_schema_digest: context.statement_schema_digest,
            engine_manifest_digest: context.engine_manifest_digest,
        };
        binding.validate(limits)?;
        Ok(binding)
    }

    /// Validate every intrinsic field under the supplied consensus limits.
    ///
    /// # Errors
    ///
    /// Rejects invalid consensus limits, an invalid context-shaped field, or
    /// the reserved all-zero genesis hash.
    pub fn validate(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyNativeConsensusBindingValidationErrorV1> {
        limits
            .validate()
            .map_err(PrivacyNativeConsensusBindingValidationErrorV1::InvalidLimits)?;
        if self.genesis_hash.iter().all(|byte| *byte == 0) {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::ZeroGenesisHash);
        }
        self.as_statement_context()
            .validate(limits)
            .map_err(PrivacyNativeConsensusBindingValidationErrorV1::InvalidContext)
    }

    /// Validate this binding and require exact equality with a statement context.
    ///
    /// # Errors
    ///
    /// Rejects an intrinsically invalid binding or context and reports the
    /// first consensus-binding axis whose canonical value differs.
    pub fn validate_against_context(
        &self,
        context: &PrivacyStatementContextV1,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyNativeConsensusBindingValidationErrorV1> {
        limits
            .validate()
            .map_err(PrivacyNativeConsensusBindingValidationErrorV1::InvalidLimits)?;
        if self.genesis_hash.iter().all(|byte| *byte == 0) {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::ZeroGenesisHash);
        }
        if self.chain_id != context.chain_id {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::ChainIdMismatch);
        }
        if self.action_index != context.action_index {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::ActionIndexMismatch);
        }
        if self.transaction_intent_digest != context.transaction_intent_digest {
            return Err(
                PrivacyNativeConsensusBindingValidationErrorV1::TransactionIntentDigestMismatch,
            );
        }
        if self.parameter_id != context.parameter_id {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::ParameterIdMismatch);
        }
        if self.parameter_digest != context.parameter_digest {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::ParameterDigestMismatch);
        }
        if self.verifier_digest != context.verifier_digest {
            return Err(PrivacyNativeConsensusBindingValidationErrorV1::VerifierDigestMismatch);
        }
        if self.statement_schema_digest != context.statement_schema_digest {
            return Err(
                PrivacyNativeConsensusBindingValidationErrorV1::StatementSchemaDigestMismatch,
            );
        }
        if self.engine_manifest_digest != context.engine_manifest_digest {
            return Err(
                PrivacyNativeConsensusBindingValidationErrorV1::EngineManifestDigestMismatch,
            );
        }
        self.validate(limits)?;
        context
            .validate(limits)
            .map_err(PrivacyNativeConsensusBindingValidationErrorV1::InvalidContext)
    }

    /// Hash the exact canonical binding in its dedicated transcript domain.
    ///
    /// Callers must validate the binding before treating the digest as a
    /// consensus input. This method remains independently useful for prover
    /// construction, where the validating constructor already established the
    /// invariant.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyNativeConsensusBindingDigestV1, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let encoded_len = u64::try_from(encoded.len()).map_err(|_| {
            norito::Error::Io(std::io::Error::other(
                "native privacy consensus-binding length exceeds u64",
            ))
        })?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_NATIVE_CONSENSUS_BINDING_DIGEST_DOMAIN_V1);
        hasher.update(&encoded_len.to_le_bytes());
        hasher.update(&encoded);
        Ok(PrivacyNativeConsensusBindingDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }

    fn as_statement_context(&self) -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            chain_id: self.chain_id.clone(),
            action_index: self.action_index,
            transaction_intent_digest: self.transaction_intent_digest,
            parameter_id: self.parameter_id,
            parameter_digest: self.parameter_digest,
            verifier_digest: self.verifier_digest,
            statement_schema_digest: self.statement_schema_digest,
            engine_manifest_digest: self.engine_manifest_digest,
        }
    }
}

/// Validation failure for [`PrivacyNativeConsensusBindingV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyNativeConsensusBindingValidationErrorV1 {
    /// Supplied consensus limits are invalid.
    #[error("native privacy consensus-binding limits are invalid: {0}")]
    InvalidLimits(PrivacyConsensusLimitsValidationError),
    /// A field shared with the statement context is intrinsically invalid.
    #[error("native privacy consensus-binding context is invalid: {0}")]
    InvalidContext(PrivacyStatementValidationError),
    /// The trusted committed genesis hash is the reserved all-zero value.
    #[error("native privacy consensus-binding genesis hash must be non-zero")]
    ZeroGenesisHash,
    /// Chain identifiers differ.
    #[error("native privacy consensus-binding chain id differs from statement context")]
    ChainIdMismatch,
    /// Privacy action indexes differ.
    #[error("native privacy consensus-binding action index differs from statement context")]
    ActionIndexMismatch,
    /// Transaction-intent digests differ.
    #[error(
        "native privacy consensus-binding transaction-intent digest differs from statement context"
    )]
    TransactionIntentDigestMismatch,
    /// Governed parameter-set identifiers differ.
    #[error("native privacy consensus-binding parameter id differs from statement context")]
    ParameterIdMismatch,
    /// Governed parameter digests differ.
    #[error("native privacy consensus-binding parameter digest differs from statement context")]
    ParameterDigestMismatch,
    /// Verifier-artifact digests differ.
    #[error("native privacy consensus-binding verifier digest differs from statement context")]
    VerifierDigestMismatch,
    /// Statement-schema digests differ.
    #[error("native privacy consensus-binding schema digest differs from statement context")]
    StatementSchemaDigestMismatch,
    /// Engine-manifest digests differ.
    #[error(
        "native privacy consensus-binding engine-manifest digest differs from statement context"
    )]
    EngineManifestDigestMismatch,
}

/// Typed encrypted output emitted by a private transfer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyEncryptedOutputV1 {
    /// Cryptographic recipient identity.
    pub recipient: PrivacyRecipientIdV1,
    /// Protocol-defined identifier for the ephemeral encryption material.
    ///
    /// Diffie-Hellman profiles use a public key here. The fixed PQ-MASP
    /// profile uses the domain-separated digest of its ML-KEM-768
    /// encapsulation ciphertext; the full 1,088-byte encapsulation remains in
    /// `ciphertext`.
    pub ephemeral_public_key: PrivacyEncryptionKeyV1,
    /// Commitment to the plaintext output.
    pub commitment: PrivacyCommitmentV1,
    /// Canonical authenticated ciphertext bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
}

/// Closed lifecycle of one authoritative ZK-ACE authorization policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "state", content = "value", deny_unknown_fields)]
pub enum PrivacyZkAcePolicyLifecycleV1 {
    /// The policy can authorize a matching proof action.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// The policy was irreversibly revoked.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(schema_name = "iroha.privacy.zk-ace.policy-digest-material.v1")]
struct PrivacyZkAcePolicyDigestMaterialV1 {
    policy_id: PrivacyPolicyIdV1,
    identity_commitment: PrivacyCommitmentV1,
    policy_digest: PrivacyPolicyDigestV1,
    authorization_epoch: u64,
    asset_definition_id: AssetDefinitionId,
    source_allowlist: Vec<AccountId>,
    lifecycle: PrivacyZkAcePolicyLifecycleV1,
}

/// Complete authoritative policy selected by a ZK-ACE authorization statement.
///
/// `record_digest` commits every preceding field. The allowlist is stored in
/// strict account-id order so snapshots, governance instructions, and proof
/// preflight all have exactly one canonical representation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAcePolicyRecordV1 {
    /// Stable lookup key for this policy lineage.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact identity commitment authorized by the current policy epoch.
    pub identity_commitment: PrivacyCommitmentV1,
    /// Digest of the governed authorization policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Strictly increasing governance epoch.
    pub authorization_epoch: u64,
    /// Exact transparent asset definition authorized by this policy.
    pub asset_definition_id: AssetDefinitionId,
    /// Strictly sorted, unique, non-empty set of authorized source accounts.
    pub source_allowlist: Vec<AccountId>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkAcePolicyLifecycleV1,
    /// Digest of every authoritative field above.
    pub record_digest: PrivacyZkAcePolicyRecordDigestV1,
}

impl PrivacyZkAcePolicyRecordV1 {
    /// Construct one canonical self-digested policy record.
    ///
    /// # Errors
    ///
    /// Rejects a zero identifier, commitment, digest, or epoch; an empty,
    /// oversized, unsorted, or duplicate allowlist; or a digest encoding
    /// failure.
    pub fn new(
        policy_id: PrivacyPolicyIdV1,
        identity_commitment: PrivacyCommitmentV1,
        policy_digest: PrivacyPolicyDigestV1,
        authorization_epoch: u64,
        asset_definition_id: AssetDefinitionId,
        source_allowlist: Vec<AccountId>,
        lifecycle: PrivacyZkAcePolicyLifecycleV1,
    ) -> Result<Self, PrivacyZkAcePolicyRecordValidationErrorV1> {
        let mut record = Self {
            policy_id,
            identity_commitment,
            policy_digest,
            authorization_epoch,
            asset_definition_id,
            source_allowlist,
            lifecycle,
            record_digest: PrivacyZkAcePolicyRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate an initial policy registration.
    ///
    /// # Errors
    ///
    /// Requires a valid active record at the canonical origin epoch.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkAcePolicyRecordValidationErrorV1> {
        self.validate()?;
        if self.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::InitialPolicyNotActive);
        }
        if self.authorization_epoch != PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1 {
            return Err(
                PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalInitialEpoch {
                    actual: self.authorization_epoch,
                },
            );
        }
        Ok(())
    }

    /// Validate this record, including its canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed authoritative field or self-digest mismatch.
    pub fn validate(&self) -> Result<(), PrivacyZkAcePolicyRecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the canonical digest of every authoritative record field.
    ///
    /// # Errors
    ///
    /// Returns an encoding error when the canonical digest material cannot be
    /// serialized.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkAcePolicyRecordDigestV1, PrivacyZkAcePolicyRecordValidationErrorV1> {
        let material = PrivacyZkAcePolicyDigestMaterialV1 {
            policy_id: self.policy_id,
            identity_commitment: self.identity_commitment,
            policy_digest: self.policy_digest,
            authorization_epoch: self.authorization_epoch,
            asset_definition_id: self.asset_definition_id.clone(),
            source_allowlist: self.source_allowlist.clone(),
            lifecycle: self.lifecycle,
        };
        let encoded = norito::encode_canonical(&material)
            .map_err(|_| PrivacyZkAcePolicyRecordValidationErrorV1::EncodingFailure)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(ZK_ACE_POLICY_RECORD_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyZkAcePolicyRecordDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkAcePolicyRecordValidationErrorV1> {
        if self.policy_id.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyId);
        }
        if self.identity_commitment.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroIdentityCommitment);
        }
        if self.policy_digest.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyDigest);
        }
        if self.authorization_epoch == 0 {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroAuthorizationEpoch);
        }
        if self.source_allowlist.is_empty() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::EmptySourceAllowlist);
        }
        if self.source_allowlist.len() > PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1 {
            return Err(
                PrivacyZkAcePolicyRecordValidationErrorV1::SourceAllowlistTooLarge {
                    actual: self.source_allowlist.len(),
                    max: PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1,
                },
            );
        }
        if self
            .source_allowlist
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalSourceAllowlist);
        }
        Ok(())
    }
}

/// Failure while validating one authoritative ZK-ACE policy record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkAcePolicyRecordValidationErrorV1 {
    /// The lookup identifier is all zero.
    #[error("ZK-ACE policy id must be non-zero")]
    ZeroPolicyId,
    /// The identity commitment is all zero.
    #[error("ZK-ACE identity commitment must be non-zero")]
    ZeroIdentityCommitment,
    /// The governed policy digest is all zero.
    #[error("ZK-ACE policy digest must be non-zero")]
    ZeroPolicyDigest,
    /// Epoch zero is not a valid governed policy state.
    #[error("ZK-ACE authorization epoch must be non-zero")]
    ZeroAuthorizationEpoch,
    /// Registration must begin at the canonical origin epoch.
    #[error(
        "initial ZK-ACE authorization epoch must be {PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1}, got {actual}"
    )]
    NonCanonicalInitialEpoch {
        /// Rejected epoch.
        actual: u64,
    },
    /// Registration cannot create an already-revoked policy.
    #[error("initial ZK-ACE policy must be active")]
    InitialPolicyNotActive,
    /// A policy must authorize at least one source account.
    #[error("ZK-ACE source allowlist must be non-empty")]
    EmptySourceAllowlist,
    /// The first-release fixed account bound was exceeded.
    #[error("ZK-ACE source allowlist has {actual} entries; maximum is {max}")]
    SourceAllowlistTooLarge {
        /// Rejected entry count.
        actual: usize,
        /// Fixed first-release maximum.
        max: usize,
    },
    /// The allowlist is not in strict unique account-id order.
    #[error("ZK-ACE source allowlist must be strictly sorted and unique")]
    NonCanonicalSourceAllowlist,
    /// Canonical encoding of the self-digest material failed.
    #[error("ZK-ACE policy record digest material could not be encoded")]
    EncodingFailure,
    /// A decoded record supplied an all-zero self-digest.
    #[error("ZK-ACE policy record self-digest must be non-zero")]
    ZeroRecordDigest,
    /// Recomputing the complete record produced a different digest.
    #[error("ZK-ACE policy record self-digest mismatch")]
    RecordDigestMismatch,
}

/// Failure while validating a canonical ZK-ACE governance transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkAcePolicyTransitionValidationErrorV1 {
    /// The persisted current record is malformed.
    #[error("current ZK-ACE policy record is invalid: {0}")]
    InvalidCurrent(PrivacyZkAcePolicyRecordValidationErrorV1),
    /// The proposed successor record is malformed.
    #[error("successor ZK-ACE policy record is invalid: {0}")]
    InvalidSuccessor(PrivacyZkAcePolicyRecordValidationErrorV1),
    /// A revoked policy cannot transition again.
    #[error("current ZK-ACE policy is not active")]
    CurrentNotActive,
    /// A rotation must retain the stable policy identifier.
    #[error("ZK-ACE transition changed policy id")]
    PolicyIdMismatch,
    /// An epoch cannot advance past `u64::MAX`.
    #[error("ZK-ACE authorization epoch overflow")]
    EpochOverflow,
    /// The successor did not advance exactly one canonical epoch.
    #[error("ZK-ACE successor epoch must be {expected}, got {actual}")]
    NonCanonicalSuccessorEpoch {
        /// Required successor epoch.
        expected: u64,
        /// Rejected successor epoch.
        actual: u64,
    },
    /// A rotation successor must remain active.
    #[error("ZK-ACE rotation successor must be active")]
    RotationSuccessorNotActive,
    /// A rotation must actually replace the identity commitment.
    #[error("ZK-ACE rotation requires a distinct identity commitment")]
    IdentityCommitmentUnchanged,
    /// A revocation successor must be revoked.
    #[error("ZK-ACE revocation successor must be revoked")]
    RevocationSuccessorNotRevoked,
    /// Revocation may change only lifecycle, epoch, and the resulting self-digest.
    #[error("ZK-ACE revocation changed immutable policy contents")]
    RevocationContentsChanged,
}

/// Validate an active-to-active canonical ZK-ACE rotation.
///
/// # Errors
///
/// Rejects malformed records, stale or skipped epochs, policy-id changes, and
/// no-op identity rotations.
pub fn validate_zk_ace_policy_rotation_v1(
    current: &PrivacyZkAcePolicyRecordV1,
    successor: &PrivacyZkAcePolicyRecordV1,
) -> Result<(), PrivacyZkAcePolicyTransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidSuccessor)?;
    if current.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch);
    }
    let expected = current
        .authorization_epoch
        .checked_add(1)
        .ok_or(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)?;
    if successor.authorization_epoch != expected {
        return Err(
            PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor.authorization_epoch,
            },
        );
    }
    if successor.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.identity_commitment == current.identity_commitment {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::IdentityCommitmentUnchanged);
    }
    Ok(())
}

/// Validate an irreversible canonical ZK-ACE revocation.
///
/// # Errors
///
/// Rejects malformed records, stale or skipped epochs, and any mutation other
/// than lifecycle, epoch, and the corresponding self-digest.
pub fn validate_zk_ace_policy_revocation_v1(
    current: &PrivacyZkAcePolicyRecordV1,
    successor: &PrivacyZkAcePolicyRecordV1,
) -> Result<(), PrivacyZkAcePolicyTransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidSuccessor)?;
    if current.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch);
    }
    let expected = current
        .authorization_epoch
        .checked_add(1)
        .ok_or(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)?;
    if successor.authorization_epoch != expected {
        return Err(
            PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor.authorization_epoch,
            },
        );
    }
    if successor.lifecycle != PrivacyZkAcePolicyLifecycleV1::Revoked {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.identity_commitment != current.identity_commitment
        || successor.policy_digest != current.policy_digest
        || successor.asset_definition_id != current.asset_definition_id
        || successor.source_allowlist != current.source_allowlist
    {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// ZK-ACE authorization statement for a public asset transfer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.zk-ace.authorization-statement.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ZkAcePqAuthorizationStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Identity commitment authorized by the policy.
    pub identity_commitment: PrivacyCommitmentV1,
    /// Exact authorization policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Digest of the authorization policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Public source account.
    pub source: AccountId,
    /// Public destination account.
    pub destination: AccountId,
    /// Public transferred asset definition.
    pub asset_definition_id: AssetDefinitionId,
    /// Atomic transfer amount.
    pub amount: u128,
    /// Ledger epoch used by authorization policy checks.
    pub authorization_epoch: u64,
    /// Per-action replay nullifier.
    pub replay_nullifier: PrivacyNullifierV1,
}

/// Anonymous PGC k-out-of-n private payment statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AnonymousPgcKOutOfNStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset transferred by the confidential payment.
    pub asset_definition_id: AssetDefinitionId,
    /// Anonymous-account pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Current encrypted PGC account-state root.
    pub account_state_root: PrivacyRootV1,
    /// Epoch at which `account_state_root` was canonical.
    pub account_state_root_epoch: u64,
    /// Resulting encrypted PGC account-state root.
    pub next_account_state_root: PrivacyRootV1,
    /// Successor epoch committed with `next_account_state_root`.
    pub next_account_state_root_epoch: u64,
    /// Ordered anonymity-set public keys `(pk_0, …, pk_{n-1})`.
    pub anonymity_set_public_keys: Vec<PrivacyP256PointV1>,
    /// Ordered transfer ciphertexts `(C_0, …, C_{n-1})`, matching the keys.
    pub transfer_ciphertexts: Vec<PrivacyP256CiphertextV1>,
    /// Number `k` of intended positive-value recipients.
    pub recipient_count: u32,
}

/// Bit width admitted by the Iroha `VeRange` Type-1 profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "bits", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVeRangeBitLengthV1 {
    /// 32-bit unsigned range.
    Bits32,
    /// 64-bit unsigned range.
    Bits64,
}

impl PrivacyVeRangeBitLengthV1 {
    /// Return the exact numeric bit width.
    #[must_use]
    pub const fn bits(self) -> u16 {
        match self {
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }
}

/// Iroha Type-1 P-256/SHA-256 unsigned range-proof statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct VeRangeTransparentRangeStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset whose atomic values are committed.
    pub asset_definition_id: AssetDefinitionId,
    /// Policy selecting the commitment domain and admitted bit width.
    pub policy_id: PrivacyPolicyIdV1,
    /// Value commitments proved in this aggregate.
    pub value_commitments: Vec<PrivacyP256PointV1>,
    /// Closed first-release range `[0, 2^N)` proved for each committed value.
    pub bit_length: PrivacyVeRangeBitLengthV1,
    /// Number of aggregated value commitments.
    pub aggregation_count: u32,
}

/// Exact wire version of [`PrivacyZkAmsPersonhoodCredentialV1`].
pub const ZK_AMS_PHC_VERSION_V1: u8 = 1;
/// Exact byte width of the closed canonical PHC payload.
pub const ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1: usize = 161;
/// The only initial epoch admitted by the ZK-AMS registry bootstrap.
pub const ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1: u64 = 1;
/// Exact issuer-policy record preimage width.
pub const ZK_AMS_ISSUER_POLICY_RECORD_PAYLOAD_BYTES_V1: usize = 129;
/// Exact registry-snapshot record preimage width.
pub const ZK_AMS_REGISTRY_RECORD_PAYLOAD_BYTES_V1: usize = 200;
/// Exact registry-bootstrap provenance preimage width.
pub const ZK_AMS_REGISTRY_BOOTSTRAP_PAYLOAD_BYTES_V1: usize = 201;

/// Canonical governed origin for one ZK-AMS admitted-identity registry.
///
/// This is the only first-release instruction payload that may initialize an
/// `AccountRegistry` root. It fixes the issuer key, admission policy, registry
/// namespace, and exact nonzero origin root in one atomic governance action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsRegistryBootstrapV1 {
    /// Credential issuer authorized to sign canonical PHCs.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Admitted-identity registry initialized by this record.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Exact governed admission policy.
    pub policy_id: PrivacyPolicyIdV1,
    /// Canonical compressed SEC1 P-256 issuer verification key.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Digest of the complete governed admission policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Nonzero origin of the proof-managed admitted-identity registry.
    pub initial_registry_root: PrivacyRootV1,
    /// Closed origin epoch; exactly
    /// [`ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1`].
    pub initial_registry_epoch: u64,
}

impl PrivacyZkAmsRegistryBootstrapV1 {
    /// Derive the sole protocol-scoped namespace governed by this bootstrap.
    #[must_use]
    pub const fn namespace(self) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyNamespaceScopeV1::IssuerRegistryPolicy(PrivacyIssuerRegistryPolicyNamespaceV1 {
                issuer_id: self.issuer_id,
                registry_id: self.registry_id,
                policy_id: self.policy_id,
            }),
        )
    }

    /// Validate every closed nonzero field and the exact origin epoch.
    ///
    /// Core additionally parses `issuer_public_key` as a canonical,
    /// non-identity P-256 point before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyZkAmsRegistryBootstrapValidationError`] when a
    /// required identifier, digest, key, or root is zero, the origin epoch is
    /// noncanonical, or the derived namespace is invalid.
    pub fn validate(&self) -> Result<(), PrivacyZkAmsRegistryBootstrapValidationError> {
        if self.issuer_id.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroIssuerId);
        }
        if self.registry_id.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroRegistryId);
        }
        if self.policy_id.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroPolicyId);
        }
        if self.issuer_public_key.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroIssuerPublicKey);
        }
        if self.policy_digest.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroPolicyDigest);
        }
        if self.initial_registry_root.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroInitialRoot);
        }
        if self.initial_registry_epoch != ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1 {
            return Err(
                PrivacyZkAmsRegistryBootstrapValidationError::NonCanonicalInitialEpoch {
                    epoch: self.initial_registry_epoch,
                },
            );
        }
        self.namespace()
            .validate()
            .map_err(|_| PrivacyZkAmsRegistryBootstrapValidationError::InvalidNamespace)
    }

    /// Derive the authoritative issuer-key/policy record digest.
    #[must_use]
    pub fn issuer_policy_record_digest(self) -> PrivacyZkAmsIssuerPolicyRecordDigestV1 {
        zk_ams_issuer_policy_record_digest_v1(
            self.issuer_id,
            self.policy_id,
            self.issuer_public_key,
            self.policy_digest,
        )
    }

    /// Derive the authoritative origin registry-snapshot record digest.
    #[must_use]
    pub fn registry_record_digest(self) -> PrivacyZkAmsRegistryRecordDigestV1 {
        zk_ams_registry_record_digest_v1(
            self.issuer_id,
            self.registry_id,
            self.policy_id,
            self.issuer_policy_record_digest(),
            self.policy_digest,
            self.initial_registry_root,
            self.initial_registry_epoch,
        )
    }

    /// Hash the exact fixed bootstrap fields in their provenance domain.
    #[must_use]
    pub fn digest(self) -> PrivacyZkAmsRegistryBootstrapDigestV1 {
        let mut payload = [0_u8; ZK_AMS_REGISTRY_BOOTSTRAP_PAYLOAD_BYTES_V1];
        payload[0..32].copy_from_slice(self.issuer_id.as_bytes());
        payload[32..64].copy_from_slice(self.registry_id.as_bytes());
        payload[64..96].copy_from_slice(self.policy_id.as_bytes());
        payload[96..129].copy_from_slice(self.issuer_public_key.as_bytes());
        payload[129..161].copy_from_slice(self.policy_digest.as_bytes());
        payload[161..193].copy_from_slice(self.initial_registry_root.as_bytes());
        payload[193..201].copy_from_slice(&self.initial_registry_epoch.to_be_bytes());
        let mut hasher = Sha256::new();
        hasher.update(ZK_AMS_REGISTRY_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            u64::try_from(payload.len())
                .expect("fixed ZK-AMS bootstrap payload length fits u64")
                .to_le_bytes(),
        );
        hasher.update(payload);
        PrivacyZkAmsRegistryBootstrapDigestV1::new(hasher.finalize().into())
    }
}

/// Derive one exact authoritative ZK-AMS issuer-key/policy record digest.
#[must_use]
pub fn zk_ams_issuer_policy_record_digest_v1(
    issuer_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    issuer_public_key: PrivacyP256PointV1,
    policy_digest: PrivacyPolicyDigestV1,
) -> PrivacyZkAmsIssuerPolicyRecordDigestV1 {
    let mut payload = [0_u8; ZK_AMS_ISSUER_POLICY_RECORD_PAYLOAD_BYTES_V1];
    payload[0..32].copy_from_slice(issuer_id.as_bytes());
    payload[32..64].copy_from_slice(policy_id.as_bytes());
    payload[64..97].copy_from_slice(issuer_public_key.as_bytes());
    payload[97..129].copy_from_slice(policy_digest.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(ZK_AMS_ISSUER_POLICY_RECORD_DIGEST_DOMAIN_V1);
    hasher.update(
        u64::try_from(payload.len())
            .expect("fixed ZK-AMS issuer-policy payload length fits u64")
            .to_le_bytes(),
    );
    hasher.update(payload);
    PrivacyZkAmsIssuerPolicyRecordDigestV1::new(hasher.finalize().into())
}

/// Derive one exact authoritative ZK-AMS registry-snapshot record digest.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn zk_ams_registry_record_digest_v1(
    issuer_id: PrivacyIssuerIdV1,
    registry_id: PrivacyZkAmsRegistryIdV1,
    policy_id: PrivacyPolicyIdV1,
    issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    policy_digest: PrivacyPolicyDigestV1,
    registry_root: PrivacyRootV1,
    registry_epoch: u64,
) -> PrivacyZkAmsRegistryRecordDigestV1 {
    let mut payload = [0_u8; ZK_AMS_REGISTRY_RECORD_PAYLOAD_BYTES_V1];
    payload[0..32].copy_from_slice(issuer_id.as_bytes());
    payload[32..64].copy_from_slice(registry_id.as_bytes());
    payload[64..96].copy_from_slice(policy_id.as_bytes());
    payload[96..128].copy_from_slice(issuer_policy_record_digest.as_bytes());
    payload[128..160].copy_from_slice(policy_digest.as_bytes());
    payload[160..192].copy_from_slice(registry_root.as_bytes());
    payload[192..200].copy_from_slice(&registry_epoch.to_be_bytes());
    let mut hasher = Sha256::new();
    hasher.update(ZK_AMS_REGISTRY_RECORD_DIGEST_DOMAIN_V1);
    hasher.update(
        u64::try_from(payload.len())
            .expect("fixed ZK-AMS registry-record payload length fits u64")
            .to_le_bytes(),
    );
    hasher.update(payload);
    PrivacyZkAmsRegistryRecordDigestV1::new(hasher.finalize().into())
}

/// Structural failure for [`PrivacyZkAmsRegistryBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkAmsRegistryBootstrapValidationError {
    /// Issuer id is the zero sentinel.
    #[error("ZK-AMS registry bootstrap issuer id must be nonzero")]
    ZeroIssuerId,
    /// Registry id is the zero sentinel.
    #[error("ZK-AMS registry bootstrap registry id must be nonzero")]
    ZeroRegistryId,
    /// Policy id is the zero sentinel.
    #[error("ZK-AMS registry bootstrap policy id must be nonzero")]
    ZeroPolicyId,
    /// Issuer public key is the all-zero sentinel.
    #[error("ZK-AMS registry bootstrap issuer public key must be nonzero")]
    ZeroIssuerPublicKey,
    /// Policy digest is the zero sentinel.
    #[error("ZK-AMS registry bootstrap policy digest must be nonzero")]
    ZeroPolicyDigest,
    /// Initial registry root is the zero sentinel.
    #[error("ZK-AMS registry bootstrap root must be nonzero")]
    ZeroInitialRoot,
    /// Initial epoch differs from the only closed first-release origin.
    #[error("ZK-AMS registry bootstrap initial epoch must be 1, got {epoch}")]
    NonCanonicalInitialEpoch {
        /// Rejected caller-provided epoch.
        epoch: u64,
    },
    /// Derived namespace is invalid.
    #[error("ZK-AMS registry bootstrap namespace is invalid")]
    InvalidNamespace,
}

/// Fixed typed Personhood Credential admitted by the Iroha ZK-AMS profile.
///
/// The issuer authenticates the domain-separated SHA-256 digest of the exact
/// canonical Norito encoding. The holder proves possession of the Ristretto
/// seed secret over the same digest in the composed admission proof. No
/// variable-length or free-form field is admitted by this first-release type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsPersonhoodCredentialV1 {
    /// Closed credential wire version; must equal one.
    pub version: u8,
    /// Governed issuer that authenticated the credential.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Governed policy under which the credential was issued.
    pub policy_id: PrivacyPolicyIdV1,
    /// Hidden commitment to the issuer-validated personhood subject.
    pub subject_commitment: PrivacyZkAmsSubjectCommitmentV1,
    /// Ristretto seed key later used for anonymous provisioning.
    pub seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
    /// Issuer-selected uniqueness nonce.
    pub credential_nonce: PrivacyZkAmsCredentialNonceV1,
}

impl PrivacyZkAmsPersonhoodCredentialV1 {
    /// Return the exact fixed typed-Norito payload signed by the issuer.
    ///
    /// The payload is closed to
    /// `version || issuer_id || policy_id || subject_commitment ||
    /// seed_public_key || credential_nonce`. Every field has a fixed width, so
    /// no optional, offset, or length table can introduce an alternative
    /// preimage.
    #[must_use]
    pub fn canonical_payload(&self) -> PrivacyZkAmsPhcCanonicalPayloadV1 {
        let mut payload = [0_u8; ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1];
        payload[0] = self.version;
        payload[1..33].copy_from_slice(self.issuer_id.as_bytes());
        payload[33..65].copy_from_slice(self.policy_id.as_bytes());
        payload[65..97].copy_from_slice(self.subject_commitment.as_bytes());
        payload[97..129].copy_from_slice(self.seed_public_key.as_bytes());
        payload[129..161].copy_from_slice(self.credential_nonce.as_bytes());
        PrivacyZkAmsPhcCanonicalPayloadV1(payload)
    }

    /// Hash the exact typed credential payload with domain and length framing.
    #[must_use]
    pub fn digest(&self) -> PrivacyZkAmsPhcHashV1 {
        let payload = self.canonical_payload();
        let mut hasher = Sha256::new();
        hasher.update(ZK_AMS_PHC_HASH_DOMAIN_V1);
        hasher.update(
            u64::try_from(payload.as_bytes().len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(payload.as_bytes());
        PrivacyZkAmsPhcHashV1::new(hasher.finalize().into())
    }
}

/// Exact fixed typed-Norito preimage of a ZK-AMS Personhood Credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyZkAmsPhcCanonicalPayloadV1(
    /// Exact closed credential payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub [u8; ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1],
);

impl PrivacyZkAmsPhcCanonicalPayloadV1 {
    /// Borrow the exact canonical payload.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1] {
        &self.0
    }
}

/// One ordered public admission anchor from ZK-AMS batch input `X`.
///
/// The order of these pairs is part of the Fiat-Shamir transcript certified
/// by the batch proof. Validation therefore preserves caller order and rejects
/// duplicate credential hashes or seed public keys without sorting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsAdmissionAnchorV1 {
    /// Hash of the canonical Personhood Credential.
    pub phc_hash: PrivacyZkAmsPhcHashV1,
    /// Seed public key later used for anonymous account provisioning.
    pub seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
}

/// Setup-free Iroha instantiation of ZK-AMS batch settlement.
///
/// The native proof recursively folds one fixed credential relation for every
/// ordered anchor and proves the final relaxed instance with a freshly masked
/// Relaxed Spartan proof. Intermediate accumulator and cross-term commitments
/// are already canonical proof sections; duplicating caller-selected digests
/// in the public statement would be circular and is deliberately forbidden.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsBatchAdmissionV1 {
    /// Current admitted-identity registry root.
    pub account_registry_root: PrivacyRootV1,
    /// Epoch at which `account_registry_root` is canonical.
    pub account_registry_root_epoch: u64,
    /// Resulting registry root after atomically recording all ordered anchors.
    pub next_account_registry_root: PrivacyRootV1,
    /// Exact successor epoch committed with `next_account_registry_root`.
    pub next_account_registry_root_epoch: u64,
    /// Ordered `{hash_PHC, pk_seed}` batch input `X`.
    pub anchors: Vec<PrivacyZkAmsAdmissionAnchorV1>,
}

/// ZK-AMS Phase-V anonymous account-provisioning public input.
///
/// The native suite verifies one LSAG over Ristretto255 with a SHA3-512
/// transcript and hash-to-group operation. Every ring key must be present in
/// the referenced admitted-identity registry. The account id is the signed
/// message binding, and the key image is the one-time replay nullifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsProvisionAccountV1 {
    /// Canonical admitted-identity registry root used for ring membership.
    pub account_registry_root: PrivacyRootV1,
    /// Epoch at which `account_registry_root` is canonical.
    pub account_registry_root_epoch: u64,
    /// Strictly increasing canonical ring of admitted seed public keys.
    pub admitted_seed_key_ring: Vec<PrivacyZkAmsSeedPublicKeyV1>,
    /// Fresh Iroha account/address bound by the LSAG signature.
    pub account_id: AccountId,
    /// Deterministic LSAG key image recorded as the provisioning nullifier.
    pub key_image: PrivacyZkAmsKeyImageV1,
}

/// Closed ZK-AMS chain action.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "action", content = "value", deny_unknown_fields)
)]
pub enum PrivacyZkAmsActionV1 {
    /// Settle one recursively accumulated admission batch.
    BatchAdmission(PrivacyZkAmsBatchAdmissionV1),
    /// Provision one anonymous account from an admitted seed-key ring.
    ProvisionAccount(PrivacyZkAmsProvisionAccountV1),
}

/// Native ZK-AMS batch-admission and anonymous-provisioning statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IrohaZkAmsStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Credential issuer governing the common admission relation.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Canonical compressed P-256 issuer key copied from authoritative state.
    ///
    /// Consensus must match this value to `issuer_policy_record_digest`; it is
    /// a transcript input, not a caller-selected trust anchor.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Digest of the authoritative issuer/policy/key record.
    pub issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    /// Admitted-identity and provisioning registry.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Digest of the authoritative registry snapshot referenced by the action.
    pub registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
    /// Admission policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Digest of the exact governed admission policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Exact batch-settlement or account-provisioning action.
    pub action: PrivacyZkAmsActionV1,
}

/// Credential document family admitted by the Vega first-release profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "document", content = "value", deny_unknown_fields)
)]
pub enum PrivacyCredentialDocumentTypeV1 {
    /// ISO/IEC 18013-5 `org.iso.18013.5.1.mDL` document.
    Iso18013_5Mdl,
}

/// Closed ISO/IEC 18013-5 namespace admitted by the Vega mDL-age profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "namespace", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaMdlNamespaceV1 {
    /// The standard mDL namespace `org.iso.18013.5.1`.
    OrgIso18013_5_1,
}

/// Closed digest algorithm used throughout the Vega mDL-age circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "digest", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaMdlDigestAlgorithmV1 {
    /// SHA-256 for issuer authentication, signed-item digests, and `H_dev`.
    Sha256,
}

/// Closed COSE signature algorithm used by issuer and device authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "signature", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaMdlSignatureAlgorithmV1 {
    /// COSE algorithm `-7`: ECDSA over P-256 with SHA-256 (`ES256`).
    CoseSign1Es256,
}

/// Forward-only lifecycle of one immutable Vega issuer governance lineage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaIssuerRecordLifecycleV1 {
    /// Credentials authenticated by this exact issuer revision may be verified.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// The issuer lineage is terminal and cannot be reactivated.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

/// One immutable authoritative Vega mDL issuer-key and algorithm-policy revision.
///
/// Revisions form a bounded append-only self-digested lineage. A proof
/// statement must bind the exact current active revision, including its P-256
/// key, so a submitter cannot manufacture a self-issued credential. Consensus
/// permanently assigns every issuer P-256 key to one issuer lineage: a key
/// retired or revoked in one lineage cannot be re-registered under another
/// issuer identity to relabel old credentials, and a key rotated out of a
/// lineage cannot later be reactivated inside that lineage. A terminal
/// revocation retains its immediately preceding key only to preserve the
/// immutable audit trail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyVegaIssuerRecordV1 {
    /// Stable credential-issuer identity selecting this lineage.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Exact canonical compressed P-256 issuer verification key.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Exact credential document family admitted by this issuer.
    pub document_type: PrivacyCredentialDocumentTypeV1,
    /// Exact mDL namespace admitted by this issuer.
    pub namespace: PrivacyVegaMdlNamespaceV1,
    /// Exact digest algorithm admitted by this issuer.
    pub digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1,
    /// Exact issuer-authentication algorithm admitted by this issuer.
    pub issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Exact device-authentication algorithm admitted by this issuer.
    pub device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyVegaIssuerRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyVegaIssuerRecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyVegaIssuerRecordDigestV1,
}

impl PrivacyVegaIssuerRecordV1 {
    /// Construct one canonical self-digested Vega issuer revision.
    ///
    /// # Errors
    ///
    /// Rejects zero identities or epochs, a malformed compressed-key shape,
    /// or a non-canonical predecessor shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issuer_id: PrivacyIssuerIdV1,
        record_epoch: u64,
        issuer_public_key: PrivacyP256PointV1,
        document_type: PrivacyCredentialDocumentTypeV1,
        namespace: PrivacyVegaMdlNamespaceV1,
        digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1,
        issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
        device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
        previous_record_digest: Option<PrivacyVegaIssuerRecordDigestV1>,
        lifecycle: PrivacyVegaIssuerRecordLifecycleV1,
    ) -> Result<Self, PrivacyVegaIssuerRecordValidationErrorV1> {
        let mut record = Self {
            issuer_id,
            record_epoch,
            issuer_public_key,
            document_type,
            namespace,
            digest_algorithm,
            issuer_authentication_algorithm,
            device_authentication_algorithm,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyVegaIssuerRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest();
        if record.record_digest.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// terminal origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyVegaIssuerRecordValidationErrorV1> {
        self.validate()?;
        if self.record_epoch != VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1 {
            return Err(
                PrivacyVegaIssuerRecordValidationErrorV1::NonCanonicalInitialEpoch {
                    actual: self.record_epoch,
                },
            );
        }
        if self.previous_record_digest.is_some() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::OriginHasPredecessor);
        }
        if self.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Active {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::InitialRecordNotActive);
        }
        Ok(())
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyVegaIssuerRecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest() != self.record_digest {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated canonical self-digest.
    #[must_use]
    pub fn compute_record_digest(&self) -> PrivacyVegaIssuerRecordDigestV1 {
        let version = VEGA_ISSUER_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let document_type = [match self.document_type {
            PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl => 0,
        }];
        let namespace = [match self.namespace {
            PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1 => 0,
        }];
        let digest_algorithm = [match self.digest_algorithm {
            PrivacyVegaMdlDigestAlgorithmV1::Sha256 => 0,
        }];
        let issuer_authentication_algorithm = [match self.issuer_authentication_algorithm {
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256 => 0,
        }];
        let device_authentication_algorithm = [match self.device_authentication_algorithm {
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256 => 0,
        }];
        let predecessor = privacy_vega_issuer_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = [match self.lifecycle {
            PrivacyVegaIssuerRecordLifecycleV1::Active => 0,
            PrivacyVegaIssuerRecordLifecycleV1::Revoked => 1,
        }];
        PrivacyVegaIssuerRecordDigestV1::new(privacy_vega_issuer_sha256_frame_v1(&[
            &version,
            self.issuer_id.as_bytes(),
            &record_epoch,
            self.issuer_public_key.as_bytes(),
            &document_type,
            &namespace,
            &digest_algorithm,
            &issuer_authentication_algorithm,
            &device_authentication_algorithm,
            &predecessor,
            &lifecycle,
        ]))
    }

    fn validate_contents(&self) -> Result<(), PrivacyVegaIssuerRecordValidationErrorV1> {
        if self.issuer_id.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroIssuerId);
        }
        if self.record_epoch == 0 {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroRecordEpoch);
        }
        if self.issuer_public_key.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroIssuerPublicKey);
        }
        if !matches!(self.issuer_public_key.as_bytes()[0], 0x02 | 0x03) {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::InvalidIssuerPublicKeyEncoding);
        }
        match (self.record_epoch, self.previous_record_digest) {
            (VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1, None) => {}
            (VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1, Some(_)) => {
                return Err(PrivacyVegaIssuerRecordValidationErrorV1::OriginHasPredecessor);
            }
            (_, None) => {
                return Err(PrivacyVegaIssuerRecordValidationErrorV1::SuccessorMissingPredecessor);
            }
            (_, Some(digest)) if digest.is_zero() => {
                return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroPreviousRecordDigest);
            }
            (_, Some(_)) => {}
        }
        Ok(())
    }
}

fn privacy_vega_issuer_sha256_frame_v1(fields: &[&[u8]]) -> [u8; 32] {
    let domain_len = u16::try_from(VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1.len())
        .expect("fixed Vega issuer-record digest domain fits u16");
    let field_count =
        u16::try_from(fields.len()).expect("fixed Vega issuer-record field count fits u16");
    let mut hash = Sha256::new();
    hash.update(VEGA_ISSUER_RECORD_HASH_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let field_len =
            u64::try_from(field.len()).expect("slice length fits u64 on supported targets");
        hash.update(field_len.to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

fn privacy_vega_issuer_predecessor_frame_v1(
    previous: Option<PrivacyVegaIssuerRecordDigestV1>,
) -> [u8; 33] {
    let mut frame = [0_u8; 33];
    if let Some(digest) = previous {
        frame[0] = 1;
        frame[1..].copy_from_slice(digest.as_bytes());
    }
    frame
}

/// Failure while validating one immutable Vega issuer governance revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyVegaIssuerRecordValidationErrorV1 {
    /// The stable issuer identifier is all zero.
    #[error("Vega issuer id must be non-zero")]
    ZeroIssuerId,
    /// Epoch zero is never authoritative.
    #[error("Vega issuer-record epoch must be non-zero")]
    ZeroRecordEpoch,
    /// Registration must begin at canonical epoch one.
    #[error(
        "initial Vega issuer-record epoch must be {VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1}, got {actual}"
    )]
    NonCanonicalInitialEpoch {
        /// Rejected epoch.
        actual: u64,
    },
    /// An origin revision cannot claim a predecessor.
    #[error("Vega epoch-one issuer record must not carry a predecessor")]
    OriginHasPredecessor,
    /// Every non-origin revision must bind its exact predecessor.
    #[error("Vega successor issuer record must carry a predecessor digest")]
    SuccessorMissingPredecessor,
    /// A predecessor digest cannot be the all-zero sentinel.
    #[error("Vega predecessor issuer-record digest must be non-zero")]
    ZeroPreviousRecordDigest,
    /// The issuer public key cannot be all zero.
    #[error("Vega issuer public key must be non-zero")]
    ZeroIssuerPublicKey,
    /// The wire key must at least have the canonical compressed SEC1 shape.
    #[error("Vega issuer public key must use compressed SEC1 encoding")]
    InvalidIssuerPublicKeyEncoding,
    /// Registration cannot create a terminal lineage.
    #[error("initial Vega issuer record must be active")]
    InitialRecordNotActive,
    /// A decoded record supplied an all-zero self-digest.
    #[error("Vega issuer-record self-digest must be non-zero")]
    ZeroRecordDigest,
    /// Recomputing every authoritative field produced a different digest.
    #[error("Vega issuer-record self-digest mismatch")]
    RecordDigestMismatch,
}

/// Failure while validating an append-only Vega issuer transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyVegaIssuerTransitionValidationErrorV1 {
    /// The persisted current revision is malformed.
    #[error("current Vega issuer record is invalid: {0}")]
    InvalidCurrent(PrivacyVegaIssuerRecordValidationErrorV1),
    /// The proposed successor revision is malformed.
    #[error("successor Vega issuer record is invalid: {0}")]
    InvalidSuccessor(PrivacyVegaIssuerRecordValidationErrorV1),
    /// A terminal lineage cannot advance.
    #[error("current Vega issuer record is not active")]
    CurrentNotActive,
    /// A transition changed its stable issuer identity.
    #[error("Vega issuer transition changed issuer id")]
    IssuerIdMismatch,
    /// An epoch cannot advance past `u64::MAX`.
    #[error("Vega issuer-record epoch overflow")]
    EpochOverflow,
    /// The successor did not advance exactly one epoch.
    #[error("Vega successor epoch must be {expected}, got {actual}")]
    NonCanonicalSuccessorEpoch {
        /// Required successor epoch.
        expected: u64,
        /// Rejected successor epoch.
        actual: u64,
    },
    /// The successor did not bind the exact current revision.
    #[error("Vega successor predecessor digest does not match the current revision")]
    PredecessorDigestMismatch,
    /// A rotation successor must remain active.
    #[error("Vega issuer rotation successor must be active")]
    RotationSuccessorNotActive,
    /// A rotation must alter the issuer key or admitted algorithm policy.
    #[error("Vega issuer rotation must change governed contents")]
    RotationContentsUnchanged,
    /// A revocation successor must be terminal.
    #[error("Vega issuer revocation successor must be revoked")]
    RevocationSuccessorNotRevoked,
    /// Revocation cannot silently change the trusted key or policy.
    #[error("Vega issuer revocation changed immutable governed contents")]
    RevocationContentsChanged,
}

fn validate_vega_issuer_transition_common_v1(
    current: &PrivacyVegaIssuerRecordV1,
    successor: &PrivacyVegaIssuerRecordV1,
) -> Result<(), PrivacyVegaIssuerTransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyVegaIssuerTransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyVegaIssuerTransitionValidationErrorV1::InvalidSuccessor)?;
    if current.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Active {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::CurrentNotActive);
    }
    if successor.issuer_id != current.issuer_id {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::IssuerIdMismatch);
    }
    let expected = current
        .record_epoch
        .checked_add(1)
        .ok_or(PrivacyVegaIssuerTransitionValidationErrorV1::EpochOverflow)?;
    if successor.record_epoch != expected {
        return Err(
            PrivacyVegaIssuerTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor.record_epoch,
            },
        );
    }
    if successor.previous_record_digest != Some(current.record_digest) {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::PredecessorDigestMismatch);
    }
    Ok(())
}

/// Validate an active-to-active Vega issuer key or policy rotation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, terminal successors, and no-op rotations.
pub fn validate_vega_issuer_rotation_v1(
    current: &PrivacyVegaIssuerRecordV1,
    successor: &PrivacyVegaIssuerRecordV1,
) -> Result<(), PrivacyVegaIssuerTransitionValidationErrorV1> {
    validate_vega_issuer_transition_common_v1(current, successor)?;
    if successor.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Active {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.issuer_public_key == current.issuer_public_key
        && successor.document_type == current.document_type
        && successor.namespace == current.namespace
        && successor.digest_algorithm == current.digest_algorithm
        && successor.issuer_authentication_algorithm == current.issuer_authentication_algorithm
        && successor.device_authentication_algorithm == current.device_authentication_algorithm
    {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RotationContentsUnchanged);
    }
    Ok(())
}

/// Validate an irreversible Vega issuer revocation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, nonterminal successors, or key/policy changes.
pub fn validate_vega_issuer_revocation_v1(
    current: &PrivacyVegaIssuerRecordV1,
    successor: &PrivacyVegaIssuerRecordV1,
) -> Result<(), PrivacyVegaIssuerTransitionValidationErrorV1> {
    validate_vega_issuer_transition_common_v1(current, successor)?;
    if successor.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Revoked {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.issuer_public_key != current.issuer_public_key
        || successor.document_type != current.document_type
        || successor.namespace != current.namespace
        || successor.digest_algorithm != current.digest_algorithm
        || successor.issuer_authentication_algorithm != current.issuer_authentication_algorithm
        || successor.device_authentication_algorithm != current.device_authentication_algorithm
    {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// Gregorian UTC calendar date used as Vega Figure 9 public input `(Y, M, D)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyVegaMdlDateV1 {
    /// Four-digit UTC year.
    pub year: u16,
    /// One-based UTC month.
    pub month: u8,
    /// One-based UTC day of month.
    pub day: u8,
}

/// Vega Figure 9 ISO/IEC 18013-5 mDL-age public statement.
///
/// The native circuit exposes only the paper's public inputs `Q_I`, `H_dev`,
/// `(Y, M, D)`, and `tau`. The exact document bytes, decoded MSO payload,
/// issuer and device signatures, device public key, validity interval,
/// birth-date `IssuerSignedItemBytes`, and every lookup hint are private
/// engine witness values.
///
/// `issuer_id`, `issuer_record_epoch`, and `issuer_record_digest` select the
/// exact active governance revision whose key and algorithm policy must match
/// this statement before native verification. The proof has no ledger effect.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct VegaExistingCredentialStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Stable governed issuer identity selecting one authoritative lineage.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact current authoritative issuer revision epoch.
    pub issuer_record_epoch: u64,
    /// Exact self-digest of the authoritative issuer revision.
    pub issuer_record_digest: PrivacyVegaIssuerRecordDigestV1,
    /// Exact supported credential document family and `docType`.
    pub document_type: PrivacyCredentialDocumentTypeV1,
    /// Exact namespace containing the `birth_date` signed item.
    pub namespace: PrivacyVegaMdlNamespaceV1,
    /// Exact digest algorithm constrained by the circuit.
    pub digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1,
    /// Exact issuer COSE authentication algorithm constrained by the circuit.
    pub issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Exact device COSE authentication algorithm constrained by the circuit.
    pub device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Public P-256 issuer key `Q_I`, copied exactly from authoritative state.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Public device-authentication digest `H_dev`.
    ///
    /// The native engine recomputes this value from the canonical consensus
    /// frame containing chain, genesis, action, all governed artifact
    /// bindings, `Q_I`, date, threshold, challenge, and session digest before
    /// performing any proof verification.
    pub device_authentication_digest: PrivacyVegaDeviceAuthenticationDigestV1,
    /// Public trusted UTC presentation date `(Y, M, D)`.
    ///
    /// Admission additionally requires exact equality with the UTC date
    /// derived from the canonical block timestamp.
    pub presentation_date: PrivacyVegaMdlDateV1,
    /// Public minimum age threshold `tau`, in completed Gregorian years.
    pub minimum_age_years: u8,
    /// Fresh reader challenge incorporated into the `H_dev` consensus frame.
    pub reader_challenge: PrivacyChallengeV1,
    /// Digest of the canonical ISO 18013-5 session transcript incorporated
    /// into the `H_dev` consensus frame.
    pub session_transcript_digest: PrivacySessionTranscriptDigestV1,
}

/// One required X.509 key-usage bit.
///
/// This is transparent in canonical Norito and JSON, so each use remains an
/// exact boolean on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
pub struct PrivacyX509KeyUsageRequirementV1(bool);

impl PrivacyX509KeyUsageRequirementV1 {
    /// Construct a key-usage requirement from its canonical boolean value.
    #[must_use]
    pub const fn new(required: bool) -> Self {
        Self(required)
    }

    /// Return whether this key usage is required.
    #[must_use]
    pub const fn is_required(self) -> bool {
        self.0
    }
}

impl From<bool> for PrivacyX509KeyUsageRequirementV1 {
    fn from(required: bool) -> Self {
        Self::new(required)
    }
}

impl From<PrivacyX509KeyUsageRequirementV1> for bool {
    fn from(requirement: PrivacyX509KeyUsageRequirementV1) -> Self {
        requirement.is_required()
    }
}

impl norito::core::NoritoSerialize for PrivacyX509KeyUsageRequirementV1 {
    fn schema_hash() -> [u8; 16] {
        <bool as norito::core::NoritoSerialize>::schema_hash()
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_exact(&self.0)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for PrivacyX509KeyUsageRequirementV1 {
    fn schema_hash() -> [u8; 16] {
        <bool as norito::core::NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self(<bool as norito::core::NoritoDeserialize>::deserialize(
            archived.cast(),
        ))
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        <bool as norito::core::NoritoDeserialize>::try_deserialize(archived.cast()).map(Self)
    }
}

impl<'de> norito::core::DecodeFromSlice<'de> for PrivacyX509KeyUsageRequirementV1 {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::core::Error> {
        <bool as norito::core::DecodeFromSlice>::decode_from_slice(bytes)
            .map(|(required, used)| (Self(required), used))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for PrivacyX509KeyUsageRequirementV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for PrivacyX509KeyUsageRequirementV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        <bool as norito::json::JsonDeserialize>::json_deserialize(parser).map(Self)
    }

    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        <bool as norito::json::JsonDeserialize>::json_from_value(value).map(Self)
    }

    fn json_from_map_key(key: &str) -> Result<Self, norito::json::Error> {
        <bool as norito::json::JsonDeserialize>::json_from_map_key(key).map(Self)
    }
}

/// X.509 key-usage requirements admitted by the first-release certificate profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyX509KeyUsageV1 {
    /// RFC 5280 digital-signature bit.
    pub digital_signature: PrivacyX509KeyUsageRequirementV1,
    /// RFC 5280 content-commitment bit.
    pub content_commitment: PrivacyX509KeyUsageRequirementV1,
    /// RFC 5280 key-encipherment bit.
    pub key_encipherment: PrivacyX509KeyUsageRequirementV1,
    /// RFC 5280 key-agreement bit.
    pub key_agreement: PrivacyX509KeyUsageRequirementV1,
}

/// Exact extended-key-usage purpose required from an admitted certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "purpose", content = "value", deny_unknown_fields)
)]
pub enum PrivacyX509ExtendedKeyUsageV1 {
    /// TLS-style client authentication.
    ClientAuthentication,
    /// Digital document signing.
    DocumentSigning,
    /// Wallet or digital-identity authentication.
    WalletIdentity,
}

/// Closed lifecycle of one immutable X.509 governance-record lineage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "value", deny_unknown_fields)
)]
pub enum PrivacyZkX509RecordLifecycleV1 {
    /// The trust-anchor or certificate-policy revision is authoritative.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// The lineage was irreversibly revoked.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

/// One immutable authoritative revision of an RFC 5280 P-256/SHA-256 trust store.
///
/// Revisions form an append-only self-digested chain. `trust_store_digest`
/// commits the complete canonically ordered trust-anchor artifact; individual
/// CA identity remains private behind the governed CA-membership root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509TrustAnchorRecordV1 {
    /// Stable lookup key for this trust-store lineage.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Digest of the exact ordered P-256/SHA-256 trust-store artifact.
    pub trust_store_digest: PrivacyX509TrustStoreDigestV1,
    /// Exact CA-membership root derived from that complete trust store.
    pub ca_membership_root: PrivacyRootV1,
    /// Canonical epoch of `ca_membership_root`.
    ///
    /// An active record requires this to equal `record_epoch`. A terminal
    /// revocation preserves the preceding active root epoch and does not
    /// manufacture a new CA-membership root.
    pub ca_membership_root_epoch: u64,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyZkX509TrustAnchorRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkX509RecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyZkX509TrustAnchorRecordDigestV1,
}

impl PrivacyZkX509TrustAnchorRecordV1 {
    /// Construct one canonical self-digested trust-anchor revision.
    ///
    /// # Errors
    ///
    /// Rejects zero fields, a root epoch inconsistent with the lifecycle, a
    /// non-canonical predecessor shape, or an invalid lifecycle/root binding.
    pub fn new(
        trust_anchor_id: PrivacyIssuerIdV1,
        record_epoch: u64,
        trust_store_digest: PrivacyX509TrustStoreDigestV1,
        ca_membership_root: PrivacyRootV1,
        ca_membership_root_epoch: u64,
        previous_record_digest: Option<PrivacyZkX509TrustAnchorRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> Result<Self, PrivacyZkX509RecordValidationErrorV1> {
        let mut record = Self {
            trust_anchor_id,
            record_epoch,
            trust_store_digest,
            ca_membership_root,
            ca_membership_root_epoch,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyZkX509TrustAnchorRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// revoked origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate()?;
        validate_zk_x509_initial_revision(
            self.record_epoch,
            self.previous_record_digest.is_some(),
            self.lifecycle,
        )
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated self-digest.
    ///
    /// # Errors
    ///
    /// Uses the explicit SHA-256 field frame shared with the proof system.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkX509TrustAnchorRecordDigestV1, PrivacyZkX509RecordValidationErrorV1> {
        let version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let ca_membership_root_epoch = self.ca_membership_root_epoch.to_be_bytes();
        let predecessor = privacy_zk_x509_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = privacy_zk_x509_lifecycle_frame_v1(self.lifecycle);
        Ok(PrivacyZkX509TrustAnchorRecordDigestV1::new(
            privacy_zk_x509_sha256_frame_v1(
                ZK_X509_TRUST_ANCHOR_RECORD_DIGEST_DOMAIN_V1,
                &[
                    &version,
                    self.trust_anchor_id.as_bytes(),
                    &record_epoch,
                    self.trust_store_digest.as_bytes(),
                    self.ca_membership_root.as_bytes(),
                    &ca_membership_root_epoch,
                    &predecessor,
                    &lifecycle,
                ],
            ),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        if self.trust_anchor_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustAnchorId);
        }
        if self.trust_store_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustStoreDigest);
        }
        if self.ca_membership_root.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCaMembershipRoot);
        }
        if self.ca_membership_root_epoch == 0 {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCaMembershipRootEpoch);
        }
        match self.lifecycle {
            PrivacyZkX509RecordLifecycleV1::Active
                if self.ca_membership_root_epoch != self.record_epoch =>
            {
                return Err(
                    PrivacyZkX509RecordValidationErrorV1::CaMembershipRootEpochMismatch {
                        record_epoch: self.record_epoch,
                        root_epoch: self.ca_membership_root_epoch,
                    },
                );
            }
            PrivacyZkX509RecordLifecycleV1::Revoked
                if self.ca_membership_root_epoch >= self.record_epoch =>
            {
                return Err(
                    PrivacyZkX509RecordValidationErrorV1::RevokedCaMembershipRootEpochNotHistorical {
                        record_epoch: self.record_epoch,
                        root_epoch: self.ca_membership_root_epoch,
                    },
                );
            }
            PrivacyZkX509RecordLifecycleV1::Active | PrivacyZkX509RecordLifecycleV1::Revoked => {}
        }
        validate_zk_x509_revision_shape(self.record_epoch, self.previous_record_digest)
    }
}

/// One immutable authoritative X.509 certificate-policy revision.
///
/// The policy fixes every public predicate selected outside the certificate
/// witness. In particular, a statement must disclose exactly the governed
/// ordered index set rather than a prover-chosen subset or superset.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509CertificatePolicyRecordV1 {
    /// Exact trust-store lineage to which this policy belongs.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Stable policy lookup key inside the trust-store namespace.
    pub policy_id: PrivacyPolicyIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Digest of the exact governed RFC 5280 certificate-policy artifact.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Required RFC 5280 leaf key-usage bits.
    pub required_key_usage: PrivacyX509KeyUsageV1,
    /// Required extended-key usages in strict enum order.
    pub required_extended_key_usages: Vec<PrivacyX509ExtendedKeyUsageV1>,
    /// Exact required selective-disclosure indices in strict numeric order.
    pub required_disclosed_attribute_indices: Vec<u8>,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyZkX509CertificatePolicyRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkX509RecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1,
}

impl PrivacyZkX509CertificatePolicyRecordV1 {
    /// Construct one canonical self-digested certificate-policy revision.
    ///
    /// # Errors
    ///
    /// Rejects zero fields, unsupported key usage, oversized or unordered
    /// policy lists or a non-canonical predecessor shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        record_epoch: u64,
        policy_digest: PrivacyPolicyDigestV1,
        required_key_usage: PrivacyX509KeyUsageV1,
        required_extended_key_usages: Vec<PrivacyX509ExtendedKeyUsageV1>,
        required_disclosed_attribute_indices: Vec<u8>,
        previous_record_digest: Option<PrivacyZkX509CertificatePolicyRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> Result<Self, PrivacyZkX509RecordValidationErrorV1> {
        let mut record = Self {
            trust_anchor_id,
            policy_id,
            record_epoch,
            policy_digest,
            required_key_usage,
            required_extended_key_usages,
            required_disclosed_attribute_indices,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// revoked origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate()?;
        validate_zk_x509_initial_revision(
            self.record_epoch,
            self.previous_record_digest.is_some(),
            self.lifecycle,
        )
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated self-digest.
    ///
    /// # Errors
    ///
    /// Uses the explicit SHA-256 field frame shared with the proof system.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkX509CertificatePolicyRecordDigestV1, PrivacyZkX509RecordValidationErrorV1>
    {
        validate_zk_x509_key_usage(self.required_key_usage)?;
        validate_zk_x509_extended_key_usages(&self.required_extended_key_usages)?;
        validate_zk_x509_disclosure_indices(&self.required_disclosed_attribute_indices)?;
        let version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let key_usage = [privacy_zk_x509_key_usage_mask_v1(self.required_key_usage)];
        let mut extended_key_usages =
            Vec::with_capacity(1 + self.required_extended_key_usages.len());
        extended_key_usages.push(
            u8::try_from(self.required_extended_key_usages.len())
                .expect("validated X.509 EKU count fits u8"),
        );
        extended_key_usages.extend(
            self.required_extended_key_usages
                .iter()
                .copied()
                .map(privacy_zk_x509_extended_key_usage_code_v1),
        );
        let mut disclosed_attributes =
            Vec::with_capacity(1 + self.required_disclosed_attribute_indices.len());
        disclosed_attributes.push(
            u8::try_from(self.required_disclosed_attribute_indices.len())
                .expect("validated X.509 disclosure count fits u8"),
        );
        disclosed_attributes.extend_from_slice(&self.required_disclosed_attribute_indices);
        let predecessor = privacy_zk_x509_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = privacy_zk_x509_lifecycle_frame_v1(self.lifecycle);
        Ok(PrivacyZkX509CertificatePolicyRecordDigestV1::new(
            privacy_zk_x509_sha256_frame_v1(
                ZK_X509_CERTIFICATE_POLICY_RECORD_DIGEST_DOMAIN_V1,
                &[
                    &version,
                    self.trust_anchor_id.as_bytes(),
                    self.policy_id.as_bytes(),
                    &record_epoch,
                    self.policy_digest.as_bytes(),
                    &key_usage,
                    &extended_key_usages,
                    &disclosed_attributes,
                    &predecessor,
                    &lifecycle,
                ],
            ),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        if self.trust_anchor_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustAnchorId);
        }
        if self.policy_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroPolicyId);
        }
        if self.policy_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroPolicyDigest);
        }
        validate_zk_x509_key_usage(self.required_key_usage)?;
        validate_zk_x509_extended_key_usages(&self.required_extended_key_usages)?;
        validate_zk_x509_disclosure_indices(&self.required_disclosed_attribute_indices)?;
        validate_zk_x509_revision_shape(self.record_epoch, self.previous_record_digest)
    }
}

/// One immutable authoritative revision of an issuer-scoped signed CRL.
///
/// The exact signed DER digest, signing-key digest, and validity window are one
/// self-digested governance object. The proof parses that complete, signed CRL
/// and checks the leaf serial against every active entry; there is deliberately
/// no second revocation accumulator whose contents could diverge from the CRL.
/// The first release assigns exactly one leaf certificate issuer and its
/// complete, non-partitioned CRL to each certificate-policy lineage;
/// revocation checks the leaf certificate only.
/// Multiple intermediates require distinct policy lineages. Consensus keeps
/// only the current self-chained record while historical transitions remain
/// committed by blocks. Revocation is terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509CrlRecordV1 {
    /// Exact trust-store lineage whose certificate chain selects this CRL.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Exact certificate-policy lineage whose leaf revocation is represented.
    pub certificate_policy_id: PrivacyPolicyIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Required RFC 5280 CRLNumber, monotonically increasing in this lineage.
    pub crl_number: u64,
    /// SHA-256 digest of the complete exact signed DER CRL.
    pub crl_der_digest: PrivacyX509CrlDerDigestV1,
    /// SHA-256 digest of the exact issuer SPKI that signs the CRL.
    pub issuer_spki_digest: PrivacyX509CrlIssuerSpkiDigestV1,
    /// Signed CRL `thisUpdate` as Unix seconds, inclusive.
    pub this_update_unix_seconds: u64,
    /// Signed CRL `nextUpdate` as Unix seconds, exclusive.
    pub next_update_unix_seconds: u64,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyZkX509CrlRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkX509RecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyZkX509CrlRecordDigestV1,
}

impl PrivacyZkX509CrlRecordV1 {
    /// Construct one canonical self-digested signed-CRL revision.
    ///
    /// # Errors
    ///
    /// Rejects zero identities or digests, an invalid signed validity window,
    /// or a non-canonical predecessor shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_anchor_id: PrivacyIssuerIdV1,
        certificate_policy_id: PrivacyPolicyIdV1,
        record_epoch: u64,
        crl_number: u64,
        crl_der_digest: PrivacyX509CrlDerDigestV1,
        issuer_spki_digest: PrivacyX509CrlIssuerSpkiDigestV1,
        this_update_unix_seconds: u64,
        next_update_unix_seconds: u64,
        previous_record_digest: Option<PrivacyZkX509CrlRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> Result<Self, PrivacyZkX509RecordValidationErrorV1> {
        let mut record = Self {
            trust_anchor_id,
            certificate_policy_id,
            record_epoch,
            crl_number,
            crl_der_digest,
            issuer_spki_digest,
            this_update_unix_seconds,
            next_update_unix_seconds,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyZkX509CrlRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// revoked origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate()?;
        validate_zk_x509_initial_revision(
            self.record_epoch,
            self.previous_record_digest.is_some(),
            self.lifecycle,
        )
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated self-digest.
    ///
    /// # Errors
    ///
    /// Uses the explicit SHA-256 field frame shared with the proof system.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkX509CrlRecordDigestV1, PrivacyZkX509RecordValidationErrorV1> {
        let version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let crl_number = self.crl_number.to_be_bytes();
        let this_update = self.this_update_unix_seconds.to_be_bytes();
        let next_update = self.next_update_unix_seconds.to_be_bytes();
        let predecessor = privacy_zk_x509_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = privacy_zk_x509_lifecycle_frame_v1(self.lifecycle);
        Ok(PrivacyZkX509CrlRecordDigestV1::new(
            privacy_zk_x509_sha256_frame_v1(
                ZK_X509_CRL_RECORD_DIGEST_DOMAIN_V1,
                &[
                    &version,
                    self.trust_anchor_id.as_bytes(),
                    self.certificate_policy_id.as_bytes(),
                    &record_epoch,
                    &crl_number,
                    self.crl_der_digest.as_bytes(),
                    self.issuer_spki_digest.as_bytes(),
                    &this_update,
                    &next_update,
                    &predecessor,
                    &lifecycle,
                ],
            ),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        if self.trust_anchor_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustAnchorId);
        }
        if self.certificate_policy_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroPolicyId);
        }
        if self.crl_der_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCrlDerDigest);
        }
        if self.issuer_spki_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCrlIssuerSpkiDigest);
        }
        if self.next_update_unix_seconds <= self.this_update_unix_seconds {
            return Err(PrivacyZkX509RecordValidationErrorV1::InvalidCrlValidityWindow);
        }
        validate_zk_x509_revision_shape(self.record_epoch, self.previous_record_digest)
    }
}

fn privacy_zk_x509_sha256_frame_v1(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let domain_len = u16::try_from(domain.len()).expect("fixed X.509 digest domain fits u16");
    let field_count = u16::try_from(fields.len()).expect("fixed X.509 digest field count fits u16");
    let mut hash = Sha256::new();
    hash.update(ZK_X509_HASH_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(domain);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let field_len =
            u64::try_from(field.len()).expect("slice length fits u64 on supported targets");
        hash.update(field_len.to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

fn privacy_zk_x509_predecessor_frame_v1<D: PrivacyDigestValueV1>(previous: Option<D>) -> [u8; 33] {
    let mut frame = [0_u8; 33];
    if let Some(digest) = previous {
        frame[0] = 1;
        frame[1..].copy_from_slice(&digest.bytes());
    }
    frame
}

const fn privacy_zk_x509_lifecycle_frame_v1(lifecycle: PrivacyZkX509RecordLifecycleV1) -> [u8; 1] {
    [match lifecycle {
        PrivacyZkX509RecordLifecycleV1::Active => 0,
        PrivacyZkX509RecordLifecycleV1::Revoked => 1,
    }]
}

fn privacy_zk_x509_key_usage_mask_v1(key_usage: PrivacyX509KeyUsageV1) -> u8 {
    u8::from(key_usage.digital_signature.is_required())
        | (u8::from(key_usage.content_commitment.is_required()) << 1)
        | (u8::from(key_usage.key_encipherment.is_required()) << 2)
        | (u8::from(key_usage.key_agreement.is_required()) << 3)
}

const fn privacy_zk_x509_extended_key_usage_code_v1(usage: PrivacyX509ExtendedKeyUsageV1) -> u8 {
    match usage {
        PrivacyX509ExtendedKeyUsageV1::ClientAuthentication => 0,
        PrivacyX509ExtendedKeyUsageV1::DocumentSigning => 1,
        PrivacyX509ExtendedKeyUsageV1::WalletIdentity => 2,
    }
}

fn validate_zk_x509_revision_shape<D: PrivacyDigestValueV1>(
    record_epoch: u64,
    previous_record_digest: Option<D>,
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if record_epoch == 0 {
        return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordEpoch);
    }
    match (record_epoch, previous_record_digest) {
        (ZK_X509_INITIAL_RECORD_EPOCH_V1, None) => Ok(()),
        (ZK_X509_INITIAL_RECORD_EPOCH_V1, Some(_)) => {
            Err(PrivacyZkX509RecordValidationErrorV1::OriginHasPredecessor)
        }
        (_, None) => Err(PrivacyZkX509RecordValidationErrorV1::SuccessorMissingPredecessor),
        (_, Some(digest)) if digest.is_zero() => {
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroPreviousRecordDigest)
        }
        (_, Some(_)) => Ok(()),
    }
}

trait PrivacyDigestValueV1: Copy {
    fn is_zero(self) -> bool;
    fn bytes(self) -> [u8; 32];
}

impl PrivacyDigestValueV1 for PrivacyZkX509TrustAnchorRecordDigestV1 {
    fn is_zero(self) -> bool {
        PrivacyZkX509TrustAnchorRecordDigestV1::is_zero(&self)
    }

    fn bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

impl PrivacyDigestValueV1 for PrivacyZkX509CertificatePolicyRecordDigestV1 {
    fn is_zero(self) -> bool {
        PrivacyZkX509CertificatePolicyRecordDigestV1::is_zero(&self)
    }

    fn bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

impl PrivacyDigestValueV1 for PrivacyZkX509CrlRecordDigestV1 {
    fn is_zero(self) -> bool {
        PrivacyZkX509CrlRecordDigestV1::is_zero(&self)
    }

    fn bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

fn validate_zk_x509_initial_revision(
    record_epoch: u64,
    has_previous: bool,
    lifecycle: PrivacyZkX509RecordLifecycleV1,
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if record_epoch != ZK_X509_INITIAL_RECORD_EPOCH_V1 {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::NonCanonicalInitialEpoch {
                actual: record_epoch,
            },
        );
    }
    if has_previous {
        return Err(PrivacyZkX509RecordValidationErrorV1::OriginHasPredecessor);
    }
    if lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509RecordValidationErrorV1::InitialRecordNotActive);
    }
    Ok(())
}

fn validate_zk_x509_key_usage(
    key_usage: PrivacyX509KeyUsageV1,
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if !key_usage.digital_signature.is_required() {
        return Err(PrivacyZkX509RecordValidationErrorV1::InvalidKeyUsage);
    }
    Ok(())
}

fn validate_zk_x509_extended_key_usages(
    usages: &[PrivacyX509ExtendedKeyUsageV1],
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if usages.is_empty() {
        return Err(PrivacyZkX509RecordValidationErrorV1::MissingExtendedKeyUsage);
    }
    if usages.len() > ZK_X509_MAX_EXTENDED_KEY_USAGES_V1 {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::TooManyExtendedKeyUsages {
                actual: usages.len(),
                max: ZK_X509_MAX_EXTENDED_KEY_USAGES_V1,
            },
        );
    }
    if usages.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PrivacyZkX509RecordValidationErrorV1::ExtendedKeyUsagesNotStrictlyIncreasing);
    }
    Ok(())
}

fn validate_zk_x509_disclosure_indices(
    indices: &[u8],
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if indices.len() > ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::TooManyDisclosedAttributes {
                actual: indices.len(),
                max: ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1,
            },
        );
    }
    for &index in indices {
        if usize::from(index) >= ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
            return Err(
                PrivacyZkX509RecordValidationErrorV1::UnsupportedDisclosedAttributeIndex { index },
            );
        }
    }
    if indices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::DisclosedAttributeIndicesNotStrictlyIncreasing,
        );
    }
    Ok(())
}

/// Failure while validating one immutable X.509 governance revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkX509RecordValidationErrorV1 {
    /// The trust-store identifier is all zero.
    #[error("X.509 trust-anchor id must be non-zero")]
    ZeroTrustAnchorId,
    /// The certificate-policy identifier is all zero.
    #[error("X.509 certificate-policy id must be non-zero")]
    ZeroPolicyId,
    /// The canonical trust-store digest is all zero.
    #[error("X.509 trust-store digest must be non-zero")]
    ZeroTrustStoreDigest,
    /// The CA-membership root derived from the trust store is all zero.
    #[error("X.509 CA-membership root must be non-zero")]
    ZeroCaMembershipRoot,
    /// A CA-membership root epoch is zero.
    #[error("X.509 CA-membership root epoch must be non-zero")]
    ZeroCaMembershipRootEpoch,
    /// The first-release trust-anchor record and CA-root epochs differ.
    #[error(
        "X.509 CA-membership root epoch {root_epoch} must equal trust-anchor record epoch {record_epoch}"
    )]
    CaMembershipRootEpochMismatch {
        /// Immutable trust-anchor record epoch.
        record_epoch: u64,
        /// Rejected CA-membership root epoch.
        root_epoch: u64,
    },
    /// A terminal trust-anchor record did not preserve a historical active root.
    #[error(
        "revoked X.509 CA-membership root epoch {root_epoch} must precede record epoch {record_epoch}"
    )]
    RevokedCaMembershipRootEpochNotHistorical {
        /// Terminal trust-anchor record epoch.
        record_epoch: u64,
        /// Rejected retained active-root epoch.
        root_epoch: u64,
    },
    /// The canonical certificate-policy digest is all zero.
    #[error("X.509 certificate-policy digest must be non-zero")]
    ZeroPolicyDigest,
    /// The SHA-256 digest of the exact signed DER CRL is all zero.
    #[error("X.509 signed DER CRL digest must be non-zero")]
    ZeroCrlDerDigest,
    /// The SHA-256 digest of the CRL issuer SPKI is all zero.
    #[error("X.509 CRL issuer SPKI digest must be non-zero")]
    ZeroCrlIssuerSpkiDigest,
    /// The signed CRL validity window is zero, empty, or reversed.
    #[error("X.509 CRL validity window must satisfy thisUpdate < nextUpdate")]
    InvalidCrlValidityWindow,
    /// Epoch zero is never authoritative.
    #[error("X.509 governance-record epoch must be non-zero")]
    ZeroRecordEpoch,
    /// Registration must begin at canonical epoch one.
    #[error(
        "initial X.509 governance-record epoch must be {ZK_X509_INITIAL_RECORD_EPOCH_V1}, got {actual}"
    )]
    NonCanonicalInitialEpoch {
        /// Rejected epoch.
        actual: u64,
    },
    /// An origin revision cannot claim a predecessor.
    #[error("X.509 epoch-one governance record must not carry a predecessor")]
    OriginHasPredecessor,
    /// Every non-origin revision must bind its exact predecessor.
    #[error("X.509 successor governance record must carry a predecessor digest")]
    SuccessorMissingPredecessor,
    /// A predecessor digest cannot be the all-zero sentinel.
    #[error("X.509 predecessor record digest must be non-zero")]
    ZeroPreviousRecordDigest,
    /// Registration cannot create a terminal lineage.
    #[error("initial X.509 governance record must be active")]
    InitialRecordNotActive,
    /// The fixed certificate relation requires digital-signature usage.
    #[error("X.509 policy must require digital-signature key usage")]
    InvalidKeyUsage,
    /// At least one closed extended-key usage must be governed.
    #[error("X.509 policy must require at least one extended-key usage")]
    MissingExtendedKeyUsage,
    /// More EKUs were supplied than the closed profile supports.
    #[error("X.509 policy has {actual} extended-key usages; maximum is {max}")]
    TooManyExtendedKeyUsages {
        /// Rejected length.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// EKUs were duplicated or reordered.
    #[error("X.509 extended-key usages must be strictly increasing")]
    ExtendedKeyUsagesNotStrictlyIncreasing,
    /// More disclosures were supplied than the closed profile supports.
    #[error("X.509 policy has {actual} disclosed attributes; maximum is {max}")]
    TooManyDisclosedAttributes {
        /// Rejected length.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// An attribute index is outside the closed C/O/OU/CN set.
    #[error("X.509 disclosed attribute index {index} is unsupported")]
    UnsupportedDisclosedAttributeIndex {
        /// Rejected index.
        index: u8,
    },
    /// Disclosure indices were duplicated or reordered.
    #[error("X.509 disclosed attribute indices must be strictly increasing")]
    DisclosedAttributeIndicesNotStrictlyIncreasing,
    /// A decoded record supplied an all-zero self-digest.
    #[error("X.509 governance-record self-digest must be non-zero")]
    ZeroRecordDigest,
    /// Recomputing every authoritative field produced a different digest.
    #[error("X.509 governance-record self-digest mismatch")]
    RecordDigestMismatch,
}

/// Failure while validating an append-only X.509 governance transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkX509TransitionValidationErrorV1 {
    /// The persisted current revision is malformed.
    #[error("current X.509 governance record is invalid: {0}")]
    InvalidCurrent(PrivacyZkX509RecordValidationErrorV1),
    /// The proposed successor revision is malformed.
    #[error("successor X.509 governance record is invalid: {0}")]
    InvalidSuccessor(PrivacyZkX509RecordValidationErrorV1),
    /// A terminal lineage cannot advance.
    #[error("current X.509 governance record is not active")]
    CurrentNotActive,
    /// A trust-anchor transition changed its stable identity.
    #[error("X.509 trust-anchor transition changed trust-anchor id")]
    TrustAnchorIdMismatch,
    /// A trust-store digest rotated without its derived CA-membership root.
    #[error("X.509 trust-store digest changed without changing its CA-membership root")]
    TrustStoreDigestChangedWithoutCaMembershipRoot,
    /// A CA-membership root rotated without its source trust-store digest.
    #[error("X.509 CA-membership root changed without changing its trust-store digest")]
    CaMembershipRootChangedWithoutTrustStoreDigest,
    /// A policy transition changed its stable identity.
    #[error("X.509 certificate-policy transition changed policy id")]
    PolicyIdMismatch,
    /// A signed-CRL lineage changed the issuer public key.
    #[error("X.509 CRL transition changed issuer SPKI digest")]
    CrlIssuerSpkiDigestMismatch,
    /// An epoch cannot advance past `u64::MAX`.
    #[error("X.509 governance-record epoch overflow")]
    EpochOverflow,
    /// The successor did not advance exactly one epoch.
    #[error("X.509 successor epoch must be {expected}, got {actual}")]
    NonCanonicalSuccessorEpoch {
        /// Required successor epoch.
        expected: u64,
        /// Rejected successor epoch.
        actual: u64,
    },
    /// The successor did not bind the exact current revision.
    #[error("X.509 successor predecessor digest does not match the current revision")]
    PredecessorDigestMismatch,
    /// A rotation successor must remain active.
    #[error("X.509 rotation successor must be active")]
    RotationSuccessorNotActive,
    /// A rotation must alter at least one governed substantive field.
    #[error("X.509 rotation must change governed contents")]
    RotationContentsUnchanged,
    /// A signed-CRL successor did not advance `thisUpdate`.
    #[error("X.509 CRL successor thisUpdate must strictly increase")]
    CrlThisUpdateNotIncreasing,
    /// A signed-CRL successor did not advance CRLNumber.
    #[error("X.509 CRL successor CRLNumber must strictly increase")]
    CrlNumberNotIncreasing,
    /// A revocation successor must be terminal.
    #[error("X.509 revocation successor must be revoked")]
    RevocationSuccessorNotRevoked,
    /// Revocation changed substantive governed contents.
    #[error("X.509 revocation changed immutable governed contents")]
    RevocationContentsChanged,
}

fn validate_zk_x509_transition_common<D: Copy + PartialEq>(
    current_epoch: u64,
    current_digest: D,
    current_lifecycle: PrivacyZkX509RecordLifecycleV1,
    successor_epoch: u64,
    successor_previous_digest: Option<D>,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    if current_lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CurrentNotActive);
    }
    let expected = current_epoch
        .checked_add(1)
        .ok_or(PrivacyZkX509TransitionValidationErrorV1::EpochOverflow)?;
    if successor_epoch != expected {
        return Err(
            PrivacyZkX509TransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor_epoch,
            },
        );
    }
    if successor_previous_digest != Some(current_digest) {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PredecessorDigestMismatch);
    }
    Ok(())
}

/// Validate an active-to-active trust-store rotation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, terminal successors, no-op rotations, and a
/// trust-store digest or CA-membership root changed without the other.
pub fn validate_zk_x509_trust_anchor_rotation_v1(
    current: &PrivacyZkX509TrustAnchorRecordV1,
    successor: &PrivacyZkX509TrustAnchorRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    let trust_store_changed = successor.trust_store_digest != current.trust_store_digest;
    let ca_membership_root_changed = successor.ca_membership_root != current.ca_membership_root;
    match (trust_store_changed, ca_membership_root_changed) {
        (false, false) => {
            return Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged);
        }
        (true, false) => {
            return Err(
                PrivacyZkX509TransitionValidationErrorV1::TrustStoreDigestChangedWithoutCaMembershipRoot,
            );
        }
        (false, true) => {
            return Err(
                PrivacyZkX509TransitionValidationErrorV1::CaMembershipRootChangedWithoutTrustStoreDigest,
            );
        }
        (true, true) => {}
    }
    Ok(())
}

/// Validate an irreversible trust-store revocation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, nonterminal successors, or trust-store/root
/// changes.
pub fn validate_zk_x509_trust_anchor_revocation_v1(
    current: &PrivacyZkX509TrustAnchorRecordV1,
    successor: &PrivacyZkX509TrustAnchorRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Revoked {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.trust_store_digest != current.trust_store_digest
        || successor.ca_membership_root != current.ca_membership_root
        || successor.ca_membership_root_epoch != current.ca_membership_root_epoch
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// Validate an active-to-active certificate-policy rotation.
///
/// # Errors
///
/// Rejects malformed records, namespace changes, stale/skipped epochs,
/// predecessor substitution, terminal successors, and no-op rotations.
pub fn validate_zk_x509_certificate_policy_rotation_v1(
    current: &PrivacyZkX509CertificatePolicyRecordV1,
    successor: &PrivacyZkX509CertificatePolicyRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.policy_digest == current.policy_digest
        && successor.required_key_usage == current.required_key_usage
        && successor.required_extended_key_usages == current.required_extended_key_usages
        && successor.required_disclosed_attribute_indices
            == current.required_disclosed_attribute_indices
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged);
    }
    Ok(())
}

/// Validate an irreversible certificate-policy revocation.
///
/// # Errors
///
/// Rejects malformed records, namespace changes, stale/skipped epochs,
/// predecessor substitution, nonterminal successors, or policy changes.
pub fn validate_zk_x509_certificate_policy_revocation_v1(
    current: &PrivacyZkX509CertificatePolicyRecordV1,
    successor: &PrivacyZkX509CertificatePolicyRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Revoked {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.policy_digest != current.policy_digest
        || successor.required_key_usage != current.required_key_usage
        || successor.required_extended_key_usages != current.required_extended_key_usages
        || successor.required_disclosed_attribute_indices
            != current.required_disclosed_attribute_indices
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// Validate an active-to-active signed-CRL rotation.
///
/// # Errors
///
/// Rejects malformed records, namespace or issuer-key changes,
/// stale/skipped epochs, predecessor substitution, non-increasing
/// `thisUpdate`, terminal successors, and no-op rotations.
pub fn validate_zk_x509_crl_rotation_v1(
    current: &PrivacyZkX509CrlRecordV1,
    successor: &PrivacyZkX509CrlRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.certificate_policy_id != current.certificate_policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    if successor.issuer_spki_digest != current.issuer_spki_digest {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlIssuerSpkiDigestMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.this_update_unix_seconds <= current.this_update_unix_seconds {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlThisUpdateNotIncreasing);
    }
    if successor.crl_number <= current.crl_number {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlNumberNotIncreasing);
    }
    if successor.crl_der_digest == current.crl_der_digest {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged);
    }
    Ok(())
}

/// Validate an irreversible signed-CRL lineage revocation.
///
/// # Errors
///
/// Rejects malformed records, namespace or issuer-key changes,
/// stale/skipped epochs, predecessor substitution, nonterminal successors,
/// or any change to the signed CRL or validity window.
pub fn validate_zk_x509_crl_revocation_v1(
    current: &PrivacyZkX509CrlRecordV1,
    successor: &PrivacyZkX509CrlRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.certificate_policy_id != current.certificate_policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    if successor.issuer_spki_digest != current.issuer_spki_digest {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlIssuerSpkiDigestMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Revoked {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.crl_number != current.crl_number
        || successor.crl_der_digest != current.crl_der_digest
        || successor.this_update_unix_seconds != current.this_update_unix_seconds
        || successor.next_update_unix_seconds != current.next_update_unix_seconds
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}
