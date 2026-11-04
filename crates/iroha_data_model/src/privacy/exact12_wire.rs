// Frozen Exact12 instruction payload grammar and explicit runtime conversions.

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(tag = "protocol", content = "value", deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::privacy::PrivacyProtocolIdV1")]
pub enum PrivacyProtocolIdV1 {
    /// Native ZK-ACE post-quantum authorization protocol v1.
    #[norito(rename = "zk-ace-pq-authorization-v1")]
    ZkAcePqAuthorizationV1,
    /// Anonymous PGC k-out-of-n payment protocol v1.
    #[norito(rename = "anonymous-pgc-k-out-of-n-v1")]
    AnonymousPgcKOutOfNV1,
    /// `VeRange` transparent range-proof protocol v1.
    #[norito(rename = "verange-transparent-range-v1")]
    VeRangeTransparentRangeV1,
    /// Native Iroha ZK-AMS admission and anonymous-account provisioning suite v1.
    #[norito(rename = "iroha-zk-ams-v1")]
    IrohaZkAmsV1,
    /// Vega proof over an existing credential v1.
    #[norito(rename = "vega-existing-credential-zk-v1")]
    VegaExistingCredentialZkV1,
    /// Native Iroha P-256 X.509 predicate STARK protocol v1.
    #[norito(rename = "iroha-zk-x509-stark-p256-v1")]
    IrohaZkX509StarkP256V1,
    /// Native Iroha Jindo batched univariate lattice polynomial-commitment protocol v1.
    #[norito(rename = "iroha-jindo-polynomial-commitment-v1")]
    IrohaJindoPolynomialCommitmentV1,
    /// Native Bootle Lantern/LNP22 module-lattice anonymous credential v1.
    #[norito(rename = "iroha-bootle-lantern-anoncred-v1")]
    IrohaBootleLanternAnoncredV1,
    /// Orchard Halo2 action protocol v1.
    #[norito(rename = "orchard-halo2-actions-v1")]
    OrchardHalo2ActionsV1,
    /// Monero FCMP++ full-chain membership protocol v1.
    #[norito(rename = "monero-fcmp-plus-plus-v1")]
    MoneroFcmpPlusPlusV1,
    /// Native IVM private-note STARK protocol v1.
    #[norito(rename = "iroha-ivm-private-note-stark-v1")]
    IrohaIvmPrivateNoteStarkV1,
    /// Post-quantum MASP STARK protocol v1.
    #[norito(rename = "pq-masp-stark-v1")]
    PqMaspStarkV1,
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(tag = "protocol", content = "limits", deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::privacy::PrivacyProtocolLimitsV1")]
pub enum PrivacyProtocolLimitsV1 {
    /// ZK-ACE has no additional first-release count limits.
    #[norito(rename = "zk-ace-pq-authorization-v1")]
    ZkAcePqAuthorizationV1,
    /// Anonymous PGC receiver policy.
    #[norito(rename = "anonymous-pgc-k-out-of-n-v1")]
    AnonymousPgcKOutOfNV1(AnonymousPgcLimitsV1),
    /// `VeRange` aggregation policy.
    #[norito(rename = "verange-transparent-range-v1")]
    VeRangeTransparentRangeV1(VeRangeLimitsV1),
    /// ZK-AMS batch-admission and account-provisioning policy.
    #[norito(rename = "iroha-zk-ams-v1")]
    IrohaZkAmsV1(ZkAmsLimitsV1),
    /// Vega has no additional first-release count limits.
    #[norito(rename = "vega-existing-credential-zk-v1")]
    VegaExistingCredentialZkV1,
    /// X.509 has fixed first-release limits encoded by its statement validator.
    #[norito(rename = "iroha-zk-x509-stark-p256-v1")]
    IrohaZkX509StarkP256V1,
    /// Jindo batched opening policy.
    #[norito(rename = "iroha-jindo-polynomial-commitment-v1")]
    IrohaJindoPolynomialCommitmentV1(JindoLimitsV1),
    /// Lantern anonymous credentials have a fixed first-release parameter profile.
    #[norito(rename = "iroha-bootle-lantern-anoncred-v1")]
    IrohaBootleLanternAnoncredV1,
    /// Orchard one-to-one action policy.
    #[norito(rename = "orchard-halo2-actions-v1")]
    OrchardHalo2ActionsV1(OrchardLimitsV1),
    /// FCMP++ input/output policy.
    #[norito(rename = "monero-fcmp-plus-plus-v1")]
    MoneroFcmpPlusPlusV1(FcmpLimitsV1),
    /// Native private-note input/output policy.
    #[norito(rename = "iroha-ivm-private-note-stark-v1")]
    IrohaIvmPrivateNoteStarkV1(IvmPrivateNoteLimitsV1),
    /// PQ-MASP input/output policy.
    #[norito(rename = "pq-masp-stark-v1")]
    PqMaspStarkV1(PqMaspLimitsV1),
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(tag = "protocol", content = "statement", deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::privacy::PrivacyStatementV1",
    frame = "iroha.privacy.statement.v1"
)]
pub enum PrivacyStatementV1 {
    /// ZK-ACE post-quantum authorization statement.
    ZkAcePqAuthorizationV1(ZkAcePqAuthorizationStatementV1),
    /// Anonymous PGC k-out-of-n payment statement.
    AnonymousPgcKOutOfNV1(AnonymousPgcKOutOfNStatementV1),
    /// `VeRange` transparent range statement.
    VeRangeTransparentRangeV1(VeRangeTransparentRangeStatementV1),
    /// Native Iroha ZK-AMS admission/provisioning statement.
    IrohaZkAmsV1(IrohaZkAmsStatementV1),
    /// Vega existing-credential predicate statement.
    VegaExistingCredentialZkV1(VegaExistingCredentialStatementV1),
    /// Native Iroha P-256 X.509 predicate STARK statement.
    IrohaZkX509StarkP256V1(IrohaZkX509StarkP256StatementV1),
    /// Native Iroha Jindo batched univariate lattice polynomial-commitment statement.
    IrohaJindoPolynomialCommitmentV1(IrohaJindoPolynomialCommitmentStatementV1),
    /// Native Bootle Lantern/LNP22 anonymous-credential statement.
    IrohaBootleLanternAnoncredV1(IrohaBootleLanternAnoncredStatementV1),
    /// Orchard Halo2 action statement.
    OrchardHalo2ActionsV1(OrchardHalo2ActionsStatementV1),
    /// Monero FCMP++ membership statement.
    MoneroFcmpPlusPlusV1(MoneroFcmpPlusPlusStatementV1),
    /// Native IVM private-note STARK statement.
    IrohaIvmPrivateNoteStarkV1(IrohaIvmPrivateNoteStarkStatementV1),
    /// Post-quantum MASP STARK statement.
    PqMaspStarkV1(PqMaspStarkStatementV1),
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(tag = "protocol", content = "proof", deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::privacy::PrivacyProofV1",
    frame = "iroha.privacy.proof.v1"
)]
pub enum PrivacyProofV1 {
    /// ZK-ACE post-quantum authorization proof.
    ZkAcePqAuthorizationV1(PrivacyProofBytesV1),
    /// Anonymous PGC k-out-of-n payment proof.
    AnonymousPgcKOutOfNV1(PrivacyProofBytesV1),
    /// `VeRange` transparent range proof.
    VeRangeTransparentRangeV1(PrivacyProofBytesV1),
    /// Native Iroha ZK-AMS admission or provisioning proof.
    IrohaZkAmsV1(IrohaZkAmsProofV1),
    /// Vega existing-credential predicate proof.
    VegaExistingCredentialZkV1(PrivacyProofBytesV1),
    /// Native Iroha P-256 X.509 predicate STARK proof.
    IrohaZkX509StarkP256V1(PrivacyProofBytesV1),
    /// Native Iroha Jindo batched univariate lattice polynomial-commitment proof.
    IrohaJindoPolynomialCommitmentV1(PrivacyProofBytesV1),
    /// Native Bootle Lantern/LNP22 anonymous-credential proof.
    IrohaBootleLanternAnoncredV1(PrivacyProofBytesV1),
    /// Orchard Halo2 action proof.
    OrchardHalo2ActionsV1(PrivacyProofBytesV1),
    /// Monero FCMP++ membership proof.
    MoneroFcmpPlusPlusV1(PrivacyProofBytesV1),
    /// Native IVM private-note STARK proof.
    IrohaIvmPrivateNoteStarkV1(PrivacyProofBytesV1),
    /// Post-quantum MASP STARK proof.
    PqMaspStarkV1(PrivacyProofBytesV1),
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::privacy::PrivacyProofEnvelopeV1",
    frame = "iroha.privacy.proof-envelope.v1"
)]
pub struct PrivacyProofEnvelopeV1 {
    /// Mandatory first-release wire marker; invalid markers fail decoding.
    pub wire_magic: PrivacyProofWireMagicV1,
    /// Commitment to the immutable base Exact12 catalog and security models.
    ///
    /// Additive protocols bind their separate protocol and profile identities without changing
    /// this deployed field; the base commitment does not activate an extension.
    pub catalog_commitment: PrivacyExact12CatalogCommitmentV1,
    /// Exact protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact proof-system profile.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Exact native verifier engine.
    pub engine_id: PrivacyEngineIdV1,
    /// Governed public-parameter set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Governed public-parameter digest.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Governed verifier artifact digest.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Governed public-statement schema digest.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Governed native engine-manifest digest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Digest of the complete protocol-tagged statement.
    pub statement_digest: PrivacyStatementDigestV1,
    /// Protocol-typed public statement.
    pub statement: PrivacyStatementV1,
    /// Protocol-typed proof bytes.
    pub proof: PrivacyProofV1,
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::privacy::PrivacyNamespaceV1")]
pub struct PrivacyNamespaceV1 {
    protocol_id: PrivacyProtocolIdV1,
    scope: PrivacyNamespaceScopeV1,
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::privacy::PrivacyRootPublicationV1")]
pub struct PrivacyRootPublicationV1 {
    /// Exact protocol-scoped root namespace.
    pub namespace: PrivacyNamespaceV1,
    /// Semantic root role inside the namespace.
    pub role: PrivacyRootRoleV1,
    /// Monotonically advancing root epoch.
    pub epoch: u64,
    /// Published canonical root.
    pub root: PrivacyRootV1,
}

/// Frozen baseline wire representation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::privacy::PrivacyPgcAccountBootstrapV1")]
pub struct PrivacyPgcAccountBootstrapV1 {
    /// Exact Anonymous PGC pool namespace.
    pub namespace: PrivacyNamespaceV1,
    /// Declared root, which core must recompute from `accounts`.
    pub initial_root: PrivacyRootV1,
    /// Canonical initial account-state epoch (exactly [`PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1`]).
    pub initial_epoch: u64,
    /// Exact public aggregate supply encrypted across the initial accounts.
    pub total_supply: u32,
    /// Complete account table in strict public-key order.
    pub accounts: Vec<PrivacyPgcAccountV1>,
}

/// A successor-only value cannot be represented by a frozen Exact12 payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("successor privacy value is not in the frozen Exact12 wire grammar")]
pub struct Exact12PrivacyWireError;

impl From<PrivacyProtocolIdV1> for PrivacyRuntimeProtocolIdV1 {
    fn from(value: PrivacyProtocolIdV1) -> Self {
        match value {
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV1 => {
                PrivacyRuntimeProtocolIdV1::ZkAcePqAuthorizationV1
            }
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
                PrivacyRuntimeProtocolIdV1::AnonymousPgcKOutOfNV1
            }
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
                PrivacyRuntimeProtocolIdV1::VeRangeTransparentRangeV1
            }
            PrivacyProtocolIdV1::IrohaZkAmsV1 => PrivacyRuntimeProtocolIdV1::IrohaZkAmsV1,
            PrivacyProtocolIdV1::VegaExistingCredentialZkV1 => {
                PrivacyRuntimeProtocolIdV1::VegaExistingCredentialZkV1
            }
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 => {
                PrivacyRuntimeProtocolIdV1::IrohaZkX509StarkP256V1
            }
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1 => {
                PrivacyRuntimeProtocolIdV1::IrohaJindoPolynomialCommitmentV1
            }
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyRuntimeProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
                PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1
            }
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                PrivacyRuntimeProtocolIdV1::MoneroFcmpPlusPlusV1
            }
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyRuntimeProtocolIdV1::IrohaIvmPrivateNoteStarkV1
            }
            PrivacyProtocolIdV1::PqMaspStarkV1 => PrivacyRuntimeProtocolIdV1::PqMaspStarkV1,
        }
    }
}
impl TryFrom<PrivacyRuntimeProtocolIdV1> for PrivacyProtocolIdV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeProtocolIdV1) -> Result<Self, Self::Error> {
        Ok(match value {
            PrivacyRuntimeProtocolIdV1::ZkAcePqAuthorizationV1 => {
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV1
            }
            PrivacyRuntimeProtocolIdV1::AnonymousPgcKOutOfNV1 => {
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
            }
            PrivacyRuntimeProtocolIdV1::VeRangeTransparentRangeV1 => {
                PrivacyProtocolIdV1::VeRangeTransparentRangeV1
            }
            PrivacyRuntimeProtocolIdV1::IrohaZkAmsV1 => PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyRuntimeProtocolIdV1::VegaExistingCredentialZkV1 => {
                PrivacyProtocolIdV1::VegaExistingCredentialZkV1
            }
            PrivacyRuntimeProtocolIdV1::IrohaZkX509StarkP256V1 => {
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V1
            }
            PrivacyRuntimeProtocolIdV1::IrohaJindoPolynomialCommitmentV1 => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1
            }
            PrivacyRuntimeProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1 => {
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1
            }
            PrivacyRuntimeProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
            }
            PrivacyRuntimeProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
            }
            PrivacyRuntimeProtocolIdV1::PqMaspStarkV1 => PrivacyProtocolIdV1::PqMaspStarkV1,
            PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 => {
                return Err(Exact12PrivacyWireError);
            }
        })
    }
}

impl From<PrivacyProtocolLimitsV1> for PrivacyRuntimeProtocolLimitsV1 {
    fn from(value: PrivacyProtocolLimitsV1) -> Self {
        match value {
            PrivacyProtocolLimitsV1::ZkAcePqAuthorizationV1 => {
                PrivacyRuntimeProtocolLimitsV1::ZkAcePqAuthorizationV1
            }
            PrivacyProtocolLimitsV1::AnonymousPgcKOutOfNV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::AnonymousPgcKOutOfNV1(value)
            }
            PrivacyProtocolLimitsV1::VeRangeTransparentRangeV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::VeRangeTransparentRangeV1(value)
            }
            PrivacyProtocolLimitsV1::IrohaZkAmsV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::IrohaZkAmsV1(value)
            }
            PrivacyProtocolLimitsV1::VegaExistingCredentialZkV1 => {
                PrivacyRuntimeProtocolLimitsV1::VegaExistingCredentialZkV1
            }
            PrivacyProtocolLimitsV1::IrohaZkX509StarkP256V1 => {
                PrivacyRuntimeProtocolLimitsV1::IrohaZkX509StarkP256V1
            }
            PrivacyProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(value)
            }
            PrivacyProtocolLimitsV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyRuntimeProtocolLimitsV1::IrohaBootleLanternAnoncredV1
            }
            PrivacyProtocolLimitsV1::OrchardHalo2ActionsV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::OrchardHalo2ActionsV1(value)
            }
            PrivacyProtocolLimitsV1::MoneroFcmpPlusPlusV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::MoneroFcmpPlusPlusV1(value)
            }
            PrivacyProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(value)
            }
            PrivacyProtocolLimitsV1::PqMaspStarkV1(value) => {
                PrivacyRuntimeProtocolLimitsV1::PqMaspStarkV1(value)
            }
        }
    }
}
impl TryFrom<PrivacyRuntimeProtocolLimitsV1> for PrivacyProtocolLimitsV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeProtocolLimitsV1) -> Result<Self, Self::Error> {
        Ok(match value {
            PrivacyRuntimeProtocolLimitsV1::ZkAcePqAuthorizationV1 => {
                PrivacyProtocolLimitsV1::ZkAcePqAuthorizationV1
            }
            PrivacyRuntimeProtocolLimitsV1::AnonymousPgcKOutOfNV1(value) => {
                PrivacyProtocolLimitsV1::AnonymousPgcKOutOfNV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::VeRangeTransparentRangeV1(value) => {
                PrivacyProtocolLimitsV1::VeRangeTransparentRangeV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::IrohaZkAmsV1(value) => {
                PrivacyProtocolLimitsV1::IrohaZkAmsV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::VegaExistingCredentialZkV1 => {
                PrivacyProtocolLimitsV1::VegaExistingCredentialZkV1
            }
            PrivacyRuntimeProtocolLimitsV1::IrohaZkX509StarkP256V1 => {
                PrivacyProtocolLimitsV1::IrohaZkX509StarkP256V1
            }
            PrivacyRuntimeProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(value) => {
                PrivacyProtocolLimitsV1::IrohaJindoPolynomialCommitmentV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyProtocolLimitsV1::IrohaBootleLanternAnoncredV1
            }
            PrivacyRuntimeProtocolLimitsV1::OrchardHalo2ActionsV1(value) => {
                PrivacyProtocolLimitsV1::OrchardHalo2ActionsV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::MoneroFcmpPlusPlusV1(value) => {
                PrivacyProtocolLimitsV1::MoneroFcmpPlusPlusV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(value) => {
                PrivacyProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::PqMaspStarkV1(value) => {
                PrivacyProtocolLimitsV1::PqMaspStarkV1(value)
            }
            PrivacyRuntimeProtocolLimitsV1::IronwoodHalo2ActionsV1(_) => {
                return Err(Exact12PrivacyWireError);
            }
        })
    }
}

impl From<PrivacyStatementV1> for PrivacyRuntimeStatementV1 {
    fn from(value: PrivacyStatementV1) -> Self {
        match value {
            PrivacyStatementV1::ZkAcePqAuthorizationV1(value) => {
                PrivacyRuntimeStatementV1::ZkAcePqAuthorizationV1(value)
            }
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(value) => {
                PrivacyRuntimeStatementV1::AnonymousPgcKOutOfNV1(value)
            }
            PrivacyStatementV1::VeRangeTransparentRangeV1(value) => {
                PrivacyRuntimeStatementV1::VeRangeTransparentRangeV1(value)
            }
            PrivacyStatementV1::IrohaZkAmsV1(value) => {
                PrivacyRuntimeStatementV1::IrohaZkAmsV1(value)
            }
            PrivacyStatementV1::VegaExistingCredentialZkV1(value) => {
                PrivacyRuntimeStatementV1::VegaExistingCredentialZkV1(value)
            }
            PrivacyStatementV1::IrohaZkX509StarkP256V1(value) => {
                PrivacyRuntimeStatementV1::IrohaZkX509StarkP256V1(value)
            }
            PrivacyStatementV1::IrohaJindoPolynomialCommitmentV1(value) => {
                PrivacyRuntimeStatementV1::IrohaJindoPolynomialCommitmentV1(value)
            }
            PrivacyStatementV1::IrohaBootleLanternAnoncredV1(value) => {
                PrivacyRuntimeStatementV1::IrohaBootleLanternAnoncredV1(value)
            }
            PrivacyStatementV1::OrchardHalo2ActionsV1(value) => {
                PrivacyRuntimeStatementV1::OrchardHalo2ActionsV1(value)
            }
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(value) => {
                PrivacyRuntimeStatementV1::MoneroFcmpPlusPlusV1(value)
            }
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(value) => {
                PrivacyRuntimeStatementV1::IrohaIvmPrivateNoteStarkV1(value)
            }
            PrivacyStatementV1::PqMaspStarkV1(value) => {
                PrivacyRuntimeStatementV1::PqMaspStarkV1(value)
            }
        }
    }
}
impl TryFrom<PrivacyRuntimeStatementV1> for PrivacyStatementV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeStatementV1) -> Result<Self, Self::Error> {
        Ok(match value {
            PrivacyRuntimeStatementV1::ZkAcePqAuthorizationV1(value) => {
                PrivacyStatementV1::ZkAcePqAuthorizationV1(value)
            }
            PrivacyRuntimeStatementV1::AnonymousPgcKOutOfNV1(value) => {
                PrivacyStatementV1::AnonymousPgcKOutOfNV1(value)
            }
            PrivacyRuntimeStatementV1::VeRangeTransparentRangeV1(value) => {
                PrivacyStatementV1::VeRangeTransparentRangeV1(value)
            }
            PrivacyRuntimeStatementV1::IrohaZkAmsV1(value) => {
                PrivacyStatementV1::IrohaZkAmsV1(value)
            }
            PrivacyRuntimeStatementV1::VegaExistingCredentialZkV1(value) => {
                PrivacyStatementV1::VegaExistingCredentialZkV1(value)
            }
            PrivacyRuntimeStatementV1::IrohaZkX509StarkP256V1(value) => {
                PrivacyStatementV1::IrohaZkX509StarkP256V1(value)
            }
            PrivacyRuntimeStatementV1::IrohaJindoPolynomialCommitmentV1(value) => {
                PrivacyStatementV1::IrohaJindoPolynomialCommitmentV1(value)
            }
            PrivacyRuntimeStatementV1::IrohaBootleLanternAnoncredV1(value) => {
                PrivacyStatementV1::IrohaBootleLanternAnoncredV1(value)
            }
            PrivacyRuntimeStatementV1::OrchardHalo2ActionsV1(value) => {
                PrivacyStatementV1::OrchardHalo2ActionsV1(value)
            }
            PrivacyRuntimeStatementV1::MoneroFcmpPlusPlusV1(value) => {
                PrivacyStatementV1::MoneroFcmpPlusPlusV1(value)
            }
            PrivacyRuntimeStatementV1::IrohaIvmPrivateNoteStarkV1(value) => {
                PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(value)
            }
            PrivacyRuntimeStatementV1::PqMaspStarkV1(value) => {
                PrivacyStatementV1::PqMaspStarkV1(value)
            }
            PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(_) => {
                return Err(Exact12PrivacyWireError);
            }
        })
    }
}

impl From<PrivacyProofV1> for PrivacyRuntimeProofV1 {
    fn from(value: PrivacyProofV1) -> Self {
        match value {
            PrivacyProofV1::ZkAcePqAuthorizationV1(value) => {
                PrivacyRuntimeProofV1::ZkAcePqAuthorizationV1(value)
            }
            PrivacyProofV1::AnonymousPgcKOutOfNV1(value) => {
                PrivacyRuntimeProofV1::AnonymousPgcKOutOfNV1(value)
            }
            PrivacyProofV1::VeRangeTransparentRangeV1(value) => {
                PrivacyRuntimeProofV1::VeRangeTransparentRangeV1(value)
            }
            PrivacyProofV1::IrohaZkAmsV1(value) => PrivacyRuntimeProofV1::IrohaZkAmsV1(value),
            PrivacyProofV1::VegaExistingCredentialZkV1(value) => {
                PrivacyRuntimeProofV1::VegaExistingCredentialZkV1(value)
            }
            PrivacyProofV1::IrohaZkX509StarkP256V1(value) => {
                PrivacyRuntimeProofV1::IrohaZkX509StarkP256V1(value)
            }
            PrivacyProofV1::IrohaJindoPolynomialCommitmentV1(value) => {
                PrivacyRuntimeProofV1::IrohaJindoPolynomialCommitmentV1(value)
            }
            PrivacyProofV1::IrohaBootleLanternAnoncredV1(value) => {
                PrivacyRuntimeProofV1::IrohaBootleLanternAnoncredV1(value)
            }
            PrivacyProofV1::OrchardHalo2ActionsV1(value) => {
                PrivacyRuntimeProofV1::OrchardHalo2ActionsV1(value)
            }
            PrivacyProofV1::MoneroFcmpPlusPlusV1(value) => {
                PrivacyRuntimeProofV1::MoneroFcmpPlusPlusV1(value)
            }
            PrivacyProofV1::IrohaIvmPrivateNoteStarkV1(value) => {
                PrivacyRuntimeProofV1::IrohaIvmPrivateNoteStarkV1(value)
            }
            PrivacyProofV1::PqMaspStarkV1(value) => PrivacyRuntimeProofV1::PqMaspStarkV1(value),
        }
    }
}
impl TryFrom<PrivacyRuntimeProofV1> for PrivacyProofV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeProofV1) -> Result<Self, Self::Error> {
        Ok(match value {
            PrivacyRuntimeProofV1::ZkAcePqAuthorizationV1(value) => {
                PrivacyProofV1::ZkAcePqAuthorizationV1(value)
            }
            PrivacyRuntimeProofV1::AnonymousPgcKOutOfNV1(value) => {
                PrivacyProofV1::AnonymousPgcKOutOfNV1(value)
            }
            PrivacyRuntimeProofV1::VeRangeTransparentRangeV1(value) => {
                PrivacyProofV1::VeRangeTransparentRangeV1(value)
            }
            PrivacyRuntimeProofV1::IrohaZkAmsV1(value) => PrivacyProofV1::IrohaZkAmsV1(value),
            PrivacyRuntimeProofV1::VegaExistingCredentialZkV1(value) => {
                PrivacyProofV1::VegaExistingCredentialZkV1(value)
            }
            PrivacyRuntimeProofV1::IrohaZkX509StarkP256V1(value) => {
                PrivacyProofV1::IrohaZkX509StarkP256V1(value)
            }
            PrivacyRuntimeProofV1::IrohaJindoPolynomialCommitmentV1(value) => {
                PrivacyProofV1::IrohaJindoPolynomialCommitmentV1(value)
            }
            PrivacyRuntimeProofV1::IrohaBootleLanternAnoncredV1(value) => {
                PrivacyProofV1::IrohaBootleLanternAnoncredV1(value)
            }
            PrivacyRuntimeProofV1::OrchardHalo2ActionsV1(value) => {
                PrivacyProofV1::OrchardHalo2ActionsV1(value)
            }
            PrivacyRuntimeProofV1::MoneroFcmpPlusPlusV1(value) => {
                PrivacyProofV1::MoneroFcmpPlusPlusV1(value)
            }
            PrivacyRuntimeProofV1::IrohaIvmPrivateNoteStarkV1(value) => {
                PrivacyProofV1::IrohaIvmPrivateNoteStarkV1(value)
            }
            PrivacyRuntimeProofV1::PqMaspStarkV1(value) => PrivacyProofV1::PqMaspStarkV1(value),
            PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(_) => {
                return Err(Exact12PrivacyWireError);
            }
        })
    }
}

impl From<PrivacyProofEnvelopeV1> for PrivacyRuntimeProofEnvelopeV1 {
    fn from(value: PrivacyProofEnvelopeV1) -> Self {
        Self {
            wire_magic: value.wire_magic,
            catalog_commitment: value.catalog_commitment,
            protocol_id: value.protocol_id.into(),
            proof_system_id: value.proof_system_id,
            engine_id: value.engine_id,
            parameter_id: value.parameter_id,
            parameter_digest: value.parameter_digest,
            verifier_digest: value.verifier_digest,
            statement_schema_digest: value.statement_schema_digest,
            engine_manifest_digest: value.engine_manifest_digest,
            statement_digest: value.statement_digest,
            statement: value.statement.into(),
            proof: value.proof.into(),
        }
    }
}
impl TryFrom<PrivacyRuntimeProofEnvelopeV1> for PrivacyProofEnvelopeV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeProofEnvelopeV1) -> Result<Self, Self::Error> {
        Ok(Self {
            wire_magic: value.wire_magic,
            catalog_commitment: value.catalog_commitment,
            protocol_id: value.protocol_id.try_into()?,
            proof_system_id: value.proof_system_id,
            engine_id: value.engine_id,
            parameter_id: value.parameter_id,
            parameter_digest: value.parameter_digest,
            verifier_digest: value.verifier_digest,
            statement_schema_digest: value.statement_schema_digest,
            engine_manifest_digest: value.engine_manifest_digest,
            statement_digest: value.statement_digest,
            statement: value.statement.try_into()?,
            proof: value.proof.try_into()?,
        })
    }
}

impl From<PrivacyNamespaceV1> for PrivacyRuntimeNamespaceV1 {
    fn from(value: PrivacyNamespaceV1) -> Self {
        Self {
            protocol_id: value.protocol_id.into(),
            scope: value.scope,
        }
    }
}
impl TryFrom<PrivacyRuntimeNamespaceV1> for PrivacyNamespaceV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeNamespaceV1) -> Result<Self, Self::Error> {
        Ok(Self {
            protocol_id: value.protocol_id.try_into()?,
            scope: value.scope,
        })
    }
}

impl From<PrivacyRootPublicationV1> for PrivacyRuntimeRootPublicationV1 {
    fn from(value: PrivacyRootPublicationV1) -> Self {
        Self {
            namespace: value.namespace.into(),
            role: value.role,
            epoch: value.epoch,
            root: value.root,
        }
    }
}
impl TryFrom<PrivacyRuntimeRootPublicationV1> for PrivacyRootPublicationV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeRootPublicationV1) -> Result<Self, Self::Error> {
        Ok(Self {
            namespace: value.namespace.try_into()?,
            role: value.role,
            epoch: value.epoch,
            root: value.root,
        })
    }
}

impl From<PrivacyPgcAccountBootstrapV1> for PrivacyRuntimePgcAccountBootstrapV1 {
    fn from(value: PrivacyPgcAccountBootstrapV1) -> Self {
        Self {
            namespace: value.namespace.into(),
            initial_root: value.initial_root,
            initial_epoch: value.initial_epoch,
            total_supply: value.total_supply,
            accounts: value.accounts,
        }
    }
}
impl TryFrom<PrivacyRuntimePgcAccountBootstrapV1> for PrivacyPgcAccountBootstrapV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimePgcAccountBootstrapV1) -> Result<Self, Self::Error> {
        Ok(Self {
            namespace: value.namespace.try_into()?,
            initial_root: value.initial_root,
            initial_epoch: value.initial_epoch,
            total_supply: value.total_supply,
            accounts: value.accounts,
        })
    }
}

impl PrivacyProtocolIdV1 {
    /// The immutable twelve-protocol roster.
    pub const COUNT: usize = 12;
    /// Every frozen baseline protocol in wire order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::ZkAcePqAuthorizationV1,
        Self::AnonymousPgcKOutOfNV1,
        Self::VeRangeTransparentRangeV1,
        Self::IrohaZkAmsV1,
        Self::VegaExistingCredentialZkV1,
        Self::IrohaZkX509StarkP256V1,
        Self::IrohaJindoPolynomialCommitmentV1,
        Self::IrohaBootleLanternAnoncredV1,
        Self::OrchardHalo2ActionsV1,
        Self::MoneroFcmpPlusPlusV1,
        Self::IrohaIvmPrivateNoteStarkV1,
        Self::PqMaspStarkV1,
    ];
    /// Stable external protocol name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        PrivacyRuntimeProtocolIdV1::from(self).canonical_label()
    }
}
impl PrivacyNamespaceV1 {
    /// Construct an original baseline namespace.
    #[must_use]
    pub const fn new(protocol_id: PrivacyProtocolIdV1, scope: PrivacyNamespaceScopeV1) -> Self {
        Self { protocol_id, scope }
    }
    /// Original protocol selection.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }
    /// Original namespace scope.
    #[must_use]
    pub const fn scope(&self) -> PrivacyNamespaceScopeV1 {
        self.scope
    }
    /// Validate the original closed namespace.
    /// # Errors
    /// The scope is invalid for its baseline protocol.
    pub fn validate(&self) -> Result<(), PrivacyNamespaceValidationError> {
        PrivacyRuntimeNamespaceV1::from(*self).validate()
    }
}
impl PrivacyStatementV1 {
    /// Shared statement context without modifying its original wire representation.
    #[must_use]
    pub fn context(&self) -> &PrivacyStatementContextV1 {
        match self {
            Self::ZkAcePqAuthorizationV1(statement) => &statement.context,
            Self::AnonymousPgcKOutOfNV1(statement) => &statement.context,
            Self::VeRangeTransparentRangeV1(statement) => &statement.context,
            Self::IrohaZkAmsV1(statement) => &statement.context,
            Self::VegaExistingCredentialZkV1(statement) => &statement.context,
            Self::IrohaZkX509StarkP256V1(statement) => &statement.context,
            Self::IrohaJindoPolynomialCommitmentV1(statement) => &statement.context,
            Self::IrohaBootleLanternAnoncredV1(statement) => &statement.context,
            Self::OrchardHalo2ActionsV1(statement) => &statement.context,
            Self::MoneroFcmpPlusPlusV1(statement) => &statement.context,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => &statement.context,
            Self::PqMaspStarkV1(statement) => &statement.context,
        }
    }
    /// Hash the exact original canonical statement frame.
    /// # Errors
    /// Canonical encoding cannot be produced.
    pub fn digest(&self) -> Result<PrivacyStatementDigestV1, norito::Error> {
        privacy_wire_statement_digest(&norito::encode_canonical(self)?)
    }
    /// Validate the closed statement and shared limits.
    /// # Errors
    /// Original intrinsic validation fails.
    pub fn validate(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyStatementValidationError> {
        PrivacyRuntimeStatementV1::from(self.clone()).validate(limits)
    }
}
fn privacy_wire_statement_digest(
    encoded: &[u8],
) -> Result<PrivacyStatementDigestV1, norito::Error> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_STATEMENT_DIGEST_DOMAIN_V1);
    hasher.update(
        &u64::try_from(encoded.len())
            .map_err(|_| norito::Error::Message("statement size overflow".into()))?
            .to_le_bytes(),
    );
    hasher.update(encoded);
    Ok(PrivacyStatementDigestV1::new(*hasher.finalize().as_bytes()))
}
impl PrivacyProofEnvelopeV1 {
    /// Bind only an original Exact12 statement/proof pair.
    /// # Errors
    /// The statement and proof are inconsistent or cannot be encoded.
    pub fn bind_v1(
        statement: PrivacyStatementV1,
        proof: PrivacyProofV1,
    ) -> Result<Self, PrivacyProofEnvelopeValidationError> {
        PrivacyRuntimeProofEnvelopeV1::bind_v1(statement.into(), proof.into())?
            .try_into()
            .map_err(|_| PrivacyProofEnvelopeValidationError::EncodingFailure)
    }
    /// Validate the original envelope without changing its signed bytes.
    /// # Errors
    /// Original intrinsic validation fails.
    pub fn validate_with_limits(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyProofEnvelopeValidationError> {
        PrivacyRuntimeProofEnvelopeV1::from(self.clone()).validate_with_limits(limits)
    }
}
impl PrivacyRootPublicationV1 {
    /// Validate original root-publication semantics.
    /// # Errors
    /// Its exact original namespace, role, epoch or root is invalid.
    pub fn validate(&self) -> Result<(), PrivacyRootPublicationValidationError> {
        PrivacyRuntimeRootPublicationV1::from(*self).validate()
    }
}
impl PrivacyPgcAccountBootstrapV1 {
    /// Validate original PGC bootstrap semantics.
    /// # Errors
    /// Its exact original namespace, root or account set is invalid.
    pub fn validate(&self) -> Result<(), PrivacyPgcAccountBootstrapValidationError> {
        PrivacyRuntimePgcAccountBootstrapV1::from(self.clone()).validate()
    }
}

impl PrivacyProofV1 {
    /// Access the frozen proof payload.
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV1(_) => PrivacyProtocolIdV1::ZkAcePqAuthorizationV1,
            Self::AnonymousPgcKOutOfNV1(_) => PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            Self::VeRangeTransparentRangeV1(_) => PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            Self::IrohaZkAmsV1(_) => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::VegaExistingCredentialZkV1(_) => PrivacyProtocolIdV1::VegaExistingCredentialZkV1,
            Self::IrohaZkX509StarkP256V1(_) => PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            Self::IrohaJindoPolynomialCommitmentV1(_) => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1
            }
            Self::IrohaBootleLanternAnoncredV1(_) => {
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            Self::OrchardHalo2ActionsV1(_) => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV1(_) => PrivacyProtocolIdV1::PqMaspStarkV1,
        }
    }
    /// Access the frozen proof payload.
    pub const fn bytes(&self) -> &PrivacyProofBytesV1 {
        match self {
            Self::ZkAcePqAuthorizationV1(bytes)
            | Self::AnonymousPgcKOutOfNV1(bytes)
            | Self::VeRangeTransparentRangeV1(bytes)
            | Self::VegaExistingCredentialZkV1(bytes)
            | Self::IrohaZkX509StarkP256V1(bytes)
            | Self::IrohaJindoPolynomialCommitmentV1(bytes)
            | Self::IrohaBootleLanternAnoncredV1(bytes)
            | Self::OrchardHalo2ActionsV1(bytes)
            | Self::MoneroFcmpPlusPlusV1(bytes)
            | Self::IrohaIvmPrivateNoteStarkV1(bytes)
            | Self::PqMaspStarkV1(bytes) => bytes,
            Self::IrohaZkAmsV1(proof) => proof.bytes(),
        }
    }
    /// Access the frozen proof payload.
    pub const fn bytes_mut(&mut self) -> &mut PrivacyProofBytesV1 {
        match self {
            Self::ZkAcePqAuthorizationV1(bytes)
            | Self::AnonymousPgcKOutOfNV1(bytes)
            | Self::VeRangeTransparentRangeV1(bytes)
            | Self::VegaExistingCredentialZkV1(bytes)
            | Self::IrohaZkX509StarkP256V1(bytes)
            | Self::IrohaJindoPolynomialCommitmentV1(bytes)
            | Self::IrohaBootleLanternAnoncredV1(bytes)
            | Self::OrchardHalo2ActionsV1(bytes)
            | Self::MoneroFcmpPlusPlusV1(bytes)
            | Self::IrohaIvmPrivateNoteStarkV1(bytes)
            | Self::PqMaspStarkV1(bytes) => bytes,
            Self::IrohaZkAmsV1(proof) => proof.bytes_mut(),
        }
    }
}

impl PrivacyStatementV1 {
    /// Access the frozen statement payload.
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV1(_) => PrivacyProtocolIdV1::ZkAcePqAuthorizationV1,
            Self::AnonymousPgcKOutOfNV1(_) => PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            Self::VeRangeTransparentRangeV1(_) => PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            Self::IrohaZkAmsV1(_) => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::VegaExistingCredentialZkV1(_) => PrivacyProtocolIdV1::VegaExistingCredentialZkV1,
            Self::IrohaZkX509StarkP256V1(_) => PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
            Self::IrohaJindoPolynomialCommitmentV1(_) => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1
            }
            Self::IrohaBootleLanternAnoncredV1(_) => {
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            Self::OrchardHalo2ActionsV1(_) => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV1(_) => PrivacyProtocolIdV1::PqMaspStarkV1,
        }
    }
    /// Access the frozen statement payload.
    pub const fn context_mut(&mut self) -> &mut PrivacyStatementContextV1 {
        match self {
            Self::ZkAcePqAuthorizationV1(statement) => &mut statement.context,
            Self::AnonymousPgcKOutOfNV1(statement) => &mut statement.context,
            Self::VeRangeTransparentRangeV1(statement) => &mut statement.context,
            Self::IrohaZkAmsV1(statement) => &mut statement.context,
            Self::VegaExistingCredentialZkV1(statement) => &mut statement.context,
            Self::IrohaZkX509StarkP256V1(statement) => &mut statement.context,
            Self::IrohaJindoPolynomialCommitmentV1(statement) => &mut statement.context,
            Self::IrohaBootleLanternAnoncredV1(statement) => &mut statement.context,
            Self::OrchardHalo2ActionsV1(statement) => &mut statement.context,
            Self::MoneroFcmpPlusPlusV1(statement) => &mut statement.context,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => &mut statement.context,
            Self::PqMaspStarkV1(statement) => &mut statement.context,
        }
    }
}

impl PrivacyRootPublicationV1 {
    /// Hash the exact frozen canonical payload.
    /// # Errors
    /// Canonical encoding cannot be produced.
    pub fn digest(&self) -> Result<PrivacyRootPublicationDigestV1, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_ROOT_PUBLICATION_DIGEST_DOMAIN_V1);
        hasher.update(&(encoded.len() as u64).to_le_bytes());
        hasher.update(&encoded);
        Ok(PrivacyRootPublicationDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}

impl PrivacyPgcAccountBootstrapV1 {
    /// Hash the exact frozen canonical payload.
    /// # Errors
    /// Canonical encoding cannot be produced.
    pub fn digest(&self) -> Result<PrivacyPgcAccountBootstrapDigestV1, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PGC_ACCOUNT_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(&(encoded.len() as u64).to_le_bytes());
        hasher.update(&encoded);
        Ok(PrivacyPgcAccountBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}
