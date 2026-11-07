// Dedicated Ironwood wire grammar; never reachable from a baseline instruction.

/// Concrete public inputs for a governed Ironwood action.
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
#[norito_schema(name = "iroha_data_model::privacy::PrivacyIronwoodActionsStatementV1")]
pub struct PrivacyIronwoodActionsStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset represented by the Orchard action.
    pub asset_definition_id: AssetDefinitionId,
    /// Exact transparent reserve partition used by directional value bridges.
    pub public_balance_scope: AssetBalanceScope,
    /// Orchard pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Admitted note-commitment tree anchor.
    pub anchor: PrivacyRootV1,
    /// Epoch at which `anchor` was canonical.
    pub anchor_epoch: u64,
    /// Non-empty ordered Orchard actions.
    ///
    /// The node derives the successor frontier and root by appending these
    /// note commitments to its authoritative pool frontier. A caller-selected
    /// successor root is intentionally unrepresentable.
    pub actions: Vec<PrivacyOrchardActionV1>,
    /// Public value balance in atomic units.
    pub value_balance: PrivacyValueBalanceV1,
    /// Last block height at which the action is valid.
    pub expiry_height: u64,
}
impl From<OrchardHalo2ActionsStatementV1> for PrivacyIronwoodActionsStatementV1 {
    fn from(value: OrchardHalo2ActionsStatementV1) -> Self {
        Self {
            context: value.context,
            asset_definition_id: value.asset_definition_id,
            public_balance_scope: value.public_balance_scope,
            pool_id: value.pool_id,
            anchor: value.anchor,
            anchor_epoch: value.anchor_epoch,
            actions: value.actions,
            value_balance: value.value_balance,
            expiry_height: value.expiry_height,
        }
    }
}
impl From<PrivacyIronwoodActionsStatementV1> for OrchardHalo2ActionsStatementV1 {
    fn from(value: PrivacyIronwoodActionsStatementV1) -> Self {
        Self {
            context: value.context,
            asset_definition_id: value.asset_definition_id,
            public_balance_scope: value.public_balance_scope,
            pool_id: value.pool_id,
            anchor: value.anchor,
            anchor_epoch: value.anchor_epoch,
            actions: value.actions,
            value_balance: value.value_balance,
            expiry_height: value.expiry_height,
        }
    }
}
impl PrivacyIronwoodActionsStatementV1 {
    /// Bind the new concrete Ironwood frame, never an Orchard statement tag.
    /// # Errors
    /// Canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyStatementDigestV1, norito::Error> {
        privacy_wire_statement_digest(&norito::encode_canonical(self)?)
    }
}

/// Artifact-bound proof envelope for the dedicated Ironwood instruction.
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
    name = "iroha_data_model::privacy::PrivacyIronwoodProofEnvelopeV1",
    frame = "iroha.privacy.ironwood-proof-envelope.v1"
)]
pub struct PrivacyIronwoodProofEnvelopeV1 {
    /// Mandatory first-release wire marker; invalid markers fail decoding.
    pub wire_magic: PrivacyProofWireMagicV1,
    /// Commitment to the immutable base Exact12 catalog and security models.
    ///
    /// Additive protocols bind their separate protocol and profile identities without changing
    /// this deployed field; the base commitment does not activate an extension.
    pub catalog_commitment: PrivacyExact12CatalogCommitmentV1,
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
    pub statement: PrivacyIronwoodActionsStatementV1,
    /// Protocol-typed proof bytes.
    pub proof: PrivacyProofBytesV1,
}
impl From<PrivacyIronwoodProofEnvelopeV1> for PrivacyRuntimeProofEnvelopeV1 {
    fn from(value: PrivacyIronwoodProofEnvelopeV1) -> Self {
        Self {
            protocol_id: PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            wire_magic: value.wire_magic,
            catalog_commitment: value.catalog_commitment,
            proof_system_id: value.proof_system_id,
            engine_id: value.engine_id,
            parameter_id: value.parameter_id,
            parameter_digest: value.parameter_digest,
            verifier_digest: value.verifier_digest,
            statement_schema_digest: value.statement_schema_digest,
            engine_manifest_digest: value.engine_manifest_digest,
            statement_digest: value.statement_digest,
            statement: PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(value.statement.into()),
            proof: PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(value.proof),
        }
    }
}
impl TryFrom<PrivacyRuntimeProofEnvelopeV1> for PrivacyIronwoodProofEnvelopeV1 {
    type Error = Exact12PrivacyWireError;
    fn try_from(value: PrivacyRuntimeProofEnvelopeV1) -> Result<Self, Self::Error> {
        if value.protocol_id != PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            return Err(Exact12PrivacyWireError);
        }
        let PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement) = value.statement else {
            return Err(Exact12PrivacyWireError);
        };
        let PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(proof) = value.proof else {
            return Err(Exact12PrivacyWireError);
        };
        Ok(Self {
            wire_magic: value.wire_magic,
            catalog_commitment: value.catalog_commitment,
            proof_system_id: value.proof_system_id,
            engine_id: value.engine_id,
            parameter_id: value.parameter_id,
            parameter_digest: value.parameter_digest,
            verifier_digest: value.verifier_digest,
            statement_schema_digest: value.statement_schema_digest,
            engine_manifest_digest: value.engine_manifest_digest,
            statement_digest: value.statement_digest,
            statement: statement.into(),
            proof,
        })
    }
}
impl PrivacyIronwoodProofEnvelopeV1 {
    /// Bind concrete Ironwood inputs to their exact artifact identities.
    /// # Errors
    /// The context or canonical statement digest cannot be assembled.
    pub fn bind_v1(
        statement: PrivacyIronwoodActionsStatementV1,
        proof: PrivacyProofBytesV1,
    ) -> Result<Self, PrivacyProofEnvelopeValidationError> {
        PrivacyRuntimeProofEnvelopeV1::bind_v1(
            PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement.into()),
            PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(proof),
        )?
        .try_into()
        .map_err(|_| PrivacyProofEnvelopeValidationError::EncodingFailure)
    }
    /// Validate the concrete envelope using the same native limits.
    /// # Errors
    /// Artifact binding or intrinsic validation fails.
    pub fn validate_with_limits(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyProofEnvelopeValidationError> {
        PrivacyRuntimeProofEnvelopeV1::from(self.clone()).validate_with_limits(limits)
    }
}

impl PrivacyRuntimeProofEnvelopeV1 {
    /// Construct the one concrete wire operation owned by this runtime proof type.
    ///
    /// Exact12 remains closed. Ironwood always uses its dedicated governed instruction.
    /// # Errors
    /// Protocol, statement and proof tags do not share the same wire grammar.
    pub fn into_submission_v1(self) -> Result<crate::isi::InstructionBox, Exact12PrivacyWireError> {
        if self.protocol_id == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            Ok(crate::isi::privacy::SubmitPrivacyIronwoodProofV1::new(self.try_into()?).into())
        } else {
            Ok(crate::isi::privacy::SubmitPrivacyProofV1::new(self.try_into()?).into())
        }
    }
}

// Encoding-only views preserve the selected concrete wire without copying proof buffers.
// They have no decoder or independently selectable protocol identity.
struct PrivacyWireBorrow<'a, T>(&'a T);
impl<T: norito::SerializePayload> norito::SerializePayload for PrivacyWireBorrow<'_, T> {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.0.serialize(encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
// Keep the zero-length wire field as an explicit alignment carrier.
#[repr(C)]
struct PrivacySelectedFrame<Wire, Payload> {
    payload: Payload,
    _wire_alignment: [Wire; 0],
}
impl<Wire: norito::NoritoSchema, Payload> norito::NoritoSchema
    for PrivacySelectedFrame<Wire, Payload>
{
    fn nominal_name() -> String {
        Wire::nominal_name()
    }
    fn static_nominal_name() -> Option<&'static str> {
        Wire::static_nominal_name()
    }
    fn frame_name() -> String {
        Wire::frame_name()
    }
    fn static_frame_name() -> Option<&'static str> {
        Wire::static_frame_name()
    }
}
impl<Wire, Payload: norito::SerializePayload> norito::SerializePayload
    for PrivacySelectedFrame<Wire, Payload>
{
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        // Framing includes concrete-type alignment. Refuse an incompatible view instead of
        // emitting a frame whose payload padding differs from the original wire type.
        if core::mem::align_of::<Self>() != core::mem::align_of::<Wire>() {
            return Err(norito::Error::LengthMismatch);
        }
        self.payload.serialize(encoder)
    }
}
fn selected_privacy_frame<Wire, Payload>(payload: Payload) -> PrivacySelectedFrame<Wire, Payload> {
    PrivacySelectedFrame {
        payload,
        _wire_alignment: [],
    }
}

#[derive(norito::SerializePayload)]
struct PrivacyExact12EnvelopeRef<'a> {
    wire_magic: PrivacyWireBorrow<'a, PrivacyProofWireMagicV1>,
    catalog_commitment: PrivacyWireBorrow<'a, PrivacyExact12CatalogCommitmentV1>,
    protocol_id: PrivacyProtocolIdV1,
    proof_system_id: PrivacyWireBorrow<'a, PrivacyProofSystemIdV1>,
    engine_id: PrivacyWireBorrow<'a, PrivacyEngineIdV1>,
    parameter_id: PrivacyWireBorrow<'a, PrivacyParameterIdV1>,
    parameter_digest: PrivacyWireBorrow<'a, PrivacyParameterDigestV1>,
    verifier_digest: PrivacyWireBorrow<'a, PrivacyVerifierDigestV1>,
    statement_schema_digest: PrivacyWireBorrow<'a, PrivacyStatementSchemaDigestV1>,
    engine_manifest_digest: PrivacyWireBorrow<'a, PrivacyEngineManifestDigestV1>,
    statement_digest: PrivacyWireBorrow<'a, PrivacyStatementDigestV1>,
    // Both tagged payloads are checked against the frozen twelve below. Their bare
    // encodings equal the original closed enums; only the concrete outer frame is emitted.
    statement: PrivacyWireBorrow<'a, PrivacyRuntimeStatementV1>,
    proof: PrivacyWireBorrow<'a, PrivacyRuntimeProofV1>,
}
#[derive(norito::SerializePayload)]
struct PrivacyIronwoodEnvelopeRef<'a> {
    wire_magic: PrivacyWireBorrow<'a, PrivacyProofWireMagicV1>,
    catalog_commitment: PrivacyWireBorrow<'a, PrivacyExact12CatalogCommitmentV1>,
    proof_system_id: PrivacyWireBorrow<'a, PrivacyProofSystemIdV1>,
    engine_id: PrivacyWireBorrow<'a, PrivacyEngineIdV1>,
    parameter_id: PrivacyWireBorrow<'a, PrivacyParameterIdV1>,
    parameter_digest: PrivacyWireBorrow<'a, PrivacyParameterDigestV1>,
    verifier_digest: PrivacyWireBorrow<'a, PrivacyVerifierDigestV1>,
    statement_schema_digest: PrivacyWireBorrow<'a, PrivacyStatementSchemaDigestV1>,
    engine_manifest_digest: PrivacyWireBorrow<'a, PrivacyEngineManifestDigestV1>,
    statement_digest: PrivacyWireBorrow<'a, PrivacyStatementDigestV1>,
    // The concrete Ironwood statement has the same fields as Orchard, with its own
    // nominal/frame identity. As a nested field only that exact bare field sequence is used.
    statement: PrivacyWireBorrow<'a, OrchardHalo2ActionsStatementV1>,
    proof: PrivacyWireBorrow<'a, PrivacyProofBytesV1>,
}
fn privacy_wire_projection_error(error: Exact12PrivacyWireError) -> norito::Error {
    norito::Error::Message(error.to_string())
}
impl PrivacyRuntimeProofEnvelopeV1 {
    fn exact12_wire_ref(&self) -> Result<PrivacyExact12EnvelopeRef<'_>, norito::Error> {
        let protocol_id = self
            .protocol_id
            .try_into()
            .map_err(privacy_wire_projection_error)?;
        PrivacyProtocolIdV1::try_from(self.statement.protocol_id())
            .map_err(privacy_wire_projection_error)?;
        PrivacyProtocolIdV1::try_from(self.proof.protocol_id())
            .map_err(privacy_wire_projection_error)?;
        Ok(PrivacyExact12EnvelopeRef {
            wire_magic: PrivacyWireBorrow(&self.wire_magic),
            catalog_commitment: PrivacyWireBorrow(&self.catalog_commitment),
            protocol_id,
            proof_system_id: PrivacyWireBorrow(&self.proof_system_id),
            engine_id: PrivacyWireBorrow(&self.engine_id),
            parameter_id: PrivacyWireBorrow(&self.parameter_id),
            parameter_digest: PrivacyWireBorrow(&self.parameter_digest),
            verifier_digest: PrivacyWireBorrow(&self.verifier_digest),
            statement_schema_digest: PrivacyWireBorrow(&self.statement_schema_digest),
            engine_manifest_digest: PrivacyWireBorrow(&self.engine_manifest_digest),
            statement_digest: PrivacyWireBorrow(&self.statement_digest),
            statement: PrivacyWireBorrow(&self.statement),
            proof: PrivacyWireBorrow(&self.proof),
        })
    }
    fn ironwood_wire_ref(&self) -> Result<PrivacyIronwoodEnvelopeRef<'_>, norito::Error> {
        let (
            PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement),
            PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(proof),
        ) = (self.protocol_id, &self.statement, &self.proof)
        else {
            return Err(privacy_wire_projection_error(Exact12PrivacyWireError));
        };
        Ok(PrivacyIronwoodEnvelopeRef {
            wire_magic: PrivacyWireBorrow(&self.wire_magic),
            catalog_commitment: PrivacyWireBorrow(&self.catalog_commitment),
            proof_system_id: PrivacyWireBorrow(&self.proof_system_id),
            engine_id: PrivacyWireBorrow(&self.engine_id),
            parameter_id: PrivacyWireBorrow(&self.parameter_id),
            parameter_digest: PrivacyWireBorrow(&self.parameter_digest),
            verifier_digest: PrivacyWireBorrow(&self.verifier_digest),
            statement_schema_digest: PrivacyWireBorrow(&self.statement_schema_digest),
            engine_manifest_digest: PrivacyWireBorrow(&self.engine_manifest_digest),
            statement_digest: PrivacyWireBorrow(&self.statement_digest),
            statement: PrivacyWireBorrow(statement),
            proof: PrivacyWireBorrow(proof),
        })
    }
    /// Encode the explicitly selected wire envelope, preserving the frozen Exact12 frame.
    /// # Errors
    /// The runtime tags do not share one grammar or canonical encoding fails.
    pub fn canonical_wire_bytes_v1(&self) -> Result<Vec<u8>, norito::Error> {
        if self.protocol_id == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            norito::encode_canonical(
                &selected_privacy_frame::<PrivacyIronwoodProofEnvelopeV1, _>(
                    self.ironwood_wire_ref()?,
                ),
            )
        } else {
            norito::encode_canonical(&selected_privacy_frame::<PrivacyProofEnvelopeV1, _>(
                self.exact12_wire_ref()?,
            ))
        }
    }
    /// Measure the exact selected envelope without copying proof or statement buffers.
    /// # Errors
    /// The runtime tags do not share one grammar or canonical measurement fails.
    pub fn canonical_wire_frame_len_v1(&self) -> Result<usize, norito::Error> {
        if self.protocol_id == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            norito::canonical_frame_len(
                &selected_privacy_frame::<PrivacyIronwoodProofEnvelopeV1, _>(
                    self.ironwood_wire_ref()?,
                ),
            )
        } else {
            norito::canonical_frame_len(&selected_privacy_frame::<PrivacyProofEnvelopeV1, _>(
                self.exact12_wire_ref()?,
            ))
        }
    }
}
impl PrivacyRuntimeNamespaceV1 {
    /// Encode the source-owned namespace grammar used by cryptographic state relations.
    /// # Errors
    /// Canonical encoding fails.
    pub fn canonical_wire_bytes_v1(self) -> Result<Vec<u8>, norito::Error> {
        if self.protocol_id() == PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1 {
            norito::encode_canonical(&self)
        } else {
            let namespace: PrivacyNamespaceV1 =
                self.try_into().map_err(|error: Exact12PrivacyWireError| {
                    norito::Error::Message(error.to_string())
                })?;
            norito::encode_canonical(&namespace)
        }
    }
}
impl PrivacyRuntimeStatementV1 {
    /// Encode the exact statement frame named by the original operation.
    /// # Errors
    /// Canonical encoding fails.
    pub fn canonical_wire_bytes_v1(&self) -> Result<Vec<u8>, norito::Error> {
        match self {
            Self::IronwoodHalo2ActionsV1(statement) => {
                norito::encode_canonical(&selected_privacy_frame::<
                    PrivacyIronwoodActionsStatementV1,
                    _,
                >(PrivacyWireBorrow(statement)))
            }
            _ => {
                PrivacyProtocolIdV1::try_from(self.protocol_id())
                    .map_err(privacy_wire_projection_error)?;
                norito::encode_canonical(&selected_privacy_frame::<PrivacyStatementV1, _>(
                    PrivacyWireBorrow(self),
                ))
            }
        }
    }
}
