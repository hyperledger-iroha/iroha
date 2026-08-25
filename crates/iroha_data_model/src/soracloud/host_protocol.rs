/// Soracloud host operation routed through the dedicated runtime syscall block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "operation", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoracloudHostOperationV1 {
    /// Read committed service-state metadata visible to the active handler.
    ReadCommittedState,
    /// Stage a deterministic service-state mutation for core-side validation/write-back.
    EmitStateMutation,
    /// Stage an outbound mailbox message for authoritative persistence.
    EmitMailboxMessage,
    /// Append runtime journal material and return its content-addressed digest.
    AppendJournal,
    /// Publish a checkpoint artifact and return its content-addressed digest.
    PublishCheckpoint,
    /// Read authoritative service config material for the active service revision.
    ReadConfig,
    /// Read an authoritative service secret envelope for the active service revision.
    ReadSecretEnvelope,
    /// Read node-local secret material exposed only through the runtime host.
    ReadSecret,
    /// Read node-local credential material exposed only through the runtime host.
    ReadCredential,
    /// Perform a bounded, policy-checked egress fetch.
    EgressFetch,
}
/// Request envelope decoded from the Soracloud request pointer-ABI payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudHostRequestEnvelopeV1 {
    /// Schema version; must equal [`SORACLOUD_HOST_REQUEST_VERSION_V1`].
    pub schema_version: u16,
    /// Requested host operation.
    pub operation: SoracloudHostOperationV1,
    /// Operation-specific payload.
    pub payload: SoracloudHostRequestPayloadV1,
}
impl SoracloudHostRequestEnvelopeV1 {
    /// Validate the request envelope schema version.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the schema version is unsupported.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "soracloud host request envelope",
            self.schema_version,
            SORACLOUD_HOST_REQUEST_VERSION_V1,
        )?;
        if self.operation != self.payload.operation() {
            return Err(invalid_field(
                "soracloud host request envelope",
                "operation",
                "must match payload type",
            ));
        }
        self.payload.validate()?;
        Ok(())
    }
}
/// Operation-specific Soracloud host request payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "payload_type", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoracloudHostRequestPayloadV1 {
    /// Request to read committed service-state metadata.
    ReadCommittedState(SoracloudReadCommittedStateRequestV1),
    /// Request to stage a deterministic service-state mutation.
    EmitStateMutation(SoracloudEmitStateMutationRequestV1),
    /// Request to emit an outbound mailbox message.
    EmitMailboxMessage(SoracloudEmitMailboxMessageRequestV1),
    /// Request to append journal material.
    AppendJournal(SoracloudAppendJournalRequestV1),
    /// Request to publish a checkpoint artifact.
    PublishCheckpoint(SoracloudPublishCheckpointRequestV1),
    /// Request to read an authoritative service config payload.
    ReadConfig(SoracloudReadConfigRequestV1),
    /// Request to read an authoritative service secret envelope.
    ReadSecretEnvelope(SoracloudReadSecretEnvelopeRequestV1),
    /// Request to read a node-local secret.
    ReadSecret(SoracloudReadSecretRequestV1),
    /// Request to read a node-local credential.
    ReadCredential(SoracloudReadCredentialRequestV1),
    /// Request to perform a bounded egress fetch.
    EgressFetch(SoracloudEgressFetchRequestV1),
}
impl SoracloudHostRequestPayloadV1 {
    /// Return the operation represented by this request payload.
    #[must_use]
    pub fn operation(&self) -> SoracloudHostOperationV1 {
        match self {
            Self::ReadCommittedState(_) => SoracloudHostOperationV1::ReadCommittedState,
            Self::EmitStateMutation(_) => SoracloudHostOperationV1::EmitStateMutation,
            Self::EmitMailboxMessage(_) => SoracloudHostOperationV1::EmitMailboxMessage,
            Self::AppendJournal(_) => SoracloudHostOperationV1::AppendJournal,
            Self::PublishCheckpoint(_) => SoracloudHostOperationV1::PublishCheckpoint,
            Self::ReadConfig(_) => SoracloudHostOperationV1::ReadConfig,
            Self::ReadSecretEnvelope(_) => SoracloudHostOperationV1::ReadSecretEnvelope,
            Self::ReadSecret(_) => SoracloudHostOperationV1::ReadSecret,
            Self::ReadCredential(_) => SoracloudHostOperationV1::ReadCredential,
            Self::EgressFetch(_) => SoracloudHostOperationV1::EgressFetch,
        }
    }
    /// Validate operation-specific host request payload constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when request hashes, paths, or payload
    /// lengths are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        match self {
            Self::ReadCommittedState(request) => request.validate(),
            Self::EmitStateMutation(request) => request.validate(),
            Self::EmitMailboxMessage(request) => request.validate(),
            Self::AppendJournal(request) => request.validate(),
            Self::PublishCheckpoint(request) => request.validate(),
            Self::ReadConfig(request) => request.validate(),
            Self::ReadSecretEnvelope(request) => request.validate(),
            Self::ReadSecret(request) => request.validate(),
            Self::ReadCredential(request) => request.validate(),
            Self::EgressFetch(request) => request.validate(),
        }
    }
}
/// Response envelope encoded into the Soracloud response pointer-ABI payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudHostResponseEnvelopeV1 {
    /// Schema version; must equal [`SORACLOUD_HOST_RESPONSE_VERSION_V1`].
    pub schema_version: u16,
    /// Operation serviced by the host.
    pub operation: SoracloudHostOperationV1,
    /// Operation-specific payload.
    pub payload: SoracloudHostResponsePayloadV1,
}
impl SoracloudHostResponseEnvelopeV1 {
    /// Validate the response envelope schema version.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the schema version is unsupported.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_schema_version(
            "soracloud host response envelope",
            self.schema_version,
            SORACLOUD_HOST_RESPONSE_VERSION_V1,
        )?;
        if self.operation != self.payload.operation() {
            return Err(invalid_field(
                "soracloud host response envelope",
                "operation",
                "must match payload type",
            ));
        }
        self.payload.validate()?;
        Ok(())
    }
}
/// Operation-specific Soracloud host response payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "payload_type", content = "value"))]
#[norito(deny_unknown_fields)]
pub enum SoracloudHostResponsePayloadV1 {
    /// Response to committed service-state metadata lookups.
    ReadCommittedState(SoracloudReadCommittedStateResponseV1),
    /// Response to staged service-state mutations.
    EmitStateMutation(SoracloudEmitStateMutationResponseV1),
    /// Response to staged outbound mailbox messages.
    EmitMailboxMessage(SoracloudEmitMailboxMessageResponseV1),
    /// Response to appended journal material.
    AppendJournal(SoracloudAppendJournalResponseV1),
    /// Response to published checkpoint material.
    PublishCheckpoint(SoracloudPublishCheckpointResponseV1),
    /// Response to service config lookups.
    ReadConfig(SoracloudReadConfigResponseV1),
    /// Response to secret-envelope lookups.
    ReadSecretEnvelope(SoracloudReadSecretEnvelopeResponseV1),
    /// Response to secret lookups.
    ReadSecret(SoracloudReadSecretResponseV1),
    /// Response to credential lookups.
    ReadCredential(SoracloudReadCredentialResponseV1),
    /// Response to bounded egress fetches.
    EgressFetch(SoracloudEgressFetchResponseV1),
}
impl SoracloudHostResponsePayloadV1 {
    /// Return the operation represented by this response payload.
    #[must_use]
    pub fn operation(&self) -> SoracloudHostOperationV1 {
        match self {
            Self::ReadCommittedState(_) => SoracloudHostOperationV1::ReadCommittedState,
            Self::EmitStateMutation(_) => SoracloudHostOperationV1::EmitStateMutation,
            Self::EmitMailboxMessage(_) => SoracloudHostOperationV1::EmitMailboxMessage,
            Self::AppendJournal(_) => SoracloudHostOperationV1::AppendJournal,
            Self::PublishCheckpoint(_) => SoracloudHostOperationV1::PublishCheckpoint,
            Self::ReadConfig(_) => SoracloudHostOperationV1::ReadConfig,
            Self::ReadSecretEnvelope(_) => SoracloudHostOperationV1::ReadSecretEnvelope,
            Self::ReadSecret(_) => SoracloudHostOperationV1::ReadSecret,
            Self::ReadCredential(_) => SoracloudHostOperationV1::ReadCredential,
            Self::EgressFetch(_) => SoracloudHostOperationV1::EgressFetch,
        }
    }
    /// Validate operation-specific host response payload constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when response hashes or nested records are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        match self {
            Self::ReadCommittedState(response) => response.validate(),
            Self::EmitStateMutation(response) => response.validate(),
            Self::EmitMailboxMessage(response) => response.validate(),
            Self::AppendJournal(response) => response.validate(),
            Self::PublishCheckpoint(response) => response.validate(),
            Self::ReadConfig(response) => response.validate(),
            Self::ReadSecretEnvelope(response) => response.validate(),
            Self::ReadSecret(response) => response.validate(),
            Self::ReadCredential(response) => response.validate(),
            Self::EgressFetch(response) => response.validate(),
        }
    }
}
/// Read committed service-state metadata for one binding/key pair.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadCommittedStateRequestV1 {
    /// Declared binding name to read from.
    pub binding_name: Name,
    /// Canonical state key scoped under the binding prefix.
    pub state_key: String,
}
impl SoracloudReadCommittedStateRequestV1 {
    /// Validate committed-state request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the state key is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_state_key("soracloud read committed state request", &self.state_key)
    }
}
/// Response to a committed service-state metadata lookup.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadCommittedStateResponseV1 {
    /// Matching entry when one exists.
    #[norito(required)]
    pub entry: Option<SoraServiceStateEntryV1>,
}
impl SoracloudReadCommittedStateResponseV1 {
    /// Validate committed-state response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the nested state entry is invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if let Some(entry) = &self.entry {
            entry.validate()?;
        }
        Ok(())
    }
}
/// Stage a deterministic service-state mutation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudEmitStateMutationRequestV1 {
    /// Binding mutated by the runtime.
    pub binding_name: Name,
    /// Canonical key scoped under the binding prefix.
    pub state_key: String,
    /// Mutation mode to apply.
    pub operation: SoraStateMutationOperationV1,
    /// Encryption contract expected by the binding.
    pub encryption: SoraStateEncryptionV1,
    /// Declared payload size when the mutation upserts content.
    #[norito(required)]
    pub payload_bytes: Option<u64>,
    /// Full payload bytes when the mutation upserts content.
    #[norito(required)]
    pub payload: Option<Vec<u8>>,
    /// Deterministic commitment over the opaque payload.
    #[norito(required)]
    pub payload_commitment: Option<Hash>,
}
impl SoracloudEmitStateMutationRequestV1 {
    /// Validate state mutation request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when state keys, payload lengths, or
    /// payload commitments are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_state_key(
            "soracloud emit state mutation request",
            &self.state_key,
        )?;
        if let Some(payload_commitment) = self.payload_commitment {
            validate_soracloud_digest_hash(
                "soracloud emit state mutation request",
                "payload_commitment",
                payload_commitment,
            )?;
        }
        if let Some(payload) = self.payload.as_ref() {
            if let Some(payload_bytes) = self.payload_bytes
                && payload_bytes != payload.len() as u64
            {
                return Err(invalid_field(
                    "soracloud emit state mutation request",
                    "payload_bytes",
                    "must match payload length",
                ));
            }
            if let Some(payload_commitment) = self.payload_commitment
                && payload_commitment != Hash::new(payload)
            {
                return Err(invalid_field(
                    "soracloud emit state mutation request",
                    "payload_commitment",
                    "must match the canonical payload hash",
                ));
            }
        }
        if self.operation == SoraStateMutationOperationV1::Delete
            && (self.payload.is_some()
                || self.payload_bytes.is_some()
                || self.payload_commitment.is_some())
        {
            return Err(invalid_field(
                "soracloud emit state mutation request",
                "payload",
                "delete mutations must not carry payload material",
            ));
        }
        Ok(())
    }
}
/// Response to a staged service-state mutation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudEmitStateMutationResponseV1 {
    /// Stable mutation digest returned by the host after staging the write-back.
    pub mutation_commitment: Hash,
}
impl SoracloudEmitStateMutationResponseV1 {
    /// Validate state mutation response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the mutation commitment is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "soracloud emit state mutation response",
            "mutation_commitment",
            self.mutation_commitment,
        )
    }
}
/// Stage an outbound Soracloud mailbox message.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudEmitMailboxMessageRequestV1 {
    /// Destination service name.
    pub to_service: Name,
    /// Destination handler name.
    pub to_handler: Name,
    /// Opaque mailbox payload bytes.
    pub payload_bytes: Vec<u8>,
    /// Number of authoritative Soracloud sequence steps to delay delivery.
    pub delivery_delay_blocks: u32,
}
impl SoracloudEmitMailboxMessageRequestV1 {
    /// Validate outbound mailbox request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when mailbox request fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        Ok(())
    }
}
/// Response to a staged outbound mailbox message.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudEmitMailboxMessageResponseV1 {
    /// Deterministic execution-local staging identifier.
    ///
    /// The ledger derives the canonical persisted mailbox message identifier after assigning the
    /// authoritative service versions and delivery schedule.
    pub message_id: Hash,
    /// Commitment over the emitted mailbox payload.
    pub payload_commitment: Hash,
}
impl SoracloudEmitMailboxMessageResponseV1 {
    /// Validate outbound mailbox response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when mailbox identifiers or payload
    /// commitments are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "soracloud emit mailbox message response",
            "message_id",
            self.message_id,
        )?;
        validate_soracloud_digest_hash(
            "soracloud emit mailbox message response",
            "payload_commitment",
            self.payload_commitment,
        )
    }
}
/// Append deterministic journal material for the active handler execution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudAppendJournalRequestV1 {
    /// Runtime-relative journal path for the appended material.
    pub artifact_path: String,
    /// Journal payload bytes.
    pub payload_bytes: Vec<u8>,
}
impl SoracloudAppendJournalRequestV1 {
    /// Validate append-journal request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the artifact path is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_artifact_path(
            "soracloud append journal request",
            &self.artifact_path,
        )
    }
}
/// Response to appended journal material.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudAppendJournalResponseV1 {
    /// Content-addressed digest of the materialized journal payload.
    pub artifact_hash: Hash,
}
impl SoracloudAppendJournalResponseV1 {
    /// Validate append-journal response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the artifact hash is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "soracloud append journal response",
            "artifact_hash",
            self.artifact_hash,
        )
    }
}
/// Publish deterministic checkpoint material for the active handler execution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudPublishCheckpointRequestV1 {
    /// Runtime-relative checkpoint path for the published material.
    pub artifact_path: String,
    /// Checkpoint payload bytes.
    pub payload_bytes: Vec<u8>,
}
impl SoracloudPublishCheckpointRequestV1 {
    /// Validate publish-checkpoint request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the artifact path is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_artifact_path(
            "soracloud publish checkpoint request",
            &self.artifact_path,
        )
    }
}
/// Response to published checkpoint material.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudPublishCheckpointResponseV1 {
    /// Content-addressed digest of the materialized checkpoint payload.
    pub artifact_hash: Hash,
}
impl SoracloudPublishCheckpointResponseV1 {
    /// Validate publish-checkpoint response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the artifact hash is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "soracloud publish checkpoint response",
            "artifact_hash",
            self.artifact_hash,
        )
    }
}
/// Read authoritative service config material for the active service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadConfigRequestV1 {
    /// Stable config identifier relative to the authoritative service-config set.
    pub config_name: String,
}
impl SoracloudReadConfigRequestV1 {
    /// Validate config request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the config name is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field(
            "soracloud read config request",
            "config_name",
            &self.config_name,
        )
    }
}
/// Response to an authoritative service config lookup.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadConfigResponseV1 {
    /// Whether the requested config was found for the active service revision.
    pub found: bool,
    /// Canonical JSON payload bytes when the lookup succeeds.
    pub payload_bytes: Vec<u8>,
}
impl SoracloudReadConfigResponseV1 {
    /// Validate config response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when found/payload flags are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_found_payload(
            "soracloud read config response",
            self.found,
            &self.payload_bytes,
        )
    }
}
/// Read an authoritative service secret envelope for the active service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadSecretEnvelopeRequestV1 {
    /// Stable secret identifier relative to the authoritative service-secret set.
    pub secret_name: String,
}
impl SoracloudReadSecretEnvelopeRequestV1 {
    /// Validate secret-envelope request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the secret name is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field(
            "soracloud read secret envelope request",
            "secret_name",
            &self.secret_name,
        )
    }
}
/// Response to an authoritative service secret-envelope lookup.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadSecretEnvelopeResponseV1 {
    /// Matching authoritative secret envelope when one exists.
    #[norito(required)]
    pub envelope: Option<SecretEnvelopeV1>,
}
impl SoracloudReadSecretEnvelopeResponseV1 {
    /// Validate secret-envelope response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the nested envelope is invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if let Some(envelope) = &self.envelope {
            envelope.validate()?;
        }
        Ok(())
    }
}
/// Read node-local secret material for the active service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadSecretRequestV1 {
    /// Stable secret identifier relative to the node-local secret root.
    pub secret_name: String,
}
impl SoracloudReadSecretRequestV1 {
    /// Validate node-local secret request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the secret name is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field(
            "soracloud read secret request",
            "secret_name",
            &self.secret_name,
        )
    }
}
/// Response to a node-local secret lookup.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadSecretResponseV1 {
    /// Whether the requested secret was found locally.
    pub found: bool,
    /// Secret payload bytes when the lookup succeeds.
    pub payload_bytes: Vec<u8>,
}
impl SoracloudReadSecretResponseV1 {
    /// Validate node-local secret response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when found/payload flags are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_found_payload(
            "soracloud read secret response",
            self.found,
            &self.payload_bytes,
        )
    }
}
/// Read node-local credential material for the active service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadCredentialRequestV1 {
    /// Stable credential identifier relative to the node-local credential root.
    pub credential_name: String,
}
impl SoracloudReadCredentialRequestV1 {
    /// Validate node-local credential request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the credential name is empty.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_nonblank_field(
            "soracloud read credential request",
            "credential_name",
            &self.credential_name,
        )
    }
}
/// Response to a node-local credential lookup.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudReadCredentialResponseV1 {
    /// Whether the requested credential was found locally.
    pub found: bool,
    /// Credential payload bytes when the lookup succeeds.
    pub payload_bytes: Vec<u8>,
}
impl SoracloudReadCredentialResponseV1 {
    /// Validate node-local credential response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when found/payload flags are inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_host_found_payload(
            "soracloud read credential response",
            self.found,
            &self.payload_bytes,
        )
    }
}
/// Perform a bounded, policy-checked egress fetch from an allowlisted host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudEgressFetchRequestV1 {
    /// Absolute URL to fetch.
    pub url: String,
    /// Maximum number of response bytes the caller is willing to accept.
    pub max_bytes: u64,
    /// Optional expected digest for content-addressed verification.
    #[norito(required)]
    pub expected_hash: Option<Hash>,
}
impl SoracloudEgressFetchRequestV1 {
    /// Validate egress fetch request fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when URL, byte cap, or expected hash
    /// fields are malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_public_url("soracloud egress fetch request", "url", &self.url)?;
        if self.max_bytes == 0 {
            return Err(invalid_field(
                "soracloud egress fetch request",
                "max_bytes",
                "must be greater than zero",
            ));
        }
        if let Some(expected_hash) = self.expected_hash {
            validate_soracloud_digest_hash(
                "soracloud egress fetch request",
                "expected_hash",
                expected_hash,
            )?;
        }
        Ok(())
    }
}
/// Response to a bounded egress fetch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SoracloudEgressFetchResponseV1 {
    /// HTTP status code returned by the fetch.
    pub status_code: u16,
    /// Content type reported by the source when present.
    #[norito(required)]
    pub content_type: Option<String>,
    /// Response body bytes, truncated only by caller/configured ceilings.
    pub body: Vec<u8>,
    /// Content-addressed hash of `body`.
    pub body_hash: Hash,
}
impl SoracloudEgressFetchResponseV1 {
    /// Validate egress fetch response fields.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when response metadata is malformed or
    /// `body_hash` does not match `body`.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        validate_soracloud_digest_hash(
            "soracloud egress fetch response",
            "body_hash",
            self.body_hash,
        )?;
        if self.body_hash != Hash::new(&self.body) {
            return Err(invalid_field(
                "soracloud egress fetch response",
                "body_hash",
                "must match the canonical response body hash",
            ));
        }
        if self
            .content_type
            .as_ref()
            .is_some_and(|content_type| content_type.trim().is_empty())
        {
            return Err(invalid_field(
                "soracloud egress fetch response",
                "content_type",
                "must not be empty when provided",
            ));
        }
        Ok(())
    }
}
fn validate_soracloud_host_state_key(
    manifest: &'static str,
    state_key: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonblank_field(manifest, "state_key", state_key)?;
    if !state_key.starts_with('/') {
        return Err(invalid_field(manifest, "state_key", "must start with '/'"));
    }
    Ok(())
}
fn validate_soracloud_host_artifact_path(
    manifest: &'static str,
    artifact_path: &str,
) -> Result<(), SoracloudManifestError> {
    validate_nonblank_field(manifest, "artifact_path", artifact_path)?;
    if !artifact_path.starts_with('/') {
        return Err(invalid_field(
            manifest,
            "artifact_path",
            "must start with '/'",
        ));
    }
    Ok(())
}
fn validate_soracloud_host_found_payload(
    manifest: &'static str,
    found: bool,
    payload_bytes: &[u8],
) -> Result<(), SoracloudManifestError> {
    if found && payload_bytes.is_empty() {
        return Err(SoracloudManifestError::EmptyField {
            manifest,
            field: "payload_bytes",
        });
    }
    if !found && !payload_bytes.is_empty() {
        return Err(invalid_field(
            manifest,
            "payload_bytes",
            "must be empty when found is false",
        ));
    }
    Ok(())
}
/// Purpose bound into a V1 Soracloud runtime provenance signature.
///
/// The discriminants are immutable wire identifiers used by external signing
/// adapters. Unknown identifiers must never be interpreted as aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SoracloudRuntimeProvenancePurposeV1 {
    /// Sign a canonical model-host heartbeat.
    ModelHostHeartbeat = 1,
    /// Sign a canonical Inrou host advertisement.
    InrouHostAdvert = 2,
    /// Sign a canonical Inrou host withdrawal.
    InrouHostWithdraw = 3,
}
impl SoracloudRuntimeProvenancePurposeV1 {
    /// Return the immutable V1 wire identifier.
    #[must_use]
    pub const fn wire_id(self) -> u8 {
        self as u8
    }
    /// Decode one immutable V1 wire identifier.
    ///
    /// # Errors
    ///
    /// Returns [`SoracloudRuntimeProvenancePurposeErrorV1`] for an unknown
    /// purpose. Unknown identifiers are never accepted as aliases.
    pub const fn try_from_wire_id(
        value: u8,
    ) -> Result<Self, SoracloudRuntimeProvenancePurposeErrorV1> {
        match value {
            1 => Ok(Self::ModelHostHeartbeat),
            2 => Ok(Self::InrouHostAdvert),
            3 => Ok(Self::InrouHostWithdraw),
            _ => Err(SoracloudRuntimeProvenancePurposeErrorV1),
        }
    }
}
/// An unknown Soracloud runtime provenance-purpose identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("unknown Soracloud runtime provenance purpose V1")]
pub struct SoracloudRuntimeProvenancePurposeErrorV1;
/// Encode one versioned, domain- and purpose-separated runtime provenance preimage.
///
/// The returned bytes are the canonical Norito encoding of this exact tuple: `(domain_tag_bytes,
/// version, purpose_wire_id, canonical_payload_bytes)`. Both byte strings are length-delimited by
/// Norito, so no purpose or payload boundary can be reinterpreted. Callers must sign the returned
/// preimage, never `canonical_payload` directly.
///
/// # Errors
///
/// Returns an encoding error when canonical Norito serialization fails.
pub fn encode_soracloud_runtime_provenance_preimage_v1(
    purpose: SoracloudRuntimeProvenancePurposeV1,
    canonical_payload: &[u8],
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        SORACLOUD_RUNTIME_PROVENANCE_DOMAIN_V1.to_vec(),
        SORACLOUD_RUNTIME_PROVENANCE_PREIMAGE_VERSION_V1,
        purpose.wire_id(),
        canonical_payload.to_vec(),
    ))
}
/// Invalid canonical Soracloud runtime provenance preimage.
///
/// Variants intentionally carry no input bytes or decoded values so callers
/// can report failures without exposing a signing payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum SoracloudRuntimeProvenancePreimageErrorV1 {
    /// The preimage is not one exact canonical V1 Norito tuple.
    #[error("malformed Soracloud runtime provenance preimage V1")]
    Malformed,
    /// The signature-domain tag is not the fixed V1 value.
    #[error("Soracloud runtime provenance domain mismatch")]
    DomainMismatch,
    /// The preimage version is not the fixed V1 value.
    #[error("Soracloud runtime provenance version mismatch")]
    VersionMismatch,
    /// The embedded purpose does not equal the caller's expected purpose.
    #[error("Soracloud runtime provenance purpose mismatch")]
    PurposeMismatch,
}
/// Validate one canonical runtime provenance preimage against an expected purpose.
///
/// Decoding uses Norito's canonical decode limits derived from `preimage.len()`; transport callers
/// must additionally enforce their deployment-owned byte ceiling before invoking this function. No
/// decoded payload bytes are returned or included in errors.
///
/// # Errors
///
/// Returns a payload-free error when the tuple is malformed or its domain,
/// version, or purpose differs from the exact V1 expectation.
pub fn validate_soracloud_runtime_provenance_preimage_v1(
    expected_purpose: SoracloudRuntimeProvenancePurposeV1,
    preimage: &[u8],
) -> Result<(), SoracloudRuntimeProvenancePreimageErrorV1> {
    let (domain, version, purpose, _payload): (Vec<u8>, u8, u8, Vec<u8>) =
        norito::decode_canonical(preimage)
            .map_err(|_| SoracloudRuntimeProvenancePreimageErrorV1::Malformed)?;
    if domain.as_slice() != SORACLOUD_RUNTIME_PROVENANCE_DOMAIN_V1 {
        return Err(SoracloudRuntimeProvenancePreimageErrorV1::DomainMismatch);
    }
    if version != SORACLOUD_RUNTIME_PROVENANCE_PREIMAGE_VERSION_V1 {
        return Err(SoracloudRuntimeProvenancePreimageErrorV1::VersionMismatch);
    }
    if purpose != expected_purpose.wire_id() {
        return Err(SoracloudRuntimeProvenancePreimageErrorV1::PurposeMismatch);
    }
    Ok(())
}
/// Encode the canonical provenance signature payload for deployment bundles.
///
/// The payload layout is the canonical Norito encoding of [`SoraDeploymentBundleV1`].
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_bundle_provenance_payload(
    bundle: &SoraDeploymentBundleV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(bundle)
}
/// Encode the canonical provenance signature payload for an app-level infrastructure mutation.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(manifest, precondition)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_app_infra_provenance_payload(
    manifest: &SoraAppInfraManifestV1,
    precondition: &SoraAppInfraMutationPreconditionV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(manifest.clone(), precondition.clone()))
}
/// Encode the canonical provenance signature payload for deployment bundles,
/// inline materials, and the exact ledger mutation precondition.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(bundle, initial_service_configs, initial_service_secrets, precondition)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_bundle_with_materials_provenance_payload(
    bundle: &SoraDeploymentBundleV1,
    initial_service_configs: &BTreeMap<String, Json>,
    initial_service_secrets: &BTreeMap<String, SecretEnvelopeV1>,
    precondition: &SoraServiceMutationPreconditionV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        bundle.clone(),
        initial_service_configs.clone(),
        initial_service_secrets.clone(),
        precondition.clone(),
    ))
}
/// Encode the canonical provenance signature payload for service rollback.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, target_version)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_rollback_provenance_payload(
    service_name: &str,
    target_version: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, target_version))
}
/// Encode the canonical provenance signature payload for service config upserts.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, config_name, value_json)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_set_service_config_provenance_payload(
    service_name: &str,
    config_name: &str,
    value_json: &Json,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, config_name, value_json.clone()))
}
/// Encode the canonical provenance signature payload for service config deletions.
///
/// The payload layout is a Norito tuple in this exact field order: `(service_name, config_name)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_delete_service_config_provenance_payload(
    service_name: &str,
    config_name: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, config_name))
}
/// Encode the canonical provenance signature payload for service secret upserts.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, secret_name, secret_envelope)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_set_service_secret_provenance_payload(
    service_name: &str,
    secret_name: &str,
    secret: &SecretEnvelopeV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, secret_name, secret.clone()))
}
/// Encode the canonical provenance signature payload for service secret deletions.
///
/// The payload layout is a Norito tuple in this exact field order: `(service_name, secret_name)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_delete_service_secret_provenance_payload(
    service_name: &str,
    secret_name: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, secret_name))
}
/// Encode the canonical provenance signature payload for state mutations.
///
/// The payload layout is a Norito tuple in this exact field order: `(service_name, binding_name,
/// key, operation, value_size_bytes, payload_commitment, encryption, governance_tx_hash,
/// fhe_input_admission_proof)`.
///
/// `operation` is expected to be a deterministic symbolic label such as `"upsert"` or `"delete"`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_state_mutation_provenance_payload(
    service_name: &str,
    binding_name: &str,
    key: &str,
    operation: &str,
    value_size_bytes: Option<u64>,
    payload_commitment: Option<Hash>,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    fhe_input_admission_proof: Option<SoracloudFheInputAdmissionProofV1>,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        binding_name,
        key,
        operation,
        value_size_bytes,
        payload_commitment,
        encryption,
        governance_tx_hash,
        fhe_input_admission_proof,
    ))
}
/// Return the Soracloud FHE input-admission public-input schema hash.
#[must_use]
pub fn soracloud_fhe_input_admission_public_inputs_schema_hash_v1() -> [u8; 32] {
    Hash::new(SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1).into()
}
/// Return the Soracloud FHE public-key proof public-input schema hash.
#[must_use]
pub fn soracloud_fhe_public_key_proof_public_inputs_schema_hash_v1() -> [u8; 32] {
    Hash::new(SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1).into()
}
/// Return the Soracloud FHE bootstrap-key proof public-input schema hash.
#[must_use]
pub fn soracloud_fhe_bootstrap_key_proof_public_inputs_schema_hash_v1() -> [u8; 32] {
    Hash::new(SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1).into()
}
/// Return the Soracloud FHE full-bootstrap execution proof public-input schema hash.
#[must_use]
pub fn soracloud_fhe_full_bootstrap_execution_proof_public_inputs_schema_hash_v1() -> [u8; 32] {
    Hash::new(SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1).into()
}
/// Derive the canonical statement hash for Soracloud FHE input admission.
///
/// The statement layout is a nested Norito tuple in this exact field order: `((service_name,
/// binding_name, key, operation, value_size_bytes, payload_commitment, encryption,
/// governance_tx_hash), (bfv_parameter_digest, bfv_rns_modulus_chain_digest,
/// bfv_key_switch_decomposition_chain_digest), ciphertext_proof_statement_digests,
/// residual_multiple_bound, ExactResidualMultiple)`.
///
/// `operation` is expected to be a deterministic symbolic label such as `"upsert"`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn derive_soracloud_fhe_input_admission_statement_hash(
    service_name: &str,
    binding_name: &str,
    key: &str,
    operation: &str,
    value_size_bytes: u64,
    payload_commitment: Hash,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    bfv_parameter_digest: Hash,
    bfv_rns_modulus_chain_digest: Hash,
    bfv_key_switch_decomposition_chain_digest: Hash,
    ciphertext_proof_statement_digests: &[Hash],
    residual_multiple_bound: u128,
) -> Result<Hash, norito::Error> {
    derive_soracloud_fhe_input_admission_statement_hash_with_bound_mode(
        service_name,
        binding_name,
        key,
        operation,
        value_size_bytes,
        payload_commitment,
        encryption,
        governance_tx_hash,
        bfv_parameter_digest,
        bfv_rns_modulus_chain_digest,
        bfv_key_switch_decomposition_chain_digest,
        ciphertext_proof_statement_digests,
        residual_multiple_bound,
        BfvCiphertextBoundModeV1::ExactResidualMultiple,
    )
}
/// Derive a canonical statement hash for Soracloud FHE input admission with bound mode.
///
/// The statement layout is a nested Norito tuple in this exact field order: `((service_name,
/// binding_name, key, operation, value_size_bytes, payload_commitment, encryption,
/// governance_tx_hash), (bfv_parameter_digest, bfv_rns_modulus_chain_digest,
/// bfv_key_switch_decomposition_chain_digest), ciphertext_proof_statement_digests,
/// residual_multiple_bound, bound_mode)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn derive_soracloud_fhe_input_admission_statement_hash_with_bound_mode(
    service_name: &str,
    binding_name: &str,
    key: &str,
    operation: &str,
    value_size_bytes: u64,
    payload_commitment: Hash,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    bfv_parameter_digest: Hash,
    bfv_rns_modulus_chain_digest: Hash,
    bfv_key_switch_decomposition_chain_digest: Hash,
    ciphertext_proof_statement_digests: &[Hash],
    residual_multiple_bound: u128,
    bound_mode: BfvCiphertextBoundModeV1,
) -> Result<Hash, norito::Error> {
    let ciphertext_proof_statement_digests = ciphertext_proof_statement_digests.to_vec();
    let payload = norito::encode_canonical(&(
        (
            service_name,
            binding_name,
            key,
            operation,
            value_size_bytes,
            payload_commitment,
            encryption,
            governance_tx_hash,
        ),
        (
            bfv_parameter_digest,
            bfv_rns_modulus_chain_digest,
            bfv_key_switch_decomposition_chain_digest,
        ),
        ciphertext_proof_statement_digests,
        residual_multiple_bound,
        bound_mode,
    ))?;
    Ok(Hash::new(&payload))
}
/// Encode the canonical provenance signature payload for rollout advancement.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, rollout_handle, healthy, promote_to_percent, governance_tx_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_rollout_provenance_payload(
    service_name: &str,
    rollout_handle: &str,
    healthy: bool,
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        rollout_handle,
        healthy,
        promote_to_percent,
        governance_tx_hash,
    ))
}
/// Encode the canonical provenance signature payload for apartment deployment.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(manifest, lease_blocks, autonomy_budget_units)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_deploy_provenance_payload(
    manifest: AgentApartmentManifestV1,
    lease_blocks: u64,
    autonomy_budget_units: Option<u64>,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(manifest, lease_blocks, autonomy_budget_units))
}
/// Encode the canonical provenance signature payload for apartment lease renewal.
///
/// The payload layout is a Norito tuple in this exact field order: `(apartment_name, lease_blocks)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_lease_renew_provenance_payload(
    apartment_name: &str,
    lease_blocks: u64,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, lease_blocks))
}
/// Encode the canonical provenance signature payload for apartment restart requests.
///
/// The payload layout is a Norito tuple in this exact field order: `(apartment_name, reason)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_restart_provenance_payload(
    apartment_name: &str,
    reason: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, reason))
}
/// Encode the canonical provenance signature payload for apartment policy revocation.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(apartment_name, capability, reason)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_policy_revoke_provenance_payload(
    apartment_name: &str,
    capability: &str,
    reason: Option<&str>,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, capability, reason))
}
/// Encode the canonical provenance signature payload for apartment wallet spend requests.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(apartment_name, asset_definition, amount)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_wallet_spend_provenance_payload(
    apartment_name: &str,
    asset_definition: &str,
    amount: &Quantity,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, asset_definition, amount.clone()))
}
/// Encode the canonical provenance signature payload for apartment wallet approvals.
///
/// The payload layout is a Norito tuple in this exact field order: `(apartment_name, request_id)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_wallet_approve_provenance_payload(
    apartment_name: &str,
    request_id: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, request_id))
}
/// Encode the canonical provenance signature payload for apartment mailbox send requests.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(from_apartment, to_apartment, channel, payload)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_message_send_provenance_payload(
    from_apartment: &str,
    to_apartment: &str,
    channel: &str,
    payload: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(from_apartment, to_apartment, channel, payload))
}
/// Encode the canonical provenance signature payload for apartment mailbox acknowledgements.
///
/// The payload layout is a Norito tuple in this exact field order: `(apartment_name, message_id)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_message_ack_provenance_payload(
    apartment_name: &str,
    message_id: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, message_id))
}
/// Encode the canonical provenance signature payload for apartment artifact allowlists.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(apartment_name, artifact_hash, provenance_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_artifact_allow_provenance_payload(
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(apartment_name, artifact_hash, provenance_hash))
}
/// Encode the canonical provenance signature payload for apartment autonomy runs.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(apartment_name, artifact_hash, provenance_hash, budget_units, run_label, workflow_input_json)`.
/// When the `json` feature is enabled, `workflow_input_json` is canonicalized
/// before encoding so client-side signing matches runtime verification.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_agent_autonomy_run_provenance_payload(
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    budget_units: u64,
    run_label: &str,
    workflow_input_json: Option<&str>,
) -> Result<Vec<u8>, norito::Error> {
    let canonical_workflow_input_json =
        canonical_agent_workflow_input_json_for_payload(workflow_input_json);
    norito::encode_canonical(&(
        apartment_name,
        artifact_hash,
        provenance_hash,
        budget_units,
        run_label,
        canonical_workflow_input_json.as_deref(),
    ))
}
fn canonical_agent_workflow_input_json_for_payload(
    workflow_input_json: Option<&str>,
) -> Option<String> {
    let workflow_input_json = workflow_input_json?;
    let trimmed = workflow_input_json.trim();
    #[cfg(feature = "json")]
    {
        if let Ok(parsed) = norito::json::from_str::<norito::json::Value>(trimmed)
            && let Ok(canonical) = norito::json::to_json(&parsed)
        {
            return Some(canonical);
        }
    }
    Some(trimmed.to_owned())
}
/// Derive the deterministic runtime request commitment for an approved Soracloud agent autonomy run.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn derive_agent_autonomy_request_commitment(
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    budget_units: u64,
    run_id: &str,
    run_label: &str,
    workflow_input_json: Option<&str>,
    process_generation: u64,
) -> Hash {
    Hash::new(Encode::encode(&(
        apartment_name,
        artifact_hash,
        provenance_hash,
        budget_units,
        run_id,
        run_label,
        workflow_input_json,
        process_generation,
    )))
}
/// Encode the canonical provenance signature payload for training-job start.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, model_name, job_id, worker_group_size, target_steps, checkpoint_interval_steps, max_retries, step_compute_units, compute_budget_units, storage_budget_bytes)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_training_job_start_provenance_payload(
    service_name: &str,
    model_name: &str,
    job_id: &str,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        model_name,
        job_id,
        worker_group_size,
        target_steps,
        checkpoint_interval_steps,
        max_retries,
        step_compute_units,
        compute_budget_units,
        storage_budget_bytes,
    ))
}
/// Encode the canonical provenance signature payload for training checkpoint updates.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, job_id, completed_step, checkpoint_size_bytes, metrics_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_training_job_checkpoint_provenance_payload(
    service_name: &str,
    job_id: &str,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        job_id,
        completed_step,
        checkpoint_size_bytes,
        metrics_hash,
    ))
}
/// Encode the canonical provenance signature payload for training retry requests.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, job_id, reason)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_training_job_retry_provenance_payload(
    service_name: &str,
    job_id: &str,
    reason: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, job_id, reason))
}
/// Encode the canonical provenance signature payload for model-artifact registration.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, model_name, training_job_id, weight_artifact_hash, dataset_ref, training_config_hash, reproducibility_hash, provenance_attestation_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_model_artifact_register_provenance_payload(
    service_name: &str,
    model_name: &str,
    training_job_id: &str,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        model_name,
        training_job_id,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
}
/// Encode the canonical provenance signature payload for model-weight registration.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, model_name, weight_version, training_job_id, parent_version, weight_artifact_hash, dataset_ref, training_config_hash, reproducibility_hash, provenance_attestation_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_model_weight_register_provenance_payload(
    service_name: &str,
    model_name: &str,
    weight_version: &str,
    training_job_id: &str,
    parent_version: Option<&str>,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        model_name,
        weight_version,
        training_job_id,
        parent_version,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
}
/// Encode the canonical provenance signature payload for model-weight promotion.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, model_name, weight_version, gate_approved, gate_report_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_model_weight_promote_provenance_payload(
    service_name: &str,
    model_name: &str,
    weight_version: &str,
    gate_approved: bool,
    gate_report_hash: Hash,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        model_name,
        weight_version,
        gate_approved,
        gate_report_hash,
    ))
}
/// Encode the canonical provenance signature payload for model-weight rollback.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, model_name, target_version, reason)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_model_weight_rollback_provenance_payload(
    service_name: &str,
    model_name: &str,
    target_version: &str,
    reason: &str,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, model_name, target_version, reason))
}
/// Encode the canonical provenance signature payload for uploaded-model bundle registration.
///
/// The payload layout is the Norito encoding of `bundle`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::needless_pass_by_value)]
pub fn encode_uploaded_model_bundle_register_provenance_payload(
    bundle: SoraUploadedModelBundleV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&bundle)
}
/// Encode the canonical provenance signature payload for uploaded-model bundle finalization.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, model_name, model_id, artifact_id, weight_version, bundle_root, weight_artifact_hash, dataset_ref, training_config_hash, reproducibility_hash, provenance_attestation_hash)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_uploaded_model_finalize_provenance_payload(
    service_name: &str,
    model_name: &str,
    model_id: &str,
    artifact_id: &str,
    weight_version: &str,
    bundle_root: Hash,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name,
        model_name,
        model_id,
        artifact_id,
        weight_version,
        bundle_root,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    ))
}
/// Encode the canonical provenance signature payload for HF shared-lease joins.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(repo_id, resolved_revision, model_name, service_name, apartment_name, storage_class, lease_term_ms, lease_asset_definition_id, base_fee)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_hf_shared_lease_join_provenance_payload(
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
    service_name: &str,
    apartment_name: Option<&str>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: &AssetDefinitionId,
    base_fee: &Quantity,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        repo_id,
        resolved_revision,
        model_name,
        service_name,
        apartment_name,
        storage_class,
        lease_term_ms,
        lease_asset_definition_id.clone(),
        base_fee.clone(),
    ))
}
/// Encode the canonical provenance signature payload for HF shared-lease leaves.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(repo_id, resolved_revision, storage_class, lease_term_ms, service_name, apartment_name)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_hf_shared_lease_leave_provenance_payload(
    repo_id: &str,
    resolved_revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    service_name: Option<&str>,
    apartment_name: Option<&str>,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        repo_id,
        resolved_revision,
        storage_class,
        lease_term_ms,
        service_name,
        apartment_name,
    ))
}
/// Encode the canonical provenance signature payload for HF shared-lease renewals.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(repo_id, resolved_revision, model_name, service_name, apartment_name, storage_class, lease_term_ms, lease_asset_definition_id, base_fee)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
#[allow(clippy::too_many_arguments)]
pub fn encode_hf_shared_lease_renew_provenance_payload(
    repo_id: &str,
    resolved_revision: &str,
    model_name: &str,
    service_name: &str,
    apartment_name: Option<&str>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: &AssetDefinitionId,
    base_fee: &Quantity,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        repo_id,
        resolved_revision,
        model_name,
        service_name,
        apartment_name,
        storage_class,
        lease_term_ms,
        lease_asset_definition_id.clone(),
        base_fee.clone(),
    ))
}
/// Encode the canonical provenance signature payload for model-host adverts.
///
/// The payload layout is the canonical Norito encoding of [`SoraModelHostCapabilityRecordV1`].
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_model_host_advertise_provenance_payload(
    capability: &SoraModelHostCapabilityRecordV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(capability)
}
/// Encode the canonical provenance signature payload for model-host heartbeats.
///
/// The semantic payload is the canonical Norito tuple `(validator_account_id,
/// heartbeat_expires_at_ms)`. The returned signature preimage wraps those bytes with
/// [`SoracloudRuntimeProvenancePurposeV1::ModelHostHeartbeat`] through
/// [`encode_soracloud_runtime_provenance_preimage_v1`].
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_model_host_heartbeat_provenance_payload(
    validator_account_id: &AccountId,
    heartbeat_expires_at_ms: u64,
) -> Result<Vec<u8>, norito::Error> {
    let canonical_payload =
        norito::encode_canonical(&(validator_account_id.clone(), heartbeat_expires_at_ms))?;
    encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::ModelHostHeartbeat,
        &canonical_payload,
    )
}
/// Encode the canonical provenance signature payload for model-host withdrawals.
///
/// The payload layout is the canonical Norito encoding of `validator_account_id`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_model_host_withdraw_provenance_payload(
    validator_account_id: &AccountId,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(validator_account_id)
}
/// Encode the canonical provenance signature payload for Inrou host adverts.
///
/// The semantic payload is the canonical Norito encoding of
/// [`SoraInrouHostCapabilityRecordV1`]. The returned signature preimage wraps
/// those bytes with [`SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert`]
/// through [`encode_soracloud_runtime_provenance_preimage_v1`].
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_inrou_host_advertise_provenance_payload(
    capability: &SoraInrouHostCapabilityRecordV1,
) -> Result<Vec<u8>, norito::Error> {
    let canonical_payload = norito::encode_canonical(capability)?;
    encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostAdvert,
        &canonical_payload,
    )
}
/// Encode the canonical provenance signature payload for Inrou host withdrawals.
///
/// The semantic payload is the canonical Norito encoding of
/// `validator_account_id`. The returned signature preimage wraps those bytes
/// with [`SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw`] through
/// [`encode_soracloud_runtime_provenance_preimage_v1`].
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_inrou_host_withdraw_provenance_payload(
    validator_account_id: &AccountId,
) -> Result<Vec<u8>, norito::Error> {
    let canonical_payload = norito::encode_canonical(validator_account_id)?;
    encode_soracloud_runtime_provenance_preimage_v1(
        SoracloudRuntimeProvenancePurposeV1::InrouHostWithdraw,
        &canonical_payload,
    )
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct FheJobRunProvenancePayloadV1<'a> {
    service_name: &'a str,
    binding_name: &'a str,
    job: FheJobSpecV1,
    policy_reference: SoracloudFhePolicyReferenceV1,
    public_key_proof: Option<SoracloudFhePublicKeyProofV1>,
    bootstrap_key_zero_refresh_proof: Option<SoracloudFheBootstrapKeyProofV1>,
    full_bootstrap_execution_proofs: Vec<SoracloudFheFullBootstrapExecutionProofV1>,
}
/// Encode the canonical provenance signature payload for FHE job execution.
///
/// The payload layout is the canonical Norito encoding of `FheJobRunProvenancePayloadV1`,
/// preserving this exact field order: `service_name`, `binding_name`, `job`, `policy_reference`,
/// `public_key_proof`, `bootstrap_key_zero_refresh_proof`, and `full_bootstrap_execution_proofs`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_fhe_job_run_provenance_payload(
    service_name: &str,
    binding_name: &str,
    job: FheJobSpecV1,
    policy_reference: SoracloudFhePolicyReferenceV1,
    public_key_proof: Option<SoracloudFhePublicKeyProofV1>,
    bootstrap_key_zero_refresh_proof: Option<SoracloudFheBootstrapKeyProofV1>,
    full_bootstrap_execution_proofs: Vec<SoracloudFheFullBootstrapExecutionProofV1>,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&FheJobRunProvenancePayloadV1 {
        service_name,
        binding_name,
        job,
        policy_reference,
        public_key_proof,
        bootstrap_key_zero_refresh_proof,
        full_bootstrap_execution_proofs,
    })
}
/// Encode the canonical provenance payload for first FHE policy registration.
///
/// # Errors
/// Returns an encoding error when canonical Norito serialization fails.
pub fn encode_fhe_policy_register_provenance_payload(
    service_name: &str,
    material: &SoracloudFheGovernedMaterialV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name.to_owned(), material.clone()))
}
/// Encode the canonical provenance payload for monotonic FHE policy rotation.
///
/// # Errors
/// Returns an encoding error when canonical Norito serialization fails.
pub fn encode_fhe_policy_rotate_provenance_payload(
    service_name: &str,
    expected_active: &SoracloudFhePolicyReferenceV1,
    material: &SoracloudFheGovernedMaterialV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(
        service_name.to_owned(),
        expected_active.clone(),
        material.clone(),
    ))
}
/// Encode the canonical provenance payload for permanent FHE policy revocation.
///
/// # Errors
/// Returns an encoding error when canonical Norito serialization fails.
pub fn encode_fhe_policy_revoke_provenance_payload(
    service_name: &str,
    expected_active: &SoracloudFhePolicyReferenceV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name.to_owned(), expected_active.clone()))
}
/// Encode the canonical provenance signature payload for decryption requests.
///
/// The payload layout is a Norito tuple in this exact field order:
/// `(service_name, policy, request)`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_decryption_request_provenance_payload(
    service_name: &str,
    policy: DecryptionAuthorityPolicyV1,
    request: DecryptionRequestV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&(service_name, policy, request))
}
/// Encode the canonical provenance signature payload for ciphertext queries.
///
/// The payload layout is the canonical Norito encoding of `CiphertextQuerySpecV1`.
///
/// # Errors
/// Returns an encoding error when Norito serialization fails.
pub fn encode_ciphertext_query_provenance_payload(
    query: &CiphertextQuerySpecV1,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(query)
}
