/// Exact compressed SEC1 encoding of a P-256 point.
///
/// This wire type fixes the external width. Native P-256 engines additionally
/// enforce canonical SEC1 form, curve membership, and non-identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PrivacyP256PointV1(
    /// The exact 33-byte compressed SEC1 value.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub [u8; 33],
);
impl PrivacyP256PointV1 {
    /// Construct a point encoding from exactly 33 bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 33]) -> Self {
        Self(bytes)
    }
    /// Borrow the exact compressed SEC1 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 33] {
        &self.0
    }
    /// Consume the value and return its compressed SEC1 bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 33] {
        self.0
    }
    /// Return `true` when every encoded byte is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}
impl From<[u8; 33]> for PrivacyP256PointV1 {
    fn from(bytes: [u8; 33]) -> Self {
        Self::new(bytes)
    }
}
impl From<PrivacyP256PointV1> for [u8; 33] {
    fn from(value: PrivacyP256PointV1) -> Self {
        value.into_bytes()
    }
}
impl AsRef<[u8; 33]> for PrivacyP256PointV1 {
    fn as_ref(&self) -> &[u8; 33] {
        self.as_bytes()
    }
}
/// Canonical twisted-ElGamal ciphertext `(C_L, C_R)` over P-256.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyP256CiphertextV1 {
    /// `C_L = pk^r`.
    pub left: PrivacyP256PointV1,
    /// `C_R = g^r h^v`.
    pub right: PrivacyP256PointV1,
}
/// Governed policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPolicyNamespaceV1 {
    /// Exact policy identity.
    pub policy_id: PrivacyPolicyIdV1,
}
/// Pool namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPoolNamespaceV1 {
    /// Exact pool identity.
    pub pool_id: PrivacyPoolIdV1,
}
/// Issuer, admitted-identity registry, and policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyIssuerRegistryPolicyNamespaceV1 {
    /// Exact credential issuer.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact admitted-identity registry.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Exact admission policy.
    pub policy_id: PrivacyPolicyIdV1,
}
/// Trust-anchor-wide namespace payload.
///
/// This scope owns the single CA-membership root derived from a complete
/// trust store. It deliberately excludes certificate-policy identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyTrustAnchorNamespaceV1 {
    /// Exact trust-anchor issuer.
    pub trust_anchor_id: PrivacyIssuerIdV1,
}
/// Trust-anchor and certificate-policy namespace payload.
///
/// This scope owns policy-specific statement state and the corresponding issuer CRL root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyTrustAnchorPolicyNamespaceV1 {
    /// Exact trust-anchor issuer.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Exact certificate policy.
    pub policy_id: PrivacyPolicyIdV1,
}
/// Governed parameter namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyParameterNamespaceV1 {
    /// Exact parameter-set identity.
    pub parameter_id: PrivacyParameterIdV1,
}
/// Issuer and selective-disclosure policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyIssuerPolicyNamespaceV1 {
    /// Exact credential issuer.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact selective-disclosure policy.
    pub policy_id: PrivacyPolicyIdV1,
}
/// Pool and private-program namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPoolProgramNamespaceV1 {
    /// Exact private-note pool.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact private program.
    pub program_id: PrivacyProgramIdV1,
}
/// Protocol-specific portion of a replay, output, or root namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "scope", content = "value", deny_unknown_fields)
)]
pub enum PrivacyNamespaceScopeV1 {
    /// Governed authorization or range policy.
    Policy(PrivacyPolicyNamespaceV1),
    /// Anonymous-account or private-note pool.
    Pool(PrivacyPoolNamespaceV1),
    /// Credential issuer, admitted-identity registry, and admission policy.
    IssuerRegistryPolicy(PrivacyIssuerRegistryPolicyNamespaceV1),
    /// Certificate trust anchor, independent of certificate policy.
    TrustAnchor(PrivacyTrustAnchorNamespaceV1),
    /// Certificate trust anchor and certificate policy.
    TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1),
    /// Governed polynomial-commitment parameter set.
    Parameter(PrivacyParameterNamespaceV1),
    /// Anonymous-credential issuer and selective-disclosure policy.
    IssuerPolicy(PrivacyIssuerPolicyNamespaceV1),
    /// Private-note pool and exact IVM program.
    PoolProgram(PrivacyPoolProgramNamespaceV1),
}
/// Namespace component selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyNamespaceComponentV1 {
    /// Policy identifier.
    Policy,
    /// Pool identifier.
    Pool,
    /// ZK-AMS admitted-identity registry identifier.
    Registry,
    /// Issuer or trust-anchor identifier.
    Issuer,
    /// Governed parameter-set identifier.
    Parameter,
    /// Private IVM program identifier.
    Program,
}
/// Closed namespace for one protocol's replay, output, and root state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyNamespaceV1 {
    protocol_id: PrivacyProtocolIdV1,
    scope: PrivacyNamespaceScopeV1,
}
impl PrivacyNamespaceV1 {
    /// Construct a namespace from a closed protocol and scope.
    #[must_use]
    pub const fn new(protocol_id: PrivacyProtocolIdV1, scope: PrivacyNamespaceScopeV1) -> Self {
        Self { protocol_id, scope }
    }
    /// Derive the only canonical namespace for a typed public statement.
    #[must_use]
    pub const fn from_statement(statement: &PrivacyStatementV1) -> Self {
        match statement {
            PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) => Self::new(
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                PrivacyNamespaceScopeV1::Policy(PrivacyPolicyNamespaceV1 {
                    policy_id: statement.policy_id,
                }),
            ),
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) => Self::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
            PrivacyStatementV1::VeRangeTransparentRangeV1(statement) => Self::new(
                PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                PrivacyNamespaceScopeV1::Policy(PrivacyPolicyNamespaceV1 {
                    policy_id: statement.policy_id,
                }),
            ),
            PrivacyStatementV1::IrohaZkAmsV1(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                PrivacyNamespaceScopeV1::IssuerRegistryPolicy(
                    PrivacyIssuerRegistryPolicyNamespaceV1 {
                        issuer_id: statement.issuer_id,
                        registry_id: statement.registry_id,
                        policy_id: statement.policy_id,
                    },
                ),
            ),
            PrivacyStatementV1::VegaExistingCredentialZkV0(statement) => Self::new(
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
                PrivacyNamespaceScopeV1::Parameter(PrivacyParameterNamespaceV1 {
                    parameter_id: statement.context.parameter_id,
                }),
            ),
            PrivacyStatementV1::IrohaZkX509StarkP256V0(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                PrivacyNamespaceScopeV1::TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1 {
                    trust_anchor_id: statement.trust_anchor_id,
                    policy_id: statement.certificate_policy_id,
                }),
            ),
            PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                PrivacyNamespaceScopeV1::Parameter(PrivacyParameterNamespaceV1 {
                    parameter_id: statement.context.parameter_id,
                }),
            ),
            PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                PrivacyNamespaceScopeV1::IssuerPolicy(PrivacyIssuerPolicyNamespaceV1 {
                    issuer_id: statement.issuer_id,
                    policy_id: statement.policy_id,
                }),
            ),
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement) => Self::new(
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) => Self::new(
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyNamespaceScopeV1::PoolProgram(PrivacyPoolProgramNamespaceV1 {
                    pool_id: statement.pool_id,
                    program_id: statement.program_id,
                }),
            ),
            PrivacyStatementV1::PqMaspStarkV0(statement) => Self::new(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
        }
    }
    /// Return the protocol owning this namespace.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }
    /// Return the protocol-specific scope.
    #[must_use]
    pub const fn scope(self) -> PrivacyNamespaceScopeV1 {
        self.scope
    }
    /// Validate protocol/scope compatibility and nonzero components.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyNamespaceValidationError`] for a mismatched closed
    /// variant or zero namespace component.
    pub fn validate(self) -> Result<(), PrivacyNamespaceValidationError> {
        let compatible = matches!(
            (self.protocol_id, self.scope),
            (
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
                    | PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                PrivacyNamespaceScopeV1::Policy(_)
            ) | (
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
                    | PrivacyProtocolIdV1::OrchardHalo2ActionsV1
                    | PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
                    | PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyNamespaceScopeV1::Pool(_)
            ) | (
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                PrivacyNamespaceScopeV1::IssuerRegistryPolicy(_)
            ) | (
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                PrivacyNamespaceScopeV1::TrustAnchor(_)
                    | PrivacyNamespaceScopeV1::TrustAnchorPolicy(_)
            ) | (
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0
                    | PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                PrivacyNamespaceScopeV1::Parameter(_)
            ) | (
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                PrivacyNamespaceScopeV1::IssuerPolicy(_)
            ) | (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyNamespaceScopeV1::PoolProgram(_)
            )
        );
        if !compatible {
            return Err(PrivacyNamespaceValidationError::IncompatibleScope {
                protocol_id: self.protocol_id,
            });
        }
        match self.scope {
            PrivacyNamespaceScopeV1::Policy(scope) => validate_namespace_component(
                !scope.policy_id.is_zero(),
                PrivacyNamespaceComponentV1::Policy,
            ),
            PrivacyNamespaceScopeV1::Pool(scope) => validate_namespace_component(
                !scope.pool_id.is_zero(),
                PrivacyNamespaceComponentV1::Pool,
            ),
            PrivacyNamespaceScopeV1::IssuerRegistryPolicy(scope) => {
                validate_namespace_component(
                    !scope.issuer_id.is_zero(),
                    PrivacyNamespaceComponentV1::Issuer,
                )?;
                validate_namespace_component(
                    !scope.registry_id.is_zero(),
                    PrivacyNamespaceComponentV1::Registry,
                )?;
                validate_namespace_component(
                    !scope.policy_id.is_zero(),
                    PrivacyNamespaceComponentV1::Policy,
                )
            }
            PrivacyNamespaceScopeV1::TrustAnchor(scope) => validate_namespace_component(
                !scope.trust_anchor_id.is_zero(),
                PrivacyNamespaceComponentV1::Issuer,
            ),
            PrivacyNamespaceScopeV1::TrustAnchorPolicy(scope) => {
                validate_namespace_component(
                    !scope.trust_anchor_id.is_zero(),
                    PrivacyNamespaceComponentV1::Issuer,
                )?;
                validate_namespace_component(
                    !scope.policy_id.is_zero(),
                    PrivacyNamespaceComponentV1::Policy,
                )
            }
            PrivacyNamespaceScopeV1::Parameter(scope) => validate_namespace_component(
                !scope.parameter_id.is_zero(),
                PrivacyNamespaceComponentV1::Parameter,
            ),
            PrivacyNamespaceScopeV1::IssuerPolicy(scope) => {
                validate_namespace_component(
                    !scope.issuer_id.is_zero(),
                    PrivacyNamespaceComponentV1::Issuer,
                )?;
                validate_namespace_component(
                    !scope.policy_id.is_zero(),
                    PrivacyNamespaceComponentV1::Policy,
                )
            }
            PrivacyNamespaceScopeV1::PoolProgram(scope) => {
                validate_namespace_component(
                    !scope.pool_id.is_zero(),
                    PrivacyNamespaceComponentV1::Pool,
                )?;
                validate_namespace_component(
                    !scope.program_id.is_zero(),
                    PrivacyNamespaceComponentV1::Program,
                )
            }
        }
    }
}
fn validate_namespace_component(
    nonzero: bool,
    component: PrivacyNamespaceComponentV1,
) -> Result<(), PrivacyNamespaceValidationError> {
    if !nonzero {
        return Err(PrivacyNamespaceValidationError::ZeroComponent { component });
    }
    Ok(())
}
/// Validation failure for [`PrivacyNamespaceV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyNamespaceValidationError {
    /// Protocol and scope closed variants are incompatible.
    #[error("privacy namespace scope is incompatible with protocol {protocol_id:?}")]
    IncompatibleScope {
        /// Protocol carrying the incompatible scope.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// A required scope component is zero.
    #[error("privacy namespace component {component:?} must be non-zero")]
    ZeroComponent {
        /// Invalid component.
        component: PrivacyNamespaceComponentV1,
    },
}
/// Authority responsible for advancing one root role after initialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "management", content = "value", deny_unknown_fields)
)]
pub enum PrivacyRootManagementV1 {
    /// Roots advance only through an admitted proof-managed state transition.
    ProofManaged,
    /// Roots advance through an authorized governance publication.
    GovernanceManaged,
}
/// Semantic role of one canonical root inside a protocol namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "role", content = "value", deny_unknown_fields)
)]
pub enum PrivacyRootRoleV1 {
    /// Mutable encrypted PGC account table.
    PgcAccountState,
    /// ZK-AMS admitted identities, seed keys, and provisioning records.
    AccountRegistry,
    /// Credential revocation accumulator.
    Revocation,
    /// X.509 CA-membership accumulator.
    CertificateAuthorityMembership,
    /// Orchard or PQ-MASP note-commitment anchor.
    NoteCommitmentAnchor,
    /// FCMP++ complete output-set accumulator.
    OutputSet,
    /// Private IVM program state.
    ProgramState,
}
impl PrivacyRootRoleV1 {
    /// Return the sole authority model for this root role.
    #[must_use]
    pub const fn management(self) -> PrivacyRootManagementV1 {
        match self {
            Self::PgcAccountState
            | Self::AccountRegistry
            | Self::NoteCommitmentAnchor
            | Self::OutputSet
            | Self::ProgramState => PrivacyRootManagementV1::ProofManaged,
            Self::Revocation | Self::CertificateAuthorityMembership => {
                PrivacyRootManagementV1::GovernanceManaged
            }
        }
    }
    /// Return whether this role is meaningful for `protocol_id`.
    #[must_use]
    pub const fn is_compatible_with(self, protocol_id: PrivacyProtocolIdV1) -> bool {
        matches!(
            (protocol_id, self),
            (
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                Self::PgcAccountState
            ) | (PrivacyProtocolIdV1::IrohaZkAmsV1, Self::AccountRegistry)
                | (
                    PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                    Self::Revocation
                )
                | (
                    PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                    Self::CertificateAuthorityMembership
                )
                | (
                    PrivacyProtocolIdV1::OrchardHalo2ActionsV1 | PrivacyProtocolIdV1::PqMaspStarkV0,
                    Self::NoteCommitmentAnchor
                )
                | (PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1, Self::OutputSet)
                | (
                    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                    Self::ProgramState
                )
        )
    }
    /// Return whether this role is meaningful for the exact namespace scope.
    ///
    /// X.509 has one trust-anchor-wide CA-membership root. Its signed CRL is a
    /// governed record, not a second root-bearing state machine.
    #[must_use]
    pub const fn is_compatible_with_namespace(self, namespace: PrivacyNamespaceV1) -> bool {
        if !self.is_compatible_with(namespace.protocol_id()) {
            return false;
        }
        match self {
            Self::CertificateAuthorityMembership => {
                matches!(namespace.scope(), PrivacyNamespaceScopeV1::TrustAnchor(_))
            }
            Self::PgcAccountState
            | Self::AccountRegistry
            | Self::Revocation
            | Self::NoteCommitmentAnchor
            | Self::OutputSet
            | Self::ProgramState => true,
        }
    }
}
/// Governance payload publishing one canonical privacy root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
impl PrivacyRootPublicationV1 {
    /// Construct and validate a root publication.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyRootPublicationValidationError`] for a malformed
    /// namespace, incompatible role, zero epoch, or zero root.
    pub fn new(
        namespace: PrivacyNamespaceV1,
        role: PrivacyRootRoleV1,
        epoch: u64,
        root: PrivacyRootV1,
    ) -> Result<Self, PrivacyRootPublicationValidationError> {
        let publication = Self {
            namespace,
            role,
            epoch,
            root,
        };
        publication.validate()?;
        Ok(publication)
    }
    /// Validate this root publication.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyRootPublicationValidationError`] for any malformed
    /// field or closed protocol/role mismatch.
    pub fn validate(&self) -> Result<(), PrivacyRootPublicationValidationError> {
        self.namespace
            .validate()
            .map_err(PrivacyRootPublicationValidationError::Namespace)?;
        if !self.role.is_compatible_with(self.namespace.protocol_id()) {
            return Err(PrivacyRootPublicationValidationError::IncompatibleRole {
                protocol_id: self.namespace.protocol_id(),
                role: self.role,
            });
        }
        if !self.role.is_compatible_with_namespace(self.namespace) {
            return Err(
                PrivacyRootPublicationValidationError::IncompatibleNamespaceScope {
                    scope: self.namespace.scope(),
                    role: self.role,
                },
            );
        }
        if self.epoch == 0 {
            return Err(PrivacyRootPublicationValidationError::ZeroEpoch);
        }
        if self.root.is_zero() {
            return Err(PrivacyRootPublicationValidationError::ZeroRoot);
        }
        Ok(())
    }
    /// Hash this publication using canonical Norito bytes and its own domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyRootPublicationDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_ROOT_PUBLICATION_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyRootPublicationDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}
/// Validation failure for [`PrivacyRootPublicationV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyRootPublicationValidationError {
    /// Namespace is malformed.
    #[error("privacy root namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
    /// Root role is incompatible with the namespace protocol.
    #[error("privacy root role {role:?} is incompatible with protocol {protocol_id:?}")]
    IncompatibleRole {
        /// Namespace protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Incompatible role.
        role: PrivacyRootRoleV1,
    },
    /// Root role is incompatible with the namespace's exact scope.
    #[error("privacy root role {role:?} is incompatible with namespace scope {scope:?}")]
    IncompatibleNamespaceScope {
        /// Rejected namespace scope.
        scope: PrivacyNamespaceScopeV1,
        /// Rejected root role.
        role: PrivacyRootRoleV1,
    },
    /// Root epoch is zero.
    #[error("privacy root publication epoch must be non-zero")]
    ZeroEpoch,
    /// Root is zero.
    #[error("privacy root publication root must be non-zero")]
    ZeroRoot,
}
/// Governance payload establishing one Orchard pool's immutable public bridge.
///
/// The payload deliberately contains neither an initial root nor an initial epoch. Core derives the
/// pinned Orchard V3 empty-tree root and installs it at [`PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1`],
/// so governance cannot choose an alternate accumulator origin.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyOrchardPoolBootstrapV1 {
    /// Stable Orchard pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset represented by this private pool.
    pub asset_definition_id: AssetDefinitionId,
    /// Immutable transparent balance partition backing this pool.
    pub public_balance_scope: AssetBalanceScope,
    /// Governed public reserve account used for deposits and withdrawals.
    pub reserve_account: AccountId,
}
impl PrivacyOrchardPoolBootstrapV1 {
    /// Construct and validate one canonical Orchard pool bootstrap.
    ///
    /// # Errors
    ///
    /// Rejects the all-zero pool identifier.
    pub fn new(
        pool_id: PrivacyPoolIdV1,
        asset_definition_id: AssetDefinitionId,
        public_balance_scope: AssetBalanceScope,
        reserve_account: AccountId,
    ) -> Result<Self, PrivacyOrchardPoolBootstrapValidationErrorV1> {
        let bootstrap = Self {
            pool_id,
            asset_definition_id,
            public_balance_scope,
            reserve_account,
        };
        bootstrap.validate()?;
        Ok(bootstrap)
    }
    /// Return the sole protocol-scoped namespace for this pool.
    #[must_use]
    pub const fn namespace(&self) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: self.pool_id,
            }),
        )
    }
    /// Validate the closed Orchard namespace fields.
    ///
    /// # Errors
    ///
    /// Rejects the all-zero pool identifier.
    pub fn validate(&self) -> Result<(), PrivacyOrchardPoolBootstrapValidationErrorV1> {
        if self.pool_id.is_zero() {
            return Err(PrivacyOrchardPoolBootstrapValidationErrorV1::ZeroPoolId);
        }
        if matches!(
            self.public_balance_scope,
            AssetBalanceScope::Dataspace(crate::nexus::DataSpaceId::UNIVERSAL)
        ) {
            return Err(PrivacyOrchardPoolBootstrapValidationErrorV1::UniversalPublicBalanceScope);
        }
        self.namespace()
            .validate()
            .map_err(PrivacyOrchardPoolBootstrapValidationErrorV1::Namespace)
    }
    /// Hash the exact canonical bootstrap in its own provenance domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyOrchardPoolBootstrapDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_ORCHARD_POOL_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyOrchardPoolBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}
/// Structural failure for [`PrivacyOrchardPoolBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyOrchardPoolBootstrapValidationErrorV1 {
    /// The stable pool identifier is all zero.
    #[error("Orchard pool bootstrap pool id must be non-zero")]
    ZeroPoolId,
    /// The universal coordinator was supplied as a concrete balance partition.
    #[error("Orchard pool public balance scope cannot be the universal dataspace")]
    UniversalPublicBalanceScope,
    /// The derived closed namespace is malformed.
    #[error("Orchard pool bootstrap namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
}
/// Exact unframed byte width of one FCMP++ `(O,I,C)` tuple.
pub const PRIVACY_FCMP_OUTPUT_TUPLE_BYTES_V1: usize = 3 * 32;
/// One complete FCMP++ output-tree leaf `(O, I, C)`.
///
/// All three values are canonical compressed prime-order Edwards points. The data model preserves
/// the exact encodings; the native FCMP++ engine performs the curve, subgroup, and non-identity
/// checks before governance or proof admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpOutputTupleV1 {
    /// One-time output key `O`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub output_key: [u8; 32],
    /// Per-output linking-tag generator `I`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub linking_tag_generator: [u8; 32],
    /// Amount commitment `C`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub amount_commitment: [u8; 32],
}
impl PrivacyFcmpOutputTupleV1 {
    /// Derive the ledger-only identifier used for duplicate detection and output lookup.
    ///
    /// The native curve tree and FCMP++ relation always consume the full tuple.
    #[must_use]
    pub fn output_id(self) -> PrivacyFcmpOutputIdV1 {
        let mut hasher = Sha256::new();
        hasher.update(PRIVACY_FCMP_OUTPUT_ID_DOMAIN_V1);
        hasher.update(self.output_key);
        hasher.update(self.linking_tag_generator);
        hasher.update(self.amount_commitment);
        PrivacyFcmpOutputIdV1::new(hasher.finalize().into())
    }
    /// Reject the reserved all-zero encoding before native curve validation.
    ///
    /// # Errors
    ///
    /// Returns the exact zero component.
    pub fn validate_nonzero(self) -> Result<(), PrivacyFcmpOutputTupleValidationErrorV1> {
        if self.output_key.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::OutputKey,
            });
        }
        if self.linking_tag_generator.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::LinkingTagGenerator,
            });
        }
        if self.amount_commitment.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::AmountCommitment,
            });
        }
        Ok(())
    }
}
/// FCMP++ output-tuple component selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyFcmpOutputComponentV1 {
    /// One-time output key `O`.
    OutputKey,
    /// Per-output linking-tag generator `I`.
    LinkingTagGenerator,
    /// Amount commitment `C`.
    AmountCommitment,
}
/// Structural failure for one FCMP++ output tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyFcmpOutputTupleValidationErrorV1 {
    /// A mandatory point uses the reserved all-zero encoding.
    #[error("FCMP++ output tuple component {component:?} must be non-zero")]
    ZeroComponent {
        /// Rejected component.
        component: PrivacyFcmpOutputComponentV1,
    },
}
/// Canonical typed root of the alternating FCMP++ Selene/Helios curve tree.
///
/// Odd layer counts identify Selene roots and even layer counts identify Helios roots. The layer
/// count is cryptographically significant and cannot be inferred from the compressed point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpTreeRootV1 {
    /// Number of alternating curve-tree layers.
    pub layers: u8,
    /// Canonical compressed Selene or Helios point selected by layer parity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub point: [u8; 32],
}
impl PrivacyFcmpTreeRootV1 {
    /// Largest layer count admitted by the first-release FCMP++ wire.
    pub const MAX_LAYERS: u8 = 32;
    /// Validate the closed structural shape before native curve validation.
    ///
    /// # Errors
    ///
    /// Rejects zero/excessive layers or the all-zero point sentinel.
    pub fn validate(self) -> Result<(), PrivacyFcmpTreeRootValidationErrorV1> {
        if self.layers == 0 || self.layers > Self::MAX_LAYERS {
            return Err(PrivacyFcmpTreeRootValidationErrorV1::InvalidLayerCount {
                layers: self.layers,
                max: Self::MAX_LAYERS,
            });
        }
        if self.point.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpTreeRootValidationErrorV1::ZeroPoint);
        }
        Ok(())
    }
    /// Commit the typed root into the shared 32-byte retained-root index.
    ///
    /// Durable FCMP++ accumulator state still stores the complete typed root;
    /// this digest is only the history/map key.
    #[must_use]
    pub fn history_commitment(self) -> PrivacyRootV1 {
        let mut hasher = Sha256::new();
        hasher.update(PRIVACY_FCMP_ROOT_COMMITMENT_DOMAIN_V1);
        hasher.update([self.layers]);
        hasher.update(self.point);
        PrivacyRootV1::new(hasher.finalize().into())
    }
}
/// Structural failure for one typed FCMP++ tree root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyFcmpTreeRootValidationErrorV1 {
    /// Layer count is outside the closed first-release bound.
    #[error("FCMP++ tree layers {layers} are outside 1..={max}")]
    InvalidLayerCount {
        /// Rejected layer count.
        layers: u8,
        /// Compiled maximum.
        max: u8,
    },
    /// Compressed root point is the reserved all-zero sentinel.
    #[error("FCMP++ tree root point must be non-zero")]
    ZeroPoint,
}
/// Complete public FCMP++ relation for one hidden consumed output.
///
/// The IFC1 proof duplicates `O~`, `I~`, and `R`; the native decoder must
/// compare those bytes exactly. `C~` and the key image `L` remain
/// statement-only public inputs to the complete relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpInputPublicV1 {
    /// Rerandomized output key `O~`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub output_key_tilde: [u8; 32],
    /// Rerandomized linking-tag generator `I~`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub linking_tag_generator_tilde: [u8; 32],
    /// Rerandomization commitment `R`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub rerandomization_commitment: [u8; 32],
    /// Pseudo output amount commitment `C~`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub pseudo_out: [u8; 32],
    /// Linkability key image `L`.
    pub key_image: PrivacyFcmpKeyImageV1,
}
impl PrivacyFcmpInputPublicV1 {
    /// Reject the reserved all-zero encoding before native curve validation.
    ///
    /// # Errors
    ///
    /// Returns the exact zero component.
    pub fn validate_nonzero(self) -> Result<(), PrivacyFcmpInputValidationErrorV1> {
        for (component, point) in [
            (
                PrivacyFcmpInputComponentV1::OutputKeyTilde,
                self.output_key_tilde,
            ),
            (
                PrivacyFcmpInputComponentV1::LinkingTagGeneratorTilde,
                self.linking_tag_generator_tilde,
            ),
            (
                PrivacyFcmpInputComponentV1::RerandomizationCommitment,
                self.rerandomization_commitment,
            ),
            (PrivacyFcmpInputComponentV1::PseudoOut, self.pseudo_out),
            (
                PrivacyFcmpInputComponentV1::KeyImage,
                self.key_image.into_bytes(),
            ),
        ] {
            if point.iter().all(|byte| *byte == 0) {
                return Err(PrivacyFcmpInputValidationErrorV1::ZeroComponent { component });
            }
        }
        Ok(())
    }
}
/// FCMP++ input component selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyFcmpInputComponentV1 {
    /// Rerandomized output key `O~`.
    OutputKeyTilde,
    /// Rerandomized linking-tag generator `I~`.
    LinkingTagGeneratorTilde,
    /// Rerandomization commitment `R`.
    RerandomizationCommitment,
    /// Pseudo output amount commitment `C~`.
    PseudoOut,
    /// Linkability key image `L`.
    KeyImage,
}
/// Structural failure for one FCMP++ public input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyFcmpInputValidationErrorV1 {
    /// A mandatory point uses the reserved all-zero encoding.
    #[error("FCMP++ public input component {component:?} must be non-zero")]
    ZeroComponent {
        /// Rejected component.
        component: PrivacyFcmpInputComponentV1,
    },
}
/// Magic prefix of the sole first-release FCMP++ wallet ciphertext codec.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_MAGIC_V1: [u8; 4] = *b"IFCE";
/// XChaCha20-Poly1305 nonce width in the FCMP++ wallet ciphertext.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_NONCE_BYTES_V1: usize = 24;
/// Fixed plaintext width: magic, output id, complete `(O,I,C)` tuple, positive
/// `u64` amount, amount-commitment mask, and the two spend-opening scalars.
pub const PRIVACY_FCMP_NOTE_PLAINTEXT_BYTES_V1: usize =
    4 + 32 + PRIVACY_FCMP_OUTPUT_TUPLE_BYTES_V1 + 8 + 32 + 32 + 32;
/// Poly1305 authentication-tag width.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_TAG_BYTES_V1: usize = 16;
/// Exact ciphertext field width, including codec magic and explicit nonce.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1: usize = 4
    + PRIVACY_FCMP_ENCRYPTED_OUTPUT_NONCE_BYTES_V1
    + PRIVACY_FCMP_NOTE_PLAINTEXT_BYTES_V1
    + PRIVACY_FCMP_ENCRYPTED_OUTPUT_TAG_BYTES_V1;
/// Magic prefix of the sole first-release private-IVM wallet ciphertext.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1: [u8; 4] = *b"IPNE";
/// XChaCha20-Poly1305 nonce width in a private-IVM wallet ciphertext.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1: usize = 24;
/// Fixed plaintext width: magic, public commitment, `u128` value, spending
/// authority, note nonce, commitment blinding, and wallet memo digest.
pub const PRIVACY_IVM_PRIVATE_NOTE_PLAINTEXT_BYTES_V1: usize = 4 + 32 + 16 + 32 + 32 + 32 + 32;
/// Poly1305 authentication-tag width.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_TAG_BYTES_V1: usize = 16;
/// Exact private-IVM ciphertext width, including codec magic and explicit nonce.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1: usize = 4
    + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1
    + PRIVACY_IVM_PRIVATE_NOTE_PLAINTEXT_BYTES_V1
    + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_TAG_BYTES_V1;
/// Typed encrypted payload for one complete FCMP++ output tuple.
///
/// The output identifier is a ledger index only. The statement and native curve tree always retain
/// and consume the corresponding full `(O, I, C)` tuple.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpEncryptedOutputV1 {
    /// Cryptographic recipient identity.
    pub recipient: PrivacyRecipientIdV1,
    /// Ephemeral public encryption key.
    pub ephemeral_public_key: PrivacyEncryptionKeyV1,
    /// Identifier of the ordered public output tuple.
    pub output_id: PrivacyFcmpOutputIdV1,
    /// Exact `IFCE || nonce || XChaCha20-Poly1305(FCMP note)` bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
}
/// Immutable governance payload for one FCMP++ complete-output-set pool.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpPoolBootstrapV1 {
    /// Stable FCMP++ output-set identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset represented by the confidential outputs.
    pub asset_definition_id: AssetDefinitionId,
    /// Non-empty complete genesis output set in strict output-identifier order.
    pub initial_outputs: Vec<PrivacyFcmpOutputTupleV1>,
}
/// Immutable governance payload for one private-IVM program pool.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyIvmPrivateNotePoolBootstrapV1 {
    /// Stable private-note pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset manipulated by the private program.
    pub asset_definition_id: AssetDefinitionId,
    /// Immutable transparent balance partition backing this pool.
    pub public_balance_scope: AssetBalanceScope,
    /// Public reserve account used by explicit value-balance bridges.
    pub reserve_account: AccountId,
    /// Exact compiled private-program digest accepted by this pool.
    pub program_id: PrivacyProgramIdV1,
    /// Non-empty genesis note set in strict commitment order.
    pub initial_note_commitments: Vec<PrivacyCommitmentV1>,
}
/// Immutable governance payload for one PQ-MASP note pool.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPqMaspPoolBootstrapV1 {
    /// Stable PQ-MASP note-pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset represented by the private notes.
    pub asset_definition_id: AssetDefinitionId,
    /// Non-empty genesis note set in strict commitment order.
    pub initial_note_commitments: Vec<PrivacyCommitmentV1>,
}
/// Closed typed bootstrap for proof-managed pools that do not use Orchard's compact frontier.
///
/// Initial roots and epochs are deliberately absent. Core derives each protocol's pinned root from
/// the complete canonical genesis commitment set at epoch one, preventing governance from selecting
/// an alternate accumulator origin.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "bootstrap", deny_unknown_fields)
)]
pub enum PrivacyProofManagedPoolBootstrapV1 {
    /// FCMP++ complete-output-set origin.
    #[cfg_attr(feature = "json", norito(rename = "monero-fcmp-plus-plus-v1"))]
    MoneroFcmpPlusPlusV1(PrivacyFcmpPoolBootstrapV1),
    /// Native private-IVM program-state origin.
    #[cfg_attr(feature = "json", norito(rename = "iroha-ivm-private-note-stark-v1"))]
    IrohaIvmPrivateNoteStarkV1(PrivacyIvmPrivateNotePoolBootstrapV1),
    /// PQ-MASP note-commitment origin.
    #[cfg_attr(feature = "json", norito(rename = "pq-masp-stark-v0"))]
    PqMaspStarkV0(PrivacyPqMaspPoolBootstrapV1),
}
impl PrivacyProofManagedPoolBootstrapV1 {
    /// Return the exact protocol initialized by this payload.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV0(_) => PrivacyProtocolIdV1::PqMaspStarkV0,
        }
    }
    /// Return the exact proof-managed root role initialized by this payload.
    #[must_use]
    pub const fn root_role(&self) -> PrivacyRootRoleV1 {
        match self {
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyRootRoleV1::OutputSet,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyRootRoleV1::ProgramState,
            Self::PqMaspStarkV0(_) => PrivacyRootRoleV1::NoteCommitmentAnchor,
        }
    }
    /// Return the sole protocol-scoped namespace initialized by this payload.
    #[must_use]
    pub const fn namespace(&self) -> PrivacyNamespaceV1 {
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: bootstrap.pool_id,
                }),
            ),
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyNamespaceScopeV1::PoolProgram(PrivacyPoolProgramNamespaceV1 {
                    pool_id: bootstrap.pool_id,
                    program_id: bootstrap.program_id,
                }),
            ),
            Self::PqMaspStarkV0(bootstrap) => PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: bootstrap.pool_id,
                }),
            ),
        }
    }
    /// Return the exact backing asset definition.
    #[must_use]
    pub const fn asset_definition_id(&self) -> &AssetDefinitionId {
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => &bootstrap.asset_definition_id,
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => &bootstrap.asset_definition_id,
            Self::PqMaspStarkV0(bootstrap) => &bootstrap.asset_definition_id,
        }
    }
    /// Return the public reserve account when the protocol supports an
    /// explicit value-balance bridge.
    #[must_use]
    pub const fn reserve_account(&self) -> Option<&AccountId> {
        match self {
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => Some(&bootstrap.reserve_account),
            Self::MoneroFcmpPlusPlusV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }
    /// Return the exact transparent balance partition for protocols with a
    /// public value-balance bridge.
    #[must_use]
    pub const fn public_balance_scope(&self) -> Option<AssetBalanceScope> {
        match self {
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => Some(bootstrap.public_balance_scope),
            Self::MoneroFcmpPlusPlusV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }
    /// Return the pinned private-program digest for private-IVM pools.
    #[must_use]
    pub const fn program_id(&self) -> Option<PrivacyProgramIdV1> {
        match self {
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => Some(bootstrap.program_id),
            Self::MoneroFcmpPlusPlusV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }
    /// Return the complete canonical genesis note-commitment set.
    ///
    /// FCMP++ uses full typed output tuples and therefore returns `None`.
    #[must_use]
    pub fn initial_note_commitments(&self) -> Option<&[PrivacyCommitmentV1]> {
        match self {
            Self::MoneroFcmpPlusPlusV1(_) => None,
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => {
                Some(&bootstrap.initial_note_commitments)
            }
            Self::PqMaspStarkV0(bootstrap) => Some(&bootstrap.initial_note_commitments),
        }
    }
    /// Return the complete canonical FCMP++ genesis output set.
    #[must_use]
    pub fn initial_fcmp_outputs(&self) -> Option<&[PrivacyFcmpOutputTupleV1]> {
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => Some(&bootstrap.initial_outputs),
            Self::IrohaIvmPrivateNoteStarkV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }
    /// Validate the exact closed namespace and required non-zero identifiers.
    ///
    /// # Errors
    ///
    /// Rejects zero pool/program identifiers or a malformed derived namespace.
    pub fn validate(&self) -> Result<(), PrivacyProofManagedPoolBootstrapValidationErrorV1> {
        let pool_id = match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => bootstrap.pool_id,
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => {
                if bootstrap.program_id.is_zero() {
                    return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroProgramId);
                }
                if matches!(
                    bootstrap.public_balance_scope,
                    AssetBalanceScope::Dataspace(crate::nexus::DataSpaceId::UNIVERSAL)
                ) {
                    return Err(
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::UniversalPublicBalanceScope,
                    );
                }
                bootstrap.pool_id
            }
            Self::PqMaspStarkV0(bootstrap) => bootstrap.pool_id,
        };
        if pool_id.is_zero() {
            return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroPoolId);
        }
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => {
                if bootstrap.initial_outputs.is_empty() {
                    return Err(
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::EmptyInitialFcmpOutputs,
                    );
                }
                if bootstrap.initial_outputs.len() > PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 {
                    return Err(
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::TooManyInitialFcmpOutputs {
                            count: bootstrap.initial_outputs.len(),
                            max: PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1,
                        },
                    );
                }
                let mut previous = None;
                for (index, output) in bootstrap.initial_outputs.iter().copied().enumerate() {
                    output.validate_nonzero().map_err(|source| {
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::InvalidInitialFcmpOutput {
                            index,
                            source,
                        }
                    })?;
                    let output_id = output.output_id();
                    if previous.is_some_and(|value| value >= output_id) {
                        return Err(
                            PrivacyProofManagedPoolBootstrapValidationErrorV1::InitialFcmpOutputIdsNotStrictlyIncreasing {
                                index,
                            },
                        );
                    }
                    previous = Some(output_id);
                }
            }
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => {
                validate_initial_note_commitments(&bootstrap.initial_note_commitments)?;
            }
            Self::PqMaspStarkV0(bootstrap) => {
                validate_initial_note_commitments(&bootstrap.initial_note_commitments)?;
            }
        }
        let namespace = self.namespace();
        namespace
            .validate()
            .map_err(PrivacyProofManagedPoolBootstrapValidationErrorV1::Namespace)?;
        if !self.root_role().is_compatible_with_namespace(namespace) {
            return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::IncompatibleRootRole);
        }
        Ok(())
    }
    /// Hash the exact canonical bootstrap in its own provenance domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyProofManagedPoolBootstrapDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PROOF_MANAGED_POOL_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyProofManagedPoolBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}
fn validate_initial_note_commitments(
    commitments: &[PrivacyCommitmentV1],
) -> Result<(), PrivacyProofManagedPoolBootstrapValidationErrorV1> {
    if commitments.is_empty() {
        return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::EmptyInitialCommitments);
    }
    if commitments.len() > PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 {
        return Err(
            PrivacyProofManagedPoolBootstrapValidationErrorV1::TooManyInitialCommitments {
                count: commitments.len(),
                max: PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1,
            },
        );
    }
    let mut previous = None;
    for (index, commitment) in commitments.iter().copied().enumerate() {
        if commitment.is_zero() {
            return Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroInitialCommitment { index },
            );
        }
        if previous.is_some_and(|value| value >= commitment) {
            return Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::InitialCommitmentsNotStrictlyIncreasing {
                    index,
                },
            );
        }
        previous = Some(commitment);
    }
    Ok(())
}
/// Structural failure for [`PrivacyProofManagedPoolBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyProofManagedPoolBootstrapValidationErrorV1 {
    /// The stable pool identifier is all zero.
    #[error("proof-managed privacy pool bootstrap pool id must be non-zero")]
    ZeroPoolId,
    /// The private-IVM program digest is all zero.
    #[error("private-IVM pool bootstrap program id must be non-zero")]
    ZeroProgramId,
    /// The universal coordinator was supplied as a concrete balance partition.
    #[error("private-IVM public balance scope cannot be the universal dataspace")]
    UniversalPublicBalanceScope,
    /// No complete FCMP++ genesis output was supplied.
    #[error("FCMP++ pool bootstrap requires at least one initial output tuple")]
    EmptyInitialFcmpOutputs,
    /// The FCMP++ genesis output set exceeds the hard first-release bound.
    #[error("FCMP++ pool bootstrap has {count} initial outputs; maximum is {max}")]
    TooManyInitialFcmpOutputs {
        /// Observed output count.
        count: usize,
        /// Hard first-release maximum.
        max: usize,
    },
    /// One FCMP++ genesis tuple has a structurally invalid component.
    #[error("FCMP++ pool bootstrap output {index} is invalid: {source}")]
    InvalidInitialFcmpOutput {
        /// Zero-based output position.
        index: usize,
        /// Exact structural tuple failure.
        source: PrivacyFcmpOutputTupleValidationErrorV1,
    },
    /// FCMP++ genesis output identifiers contain a duplicate or are reordered.
    #[error("FCMP++ pool bootstrap output ids must be strictly increasing at index {index}")]
    InitialFcmpOutputIdsNotStrictlyIncreasing {
        /// First non-increasing output position.
        index: usize,
    },
    /// No genesis commitment was supplied.
    #[error("proof-managed privacy pool bootstrap requires at least one initial commitment")]
    EmptyInitialCommitments,
    /// The genesis commitment set exceeds the hard first-release bound.
    #[error(
        "proof-managed privacy pool bootstrap has {count} initial commitments; maximum is {max}"
    )]
    TooManyInitialCommitments {
        /// Observed commitment count.
        count: usize,
        /// Hard first-release maximum.
        max: usize,
    },
    /// One genesis commitment is the all-zero sentinel.
    #[error("proof-managed privacy pool bootstrap commitment {index} must be non-zero")]
    ZeroInitialCommitment {
        /// Zero-based commitment index.
        index: usize,
    },
    /// Genesis commitments are duplicated or reordered.
    #[error(
        "proof-managed privacy pool bootstrap commitments stop increasing strictly at index {index}"
    )]
    InitialCommitmentsNotStrictlyIncreasing {
        /// First invalid zero-based position.
        index: usize,
    },
    /// The derived closed namespace is malformed.
    #[error("proof-managed privacy pool bootstrap namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
    /// The inferred root role is incompatible with the exact namespace.
    #[error("proof-managed privacy pool bootstrap root role is incompatible")]
    IncompatibleRootRole,
}
/// One canonical encrypted account in a PGC account-state bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPgcAccountV1 {
    /// Canonical compressed P-256 account public key.
    pub public_key: PrivacyP256PointV1,
    /// Initial twisted-ElGamal encrypted balance.
    pub encrypted_balance: PrivacyP256CiphertextV1,
}
/// Point position selected by PGC bootstrap validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyPgcAccountPointV1 {
    /// Account public key.
    PublicKey,
    /// Encrypted-balance left component.
    EncryptedBalanceLeft,
    /// Encrypted-balance right component.
    EncryptedBalanceRight,
}
/// Exact canonical native proof for an Anonymous PGC account bootstrap.
///
/// This proof has a dedicated wire type and a tighter first-release bound than
/// ordinary privacy actions. The native verifier must additionally perform an
/// exact decode and require byte-for-byte canonical re-encoding before core
/// derives [`PrivacyPgcBootstrapProofDigestV1`] for persisted provenance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PrivacyPgcBootstrapProofBytesV1 {
    /// Exact native proof encoding.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub bytes: Vec<u8>,
}
impl PrivacyPgcBootstrapProofBytesV1 {
    /// Construct proof bytes for subsequent native validation.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
    /// Borrow the exact proof bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Validate presence, non-degeneracy, and the fixed Taira byte cap.
    ///
    /// # Errors
    ///
    /// Rejects an empty, all-zero, unrepresentable, or oversized payload.
    pub fn validate(&self) -> Result<(), PrivacyPgcBootstrapProofValidationError> {
        if self.bytes.is_empty() {
            return Err(PrivacyPgcBootstrapProofValidationError::Empty);
        }
        if self.bytes.iter().all(|byte| *byte == 0) {
            return Err(PrivacyPgcBootstrapProofValidationError::AllZero);
        }
        let len = u64::try_from(self.bytes.len())
            .map_err(|_| PrivacyPgcBootstrapProofValidationError::LengthOverflow)?;
        if len > u64::from(TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1) {
            return Err(PrivacyPgcBootstrapProofValidationError::TooLarge {
                bytes: len,
                max: TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
            });
        }
        Ok(())
    }
    /// Derive the audit digest of these exact proof bytes.
    ///
    /// Callers admitting a proof must invoke this only after the native verifier has performed
    /// exact decode and byte-for-byte canonical re-encoding. The method repeats structural
    /// validation and never accepts a caller-supplied digest.
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`Self::validate`].
    pub fn digest(
        &self,
    ) -> Result<PrivacyPgcBootstrapProofDigestV1, PrivacyPgcBootstrapProofValidationError> {
        self.validate()?;
        let len = u64::try_from(self.bytes.len())
            .map_err(|_| PrivacyPgcBootstrapProofValidationError::LengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PGC_BOOTSTRAP_PROOF_DIGEST_DOMAIN_V1);
        hasher.update(&len.to_le_bytes());
        hasher.update(&self.bytes);
        Ok(PrivacyPgcBootstrapProofDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}
/// Structural failure for [`PrivacyPgcBootstrapProofBytesV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyPgcBootstrapProofValidationError {
    /// Proof payload is absent.
    #[error("PGC bootstrap proof bytes must not be empty")]
    Empty,
    /// Proof payload is degenerate.
    #[error("PGC bootstrap proof bytes must not be all zero")]
    AllZero,
    /// Proof payload exceeds the fixed first-release cap.
    #[error("PGC bootstrap proof uses {bytes} bytes, exceeding maximum {max}")]
    TooLarge {
        /// Observed byte length.
        bytes: u64,
        /// Fixed maximum byte length.
        max: u32,
    },
    /// Platform collection length cannot be represented canonically.
    #[error("PGC bootstrap proof length exceeds u64")]
    LengthOverflow,
}
/// Governed bootstrap payload for a complete PGC encrypted account table.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
impl PrivacyPgcAccountBootstrapV1 {
    /// Validate the closed namespace, size, ordering, and nonzero wire values.
    ///
    /// Core must additionally recompute `initial_root` from the canonical entries under
    /// [`PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1`] before admitting this payload.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyPgcAccountBootstrapValidationError`] for any malformed bootstrap field.
    pub fn validate(&self) -> Result<(), PrivacyPgcAccountBootstrapValidationError> {
        self.namespace
            .validate()
            .map_err(PrivacyPgcAccountBootstrapValidationError::Namespace)?;
        if self.namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
            return Err(PrivacyPgcAccountBootstrapValidationError::WrongProtocol {
                protocol_id: self.namespace.protocol_id(),
            });
        }
        if self.initial_root.is_zero() {
            return Err(PrivacyPgcAccountBootstrapValidationError::ZeroRoot);
        }
        if self.initial_epoch != PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1 {
            return Err(
                PrivacyPgcAccountBootstrapValidationError::NonCanonicalInitialEpoch {
                    epoch: self.initial_epoch,
                },
            );
        }
        if self.total_supply == 0 {
            return Err(PrivacyPgcAccountBootstrapValidationError::ZeroTotalSupply);
        }
        let account_count = u32::try_from(self.accounts.len())
            .map_err(|_| PrivacyPgcAccountBootstrapValidationError::AccountLengthOverflow)?;
        if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&account_count) {
            return Err(
                PrivacyPgcAccountBootstrapValidationError::InvalidAccountCount {
                    count: account_count,
                },
            );
        }
        for (index, account) in self.accounts.iter().enumerate() {
            let encoded_index = u32::try_from(index)
                .map_err(|_| PrivacyPgcAccountBootstrapValidationError::AccountLengthOverflow)?;
            if account.public_key.is_zero() {
                return Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                    index: encoded_index,
                    point: PrivacyPgcAccountPointV1::PublicKey,
                });
            }
            if account.encrypted_balance.left.is_zero() {
                return Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                    index: encoded_index,
                    point: PrivacyPgcAccountPointV1::EncryptedBalanceLeft,
                });
            }
            if account.encrypted_balance.right.is_zero() {
                return Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                    index: encoded_index,
                    point: PrivacyPgcAccountPointV1::EncryptedBalanceRight,
                });
            }
            if index > 0 && self.accounts[index - 1].public_key >= account.public_key {
                return Err(PrivacyPgcAccountBootstrapValidationError::KeysNotStrictlyIncreasing);
            }
        }
        Ok(())
    }
    /// Hash this bootstrap payload in its distinct provenance domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyPgcAccountBootstrapDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PGC_ACCOUNT_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyPgcAccountBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}
/// Validation failure for [`PrivacyPgcAccountBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyPgcAccountBootstrapValidationError {
    /// Namespace is malformed.
    #[error("PGC bootstrap namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
    /// Namespace belongs to another protocol.
    #[error("PGC bootstrap namespace uses protocol {protocol_id:?}")]
    WrongProtocol {
        /// Unexpected namespace protocol.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Declared initial root is zero.
    #[error("PGC bootstrap initial root must be non-zero")]
    ZeroRoot,
    /// Declared initial epoch differs from the closed first-release origin.
    #[error("PGC bootstrap initial epoch must be 1, got {epoch}")]
    NonCanonicalInitialEpoch {
        /// Rejected caller-provided epoch.
        epoch: u64,
    },
    /// Declared aggregate supply is zero.
    #[error("PGC bootstrap total supply must be non-zero")]
    ZeroTotalSupply,
    /// Account count is not one of the closed profile sizes.
    #[error("PGC bootstrap account count {count} is not one of 16, 32, or 64")]
    InvalidAccountCount {
        /// Observed account count.
        count: u32,
    },
    /// A P-256 point placeholder is all zero.
    #[error("PGC bootstrap account {index} point {point:?} must be non-zero")]
    ZeroPoint {
        /// Zero-based account index.
        index: u32,
        /// Invalid point role.
        point: PrivacyPgcAccountPointV1,
    },
    /// Account public keys are duplicated or not canonically sorted.
    #[error("PGC bootstrap account public keys must be strictly increasing")]
    KeysNotStrictlyIncreasing,
    /// Platform collection length cannot be represented canonically.
    #[error("PGC bootstrap account length exceeds u32")]
    AccountLengthOverflow,
}
/// Field within [`PrivacyConsensusLimitsV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLimitFieldV1 {
    /// Actions per transaction.
    ActionsPerTransaction,
    /// Actions per block.
    ActionsPerBlock,
    /// Proof bytes per action.
    ProofBytesPerAction,
    /// Encoded action bytes.
    ActionBytes,
    /// Privacy bytes per transaction.
    PrivacyBytesPerTransaction,
    /// Privacy bytes per block.
    PrivacyBytesPerBlock,
    /// Public statement and encrypted-output bytes per transaction.
    StatementAndEncryptedOutputBytesPerTransaction,
    /// Nullifiers per action.
    NullifiersPerAction,
    /// Commitments per action.
    CommitmentsPerAction,
    /// Retained recent roots.
    RetainedRootCount,
}
/// Consensus-enforced privacy resource limits.
///
/// The first release permits governance to lower these values but not exceed
/// the Taira hard ceilings. Raising a ceiling requires an explicit data-model
/// release so old validators cannot silently admit a larger resource surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyConsensusLimitsV1 {
    /// Maximum privacy actions in one transaction.
    pub max_actions_per_transaction: u32,
    /// Maximum privacy actions in one block.
    pub max_actions_per_block: u32,
    /// Maximum proof payload bytes in one action.
    pub max_proof_bytes_per_action: u32,
    /// Maximum encoded bytes in one action.
    pub max_action_bytes: u32,
    /// Maximum privacy bytes in one transaction.
    pub max_privacy_bytes_per_transaction: u32,
    /// Maximum privacy bytes in one block.
    pub max_privacy_bytes_per_block: u32,
    /// Maximum public-input and encrypted-output bytes in one transaction.
    pub max_statement_and_encrypted_output_bytes_per_transaction: u32,
    /// Maximum nullifiers emitted by one action.
    pub max_nullifiers_per_action: u32,
    /// Maximum commitments emitted by one action.
    pub max_commitments_per_action: u32,
    /// Number of recent commitment roots retained for proof admission.
    pub retained_root_count: u32,
}
impl PrivacyConsensusLimitsV1 {
    /// Return the approved first-release Taira profile.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            max_actions_per_transaction: TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
            max_actions_per_block: TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1,
            max_proof_bytes_per_action: TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            max_action_bytes: TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
            max_privacy_bytes_per_transaction: TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
            max_privacy_bytes_per_block: TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
            max_statement_and_encrypted_output_bytes_per_transaction:
                TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
            max_nullifiers_per_action: TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
            max_commitments_per_action: TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1,
            retained_root_count: TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1,
        }
    }
    /// Validate non-zero, hard-ceiling, and cross-field ordering invariants.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyConsensusLimitsValidationError`] for the first invalid
    /// field or relationship in deterministic field order.
    pub fn validate(&self) -> Result<(), PrivacyConsensusLimitsValidationError> {
        let fields = [
            (
                PrivacyLimitFieldV1::ActionsPerTransaction,
                self.max_actions_per_transaction,
                TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::ActionsPerBlock,
                self.max_actions_per_block,
                TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1,
            ),
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                self.max_proof_bytes_per_action,
                TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                self.max_action_bytes,
                TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                self.max_privacy_bytes_per_transaction,
                TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                self.max_privacy_bytes_per_block,
                TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
            ),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                self.max_statement_and_encrypted_output_bytes_per_transaction,
                TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::NullifiersPerAction,
                self.max_nullifiers_per_action,
                TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::CommitmentsPerAction,
                self.max_commitments_per_action,
                TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::RetainedRootCount,
                self.retained_root_count,
                TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1,
            ),
        ];
        for (field, value, hard_max) in fields {
            if value == 0 {
                return Err(PrivacyConsensusLimitsValidationError::Zero { field });
            }
            if value > hard_max {
                return Err(PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                    field,
                    value,
                    hard_max,
                });
            }
        }
        validate_limit_order(
            PrivacyLimitFieldV1::ActionsPerTransaction,
            self.max_actions_per_transaction,
            PrivacyLimitFieldV1::ActionsPerBlock,
            self.max_actions_per_block,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::ProofBytesPerAction,
            self.max_proof_bytes_per_action,
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
            PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
            self.max_privacy_bytes_per_transaction,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
            self.max_privacy_bytes_per_transaction,
            PrivacyLimitFieldV1::PrivacyBytesPerBlock,
            self.max_privacy_bytes_per_block,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
            self.max_statement_and_encrypted_output_bytes_per_transaction,
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
        )?;
        Ok(())
    }
    /// Validate `next` as a strict component-wise tightening of this policy.
    ///
    /// # Errors
    ///
    /// Rejects invalid profiles, any increased component, and a no-op update.
    pub fn validate_tightening_to(
        &self,
        next: &Self,
    ) -> Result<(), PrivacyConsensusLimitsTighteningErrorV1> {
        self.validate()
            .map_err(PrivacyConsensusLimitsTighteningErrorV1::InvalidCurrent)?;
        next.validate()
            .map_err(PrivacyConsensusLimitsTighteningErrorV1::InvalidNext)?;
        let fields = [
            (
                PrivacyLimitFieldV1::ActionsPerTransaction,
                self.max_actions_per_transaction,
                next.max_actions_per_transaction,
            ),
            (
                PrivacyLimitFieldV1::ActionsPerBlock,
                self.max_actions_per_block,
                next.max_actions_per_block,
            ),
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                self.max_proof_bytes_per_action,
                next.max_proof_bytes_per_action,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                self.max_action_bytes,
                next.max_action_bytes,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                self.max_privacy_bytes_per_transaction,
                next.max_privacy_bytes_per_transaction,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                self.max_privacy_bytes_per_block,
                next.max_privacy_bytes_per_block,
            ),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                self.max_statement_and_encrypted_output_bytes_per_transaction,
                next.max_statement_and_encrypted_output_bytes_per_transaction,
            ),
            (
                PrivacyLimitFieldV1::NullifiersPerAction,
                self.max_nullifiers_per_action,
                next.max_nullifiers_per_action,
            ),
            (
                PrivacyLimitFieldV1::CommitmentsPerAction,
                self.max_commitments_per_action,
                next.max_commitments_per_action,
            ),
            (
                PrivacyLimitFieldV1::RetainedRootCount,
                self.retained_root_count,
                next.retained_root_count,
            ),
        ];
        for (field, current, candidate) in fields {
            if candidate > current {
                return Err(PrivacyConsensusLimitsTighteningErrorV1::Increase {
                    field,
                    current,
                    candidate,
                });
            }
        }
        if self == next {
            return Err(PrivacyConsensusLimitsTighteningErrorV1::NoChange);
        }
        Ok(())
    }
}
impl Default for PrivacyConsensusLimitsV1 {
    fn default() -> Self {
        Self::taira_default()
    }
}
fn validate_limit_order(
    smaller_field: PrivacyLimitFieldV1,
    smaller_value: u32,
    larger_field: PrivacyLimitFieldV1,
    larger_value: u32,
) -> Result<(), PrivacyConsensusLimitsValidationError> {
    if smaller_value > larger_value {
        return Err(PrivacyConsensusLimitsValidationError::InconsistentOrder {
            smaller_field,
            smaller_value,
            larger_field,
            larger_value,
        });
    }
    Ok(())
}
/// Validation failure for [`PrivacyConsensusLimitsV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyConsensusLimitsValidationError {
    /// A consensus limit is zero.
    #[error("privacy limit {field:?} must be non-zero")]
    Zero {
        /// Invalid field.
        field: PrivacyLimitFieldV1,
    },
    /// A consensus limit exceeds its first-release hard maximum.
    #[error("privacy limit {field:?} value {value} exceeds hard maximum {hard_max}")]
    ExceedsHardMaximum {
        /// Invalid field.
        field: PrivacyLimitFieldV1,
        /// Configured value.
        value: u32,
        /// First-release hard maximum.
        hard_max: u32,
    },
    /// A smaller-scope resource limit exceeds its containing scope.
    #[error(
        "privacy limit {smaller_field:?} value {smaller_value} exceeds {larger_field:?} value {larger_value}"
    )]
    InconsistentOrder {
        /// Field that must not exceed the containing field.
        smaller_field: PrivacyLimitFieldV1,
        /// Value of the smaller-scope field.
        smaller_value: u32,
        /// Containing field.
        larger_field: PrivacyLimitFieldV1,
        /// Value of the containing field.
        larger_value: u32,
    },
}
/// Validation failure for a component-wise consensus-policy tightening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyConsensusLimitsTighteningErrorV1 {
    /// The currently persisted policy is malformed.
    #[error("current privacy consensus limits are invalid: {0}")]
    InvalidCurrent(PrivacyConsensusLimitsValidationError),
    /// The proposed successor policy is malformed.
    #[error("next privacy consensus limits are invalid: {0}")]
    InvalidNext(PrivacyConsensusLimitsValidationError),
    /// A purported tightening increases one component.
    #[error(
        "privacy limit {field:?} cannot increase from {current} to {candidate} in a tightening"
    )]
    Increase {
        /// Increased component.
        field: PrivacyLimitFieldV1,
        /// Current component value.
        current: u32,
        /// Rejected successor value.
        candidate: u32,
    },
    /// A tightening must change at least one component.
    #[error("privacy consensus policy tightening is a no-op")]
    NoChange,
}
/// Scheduled successor for the singleton chain-wide privacy policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyConsensusPolicyTighteningV1 {
    /// Exact block which admitted this schedule.
    pub scheduled_at_height: u64,
    /// Exact incoming block at whose start the successor becomes effective.
    pub effective_at_height: u64,
    /// Complete component-wise-lower successor policy.
    pub next_limits: PrivacyConsensusLimitsV1,
}
impl PrivacyConsensusPolicyTighteningV1 {
    /// Validate schedule timing and component-wise monotonicity.
    ///
    /// # Errors
    ///
    /// Rejects zero/overflowing heights, insufficient notice, an invalid
    /// successor, any increase, or a no-op.
    pub fn validate_against(
        &self,
        current_limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyPolicyValidationErrorV1> {
        validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )?;
        current_limits
            .validate_tightening_to(&self.next_limits)
            .map_err(PrivacyPolicyValidationErrorV1::ConsensusTightening)
    }
}
/// Singleton chain-wide privacy admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyConsensusPolicyV1 {
    /// Limits effective for the current committed state.
    pub current_limits: PrivacyConsensusLimitsV1,
    /// At most one delayed, component-wise tightening.
    pub pending_tightening: Option<PrivacyConsensusPolicyTighteningV1>,
}
impl PrivacyConsensusPolicyV1 {
    /// Construct the first-release Taira policy with no pending change.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            current_limits: PrivacyConsensusLimitsV1::taira_default(),
            pending_tightening: None,
        }
    }
    /// Validate the complete persisted policy independent of chain height.
    ///
    /// # Errors
    ///
    /// Rejects invalid current limits or a malformed pending tightening.
    pub fn validate(&self) -> Result<(), PrivacyPolicyValidationErrorV1> {
        self.current_limits
            .validate()
            .map_err(PrivacyPolicyValidationErrorV1::InvalidCurrentLimits)?;
        if let Some(pending) = self.pending_tightening {
            pending.validate_against(&self.current_limits)?;
        }
        Ok(())
    }
    /// Validate a restored policy against the latest committed block height.
    ///
    /// A pending transition at height `E` is valid in a snapshot committed at
    /// `E - 1`, and invalid in a snapshot already committed at `E`.
    ///
    /// # Errors
    ///
    /// Rejects an intrinsically invalid policy or a missed/due transition.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyPolicyValidationErrorV1> {
        self.validate()?;
        if let Some(pending) = self.pending_tightening {
            if pending.scheduled_at_height > committed_height {
                return Err(
                    PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                        scheduled_at_height: pending.scheduled_at_height,
                        committed_height,
                    },
                );
            }
            if pending.effective_at_height <= committed_height {
                return Err(PrivacyPolicyValidationErrorV1::PendingNotFuture {
                    effective_at_height: pending.effective_at_height,
                    committed_height,
                });
            }
        }
        Ok(())
    }
    /// Root-retention cap enforced while admitting new roots.
    ///
    /// During the notice window new histories must already satisfy the pending
    /// lower limit so the effective-height transition is deterministic.
    #[must_use]
    pub const fn admission_retained_root_count(&self) -> u32 {
        match self.pending_tightening {
            Some(pending)
                if pending.next_limits.retained_root_count
                    < self.current_limits.retained_root_count =>
            {
                pending.next_limits.retained_root_count
            }
            _ => self.current_limits.retained_root_count,
        }
    }
}
impl Default for PrivacyConsensusPolicyV1 {
    fn default() -> Self {
        Self::taira_default()
    }
}
/// Validation failure for a singleton privacy-policy value or schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyPolicyValidationErrorV1 {
    /// Current limits are malformed.
    #[error("current privacy policy limits are invalid: {0}")]
    InvalidCurrentLimits(PrivacyConsensusLimitsValidationError),
    /// A scheduled consensus tightening is invalid.
    #[error("privacy consensus limits tightening is invalid: {0}")]
    ConsensusTightening(PrivacyConsensusLimitsTighteningErrorV1),
    /// Schedule admission height must be a real block height.
    #[error("privacy policy scheduled-at height must be non-zero")]
    ZeroScheduledHeight,
    /// Effective height must be strictly later than schedule admission.
    #[error(
        "privacy policy effective height {effective_at_height} must be later than scheduled height {scheduled_at_height}"
    )]
    EffectiveNotLater {
        /// Exact admission height.
        scheduled_at_height: u64,
        /// Rejected effective height.
        effective_at_height: u64,
    },
    /// The schedule does not provide the consensus minimum notice.
    #[error(
        "privacy policy effective height {effective_at_height} is earlier than minimum {earliest_effective_height}"
    )]
    LeadTimeTooShort {
        /// Rejected effective height.
        effective_at_height: u64,
        /// Earliest admissible effective height.
        earliest_effective_height: u64,
    },
    /// Adding the minimum notice overflows the height domain.
    #[error("privacy policy schedule height overflow")]
    HeightOverflow,
    /// A restored schedule claims admission after the snapshot it inhabits.
    #[error(
        "privacy policy scheduled-at height {scheduled_at_height} is after committed height {committed_height}"
    )]
    PendingScheduledAfterCommitted {
        /// Persisted admission height.
        scheduled_at_height: u64,
        /// Latest committed height.
        committed_height: u64,
    },
    /// A restored state retained a schedule which is already due or missed.
    #[error(
        "privacy policy effective height {effective_at_height} is not after committed height {committed_height}"
    )]
    PendingNotFuture {
        /// Persisted effective height.
        effective_at_height: u64,
        /// Latest committed height.
        committed_height: u64,
    },
}
fn validate_privacy_policy_schedule_heights_v1(
    scheduled_at_height: u64,
    effective_at_height: u64,
) -> Result<(), PrivacyPolicyValidationErrorV1> {
    if scheduled_at_height == 0 {
        return Err(PrivacyPolicyValidationErrorV1::ZeroScheduledHeight);
    }
    if effective_at_height <= scheduled_at_height {
        return Err(PrivacyPolicyValidationErrorV1::EffectiveNotLater {
            scheduled_at_height,
            effective_at_height,
        });
    }
    let earliest_effective_height = scheduled_at_height
        .checked_add(MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1)
        .ok_or(PrivacyPolicyValidationErrorV1::HeightOverflow)?;
    if effective_at_height < earliest_effective_height {
        return Err(PrivacyPolicyValidationErrorV1::LeadTimeTooShort {
            effective_at_height,
            earliest_effective_height,
        });
    }
    Ok(())
}
/// Proposed lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProposedLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// Scheduled first active height.
    pub activate_at_height: u64,
}
/// Active lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyActiveLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// Height at which the protocol was first activated.
    pub activated_at_height: u64,
    /// Height at which the current active interval began.
    pub state_since_height: u64,
}
/// Suspended lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacySuspendedLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// Height at which the protocol was first activated.
    pub activated_at_height: u64,
    /// Height at which the current suspension began.
    pub state_since_height: u64,
}
/// Retired lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyRetiredLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// First activation height, or `None` if retired before activation.
    pub activated_at_height: Option<u64>,
    /// Height at which retirement became effective.
    pub state_since_height: u64,
}
/// Governed lifecycle of a protocol activation record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "record", deny_unknown_fields)
)]
pub enum PrivacyProtocolLifecycleV1 {
    /// Governance approved a future activation height.
    #[cfg_attr(feature = "json", norito(rename = "proposed"))]
    Proposed(PrivacyProposedLifecycleV1),
    /// The protocol is currently active.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active(PrivacyActiveLifecycleV1),
    /// The protocol is temporarily fail-closed.
    #[cfg_attr(feature = "json", norito(rename = "suspended"))]
    Suspended(PrivacySuspendedLifecycleV1),
    /// The protocol is permanently unavailable.
    #[cfg_attr(feature = "json", norito(rename = "retired"))]
    Retired(PrivacyRetiredLifecycleV1),
}
impl PrivacyProtocolLifecycleV1 {
    /// Validate internal height ordering.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyLifecycleValidationError`] when a transition height is
    /// zero, equal to, or earlier than the height that must precede it.
    pub fn validate(&self) -> Result<(), PrivacyLifecycleValidationError> {
        match *self {
            Self::Proposed(state) => validate_strictly_later(
                PrivacyLifecycleHeightFieldV1::Proposed,
                state.proposed_at_height,
                PrivacyLifecycleHeightFieldV1::Activated,
                state.activate_at_height,
            ),
            Self::Active(state) => {
                validate_strictly_later(
                    PrivacyLifecycleHeightFieldV1::Proposed,
                    state.proposed_at_height,
                    PrivacyLifecycleHeightFieldV1::Activated,
                    state.activated_at_height,
                )?;
                if state.state_since_height < state.activated_at_height {
                    return Err(PrivacyLifecycleValidationError::HeightOrder {
                        earlier_field: PrivacyLifecycleHeightFieldV1::Activated,
                        earlier_height: state.activated_at_height,
                        later_field: PrivacyLifecycleHeightFieldV1::StateSince,
                        later_height: state.state_since_height,
                    });
                }
                Ok(())
            }
            Self::Suspended(state) => {
                validate_strictly_later(
                    PrivacyLifecycleHeightFieldV1::Proposed,
                    state.proposed_at_height,
                    PrivacyLifecycleHeightFieldV1::Activated,
                    state.activated_at_height,
                )?;
                validate_strictly_later(
                    PrivacyLifecycleHeightFieldV1::Activated,
                    state.activated_at_height,
                    PrivacyLifecycleHeightFieldV1::StateSince,
                    state.state_since_height,
                )
            }
            Self::Retired(state) => {
                if let Some(activated_at_height) = state.activated_at_height {
                    validate_strictly_later(
                        PrivacyLifecycleHeightFieldV1::Proposed,
                        state.proposed_at_height,
                        PrivacyLifecycleHeightFieldV1::Activated,
                        activated_at_height,
                    )?;
                    validate_strictly_later(
                        PrivacyLifecycleHeightFieldV1::Activated,
                        activated_at_height,
                        PrivacyLifecycleHeightFieldV1::StateSince,
                        state.state_since_height,
                    )
                } else {
                    validate_strictly_later(
                        PrivacyLifecycleHeightFieldV1::Proposed,
                        state.proposed_at_height,
                        PrivacyLifecycleHeightFieldV1::StateSince,
                        state.state_since_height,
                    )
                }
            }
        }
    }
    /// Return `true` only for the active lifecycle state.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }
    /// Return whether `next` is a valid forward lifecycle transition.
    #[must_use]
    pub fn can_transition_to(&self, next: &Self) -> bool {
        self.validate_transition_to(next).is_ok()
    }
    /// Validate a forward lifecycle transition and its immutable history.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyLifecycleTransitionError`] for invalid states, unsupported edges,
    /// mismatched proposal/activation history, or a non-increasing transition height.
    pub fn validate_transition_to(
        &self,
        next: &Self,
    ) -> Result<(), PrivacyLifecycleTransitionError> {
        self.validate()
            .map_err(PrivacyLifecycleTransitionError::CurrentState)?;
        next.validate()
            .map_err(PrivacyLifecycleTransitionError::NextState)?;
        match (*self, *next) {
            (Self::Proposed(current), Self::Active(next))
                if proposed_activation_history_matches(current, next) =>
            {
                Ok(())
            }
            (Self::Proposed(current), Self::Retired(next))
                if current.proposed_at_height == next.proposed_at_height
                    && next.activated_at_height.is_none()
                    && next.state_since_height <= current.activate_at_height =>
            {
                Ok(())
            }
            (Self::Active(current), Self::Suspended(next))
                if current.proposed_at_height == next.proposed_at_height
                    && current.activated_at_height == next.activated_at_height
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Active(current), Self::Retired(next))
                if current.proposed_at_height == next.proposed_at_height
                    && next.activated_at_height == Some(current.activated_at_height)
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Suspended(current), Self::Active(next))
                if current.proposed_at_height == next.proposed_at_height
                    && current.activated_at_height == next.activated_at_height
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Suspended(current), Self::Retired(next))
                if current.proposed_at_height == next.proposed_at_height
                    && next.activated_at_height == Some(current.activated_at_height)
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Retired(_), _) => Err(PrivacyLifecycleTransitionError::RetiredIsTerminal),
            _ => Err(PrivacyLifecycleTransitionError::InvalidTransition),
        }
    }
}
const fn proposed_activation_history_matches(
    proposed: PrivacyProposedLifecycleV1,
    active: PrivacyActiveLifecycleV1,
) -> bool {
    if proposed.proposed_at_height != active.proposed_at_height {
        return false;
    }
    if proposed.activate_at_height != active.activated_at_height {
        return false;
    }
    active.activated_at_height == active.state_since_height
}
fn validate_strictly_later(
    earlier_field: PrivacyLifecycleHeightFieldV1,
    earlier_height: u64,
    later_field: PrivacyLifecycleHeightFieldV1,
    later_height: u64,
) -> Result<(), PrivacyLifecycleValidationError> {
    if earlier_height == 0 {
        return Err(PrivacyLifecycleValidationError::ZeroHeight {
            field: earlier_field,
        });
    }
    if later_height == 0 {
        return Err(PrivacyLifecycleValidationError::ZeroHeight { field: later_field });
    }
    if later_height <= earlier_height {
        return Err(PrivacyLifecycleValidationError::HeightOrder {
            earlier_field,
            earlier_height,
            later_field,
            later_height,
        });
    }
    Ok(())
}
/// Height field within a privacy lifecycle record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLifecycleHeightFieldV1 {
    /// Proposal height.
    Proposed,
    /// First activation height.
    Activated,
    /// Height at which the current state began.
    StateSince,
}
/// Validation failure for one [`PrivacyProtocolLifecycleV1`] value.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyLifecycleValidationError {
    /// A lifecycle height is zero.
    #[error("privacy lifecycle height {field:?} must be non-zero")]
    ZeroHeight {
        /// Invalid height field.
        field: PrivacyLifecycleHeightFieldV1,
    },
    /// A later lifecycle height is not strictly later.
    #[error(
        "privacy lifecycle {later_field:?} height {later_height} must be later than {earlier_field:?} height {earlier_height}"
    )]
    HeightOrder {
        /// Earlier height field.
        earlier_field: PrivacyLifecycleHeightFieldV1,
        /// Earlier height value.
        earlier_height: u64,
        /// Later height field.
        later_field: PrivacyLifecycleHeightFieldV1,
        /// Later height value.
        later_height: u64,
    },
}
/// Validation failure for a lifecycle state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyLifecycleTransitionError {
    /// The current state is internally invalid.
    #[error("current privacy lifecycle state is invalid: {0}")]
    CurrentState(PrivacyLifecycleValidationError),
    /// The proposed next state is internally invalid.
    #[error("next privacy lifecycle state is invalid: {0}")]
    NextState(PrivacyLifecycleValidationError),
    /// The lifecycle edge or immutable history is invalid.
    #[error("privacy lifecycle transition is invalid")]
    InvalidTransition,
    /// A retired protocol cannot transition again.
    #[error("retired privacy protocol lifecycle is terminal")]
    RetiredIsTerminal,
}
/// Assurance classification for a first-release privacy activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "assurance", content = "value", deny_unknown_fields)
)]
pub enum PrivacyAssuranceV1 {
    /// Testnet-only experimental; not security-audited and not a production-readiness claim.
    #[cfg_attr(feature = "json", norito(rename = "experimental"))]
    Experimental,
}
/// Activation-specific Anonymous PGC policy limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AnonymousPgcActivationLimitsV1 {
    /// Maximum anonymity-set size `n` for this activation.
    pub max_anonymity_set_size: u32,
    /// Maximum intended recipient count `k` for this activation.
    pub max_recipient_count: u32,
}
/// Activation-specific `VeRange` aggregation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct VeRangeActivationLimitsV1 {
    /// Maximum aggregation count `T` admitted by this activation.
    pub max_aggregation_count: u32,
}
/// Activation-specific ZK-AMS admission and provisioning policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ZkAmsActivationLimitsV1 {
    /// Maximum ordered admission anchors in one batch settlement.
    pub max_batch_size: u32,
    /// Maximum admitted seed-key ring size in one provisioning action.
    pub max_ring_size: u32,
}
/// Activation-specific Jindo batched univariate-opening policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct JindoActivationLimitsV1 {
    /// Maximum polynomial commitments per statement.
    pub max_polynomial_count: u32,
}
/// Activation-specific Orchard action policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct OrchardActivationLimitsV1 {
    /// Maximum one-to-one spend/output actions per statement.
    pub max_action_count: u32,
}
/// Activation-specific FCMP++ transfer policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct FcmpActivationLimitsV1 {
    /// Maximum consumed outputs per transfer.
    pub max_input_count: u32,
    /// Maximum new outputs per transfer.
    pub max_output_count: u32,
}
/// Activation-specific native IVM private-note policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IvmPrivateNoteActivationLimitsV1 {
    /// Maximum consumed notes per action.
    pub max_input_count: u32,
    /// Maximum new notes per action.
    pub max_output_count: u32,
}
/// Activation-specific PQ-MASP policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PqMaspActivationLimitsV1 {
    /// Maximum consumed notes per action.
    pub max_input_count: u32,
    /// Maximum new notes per action.
    pub max_output_count: u32,
}
/// Protocol-specific governed limits carried by an activation record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "limits", deny_unknown_fields)
)]
pub enum PrivacyProtocolActivationLimitsV1 {
    /// ZK-ACE has no additional first-release count limits.
    #[cfg_attr(feature = "json", norito(rename = "zk-ace-pq-authorization-v0"))]
    ZkAcePqAuthorizationV0,
    /// Anonymous PGC receiver policy.
    #[cfg_attr(feature = "json", norito(rename = "anonymous-pgc-k-out-of-n-v1"))]
    AnonymousPgcKOutOfNV1(AnonymousPgcActivationLimitsV1),
    /// `VeRange` aggregation policy.
    #[cfg_attr(feature = "json", norito(rename = "verange-transparent-range-v1"))]
    VeRangeTransparentRangeV1(VeRangeActivationLimitsV1),
    /// ZK-AMS batch-admission and account-provisioning policy.
    #[cfg_attr(feature = "json", norito(rename = "iroha-zk-ams-v1"))]
    IrohaZkAmsV1(ZkAmsActivationLimitsV1),
    /// Vega has no additional first-release count limits.
    #[cfg_attr(feature = "json", norito(rename = "vega-existing-credential-zk-v0"))]
    VegaExistingCredentialZkV0,
    /// X.509 has fixed first-release limits encoded by its statement validator.
    #[cfg_attr(feature = "json", norito(rename = "iroha-zk-x509-stark-p256-v0"))]
    IrohaZkX509StarkP256V0,
    /// Jindo batched opening policy.
    #[cfg_attr(
        feature = "json",
        norito(rename = "iroha-jindo-polynomial-commitment-v0")
    )]
    IrohaJindoPolynomialCommitmentV0(JindoActivationLimitsV1),
    /// Lantern anonymous credentials have a fixed first-release parameter profile.
    #[cfg_attr(feature = "json", norito(rename = "iroha-bootle-lantern-anoncred-v1"))]
    IrohaBootleLanternAnoncredV1,
    /// Orchard one-to-one action policy.
    #[cfg_attr(feature = "json", norito(rename = "orchard-halo2-actions-v1"))]
    OrchardHalo2ActionsV1(OrchardActivationLimitsV1),
    /// FCMP++ input/output policy.
    #[cfg_attr(feature = "json", norito(rename = "monero-fcmp-plus-plus-v1"))]
    MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1),
    /// Native private-note input/output policy.
    #[cfg_attr(feature = "json", norito(rename = "iroha-ivm-private-note-stark-v1"))]
    IrohaIvmPrivateNoteStarkV1(IvmPrivateNoteActivationLimitsV1),
    /// PQ-MASP input/output policy.
    #[cfg_attr(feature = "json", norito(rename = "pq-masp-stark-v0"))]
    PqMaspStarkV0(PqMaspActivationLimitsV1),
}
impl PrivacyProtocolActivationLimitsV1 {
    /// Exact protocol to which these activation-specific limits apply.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0 => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::AnonymousPgcKOutOfNV1(_) => PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            Self::VeRangeTransparentRangeV1(_) => PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            Self::IrohaZkAmsV1(_) => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::VegaExistingCredentialZkV0 => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::IrohaZkX509StarkP256V0 => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::IrohaJindoPolynomialCommitmentV0(_) => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0
            }
            Self::IrohaBootleLanternAnoncredV1 => PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            Self::OrchardHalo2ActionsV1(_) => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV0(_) => PrivacyProtocolIdV1::PqMaspStarkV0,
        }
    }
    /// Validate activation-specific values against first-release hard ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProtocolActivationLimitsValidationError`] for zero or
    /// over-ceiling configuration values.
    pub fn validate(&self) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
        match *self {
            Self::AnonymousPgcKOutOfNV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::AnonymousPgcAnonymitySetSize,
                    limits.max_anonymity_set_size,
                    ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1,
                )?;
                if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&limits.max_anonymity_set_size) {
                    return Err(
                        PrivacyProtocolActivationLimitsValidationError::InvalidPgcAnonymitySetSize {
                            size: limits.max_anonymity_set_size,
                        },
                    );
                }
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::AnonymousPgcRecipientCount,
                    limits.max_recipient_count,
                    ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
                )
            }
            Self::VeRangeTransparentRangeV1(limits) => validate_profile_limit(
                PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                limits.max_aggregation_count,
                VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1,
            ),
            Self::IrohaZkAmsV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::ZkAmsBatchSize,
                    limits.max_batch_size,
                    ZK_AMS_MAX_BATCH_SIZE_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::ZkAmsRingSize,
                    limits.max_ring_size,
                    ZK_AMS_MAX_RING_SIZE_V1,
                )?;
                if !ZK_AMS_RING_SIZES_V1.contains(&limits.max_ring_size) {
                    return Err(
                        PrivacyProtocolActivationLimitsValidationError::InvalidZkAmsRingSize {
                            size: limits.max_ring_size,
                        },
                    );
                }
                Ok(())
            }
            Self::IrohaJindoPolynomialCommitmentV0(limits) => validate_profile_limit(
                PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                limits.max_polynomial_count,
                IROHA_JINDO_MAX_POLYNOMIALS_V1,
            ),
            Self::OrchardHalo2ActionsV1(limits) => validate_profile_limit(
                PrivacyActivationLimitFieldV1::OrchardActionCount,
                limits.max_action_count,
                ORCHARD_MAX_ACTIONS_V1,
            ),
            Self::MoneroFcmpPlusPlusV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::FcmpInputCount,
                    limits.max_input_count,
                    FCMP_MAX_INPUTS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::FcmpOutputCount,
                    limits.max_output_count,
                    FCMP_MAX_OUTPUTS_V1,
                )
            }
            Self::IrohaIvmPrivateNoteStarkV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteInputCount,
                    limits.max_input_count,
                    IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteOutputCount,
                    limits.max_output_count,
                    IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                )
            }
            Self::PqMaspStarkV0(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::PqMaspInputCount,
                    limits.max_input_count,
                    PQ_MASP_MAX_INPUTS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::PqMaspOutputCount,
                    limits.max_output_count,
                    PQ_MASP_MAX_OUTPUTS_V1,
                )
            }
            _ => Ok(()),
        }
    }
    /// Validate this governed protocol policy against a compiled ceiling.
    ///
    /// Both values first undergo their intrinsic nonzero, hard-maximum, and closed-set validation.
    /// The protocol variants must then match exactly, and every governed component must be less
    /// than or equal to its compiled counterpart.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProtocolActivationLimitsValidationError`] for an
    /// intrinsically invalid value or ceiling, a protocol-variant mismatch, or
    /// a component exceeding its configured ceiling.
    pub fn validate_with_ceiling(
        &self,
        ceiling: &Self,
    ) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
        self.validate()?;
        ceiling.validate()?;
        match (*self, *ceiling) {
            (Self::ZkAcePqAuthorizationV0, Self::ZkAcePqAuthorizationV0)
            | (Self::VegaExistingCredentialZkV0, Self::VegaExistingCredentialZkV0)
            | (Self::IrohaZkX509StarkP256V0, Self::IrohaZkX509StarkP256V0)
            | (Self::IrohaBootleLanternAnoncredV1, Self::IrohaBootleLanternAnoncredV1) => Ok(()),
            (Self::AnonymousPgcKOutOfNV1(value), Self::AnonymousPgcKOutOfNV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::AnonymousPgcAnonymitySetSize,
                    value.max_anonymity_set_size,
                    max.max_anonymity_set_size,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::AnonymousPgcRecipientCount,
                    value.max_recipient_count,
                    max.max_recipient_count,
                )
            }
            (Self::VeRangeTransparentRangeV1(value), Self::VeRangeTransparentRangeV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                    value.max_aggregation_count,
                    max.max_aggregation_count,
                )
            }
            (Self::IrohaZkAmsV1(value), Self::IrohaZkAmsV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::ZkAmsBatchSize,
                    value.max_batch_size,
                    max.max_batch_size,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::ZkAmsRingSize,
                    value.max_ring_size,
                    max.max_ring_size,
                )
            }
            (
                Self::IrohaJindoPolynomialCommitmentV0(value),
                Self::IrohaJindoPolynomialCommitmentV0(max),
            ) => validate_profile_limit_ceiling(
                PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                value.max_polynomial_count,
                max.max_polynomial_count,
            ),
            (Self::OrchardHalo2ActionsV1(value), Self::OrchardHalo2ActionsV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::OrchardActionCount,
                    value.max_action_count,
                    max.max_action_count,
                )
            }
            (Self::MoneroFcmpPlusPlusV1(value), Self::MoneroFcmpPlusPlusV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::FcmpInputCount,
                    value.max_input_count,
                    max.max_input_count,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::FcmpOutputCount,
                    value.max_output_count,
                    max.max_output_count,
                )
            }
            (Self::IrohaIvmPrivateNoteStarkV1(value), Self::IrohaIvmPrivateNoteStarkV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteInputCount,
                    value.max_input_count,
                    max.max_input_count,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteOutputCount,
                    value.max_output_count,
                    max.max_output_count,
                )
            }
            (Self::PqMaspStarkV0(value), Self::PqMaspStarkV0(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::PqMaspInputCount,
                    value.max_input_count,
                    max.max_input_count,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::PqMaspOutputCount,
                    value.max_output_count,
                    max.max_output_count,
                )
            }
            _ => Err(
                PrivacyProtocolActivationLimitsValidationError::ProtocolMismatch {
                    actual: self.protocol_id(),
                    ceiling: ceiling.protocol_id(),
                },
            ),
        }
    }
}
fn validate_profile_limit_ceiling(
    field: PrivacyActivationLimitFieldV1,
    value: u32,
    ceiling: u32,
) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
    if value > ceiling {
        return Err(
            PrivacyProtocolActivationLimitsValidationError::ExceedsConfiguredCeiling {
                field,
                value,
                ceiling,
            },
        );
    }
    Ok(())
}
fn validate_profile_limit(
    field: PrivacyActivationLimitFieldV1,
    value: u32,
    hard_max: u32,
) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
    if value == 0 {
        return Err(PrivacyProtocolActivationLimitsValidationError::Zero { field });
    }
    if value > hard_max {
        return Err(
            PrivacyProtocolActivationLimitsValidationError::ExceedsHardMaximum {
                field,
                value,
                hard_max,
            },
        );
    }
    Ok(())
}
/// Activation-specific limit field selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyActivationLimitFieldV1 {
    /// Anonymous PGC anonymity-set size.
    AnonymousPgcAnonymitySetSize,
    /// Anonymous PGC intended recipient count.
    AnonymousPgcRecipientCount,
    /// `VeRange` aggregation count.
    VeRangeAggregationCount,
    /// ZK-AMS batch size.
    ZkAmsBatchSize,
    /// ZK-AMS admitted seed-key ring size.
    ZkAmsRingSize,
    /// Jindo polynomial count.
    JindoPolynomialCount,
    /// Orchard one-to-one action count.
    OrchardActionCount,
    /// FCMP++ input count.
    FcmpInputCount,
    /// FCMP++ output count.
    FcmpOutputCount,
    /// Native IVM private-note input count.
    IvmPrivateNoteInputCount,
    /// Native IVM private-note output count.
    IvmPrivateNoteOutputCount,
    /// PQ-MASP input count.
    PqMaspInputCount,
    /// PQ-MASP output count.
    PqMaspOutputCount,
}
/// Validation failure for protocol-specific activation limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyProtocolActivationLimitsValidationError {
    /// One activation-specific limit is zero.
    #[error("privacy activation limit {field:?} must be non-zero")]
    Zero {
        /// Invalid field.
        field: PrivacyActivationLimitFieldV1,
    },
    /// One activation-specific limit exceeds its first-release hard maximum.
    #[error("privacy activation limit {field:?} value {value} exceeds hard maximum {hard_max}")]
    ExceedsHardMaximum {
        /// Invalid field.
        field: PrivacyActivationLimitFieldV1,
        /// Configured value.
        value: u32,
        /// First-release hard maximum.
        hard_max: u32,
    },
    /// Activation limits and their configured ceiling target different protocols.
    #[error(
        "privacy activation limit protocol {actual:?} differs from ceiling protocol {ceiling:?}"
    )]
    ProtocolMismatch {
        /// Governed protocol variant.
        actual: PrivacyProtocolIdV1,
        /// Compiled-ceiling protocol variant.
        ceiling: PrivacyProtocolIdV1,
    },
    /// One activation-specific limit exceeds a valid configured ceiling.
    #[error(
        "privacy activation limit {field:?} value {value} exceeds configured ceiling {ceiling}"
    )]
    ExceedsConfiguredCeiling {
        /// Invalid field.
        field: PrivacyActivationLimitFieldV1,
        /// Governed value.
        value: u32,
        /// Component-wise ceiling.
        ceiling: u32,
    },
    /// Anonymous PGC activation size is not one of the closed set sizes.
    #[error("Anonymous PGC anonymity-set size {size} is not one of 16, 32, or 64")]
    InvalidPgcAnonymitySetSize {
        /// Invalid configured size.
        size: u32,
    },
    /// ZK-AMS activation ring size is not one of the closed profile sizes.
    #[error("ZK-AMS ring size {size} is not one of 16, 32, or 64")]
    InvalidZkAmsRingSize {
        /// Invalid configured size.
        size: u32,
    },
}
/// Scheduled component-wise tightening for one protocol activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProtocolLimitsTighteningV1 {
    /// Exact block which admitted this schedule.
    pub scheduled_at_height: u64,
    /// Exact incoming block at whose start the successor becomes effective.
    pub effective_at_height: u64,
    /// Complete protocol-tagged successor limit set.
    pub next_limits: PrivacyProtocolActivationLimitsV1,
}
impl PrivacyProtocolLimitsTighteningV1 {
    /// Validate schedule timing and strict component-wise monotonicity.
    ///
    /// # Errors
    ///
    /// Rejects insufficient notice, a protocol mismatch, invalid limits, an increase, or a no-op.
    pub fn validate_against(
        &self,
        current_limits: &PrivacyProtocolActivationLimitsV1,
    ) -> Result<(), PrivacyProtocolLimitsTighteningValidationErrorV1> {
        validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )
        .map_err(PrivacyProtocolLimitsTighteningValidationErrorV1::Schedule)?;
        self.next_limits
            .validate_with_ceiling(current_limits)
            .map_err(PrivacyProtocolLimitsTighteningValidationErrorV1::Limits)?;
        if self.next_limits == *current_limits {
            return Err(PrivacyProtocolLimitsTighteningValidationErrorV1::NoChange);
        }
        Ok(())
    }
}
/// Validation failure for a scheduled protocol-specific tightening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyProtocolLimitsTighteningValidationErrorV1 {
    /// Scheduled/effective heights violate the chain-wide notice rule.
    #[error("privacy protocol-limit schedule is invalid: {0}")]
    Schedule(PrivacyPolicyValidationErrorV1),
    /// Successor limits are invalid, mismatched, or increase a component.
    #[error("privacy protocol-limit tightening is invalid: {0}")]
    Limits(PrivacyProtocolActivationLimitsValidationError),
    /// A tightening must change at least one component.
    #[error("privacy protocol-limit tightening is a no-op")]
    NoChange,
}
/// Governed activation record for one exact privacy protocol implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProtocolActivationRecordV1 {
    /// Exact protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact proof-system profile.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Exact native engine identity.
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
    /// Current governed lifecycle.
    pub lifecycle: PrivacyProtocolLifecycleV1,
    /// Protocol-specific governed count limits.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
    /// At most one delayed, component-wise protocol-limit tightening.
    pub pending_protocol_limits_tightening: Option<PrivacyProtocolLimitsTighteningV1>,
    /// Testnet assurance classification.
    pub assurance: PrivacyAssuranceV1,
}
impl PrivacyProtocolActivationRecordV1 {
    /// Validate exact protocol mappings, non-zero digests, lifecycle, and limits.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyActivationValidationError`] on the first invalid
    /// binding in deterministic field order.
    pub fn validate(&self) -> Result<(), PrivacyActivationValidationError> {
        let expected_proof_system = self.protocol_id.expected_proof_system();
        if self.proof_system_id != expected_proof_system {
            return Err(PrivacyActivationValidationError::ProofSystemMismatch {
                protocol_id: self.protocol_id,
                expected: expected_proof_system,
                actual: self.proof_system_id,
            });
        }
        let expected_engine = self.protocol_id.expected_engine();
        if self.engine_id != expected_engine {
            return Err(PrivacyActivationValidationError::EngineMismatch {
                protocol_id: self.protocol_id,
                expected: expected_engine,
                actual: self.engine_id,
            });
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroEngineManifestDigest);
        }
        let limits_protocol = self.protocol_limits.protocol_id();
        if limits_protocol != self.protocol_id {
            return Err(PrivacyActivationValidationError::ProtocolLimitsMismatch {
                protocol_id: self.protocol_id,
                limits_protocol,
            });
        }
        self.protocol_limits
            .validate()
            .map_err(PrivacyActivationValidationError::ProtocolLimits)?;
        if let Some(pending) = self.pending_protocol_limits_tightening {
            pending
                .validate_against(&self.protocol_limits)
                .map_err(PrivacyActivationValidationError::PendingProtocolLimits)?;
        }
        self.lifecycle
            .validate()
            .map_err(PrivacyActivationValidationError::Lifecycle)
    }
}
/// Validation failure for [`PrivacyProtocolActivationRecordV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyActivationValidationError {
    /// Protocol and proof-system identities do not match.
    #[error("privacy protocol {protocol_id:?} requires proof system {expected:?}, got {actual:?}")]
    ProofSystemMismatch {
        /// Protocol being activated.
        protocol_id: PrivacyProtocolIdV1,
        /// Required proof system.
        expected: PrivacyProofSystemIdV1,
        /// Supplied proof system.
        actual: PrivacyProofSystemIdV1,
    },
    /// Protocol and native engine identities do not match.
    #[error("privacy protocol {protocol_id:?} requires engine {expected:?}, got {actual:?}")]
    EngineMismatch {
        /// Protocol being activated.
        protocol_id: PrivacyProtocolIdV1,
        /// Required native engine.
        expected: PrivacyEngineIdV1,
        /// Supplied native engine.
        actual: PrivacyEngineIdV1,
    },
    /// Governed parameter-set identifier is zero.
    #[error("privacy activation parameter id must be non-zero")]
    ZeroParameterId,
    /// Governed parameter digest is zero.
    #[error("privacy activation parameter digest must be non-zero")]
    ZeroParameterDigest,
    /// Governed verifier digest is zero.
    #[error("privacy activation verifier digest must be non-zero")]
    ZeroVerifierDigest,
    /// Governed statement-schema digest is zero.
    #[error("privacy activation statement-schema digest must be non-zero")]
    ZeroStatementSchemaDigest,
    /// Governed engine-manifest digest is zero.
    #[error("privacy activation engine-manifest digest must be non-zero")]
    ZeroEngineManifestDigest,
    /// Protocol-specific limits are tagged for another protocol.
    #[error(
        "privacy activation protocol {protocol_id:?} differs from protocol-limit tag {limits_protocol:?}"
    )]
    ProtocolLimitsMismatch {
        /// Activated protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Protocol encoded by the activation-specific limits.
        limits_protocol: PrivacyProtocolIdV1,
    },
    /// Protocol-specific activation limits are invalid.
    #[error("privacy activation protocol limits are invalid: {0}")]
    ProtocolLimits(PrivacyProtocolActivationLimitsValidationError),
    /// A pending protocol-specific tightening is invalid.
    #[error("privacy activation pending protocol limits are invalid: {0}")]
    PendingProtocolLimits(PrivacyProtocolLimitsTighteningValidationErrorV1),
    /// Lifecycle is invalid.
    #[error("privacy activation lifecycle is invalid: {0}")]
    Lifecycle(PrivacyLifecycleValidationError),
}
/// Exact public capability-snapshot wire version.
pub const PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1: u32 = 1;
/// Exact local compiled-profile catalog wire version.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1: u32 = 1;
/// Exact locally compiled bindings exposed by the public privacy snapshot.
///
/// This is a wire-model counterpart of the core-only compiled profile. It
/// deliberately contains no lifecycle or readiness boolean: governance state
/// is carried separately by [`PrivacyCapabilityRowV1::activation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCompiledProfileSnapshotV1 {
    /// Closed protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Closed proof-system identity.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Closed native-engine identity.
    pub engine_id: PrivacyEngineIdV1,
    /// Deterministic identifier of the compiled parameter set.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the exact compiled parameters.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier relation and proof wire.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of the exact public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the complete compiled engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Exact protocol-specific limits compiled into the verifier.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
}
impl PrivacyCompiledProfileSnapshotV1 {
    /// Validate the closed protocol mappings and every fixed binding.
    ///
    /// # Errors
    ///
    /// Returns a deterministic error for the first mismatched identity, zero
    /// binding, protocol-tag mismatch, or invalid limit.
    pub fn validate(&self) -> Result<(), PrivacyCompiledProfileSnapshotValidationErrorV1> {
        let expected_proof_system = self.protocol_id.expected_proof_system();
        if self.proof_system_id != expected_proof_system {
            return Err(
                PrivacyCompiledProfileSnapshotValidationErrorV1::ProofSystemMismatch {
                    protocol_id: self.protocol_id,
                    expected: expected_proof_system,
                    actual: self.proof_system_id,
                },
            );
        }
        let expected_engine = self.protocol_id.expected_engine();
        if self.engine_id != expected_engine {
            return Err(
                PrivacyCompiledProfileSnapshotValidationErrorV1::EngineMismatch {
                    protocol_id: self.protocol_id,
                    expected: expected_engine,
                    actual: self.engine_id,
                },
            );
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroEngineManifestDigest);
        }
        let limits_protocol = self.protocol_limits.protocol_id();
        if limits_protocol != self.protocol_id {
            return Err(
                PrivacyCompiledProfileSnapshotValidationErrorV1::ProtocolLimitsMismatch {
                    protocol_id: self.protocol_id,
                    limits_protocol,
                },
            );
        }
        self.protocol_limits
            .validate()
            .map_err(PrivacyCompiledProfileSnapshotValidationErrorV1::ProtocolLimits)
    }
}
/// Validation failure for [`PrivacyCompiledProfileSnapshotV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCompiledProfileSnapshotValidationErrorV1 {
    /// Protocol and proof-system identities differ.
    #[error(
        "compiled privacy protocol {protocol_id:?} requires proof system {expected:?}, got {actual:?}"
    )]
    ProofSystemMismatch {
        /// Protocol in the compiled profile.
        protocol_id: PrivacyProtocolIdV1,
        /// Required proof system.
        expected: PrivacyProofSystemIdV1,
        /// Rejected proof system.
        actual: PrivacyProofSystemIdV1,
    },
    /// Protocol and native-engine identities differ.
    #[error(
        "compiled privacy protocol {protocol_id:?} requires engine {expected:?}, got {actual:?}"
    )]
    EngineMismatch {
        /// Protocol in the compiled profile.
        protocol_id: PrivacyProtocolIdV1,
        /// Required engine.
        expected: PrivacyEngineIdV1,
        /// Rejected engine.
        actual: PrivacyEngineIdV1,
    },
    /// Compiled parameter-set identifier is zero.
    #[error("compiled privacy parameter id must be non-zero")]
    ZeroParameterId,
    /// Compiled parameter digest is zero.
    #[error("compiled privacy parameter digest must be non-zero")]
    ZeroParameterDigest,
    /// Compiled verifier digest is zero.
    #[error("compiled privacy verifier digest must be non-zero")]
    ZeroVerifierDigest,
    /// Compiled statement-schema digest is zero.
    #[error("compiled privacy statement-schema digest must be non-zero")]
    ZeroStatementSchemaDigest,
    /// Compiled engine-manifest digest is zero.
    #[error("compiled privacy engine-manifest digest must be non-zero")]
    ZeroEngineManifestDigest,
    /// Compiled limits are tagged for another protocol.
    #[error(
        "compiled privacy protocol {protocol_id:?} differs from protocol-limit tag {limits_protocol:?}"
    )]
    ProtocolLimitsMismatch {
        /// Compiled protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Protocol encoded by the compiled limit variant.
        limits_protocol: PrivacyProtocolIdV1,
    },
    /// Compiled protocol-specific limits are malformed.
    #[error("compiled privacy protocol limits are invalid: {0}")]
    ProtocolLimits(PrivacyProtocolActivationLimitsValidationError),
}
/// Typed failure canonicalizing a compiled public-statement schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "schema_error", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCompiledStatementSchemaErrorV1 {
    /// Two types reused one stable identifier for incompatible shapes.
    #[cfg_attr(feature = "json", norito(rename = "conflicting-stable-type-id"))]
    ConflictingStableTypeId,
    /// A schema referenced a type absent from the canonical map.
    #[cfg_attr(feature = "json", norito(rename = "missing-type-reference"))]
    MissingTypeReference,
}
/// Typed reason why one closed protocol has no executable compiled profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "reason", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCompiledProfileUnavailableReasonV1 {
    /// This binary contains no complete end-to-end engine for the protocol.
    #[cfg_attr(feature = "json", norito(rename = "engine-unavailable"))]
    EngineUnavailable,
    /// Deterministic transparent parameter initialization failed.
    #[cfg_attr(feature = "json", norito(rename = "profile-initialization-failed"))]
    ProfileInitializationFailed,
    /// The locally generated statement schema was ambiguous or incomplete.
    #[cfg_attr(feature = "json", norito(rename = "statement-schema-invalid"))]
    StatementSchemaInvalid(PrivacyCompiledStatementSchemaErrorV1),
}
/// Closed result of obtaining one locally compiled privacy profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", deny_unknown_fields)
)]
pub enum PrivacyCompiledProfileResultV1 {
    /// The exact native profile is executable in this binary.
    #[cfg_attr(feature = "json", norito(rename = "available"))]
    Available(PrivacyCompiledProfileSnapshotV1),
    /// The protocol remains explicitly unavailable and fail-closed.
    #[cfg_attr(feature = "json", norito(rename = "unavailable"))]
    Unavailable(PrivacyCompiledProfileUnavailableReasonV1),
}
/// One local build result in the canonical compiled-profile catalog.
///
/// This row deliberately has no activation, lifecycle, committed height, or consensus-policy field.
/// It describes only what the current binary was compiled to execute. Authoritative readiness comes
/// exclusively from a committed [`PrivacyExact12CapabilityManifestV1`] returned by Torii.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCompiledProfileCatalogRowV1 {
    /// Closed protocol identity for this row.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact local compiled-profile result.
    pub compiled_profile: PrivacyCompiledProfileResultV1,
}
impl PrivacyCompiledProfileCatalogRowV1 {
    /// Validate the row's closed identity and compiled profile.
    ///
    /// # Errors
    ///
    /// Rejects a malformed available profile or a profile tagged for a different protocol.
    pub fn validate(&self) -> Result<(), PrivacyCompiledProfileCatalogRowValidationErrorV1> {
        let PrivacyCompiledProfileResultV1::Available(profile) = self.compiled_profile else {
            return Ok(());
        };
        profile
            .validate()
            .map_err(PrivacyCompiledProfileCatalogRowValidationErrorV1::CompiledProfile)?;
        if profile.protocol_id != self.protocol_id {
            return Err(
                PrivacyCompiledProfileCatalogRowValidationErrorV1::CompiledProfileProtocolMismatch {
                    row_protocol: self.protocol_id,
                    profile_protocol: profile.protocol_id,
                },
            );
        }
        Ok(())
    }
}
/// Validation failure for one [`PrivacyCompiledProfileCatalogRowV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCompiledProfileCatalogRowValidationErrorV1 {
    /// Locally compiled profile is malformed.
    #[error("compiled privacy catalog profile is invalid: {0}")]
    CompiledProfile(PrivacyCompiledProfileSnapshotValidationErrorV1),
    /// Row and compiled-profile identities differ.
    #[error(
        "compiled privacy catalog row {row_protocol:?} differs from profile {profile_protocol:?}"
    )]
    CompiledProfileProtocolMismatch {
        /// Row identity.
        row_protocol: PrivacyProtocolIdV1,
        /// Embedded profile identity.
        profile_protocol: PrivacyProtocolIdV1,
    },
}
/// Canonical local compiled-profile catalog for the closed first-release registry.
///
/// `protocols` contains exactly [`PrivacyProtocolIdV1::ALL`] in Norito
/// discriminant order. This catalog is local build metadata, not committed
/// governance state and not evidence that a protocol is live on any network.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.compiled-profile-catalog.v1")]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCompiledProfileCatalogV1 {
    /// Exact catalog schema version.
    pub version: u32,
    /// Exactly twelve local results in canonical discriminant order.
    pub protocols: Vec<PrivacyCompiledProfileCatalogRowV1>,
}
impl PrivacyCompiledProfileCatalogV1 {
    /// Validate the complete closed local catalog.
    ///
    /// # Errors
    ///
    /// Rejects an unknown version, any row-count or ordering drift, or an
    /// invalid compiled-profile row.
    pub fn validate(&self) -> Result<(), PrivacyCompiledProfileCatalogValidationErrorV1> {
        if self.version != PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1 {
            return Err(PrivacyCompiledProfileCatalogValidationErrorV1::Version {
                expected: PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1,
                actual: self.version,
            });
        }
        if self.protocols.len() != PrivacyProtocolIdV1::COUNT {
            return Err(
                PrivacyCompiledProfileCatalogValidationErrorV1::ProtocolCount {
                    expected: PrivacyProtocolIdV1::COUNT,
                    actual: self.protocols.len(),
                },
            );
        }
        for (index, (row, expected)) in self
            .protocols
            .iter()
            .zip(PrivacyProtocolIdV1::ALL)
            .enumerate()
        {
            if row.protocol_id != expected {
                return Err(
                    PrivacyCompiledProfileCatalogValidationErrorV1::ProtocolOrder {
                        index,
                        expected,
                        actual: row.protocol_id,
                    },
                );
            }
            row.validate().map_err(|source| {
                PrivacyCompiledProfileCatalogValidationErrorV1::ProtocolRow {
                    protocol_id: expected,
                    source,
                }
            })?;
        }
        Ok(())
    }
}
/// Validation failure for [`PrivacyCompiledProfileCatalogV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCompiledProfileCatalogValidationErrorV1 {
    /// Catalog wire version is not the exact first-release version.
    #[error("compiled privacy catalog version {actual} differs from required {expected}")]
    Version {
        /// Required version.
        expected: u32,
        /// Rejected version.
        actual: u32,
    },
    /// Protocol row count differs from the closed registry.
    #[error("compiled privacy catalog has {actual} rows; expected {expected}")]
    ProtocolCount {
        /// Closed first-release row count.
        expected: usize,
        /// Rejected row count.
        actual: usize,
    },
    /// A row is missing, duplicated, or reordered.
    #[error(
        "compiled privacy catalog row {index} is {actual:?}; expected canonical protocol {expected:?}"
    )]
    ProtocolOrder {
        /// Zero-based row index.
        index: usize,
        /// Required protocol at this index.
        expected: PrivacyProtocolIdV1,
        /// Rejected protocol at this index.
        actual: PrivacyProtocolIdV1,
    },
    /// One canonical row is invalid.
    #[error("compiled privacy catalog row {protocol_id:?} is invalid: {source}")]
    ProtocolRow {
        /// Protocol selected by row order.
        protocol_id: PrivacyProtocolIdV1,
        /// Exact row validation failure.
        source: PrivacyCompiledProfileCatalogRowValidationErrorV1,
    },
}
/// One protocol row in the canonical public capability snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCapabilityRowV1 {
    /// Closed protocol identity for this row.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact local compiled-profile result.
    pub compiled_profile: PrivacyCompiledProfileResultV1,
    /// Exact committed governance record, if registered.
    pub activation: Option<PrivacyProtocolActivationRecordV1>,
}
impl PrivacyCapabilityRowV1 {
    /// Validate a row against its committed snapshot height.
    ///
    /// # Errors
    ///
    /// Rejects embedded identity mismatches, malformed compiled profiles,
    /// activation without an executable engine, activation/profile binding
    /// drift, and lifecycle or policy heights inconsistent with the snapshot.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyCapabilityRowValidationErrorV1> {
        let profile = match self.compiled_profile {
            PrivacyCompiledProfileResultV1::Available(profile) => {
                profile
                    .validate()
                    .map_err(PrivacyCapabilityRowValidationErrorV1::CompiledProfile)?;
                if profile.protocol_id != self.protocol_id {
                    return Err(
                        PrivacyCapabilityRowValidationErrorV1::CompiledProfileProtocolMismatch {
                            row_protocol: self.protocol_id,
                            profile_protocol: profile.protocol_id,
                        },
                    );
                }
                Some(profile)
            }
            PrivacyCompiledProfileResultV1::Unavailable(_) => None,
        };
        let Some(activation) = self.activation else {
            return Ok(());
        };
        let Some(profile) = profile else {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::UnavailableActivation {
                    protocol_id: self.protocol_id,
                },
            );
        };
        activation
            .validate()
            .map_err(PrivacyCapabilityRowValidationErrorV1::Activation)?;
        if activation.protocol_id != self.protocol_id {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::ActivationProtocolMismatch {
                    row_protocol: self.protocol_id,
                    activation_protocol: activation.protocol_id,
                },
            );
        }
        validate_privacy_capability_activation_profile_v1(&activation, &profile)?;
        validate_privacy_capability_activation_height_v1(&activation, committed_height)
    }
}
fn validate_privacy_capability_activation_profile_v1(
    activation: &PrivacyProtocolActivationRecordV1,
    profile: &PrivacyCompiledProfileSnapshotV1,
) -> Result<(), PrivacyCapabilityRowValidationErrorV1> {
    if activation.proof_system_id != profile.proof_system_id {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::ProofSystem,
            },
        );
    }
    if activation.engine_id != profile.engine_id {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::Engine,
            },
        );
    }
    if activation.parameter_id != profile.parameter_id {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::ParameterId,
            },
        );
    }
    if activation.parameter_digest != profile.parameter_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::ParameterDigest,
            },
        );
    }
    if activation.verifier_digest != profile.verifier_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::VerifierDigest,
            },
        );
    }
    if activation.statement_schema_digest != profile.statement_schema_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::StatementSchemaDigest,
            },
        );
    }
    if activation.engine_manifest_digest != profile.engine_manifest_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::EngineManifestDigest,
            },
        );
    }
    activation
        .protocol_limits
        .validate_with_ceiling(&profile.protocol_limits)
        .map_err(PrivacyCapabilityRowValidationErrorV1::ActivationProtocolLimits)
}
fn validate_privacy_capability_activation_height_v1(
    activation: &PrivacyProtocolActivationRecordV1,
    committed_height: u64,
) -> Result<(), PrivacyCapabilityRowValidationErrorV1> {
    let (proposed_at_height, activated_at_height, state_since_height) = match activation.lifecycle {
        PrivacyProtocolLifecycleV1::Proposed(state) => {
            if state.activate_at_height <= committed_height {
                return Err(
                    PrivacyCapabilityRowValidationErrorV1::UnpromotedDueActivation {
                        activate_at_height: state.activate_at_height,
                        committed_height,
                    },
                );
            }
            (state.proposed_at_height, None, None)
        }
        PrivacyProtocolLifecycleV1::Active(state) => (
            state.proposed_at_height,
            Some(state.activated_at_height),
            Some(state.state_since_height),
        ),
        PrivacyProtocolLifecycleV1::Suspended(state) => (
            state.proposed_at_height,
            Some(state.activated_at_height),
            Some(state.state_since_height),
        ),
        PrivacyProtocolLifecycleV1::Retired(state) => (
            state.proposed_at_height,
            state.activated_at_height,
            Some(state.state_since_height),
        ),
    };
    if proposed_at_height > committed_height {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ProposalAfterCommitted {
                proposed_at_height,
                committed_height,
            },
        );
    }
    if let Some(activated_at_height) = activated_at_height
        && activated_at_height > committed_height
    {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationAfterCommitted {
                activated_at_height,
                committed_height,
            },
        );
    }
    if let Some(state_since_height) = state_since_height
        && state_since_height > committed_height
    {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::LifecycleStateAfterCommitted {
                state_since_height,
                committed_height,
            },
        );
    }
    if let Some(pending) = activation.pending_protocol_limits_tightening {
        if pending.scheduled_at_height > committed_height {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::ProtocolLimitsScheduledAfterCommitted {
                    scheduled_at_height: pending.scheduled_at_height,
                    committed_height,
                },
            );
        }
        if pending.effective_at_height <= committed_height {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::ProtocolLimitsNotFuture {
                    effective_at_height: pending.effective_at_height,
                    committed_height,
                },
            );
        }
    }
    Ok(())
}
/// Immutable binding selected when comparing activation and compiled profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyCapabilityBindingFieldV1 {
    /// Proof-system identity.
    ProofSystem,
    /// Native-engine identity.
    Engine,
    /// Parameter-set identifier.
    ParameterId,
    /// Parameter-set digest.
    ParameterDigest,
    /// Verifier digest.
    VerifierDigest,
    /// Statement-schema digest.
    StatementSchemaDigest,
    /// Engine-manifest digest.
    EngineManifestDigest,
}
/// Validation failure for one [`PrivacyCapabilityRowV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCapabilityRowValidationErrorV1 {
    /// Locally compiled profile is malformed.
    #[error("privacy capability compiled profile is invalid: {0}")]
    CompiledProfile(PrivacyCompiledProfileSnapshotValidationErrorV1),
    /// Row and compiled-profile identities differ.
    #[error(
        "privacy capability row protocol {row_protocol:?} differs from compiled profile {profile_protocol:?}"
    )]
    CompiledProfileProtocolMismatch {
        /// Row identity.
        row_protocol: PrivacyProtocolIdV1,
        /// Embedded profile identity.
        profile_protocol: PrivacyProtocolIdV1,
    },
    /// A governance activation exists for an unavailable local engine.
    #[error("unavailable privacy protocol {protocol_id:?} cannot have an activation")]
    UnavailableActivation {
        /// Unavailable protocol.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Governed activation is malformed.
    #[error("privacy capability activation is invalid: {0}")]
    Activation(PrivacyActivationValidationError),
    /// Row and governed activation identities differ.
    #[error(
        "privacy capability row protocol {row_protocol:?} differs from activation {activation_protocol:?}"
    )]
    ActivationProtocolMismatch {
        /// Row identity.
        row_protocol: PrivacyProtocolIdV1,
        /// Embedded activation identity.
        activation_protocol: PrivacyProtocolIdV1,
    },
    /// An immutable governed binding differs from the compiled profile.
    #[error("privacy activation differs from compiled profile at {field:?}")]
    ActivationProfileMismatch {
        /// Mismatched immutable field.
        field: PrivacyCapabilityBindingFieldV1,
    },
    /// Governed protocol limits exceed the compiled profile.
    #[error("privacy activation limits differ from compiled profile: {0}")]
    ActivationProtocolLimits(PrivacyProtocolActivationLimitsValidationError),
    /// Proposal admission is later than the snapshot that contains it.
    #[error(
        "privacy proposal height {proposed_at_height} is after committed height {committed_height}"
    )]
    ProposalAfterCommitted {
        /// Persisted proposal height.
        proposed_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// A due proposal remained unpromoted in committed state.
    #[error(
        "privacy activation at height {activate_at_height} remained proposed at committed height {committed_height}"
    )]
    UnpromotedDueActivation {
        /// Scheduled activation height.
        activate_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// First activation is later than the committed snapshot.
    #[error(
        "privacy activation height {activated_at_height} is after committed height {committed_height}"
    )]
    ActivationAfterCommitted {
        /// Claimed first activation height.
        activated_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// Current lifecycle interval begins after the committed snapshot.
    #[error(
        "privacy lifecycle state height {state_since_height} is after committed height {committed_height}"
    )]
    LifecycleStateAfterCommitted {
        /// Claimed current-state start height.
        state_since_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// Protocol-limit schedule claims admission after the snapshot.
    #[error(
        "privacy protocol-limit schedule height {scheduled_at_height} is after committed height {committed_height}"
    )]
    ProtocolLimitsScheduledAfterCommitted {
        /// Claimed admission height.
        scheduled_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// Protocol-limit schedule was retained after its exact effective height.
    #[error(
        "privacy protocol-limit effective height {effective_at_height} is not after committed height {committed_height}"
    )]
    ProtocolLimitsNotFuture {
        /// Scheduled effective height.
        effective_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
}
/// Authoritative committed privacy capability snapshot.
///
/// `protocols` must contain exactly [`PrivacyProtocolIdV1::ALL`] in Norito
/// discriminant order. The ordering rule makes missing, duplicate, and
/// reordered rows fail closed without accepting aliases.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCapabilitySnapshotV1 {
    /// Exact snapshot schema version.
    pub version: u32,
    /// Height of the committed state from which this snapshot was read.
    pub committed_height: u64,
    /// Authoritative singleton chain-wide privacy policy.
    pub consensus_policy: PrivacyConsensusPolicyV1,
    /// Exactly twelve protocol rows in canonical discriminant order.
    pub protocols: Vec<PrivacyCapabilityRowV1>,
}
impl PrivacyCapabilitySnapshotV1 {
    /// Validate the complete public snapshot and all embedded state.
    ///
    /// # Errors
    ///
    /// Rejects an unknown version, invalid singleton policy, any row-count or
    /// ordering drift, or an invalid protocol row.
    pub fn validate(&self) -> Result<(), PrivacyCapabilitySnapshotValidationErrorV1> {
        if self.version != PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1 {
            return Err(PrivacyCapabilitySnapshotValidationErrorV1::Version {
                expected: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
                actual: self.version,
            });
        }
        self.consensus_policy
            .validate_at_committed_height(self.committed_height)
            .map_err(PrivacyCapabilitySnapshotValidationErrorV1::ConsensusPolicy)?;
        if self.protocols.len() != PrivacyProtocolIdV1::COUNT {
            return Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolCount {
                expected: PrivacyProtocolIdV1::COUNT,
                actual: self.protocols.len(),
            });
        }
        for (index, (row, expected)) in self
            .protocols
            .iter()
            .zip(PrivacyProtocolIdV1::ALL)
            .enumerate()
        {
            if row.protocol_id != expected {
                return Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolOrder {
                    index,
                    expected,
                    actual: row.protocol_id,
                });
            }
            row.validate_at_committed_height(self.committed_height)
                .map_err(
                    |source| PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                        protocol_id: expected,
                        source,
                    },
                )?;
        }
        Ok(())
    }
}
/// Exact native bridge ABI required by the first-release privacy SDK surface.
pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;
/// Maximum accepted size of one canonical local compiled-profile catalog archive.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES_V1: usize = 256 * 1024;
/// Maximum elements accepted in one catalog sequence.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_SEQUENCE_ELEMENTS_V1: usize =
    PrivacyProtocolIdV1::COUNT;
/// Maximum cumulative elements accepted while decoding one catalog.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_TOTAL_ELEMENTS_V1: usize =
    PrivacyProtocolIdV1::COUNT;
/// Maximum bytes accepted in one length-delimited catalog field.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_FIELD_BYTES_V1: usize = 128 * 1024;
/// Maximum cumulative allocation permitted while decoding one catalog.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_TOTAL_ALLOCATION_BYTES_V1: usize =
    256 * 1024;
/// Maximum data-dependent nesting depth permitted in one catalog.
pub const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_NESTING_DEPTH_V1: usize = 32;
/// Stable result codes returned by the native compiled-profile catalog validator.
///
/// These codes validate local build metadata only. A valid catalog does not
/// imply that any protocol is activated, permitted, or ready on a network.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyCompiledProfileCatalogArchiveValidationStatusV1 {
    /// The archive is a canonical, structurally valid local catalog.
    Valid = 0,
    /// A native ABI caller supplied a null archive pointer.
    NullPointer = 1,
    /// The archive contains no bytes.
    Empty = 2,
    /// The archive exceeds the fixed byte ceiling.
    ArchiveTooLarge = 3,
    /// Norito rejected a declared sequence, field, allocation, or nesting budget.
    DecodeResourceLimit = 4,
    /// The archive schema is not exactly [`PrivacyCompiledProfileCatalogV1`].
    SchemaMismatch = 5,
    /// The bytes are not the one canonical uncompressed V1 encoding.
    NonCanonical = 6,
    /// The archive is malformed, truncated, has trailing bytes, or fails its checksum.
    MalformedArchive = 7,
    /// The typed catalog violates its closed first-release semantic invariants.
    InvalidCatalog = 8,
}
impl PrivacyCompiledProfileCatalogArchiveValidationStatusV1 {
    /// Return the stable ABI-22 integer representation.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }
    /// Return whether the archive was accepted.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}
/// Validate one untrusted canonical typed local compiled-profile catalog.
///
/// This validator enforces fixed byte, sequence, cumulative-element, allocation, field, and nesting
/// ceilings before validating the exact twelve rows in [`PrivacyProtocolIdV1::ALL`] order. It
/// establishes only that the bytes are a well-formed catalog; callers comparing against the current
/// binary must additionally require exact equality with that binary's catalog.
#[must_use]
pub fn validate_privacy_compiled_profile_catalog_archive_v1(
    archive: &[u8],
) -> PrivacyCompiledProfileCatalogArchiveValidationStatusV1 {
    use PrivacyCompiledProfileCatalogArchiveValidationStatusV1 as Status;
    if archive.is_empty() {
        return Status::Empty;
    }
    if archive.len() > PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES_V1 {
        return Status::ArchiveTooLarge;
    }
    let limits = norito::DecodeLimits::new(
        PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_SEQUENCE_ELEMENTS_V1,
        PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_FIELD_BYTES_V1,
        PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_TOTAL_ELEMENTS_V1,
        PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_TOTAL_ALLOCATION_BYTES_V1,
        PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_NESTING_DEPTH_V1,
    );
    let catalog = match norito::decode_canonical_with_limits::<PrivacyCompiledProfileCatalogV1>(
        archive, limits,
    ) {
        Ok(catalog) => catalog,
        Err(error) if error.is_decode_resource_limit() => return Status::DecodeResourceLimit,
        Err(norito::Error::SchemaMismatch) => return Status::SchemaMismatch,
        Err(
            norito::Error::NonCanonicalEncoding
            | norito::Error::DecodeFlagsMismatch { .. }
            | norito::Error::UnsupportedCompression { .. },
        ) => return Status::NonCanonical,
        Err(_) => return Status::MalformedArchive,
    };
    if catalog.validate().is_err() {
        return Status::InvalidCatalog;
    }
    Status::Valid
}
/// Maximum accepted size of one canonical privacy capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1: usize = 256 * 1024;
/// Maximum elements accepted in any sequence while decoding a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_SEQUENCE_ELEMENTS_V1: usize = PrivacyProtocolIdV1::COUNT;
/// Maximum cumulative elements accepted while decoding a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ELEMENTS_V1: usize = PrivacyProtocolIdV1::COUNT;
/// Maximum bytes accepted in one length-delimited capability-archive field.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_FIELD_BYTES_V1: usize = 128 * 1024;
/// Maximum cumulative allocation permitted while decoding a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ALLOCATION_BYTES_V1: usize = 256 * 1024;
/// Maximum data-dependent nesting depth permitted in a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_NESTING_DEPTH_V1: usize = 32;
/// Stable result codes returned by every native privacy capability validator.
///
/// These numeric discriminants are part of ABI 22. SDKs must accept only
/// [`Self::Valid`]; every other value is a fail-closed rejection.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyCapabilityArchiveValidationStatusV1 {
    /// The archive is the exact canonical typed manifest and is semantically valid.
    Valid = 0,
    /// A native ABI caller supplied a null archive pointer.
    NullPointer = 1,
    /// The archive contains no bytes.
    Empty = 2,
    /// The archive exceeds [`PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1`].
    ArchiveTooLarge = 3,
    /// Norito rejected a declared sequence, field, allocation, or nesting budget.
    DecodeResourceLimit = 4,
    /// The archive schema is not exactly [`PrivacyExact12CapabilityManifestV1`].
    SchemaMismatch = 5,
    /// The bytes are not the one canonical uncompressed V1 encoding.
    NonCanonical = 6,
    /// The archive is malformed, truncated, or fails its checksum.
    MalformedArchive = 7,
    /// The typed manifest violates its closed first-release semantic invariants.
    InvalidManifest = 8,
}
impl PrivacyCapabilityArchiveValidationStatusV1 {
    /// Return the stable ABI-22 integer representation.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }
    /// Return whether the archive was accepted.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}
/// Validate one untrusted canonical typed privacy capability archive.
///
/// Admission first applies the fixed 256 KiB byte ceiling, then decodes with tight per-sequence,
/// cumulative-element, allocation, and nesting budgets. The canonical decoder enforces the exact
/// [`PrivacyExact12CapabilityManifestV1`] schema and byte-for-byte canonical re-encoding. Finally
/// [`PrivacyExact12CapabilityManifestV1::validate`] enforces the exact twelve rows in
/// [`PrivacyProtocolIdV1::ALL`] order and all operation, profile, readiness, activation, policy,
/// limitation, and self-digest bindings. The legacy snapshot schema is rejected rather than treated
/// as a compatibility representation.
#[must_use]
pub fn validate_privacy_capability_archive_v1(
    archive: &[u8],
) -> PrivacyCapabilityArchiveValidationStatusV1 {
    use PrivacyCapabilityArchiveValidationStatusV1 as Status;
    if archive.is_empty() {
        return Status::Empty;
    }
    if archive.len() > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 {
        return Status::ArchiveTooLarge;
    }
    let limits = norito::DecodeLimits::new(
        PRIVACY_CAPABILITY_ARCHIVE_MAX_SEQUENCE_ELEMENTS_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_FIELD_BYTES_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ELEMENTS_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ALLOCATION_BYTES_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_NESTING_DEPTH_V1,
    );
    let manifest = match norito::decode_canonical_with_limits::<PrivacyExact12CapabilityManifestV1>(
        archive, limits,
    ) {
        Ok(manifest) => manifest,
        Err(error) if error.is_decode_resource_limit() => return Status::DecodeResourceLimit,
        Err(norito::Error::SchemaMismatch) => return Status::SchemaMismatch,
        Err(
            norito::Error::NonCanonicalEncoding
            | norito::Error::DecodeFlagsMismatch { .. }
            | norito::Error::UnsupportedCompression { .. },
        ) => return Status::NonCanonical,
        Err(_) => return Status::MalformedArchive,
    };
    if manifest.validate().is_err() {
        return Status::InvalidManifest;
    }
    Status::Valid
}
/// Validation failure for [`PrivacyCapabilitySnapshotV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCapabilitySnapshotValidationErrorV1 {
    /// Snapshot wire version is not the exact first-release version.
    #[error("privacy capability snapshot version {actual} differs from required {expected}")]
    Version {
        /// Required version.
        expected: u32,
        /// Rejected version.
        actual: u32,
    },
    /// Singleton policy is invalid at the committed height.
    #[error("privacy capability consensus policy is invalid: {0}")]
    ConsensusPolicy(PrivacyPolicyValidationErrorV1),
    /// Protocol row count differs from the closed registry.
    #[error("privacy capability snapshot has {actual} rows; expected {expected}")]
    ProtocolCount {
        /// Closed first-release row count.
        expected: usize,
        /// Rejected row count.
        actual: usize,
    },
    /// A row is missing, duplicated, or reordered.
    #[error(
        "privacy capability row {index} is {actual:?}; expected canonical protocol {expected:?}"
    )]
    ProtocolOrder {
        /// Zero-based row index.
        index: usize,
        /// Required protocol at this index.
        expected: PrivacyProtocolIdV1,
        /// Rejected protocol at this index.
        actual: PrivacyProtocolIdV1,
    },
    /// One canonical row is invalid.
    #[error("privacy capability row {protocol_id:?} is invalid: {source}")]
    ProtocolRow {
        /// Protocol selected by row order.
        protocol_id: PrivacyProtocolIdV1,
        /// Exact row validation failure.
        source: PrivacyCapabilityRowValidationErrorV1,
    },
}
