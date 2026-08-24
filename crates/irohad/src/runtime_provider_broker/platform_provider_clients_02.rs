#[derive(Clone)]
struct PopBrokerIssuerSigner {
    provider: PopBrokerProvider,
    key_id: String,
    public_key: [u8; 32],
}
impl_broker_debug_fields!(PopBrokerIssuerSigner as value {
    "key_id" => value.key_id,
    "private_signer" => "[REDACTED]",
} => finish_non_exhaustive);
impl sorafs_node::pop_credentials::PopIssuerSigner for PopBrokerIssuerSigner {
    fn key_id(&self) -> &str {
        &self.key_id
    }
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn sign_digest(
        &self,
        purpose: sorafs_node::pop_credentials::PopIssuerSigningPurposeV1,
        digest: [u8; 32],
    ) -> Result<[u8; 64], String> {
        if digest == [0; 32] {
            return Err("PoP runtime provider unavailable".to_owned());
        }
        let payload = encode_canonical(
            &PopIssuerSignRequestWireV1 {
                purpose: purpose.wire_id(),
                digest,
            },
            MAX_POP_RUNTIME_FRAME_BYTES_V1,
        )
        .map_err(|error| self.provider.redacted_string_error(error))?;
        let result = self
            .provider
            .call(OPERATION_POP_ISSUER_SIGN_V1, payload, false)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let signed = self
            .provider
            .decode::<PopIssuerSignResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        verify_evidence_viewer_ed25519_signature(self.public_key, signed.signature, &digest)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        Ok(signed.signature)
    }
}
#[derive(Clone)]
struct PopBrokerAuthenticator {
    provider: PopBrokerProvider,
}
impl fmt::Debug for PopBrokerAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopBrokerAuthenticator([REDACTED])")
    }
}
impl sorafs_node::pop_credentials::PopCredentialApiAuthenticator for PopBrokerAuthenticator {
    fn authenticate(
        &self,
        opaque_credential: &[u8],
        action: sorafs_node::pop_credentials::PopCredentialApiActionV1,
        request_binding: [u8; 32],
        now_epoch: u64,
    ) -> Result<sorafs_node::pop_credentials::PopAuthenticatedPrincipalV1, String> {
        let wire = PopAuthenticateRequestWireV1 {
            opaque_credential: opaque_credential.to_vec(),
            action: pop_action_to_wire(action),
            request_binding,
            now_epoch,
        };
        validate_pop_authenticate_request(&wire)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let payload = encode_canonical(&wire, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let result = self
            .provider
            .call(OPERATION_POP_AUTHENTICATE_V1, payload, false)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let principal = self
            .provider
            .decode::<PopAuthenticatedPrincipalWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        validate_pop_principal(principal, &wire)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        Ok(sorafs_node::pop_credentials::PopAuthenticatedPrincipalV1 {
            principal_digest: principal.principal_digest,
            expires_at_epoch: principal.expires_at_epoch,
            request_authority: if principal.caller_signed_transaction {
                sorafs_node::pop_credentials::PopRequestAuthorityV1::CallerSignedTransaction
            } else {
                sorafs_node::pop_credentials::PopRequestAuthorityV1::AuthenticatedRequest
            },
        })
    }
}
#[derive(Clone)]
struct PopBrokerRegistrySubmitter {
    provider: PopBrokerProvider,
}
impl fmt::Debug for PopBrokerRegistrySubmitter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopBrokerRegistrySubmitter([REDACTED])")
    }
}
impl sorafs_node::pop_credentials::PopRegistrySubmitter for PopBrokerRegistrySubmitter {
    fn submit(
        &self,
        idempotency_key: [u8; 32],
        operation: &sorafs_node::pop_credentials::PopRegistryOperationV1,
    ) -> Result<(), String> {
        operation
            .validate()
            .map_err(|_| "PoP runtime provider unavailable".to_owned())?;
        let wire = PopRegistrySubmitRequestWireV1 {
            idempotency_key,
            operation: operation.clone(),
        };
        let payload = encode_canonical(&wire, MAX_POP_REGISTRY_OPERATION_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let result = self
            .provider
            .call(OPERATION_POP_REGISTRY_SUBMIT_V1, payload, true)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        self.provider
            .decode::<()>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))
    }
}
#[derive(Clone)]
struct PopBrokerRegistryReader {
    provider: PopBrokerProvider,
}
impl fmt::Debug for PopBrokerRegistryReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopBrokerRegistryReader([REDACTED])")
    }
}
impl sorafs_node::pop_credentials::PopFinalizedRegistryReader for PopBrokerRegistryReader {
    fn next_after(
        &self,
        cursor: Option<sorafs_node::pop_credentials::PopFinalizedCursorV1>,
    ) -> Result<Option<sorafs_node::pop_credentials::PopFinalizedRegistryProjectionV1>, String>
    {
        if let Some(cursor) = cursor {
            validate_pop_cursor(cursor)
                .map_err(|error| self.provider.redacted_string_error(error))?;
        }
        let payload = encode_canonical(
            &PopRegistryNextRequestWireV1 { cursor },
            MAX_POP_RUNTIME_FRAME_BYTES_V1,
        )
        .map_err(|error| self.provider.redacted_string_error(error))?;
        let result = self
            .provider
            .call(OPERATION_POP_REGISTRY_NEXT_V1, payload, false)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let outcome = self
            .provider
            .decode::<PopRegistryNextResultWireV1>(&result, MAX_POP_PROJECTION_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        if let Some(projection) = outcome.projection.as_ref() {
            let exact = self
                .provider
                .binding
                .pop_credential_runtime_binding
                .as_ref()
                .ok_or_else(|| {
                    self.provider
                        .redacted_string_error(BrokerError::BindingMismatch)
                })?;
            validate_pop_projection(projection, exact)
                .map_err(|error| self.provider.redacted_string_error(error))?;
        }
        Ok(outcome.projection)
    }
}
#[derive(Clone)]
struct PopBrokerIssuanceDraftProvider {
    provider: PopBrokerProvider,
}
impl fmt::Debug for PopBrokerIssuanceDraftProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopBrokerIssuanceDraftProvider([REDACTED])")
    }
}
impl iroha_torii::sorafs::pop_api::PopIssuanceDraftProviderV1 for PopBrokerIssuanceDraftProvider {
    fn resolve(
        &self,
        request_id: [u8; 32],
        now_epoch: u64,
    ) -> Result<
        sorafs_node::pop_credentials::PopIssuanceDraftV1,
        iroha_torii::sorafs::pop_api::PopPrivateMaterialProviderErrorV1,
    > {
        use iroha_torii::sorafs::pop_api::PopPrivateMaterialProviderErrorV1 as Error;
        let wire = PopIssuanceDraftRequestWireV1 {
            request_id,
            now_epoch,
        };
        if request_id == [0; 32] || now_epoch == 0 {
            return Err(Error::Unavailable);
        }
        let payload = encode_canonical(&wire, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|_| Error::Unavailable)?;
        let result = self
            .provider
            .call(OPERATION_POP_ISSUANCE_DRAFT_V1, payload, false)
            .map_err(|_| Error::Unavailable)?;
        let outcome = self
            .provider
            .decode::<PopIssuanceDraftResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|_| Error::Unavailable)?;
        let exact = self
            .provider
            .binding
            .pop_credential_runtime_binding
            .as_ref()
            .ok_or(Error::Unavailable)?;
        validate_pop_draft(&outcome, wire, exact).map_err(|_| Error::Unavailable)?;
        Ok(sorafs_node::pop_credentials::PopIssuanceDraftV1 {
            request_id: outcome.request_id,
            credential: outcome.credential,
            commitment_root: outcome.commitment_root,
            revocation_list: outcome.revocation_list,
            witness: outcome.witness.into_witness(),
        })
    }
}
#[derive(Clone)]
struct PopBrokerWalletKeyWrapper {
    provider: PopBrokerProvider,
    active_key_id: String,
}
impl_broker_debug_fields!(PopBrokerWalletKeyWrapper as value {
    "active_key_id" => value.active_key_id,
    "private_wrapper" => "[REDACTED]",
} => finish_non_exhaustive);
impl sorafs_node::pop_credentials::PopWalletKeyWrapper for PopBrokerWalletKeyWrapper {
    fn active_key_id(&self) -> &str {
        &self.active_key_id
    }
    fn wrap_dek(&self, context: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
        let wire = PopWalletWrapDekRequestWireV1 { context, dek: *dek };
        if context == [0; 32] || *dek == [0; 32] {
            return Err("PoP runtime provider unavailable".to_owned());
        }
        let payload = encode_canonical(&wire, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let result = self
            .provider
            .call(OPERATION_POP_WALLET_WRAP_DEK_V1, payload, true)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let wrapped = self
            .provider
            .decode::<PopWalletWrapDekResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        if wrapped.wrapped_dek.is_empty()
            || wrapped.wrapped_dek.len() > MAX_POP_WRAPPED_DEK_BYTES_V1
        {
            self.provider.session.poison();
            return Err("PoP runtime provider unavailable".to_owned());
        }
        Ok(wrapped.wrapped_dek.clone())
    }
    fn unwrap_dek(
        &self,
        key_id: &str,
        context: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], String> {
        if key_id != self.active_key_id
            || context == [0; 32]
            || wrapped_dek.is_empty()
            || wrapped_dek.len() > MAX_POP_WRAPPED_DEK_BYTES_V1
        {
            return Err("PoP runtime provider unavailable".to_owned());
        }
        let wire = PopWalletUnwrapDekRequestWireV1 {
            key_id: key_id.to_owned(),
            context,
            wrapped_dek: wrapped_dek.to_vec(),
        };
        let payload = encode_canonical(&wire, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let result = self
            .provider
            .call(OPERATION_POP_WALLET_UNWRAP_DEK_V1, payload, false)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        let unwrapped = self
            .provider
            .decode::<PopWalletUnwrapDekResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|error| self.provider.redacted_string_error(error))?;
        if unwrapped.dek == [0; 32] {
            self.provider.session.poison();
            return Err("PoP runtime provider unavailable".to_owned());
        }
        Ok(unwrapped.dek)
    }
}
#[derive(Clone)]
struct PopBrokerWalletWitnessProvider {
    provider: PopBrokerProvider,
}
impl fmt::Debug for PopBrokerWalletWitnessProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopBrokerWalletWitnessProvider([REDACTED])")
    }
}
impl iroha_torii::sorafs::pop_api::PopWalletWitnessProviderV1 for PopBrokerWalletWitnessProvider {
    fn resolve(
        &self,
        credential_commitment: [u8; 32],
        projection: &sorafs_node::pop_credentials::PopFinalizedRegistryProjectionV1,
    ) -> Result<
        sorafs_manifest::pop_credentials::PopMembershipWitnessV1,
        iroha_torii::sorafs::pop_api::PopPrivateMaterialProviderErrorV1,
    > {
        use iroha_torii::sorafs::pop_api::PopPrivateMaterialProviderErrorV1 as Error;
        let exact = self
            .provider
            .binding
            .pop_credential_runtime_binding
            .as_ref()
            .ok_or(Error::Unavailable)?;
        if credential_commitment == [0; 32] || validate_pop_projection(projection, exact).is_err() {
            return Err(Error::Unavailable);
        }
        let wire = PopWalletWitnessRequestWireV1 {
            credential_commitment,
            projection: projection.clone(),
        };
        let payload =
            encode_canonical(&wire, MAX_POP_PROJECTION_BYTES_V1).map_err(|_| Error::Unavailable)?;
        let result = self
            .provider
            .call(OPERATION_POP_WALLET_WITNESS_V1, payload, false)
            .map_err(|_| Error::Unavailable)?;
        let witness = self
            .provider
            .decode::<PopMembershipWitnessWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|_| Error::Unavailable)?;
        validate_pop_witness_wire(&witness).map_err(|_| Error::Unavailable)?;
        Ok(witness.into_witness())
    }
}
#[derive(Clone)]
struct PopBrokerFinalizedTimeProvider {
    provider: PopBrokerProvider,
}
impl fmt::Debug for PopBrokerFinalizedTimeProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PopBrokerFinalizedTimeProvider([REDACTED])")
    }
}
impl iroha_torii::sorafs::pop_api::PopFinalizedTimeProviderV1 for PopBrokerFinalizedTimeProvider {
    fn sample(
        &self,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopFinalizedTimeSampleV1,
        iroha_torii::sorafs::pop_api::PopFinalizedTimeProviderErrorV1,
    > {
        use iroha_torii::sorafs::pop_api::PopFinalizedTimeProviderErrorV1 as Error;
        let payload = encode_canonical(&(), MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|_| Error::Unavailable)?;
        let result = self
            .provider
            .call(OPERATION_POP_FINALIZED_TIME_V1, payload, false)
            .map_err(|_| Error::Unavailable)?;
        let sample = self
            .provider
            .decode::<PopFinalizedTimeResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(|_| Error::Unavailable)?;
        validate_pop_finalized_time(sample).map_err(|_| Error::Unavailable)?;
        Ok(iroha_torii::sorafs::pop_api::PopFinalizedTimeSampleV1 {
            finalized_block_height: sample.finalized_block_height,
            finalized_block_hash: sample.finalized_block_hash,
            finalized_epoch: sample.finalized_epoch,
            observed_epoch: sample.observed_epoch,
        })
    }
}
fn por_replay_archive_external_error(
    error: BrokerError,
) -> sorafs_node::PorFinalizedReplayArchiveExternalErrorV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected
        }
    }
}
fn privacy_cycle_prf_error(error: BrokerError) -> sorafs_node::PrivacyCyclePrfProviderErrorV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous | BrokerError::StaleOrRevoked => {
            sorafs_node::PrivacyCyclePrfProviderErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => sorafs_node::PrivacyCyclePrfProviderErrorV1::Internal,
    }
}
#[derive(Clone)]
struct PrivacyCyclePrfBrokerProvider {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(PrivacyCyclePrfBrokerProvider as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl PrivacyCyclePrfBrokerProvider {
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, BrokerError> {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            sorafs_node::TransparencyRuntimeProviderQualificationV1::new(
                qualification.revision,
                qualification.policy_digest,
            ),
        )
    }
}
impl sorafs_node::ProductionTransparencyRuntimeProviderV1 for PrivacyCyclePrfBrokerProvider {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
}
impl sorafs_node::PrivacyCyclePrfProviderV1 for PrivacyCyclePrfBrokerProvider {
    fn derive_cycle_output(
        &self,
        request: &sorafs_node::PrivacyCyclePrfRequestV1,
    ) -> Result<sorafs_node::PrivacyCyclePrfOutputV1, sorafs_node::PrivacyCyclePrfProviderErrorV1>
    {
        self.live_qualification().map_err(privacy_cycle_prf_error)?;
        let wire = PrivacyCyclePrfRequestWireV1::from_request(request);
        wire.to_request().map_err(privacy_cycle_prf_error)?;
        let payload = encode_sensitive_canonical(&wire, MAX_TRANSPARENCY_PRF_FRAME_BYTES_V1)
            .map_err(privacy_cycle_prf_error)?;
        let result = provider_call!(
            self,
            call_sensitive,
            OPERATION_PRIVACY_CYCLE_PRF_DERIVE_V1,
            payload,
            false,
        )
        .map_err(privacy_cycle_prf_error)?;
        if self.live_qualification().is_err() {
            self.session.poison();
            return Err(sorafs_node::PrivacyCyclePrfProviderErrorV1::Unavailable);
        }
        let output = decode_scrubbed_canonical::<PrivacyCyclePrfOutputWireV1>(
            &result,
            MAX_TRANSPARENCY_PRF_FRAME_BYTES_V1,
        )
        .map_err(|error| {
            self.session.poison();
            privacy_cycle_prf_error(error)
        })?;
        sorafs_node::PrivacyCyclePrfOutputV1::new(output.output).map_err(|_| {
            self.session.poison();
            sorafs_node::PrivacyCyclePrfProviderErrorV1::Internal
        })
    }
}
fn privacy_release_anchor_error(error: BrokerError) -> sorafs_node::PrivacyReleaseAnchorErrorV1 {
    match error {
        BrokerError::Conflict => sorafs_node::PrivacyReleaseAnchorErrorV1::Conflict,
        BrokerError::Unavailable | BrokerError::Ambiguous | BrokerError::StaleOrRevoked => {
            sorafs_node::PrivacyReleaseAnchorErrorV1::Unavailable
        }
        BrokerError::Rejected | BrokerError::Protocol | BrokerError::BindingMismatch => {
            sorafs_node::PrivacyReleaseAnchorErrorV1::InvalidState
        }
    }
}
#[derive(Clone)]
struct PrivacyReleaseAnchorBroker {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(PrivacyReleaseAnchorBroker as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl PrivacyReleaseAnchorBroker {
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, BrokerError> {
        let exact = transparency_runtime_binding_from_wire(&self.binding)?;
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        if qualification.revision != exact.qualification().revision()
            || qualification.policy_digest != exact.qualification().policy_digest()
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(exact.qualification())
    }
    fn call(
        &self,
        operation: u16,
        payload: Vec<u8>,
        mutating: bool,
    ) -> Result<ScrubbedBytes, BrokerError> {
        self.live_qualification()?;
        let result = provider_call!(self, call, operation, payload, mutating,)?;
        if self.live_qualification().is_err() {
            self.session.poison();
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::StaleOrRevoked
            });
        }
        Ok(result)
    }
}
impl sorafs_node::ProductionTransparencyRuntimeProviderV1 for PrivacyReleaseAnchorBroker {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
}
impl sorafs_node::PrivacyReleaseAnchorV1 for PrivacyReleaseAnchorBroker {
    fn finalized_head(
        &self,
        query_id: [u8; 32],
    ) -> Result<sorafs_node::PrivacyReleaseAnchorHeadV1, sorafs_node::PrivacyReleaseAnchorErrorV1>
    {
        let request = PrivacyReleaseAnchorFinalizedHeadRequestWireV1 { query_id };
        validate_privacy_release_anchor_query(request).map_err(privacy_release_anchor_error)?;
        let payload = encode_canonical(&request, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
            .map_err(privacy_release_anchor_error)?;
        let result = self
            .call(
                OPERATION_PRIVACY_RELEASE_ANCHOR_FINALIZED_HEAD_V1,
                payload,
                false,
            )
            .map_err(privacy_release_anchor_error)?;
        let head = decode_scrubbed_canonical::<PrivacyReleaseAnchorHeadWireV1>(
            &result,
            MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
        )
        .and_then(PrivacyReleaseAnchorHeadWireV1::to_head)
        .map_err(|error| {
            self.session.poison();
            privacy_release_anchor_error(error)
        })?;
        if head.query_id() != query_id {
            self.session.poison();
            return Err(sorafs_node::PrivacyReleaseAnchorErrorV1::InvalidState);
        }
        Ok(head)
    }
    fn compare_and_set_finalized_head(
        &self,
        expected: sorafs_node::PrivacyReleaseAnchorHeadV1,
        next: sorafs_node::PrivacyReleaseAnchorHeadV1,
        lease: &sorafs_node::TransparencyLeaderLeaseGrantV1,
    ) -> Result<(), sorafs_node::PrivacyReleaseAnchorErrorV1> {
        let request = PrivacyReleaseAnchorCompareAndSetRequestWireV1 {
            expected: PrivacyReleaseAnchorHeadWireV1::from_head(expected),
            next: PrivacyReleaseAnchorHeadWireV1::from_head(next),
            lease: TransparencyLeaderLeaseGrantWireV1::from_grant(lease),
        };
        validate_privacy_release_anchor_compare_and_set(&request)
            .map_err(privacy_release_anchor_error)?;
        let payload = encode_canonical(&request, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
            .map_err(privacy_release_anchor_error)?;
        let result = self
            .call(
                OPERATION_PRIVACY_RELEASE_ANCHOR_COMPARE_AND_SET_V1,
                payload,
                true,
            )
            .map_err(privacy_release_anchor_error)?;
        decode_scrubbed_canonical::<()>(&result, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1).map_err(
            |error| {
                self.session.poison();
                privacy_release_anchor_error(error)
            },
        )
    }
}
const fn transparency_leader_lease_error(
    error: BrokerError,
) -> sorafs_node::TransparencyLeaderLeaseProviderErrorV1 {
    match error {
        BrokerError::Conflict => sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Conflict,
        BrokerError::Ambiguous => sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous,
        BrokerError::Unavailable => {
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Unavailable
        }
        BrokerError::StaleOrRevoked | BrokerError::BindingMismatch => {
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1::AuthenticationFailed
        }
        BrokerError::Rejected | BrokerError::Protocol => {
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Internal
        }
    }
}
#[derive(Clone)]
struct TransparencyLeaderLeaseBroker {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(TransparencyLeaderLeaseBroker as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl TransparencyLeaderLeaseBroker {
    fn exact_binding(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderBindingV1, BrokerError> {
        transparency_runtime_binding_from_wire(&self.binding)
    }
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, BrokerError> {
        let exact = self.exact_binding()?;
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        if qualification.revision != exact.qualification().revision()
            || qualification.policy_digest != exact.qualification().policy_digest()
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(exact.qualification())
    }
    fn call(&self, operation: u16, payload: Vec<u8>) -> Result<ScrubbedBytes, BrokerError> {
        self.live_qualification()?;
        let result = provider_call!(self, call, operation, payload, true,)?;
        if self.live_qualification().is_err() {
            self.session.poison();
            return Err(BrokerError::Ambiguous);
        }
        Ok(result)
    }
}
impl sorafs_node::ProductionTransparencyRuntimeProviderV1 for TransparencyLeaderLeaseBroker {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::TransparencyRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
}
impl sorafs_node::TransparencyLeaderLeaseProviderV1 for TransparencyLeaderLeaseBroker {
    fn acquire(
        &self,
        request: &sorafs_node::TransparencyLeaderLeaseAcquireRequestV1,
    ) -> Result<
        sorafs_node::TransparencyLeaderLeaseGrantV1,
        sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
    > {
        let configured = self
            .exact_binding()
            .map_err(transparency_leader_lease_error)?;
        let wire = TransparencyLeaderLeaseAcquireRequestWireV1::from_request(request);
        validate_transparency_leader_lease_acquire(&wire, &configured)
            .map_err(transparency_leader_lease_error)?;
        let payload = encode_canonical(&wire, MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1)
            .map_err(transparency_leader_lease_error)?;
        let result = self
            .call(OPERATION_TRANSPARENCY_LEADER_LEASE_ACQUIRE_V1, payload)
            .map_err(transparency_leader_lease_error)?;
        let grant = decode_scrubbed_canonical::<TransparencyLeaderLeaseGrantWireV1>(
            &result,
            MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
        )
        .and_then(|wire| wire.to_grant())
        .map_err(|_| {
            self.session.poison();
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous
        })?;
        validate_transparency_leader_lease_acquire_grant(request, &grant, &configured).map_err(
            |_| {
                self.session.poison();
                sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous
            },
        )?;
        Ok(grant)
    }
    fn renew(
        &self,
        request: &sorafs_node::TransparencyLeaderLeaseRenewRequestV1,
    ) -> Result<
        sorafs_node::TransparencyLeaderLeaseGrantV1,
        sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
    > {
        let configured = self
            .exact_binding()
            .map_err(transparency_leader_lease_error)?;
        let wire = TransparencyLeaderLeaseRenewRequestWireV1::from_request(request);
        validate_transparency_leader_lease_renew(&wire, &configured)
            .map_err(transparency_leader_lease_error)?;
        let payload = encode_canonical(&wire, MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1)
            .map_err(transparency_leader_lease_error)?;
        let result = self
            .call(OPERATION_TRANSPARENCY_LEADER_LEASE_RENEW_V1, payload)
            .map_err(transparency_leader_lease_error)?;
        let grant = decode_scrubbed_canonical::<TransparencyLeaderLeaseGrantWireV1>(
            &result,
            MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
        )
        .and_then(|wire| wire.to_grant())
        .map_err(|_| {
            self.session.poison();
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous
        })?;
        validate_transparency_leader_lease_renew_grant(request, &grant, &configured).map_err(
            |_| {
                self.session.poison();
                sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous
            },
        )?;
        Ok(grant)
    }
    fn release(
        &self,
        request: &sorafs_node::TransparencyLeaderLeaseReleaseRequestV1,
    ) -> Result<
        sorafs_node::TransparencyLeaderLeaseReleaseReceiptV1,
        sorafs_node::TransparencyLeaderLeaseProviderErrorV1,
    > {
        let configured = self
            .exact_binding()
            .map_err(transparency_leader_lease_error)?;
        let wire = TransparencyLeaderLeaseReleaseRequestWireV1::from_request(request);
        validate_transparency_leader_lease_release(&wire, &configured)
            .map_err(transparency_leader_lease_error)?;
        let payload = encode_canonical(&wire, MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1)
            .map_err(transparency_leader_lease_error)?;
        let result = self
            .call(OPERATION_TRANSPARENCY_LEADER_LEASE_RELEASE_V1, payload)
            .map_err(transparency_leader_lease_error)?;
        let receipt = decode_scrubbed_canonical::<TransparencyLeaderLeaseReleaseReceiptWireV1>(
            &result,
            MAX_TRANSPARENCY_LEADER_LEASE_FRAME_BYTES_V1,
        )
        .and_then(|wire| wire.to_receipt())
        .map_err(|_| {
            self.session.poison();
            sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous
        })?;
        validate_transparency_leader_lease_release_receipt(request, &receipt, &configured)
            .map_err(|_| {
                self.session.poison();
                sorafs_node::TransparencyLeaderLeaseProviderErrorV1::Ambiguous
            })?;
        Ok(receipt)
    }
}
const fn fenced_privacy_broker_error(
    error: BrokerError,
) -> sorafs_node::FencedTransparencyPublishErrorV1 {
    match error {
        BrokerError::Conflict => sorafs_node::FencedTransparencyPublishErrorV1::CompareConflict,
        BrokerError::Ambiguous => sorafs_node::FencedTransparencyPublishErrorV1::Ambiguous,
        BrokerError::Unavailable | BrokerError::StaleOrRevoked | BrokerError::BindingMismatch => {
            sorafs_node::FencedTransparencyPublishErrorV1::UnqualifiedProvider
        }
        BrokerError::Rejected => sorafs_node::FencedTransparencyPublishErrorV1::Rejected,
        BrokerError::Protocol => sorafs_node::FencedTransparencyPublishErrorV1::InvalidReceipt,
    }
}
#[derive(Clone)]
struct FencedPrivacyPublisherBroker {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(FencedPrivacyPublisherBroker as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl FencedPrivacyPublisherBroker {
    fn expected_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, BrokerError> {
        qualification_from_binding(&self.binding)
    }
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, BrokerError> {
        let expected = self.expected_qualification()?;
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        if qualification.revision != expected.revision
            || qualification.policy_digest != expected.policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
    fn call(&self, payload: Vec<u8>) -> Result<ScrubbedBytes, BrokerError> {
        self.live_qualification()?;
        let result = provider_call!(
            self,
            call,
            OPERATION_FENCED_PRIVACY_COMPARE_AND_APPEND_V1,
            payload,
            true,
        )?;
        if self.live_qualification().is_err() {
            self.session.poison();
            return Err(BrokerError::Ambiguous);
        }
        Ok(result)
    }
}
impl sorafs_node::FencedTransparencyPublisherV1 for FencedPrivacyPublisherBroker {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
    fn compare_and_append_privacy(
        &self,
        request: &sorafs_node::FencedPrivacyPublicationRequestV1,
    ) -> Result<
        sorafs_node::FencedPrivacyPublicationReceiptV1,
        sorafs_node::FencedTransparencyPublishErrorV1,
    > {
        request.validate()?;
        let wire = FencedPrivacyPublicationRequestWireV1::from_request(request);
        wire.to_request()
            .map_err(|_| sorafs_node::FencedTransparencyPublishErrorV1::InvalidRequest)?;
        let payload = encode_canonical(&wire, MAX_FENCED_PRIVACY_PUBLICATION_FRAME_BYTES_V1)
            .map_err(|_| sorafs_node::FencedTransparencyPublishErrorV1::InvalidRequest)?;
        let result = self.call(payload).map_err(fenced_privacy_broker_error)?;
        let qualification = self
            .expected_qualification()
            .map_err(fenced_privacy_broker_error)?;
        let receipt = decode_scrubbed_canonical::<FencedPrivacyPublicationReceiptWireV1>(
            &result,
            MAX_FENCED_PRIVACY_PUBLICATION_FRAME_BYTES_V1,
        )
        .and_then(|wire| wire.to_receipt(request, &self.binding.handle, qualification))
        .map_err(|_| {
            self.session.poison();
            sorafs_node::FencedTransparencyPublishErrorV1::InvalidReceipt
        })?;
        Ok(receipt)
    }
}
#[derive(Clone)]
struct FencedPrivacyHeadReaderBroker {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(FencedPrivacyHeadReaderBroker as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl FencedPrivacyHeadReaderBroker {
    fn expected_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, BrokerError> {
        qualification_from_binding(&self.binding)
    }
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, BrokerError> {
        let expected = self.expected_qualification()?;
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        if qualification.revision != expected.revision
            || qualification.policy_digest != expected.policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
    fn call(&self, payload: Vec<u8>) -> Result<ScrubbedBytes, BrokerError> {
        self.live_qualification()?;
        let result = provider_call!(
            self,
            call,
            OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1,
            payload,
            false,
        )?;
        self.live_qualification()?;
        Ok(result)
    }
}
impl sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1 for FencedPrivacyHeadReaderBroker {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
    fn read_authoritative_head_with_ancestry(
        &self,
        required_ancestors: &[sorafs_node::FencedTransparencyTargetHeadV1],
        required_publications: &[sorafs_node::FencedTransparencyPublicationInclusionV1],
    ) -> Result<sorafs_node::FencedTransparencyHeadAncestryProofV1, String> {
        let wire = FencedPrivacyHeadReadRequestWireV1::from_required_evidence(
            required_ancestors,
            required_publications,
        );
        wire.to_required_evidence()
            .map_err(redacted_provider_error)?;
        let payload = encode_canonical(&wire, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
            .map_err(redacted_provider_error)?;
        let result = self.call(payload).map_err(redacted_provider_error)?;
        decode_scrubbed_canonical::<FencedTransparencyHeadAncestryProofWireV1>(
            &result,
            MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1,
        )
        .and_then(|wire| wire.to_proof(required_ancestors, required_publications))
        .map_err(|_| {
            self.session.poison();
            ERROR_REJECTED.to_owned()
        })
    }
}
#[derive(Clone)]
struct PorReplayArchiveBroker {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(PorReplayArchiveBroker as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl PorReplayArchiveBroker {
    fn exact(&self) -> Result<sorafs_node::PorFinalizedReplayArchiveBindingV1, BrokerError> {
        por_replay_archive_exact_binding(&self.binding)
    }
    fn live_binding(&self) -> Result<sorafs_node::PorFinalizedReplayArchiveBindingV1, BrokerError> {
        let exact = self.exact()?;
        live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(exact)
    }
    fn call(
        &self,
        operation: u16,
        payload: Vec<u8>,
        mutating: bool,
    ) -> Result<ScrubbedBytes, BrokerError> {
        self.live_binding()?;
        let result = provider_call!(self, call, operation, payload, mutating,)?;
        if self.live_binding().is_err() {
            self.session.poison();
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::StaleOrRevoked
            });
        }
        Ok(result)
    }
    fn decode<T>(&self, bytes: &ScrubbedBytes, max_bytes: usize) -> Result<T, BrokerError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        let _scope = bytes.enter_decode_admission();
        decode_canonical(bytes, max_bytes).inspect_err(|_| self.session.poison())
    }
}
impl sorafs_node::PorFinalizedReplayArchiveV1 for PorReplayArchiveBroker {
    fn runtime_handle(&self) -> &str {
        &self.binding.handle
    }
    fn binding(
        &self,
    ) -> Result<
        sorafs_node::PorFinalizedReplayArchiveBindingV1,
        sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
    > {
        self.live_binding()
            .map_err(por_replay_archive_external_error)
    }
    fn check_readiness(&self) -> Result<(), sorafs_node::PorFinalizedReplayArchiveExternalErrorV1> {
        let payload = encode_canonical(&(), MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
            .map_err(por_replay_archive_external_error)?;
        let result = self
            .call(OPERATION_POR_REPLAY_ARCHIVE_READINESS_V1, payload, false)
            .map_err(por_replay_archive_external_error)?;
        self.decode::<()>(&result, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
            .map_err(por_replay_archive_external_error)
    }
    fn current_head(
        &self,
    ) -> Result<
        Option<sorafs_node::PorFinalizedReplayArchiveReceiptV1>,
        sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
    > {
        let exact = self
            .live_binding()
            .map_err(por_replay_archive_external_error)?;
        let payload = encode_canonical(&(), MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
            .map_err(por_replay_archive_external_error)?;
        let result = self
            .call(OPERATION_POR_REPLAY_ARCHIVE_CURRENT_HEAD_V1, payload, false)
            .map_err(por_replay_archive_external_error)?;
        let head = self
            .decode::<Option<sorafs_node::PorFinalizedReplayArchiveReceiptV1>>(
                &result,
                MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
            )
            .map_err(por_replay_archive_external_error)?;
        if let Some(head) = head {
            validate_por_replay_archive_receipt(&head, exact)
                .map_err(por_replay_archive_external_error)?;
        }
        Ok(head)
    }
    fn append(
        &self,
        record: &sorafs_node::PorFinalizedReplayArchiveRecordV1,
        expected_previous_head: Option<[u8; 32]>,
    ) -> Result<
        sorafs_node::PorFinalizedReplayArchiveReceiptV1,
        sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
    > {
        let exact = self
            .live_binding()
            .map_err(por_replay_archive_external_error)?;
        let wire = PorReplayArchiveAppendRequestWireV1 {
            canonical_record: encode_por_replay_archive_record(record)
                .map_err(por_replay_archive_external_error)?,
            expected_previous_head,
        };
        validate_por_replay_archive_append_request(&wire)
            .map_err(por_replay_archive_external_error)?;
        let payload = encode_canonical(&wire, MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1)
            .map_err(por_replay_archive_external_error)?;
        let result = self
            .call(OPERATION_POR_REPLAY_ARCHIVE_APPEND_V1, payload, true)
            .map_err(por_replay_archive_external_error)?;
        let receipt = self
            .decode::<sorafs_node::PorFinalizedReplayArchiveReceiptV1>(
                &result,
                MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1,
            )
            .map_err(por_replay_archive_external_error)?;
        receipt
            .validate_record(exact, record, Some(expected_previous_head))
            .map_err(|_| {
                self.session.poison();
                sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Unavailable
            })?;
        Ok(receipt)
    }
    fn lookup(
        &self,
        challenge_id: [u8; 32],
        expected_checkpoint_head: sorafs_node::PorFinalizedReplayArchiveReceiptV1,
        proof_bounds: sorafs_node::PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<
        sorafs_node::PorFinalizedReplayArchiveLookupV1,
        sorafs_node::PorFinalizedReplayArchiveExternalErrorV1,
    > {
        self.live_binding()
            .map_err(por_replay_archive_external_error)?;
        let max_successor_receipts = u32::try_from(proof_bounds.max_successor_receipts())
            .map_err(|_| sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected)?;
        let wire = PorReplayArchiveLookupRequestWireV1 {
            challenge_id,
            expected_checkpoint_head,
            max_successor_receipts,
            max_successor_proof_bytes: proof_bounds.max_successor_proof_bytes(),
        };
        validate_por_replay_archive_lookup_request(&wire, &self.binding)
            .map_err(por_replay_archive_external_error)?;
        let payload = encode_canonical(&wire, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
            .map_err(por_replay_archive_external_error)?;
        let result = self
            .call(OPERATION_POR_REPLAY_ARCHIVE_LOOKUP_V1, payload, false)
            .map_err(por_replay_archive_external_error)?;
        let outcome = self
            .decode::<PorReplayArchiveLookupOutcomeWireV1>(
                &result,
                MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1,
            )
            .map_err(por_replay_archive_external_error)?;
        por_replay_archive_lookup_from_wire(&outcome, &wire, &self.binding).map_err(|_| {
            self.session.poison();
            sorafs_node::PorFinalizedReplayArchiveExternalErrorV1::Rejected
        })
    }
}
#[derive(Clone)]
struct GatewayAcmeBrokerClient {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(GatewayAcmeBrokerClient as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl GatewayAcmeBrokerClient {
    fn live_identity(
        &self,
    ) -> Result<iroha_torii::sorafs::gateway::AcmeClientIdentityV1, BrokerError> {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(iroha_torii::sorafs::gateway::AcmeClientIdentityV1 {
            provider_handle: self.binding.handle.clone(),
            revision: qualification.revision,
            policy_digest: qualification.policy_digest,
            test_marked: false,
        })
    }
}
impl iroha_torii::sorafs::gateway::AcmeClient for GatewayAcmeBrokerClient {
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::gateway::AcmeClientIdentityV1,
        iroha_torii::sorafs::gateway::AcmeClientProbeError,
    > {
        self.live_identity()
            .map_err(|_| iroha_torii::sorafs::gateway::AcmeClientProbeError)
    }
    fn order_certificate(
        &self,
        order: &iroha_torii::sorafs::gateway::CertificateOrder,
    ) -> Result<
        iroha_torii::sorafs::gateway::CertificateBundle,
        iroha_torii::sorafs::gateway::AcmeClientError,
    > {
        use iroha_torii::sorafs::gateway::AcmeClientError as Error;
        self.live_identity().map_err(|_| Error::Transport)?;
        let wire = GatewayAcmeOrderRequestWireV1 {
            hostnames: order.hostnames.clone(),
            account_email: order.account_email.clone(),
            directory_url: order.directory_url.clone(),
            dns_provider_id: order.dns_provider_id.clone(),
            dns01: order.challenge.dns01,
            tls_alpn_01: order.challenge.tls_alpn_01,
        };
        validate_gateway_acme_order(&wire).map_err(|_| Error::Rejected)?;
        let payload = encode_canonical(&wire, MAX_GATEWAY_ACME_FRAME_BYTES_V1)
            .map_err(|_| Error::Rejected)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_GATEWAY_ACME_ORDER_CERTIFICATE_V1,
            payload,
            true,
        )
        .map_err(|error| match error {
            BrokerError::Rejected => Error::Rejected,
            _ => Error::Transport,
        })?;
        let outcome = self
            .session
            .decode_result::<GatewayAcmeOrderOutcomeWireV1>(&result)
            .map_err(|_| Error::Transport)?;
        if validate_gateway_acme_outcome(&outcome).is_err() {
            self.session.poison();
            return Err(Error::Transport);
        }
        if self.live_identity().is_err() {
            self.session.poison();
            return Err(Error::Transport);
        }
        match outcome.outcome {
            0 => Ok(iroha_torii::sorafs::gateway::CertificateBundle {
                certificate_pem: outcome.certificate_pem.clone(),
                private_key_pem: outcome.private_key_pem.clone(),
                ech_config: outcome.ech_config.clone(),
                not_after: outcome
                    .not_after
                    .ok_or(Error::Transport)?
                    .to_system_time()
                    .map_err(|_| Error::Transport)?,
            }),
            1 => Err(Error::Rejected),
            2 => Err(Error::Temporary {
                retry_after: outcome
                    .retry_after
                    .map(DurationWireV1::to_duration)
                    .transpose()
                    .map_err(|_| Error::Transport)?,
            }),
            3 => Err(Error::Transport),
            _ => {
                self.session.poison();
                Err(Error::Transport)
            }
        }
    }
}
#[derive(Clone)]
struct GatewayComplianceBrokerFeedTransport {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(GatewayComplianceBrokerFeedTransport as value {
    "handle" => value.binding.handle,
} => finish_non_exhaustive);
impl GatewayComplianceBrokerFeedTransport {
    fn live_identity(
        &self,
    ) -> Result<iroha_torii::sorafs::gateway::GatewayComplianceFeedTransportIdentityV1, BrokerError>
    {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            iroha_torii::sorafs::gateway::GatewayComplianceFeedTransportIdentityV1 {
                provider_handle: self.binding.handle.clone(),
                revision: qualification.revision,
                policy_digest: qualification.policy_digest,
                test_marked: false,
            },
        )
    }
    fn operation_error(
        &self,
        error: BrokerError,
    ) -> iroha_torii::sorafs::gateway::GatewayComplianceError {
        if matches!(
            error,
            BrokerError::Protocol
                | BrokerError::BindingMismatch
                | BrokerError::StaleOrRevoked
                | BrokerError::Ambiguous
        ) {
            self.session.poison();
        }
        iroha_torii::sorafs::gateway::GatewayComplianceError::FeedTransportOperationFailed
    }
}
impl iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport
    for GatewayComplianceBrokerFeedTransport
{
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::gateway::GatewayComplianceFeedTransportIdentityV1,
        iroha_torii::sorafs::gateway::GatewayComplianceFeedTransportProbeError,
    > {
        self.live_identity()
            .map_err(|_| iroha_torii::sorafs::gateway::GatewayComplianceFeedTransportProbeError)
    }
    fn resolve(
        &self,
        hostname: &str,
        timeout: Duration,
    ) -> Result<Vec<std::net::IpAddr>, iroha_torii::sorafs::gateway::GatewayComplianceError> {
        let wire = GatewayComplianceResolveRequestWireV1 {
            hostname: hostname.to_owned(),
            timeout: DurationWireV1::from_duration(timeout),
        };
        validate_gateway_compliance_resolve_request(&wire)
            .map_err(|error| self.operation_error(error))?;
        let payload =
            encode_canonical(&wire, 128 * 1024).map_err(|error| self.operation_error(error))?;
        let result = provider_call!(
            self,
            call,
            OPERATION_GATEWAY_COMPLIANCE_RESOLVE_V1,
            payload,
            false,
        )
        .map_err(|error| self.operation_error(error))?;
        let outcome = self
            .session
            .decode_result::<GatewayComplianceResolveOutcomeWireV1>(&result)
            .map_err(|error| self.operation_error(error))?;
        validate_gateway_compliance_resolve_outcome(&outcome)
            .map_err(|error| self.operation_error(error))?;
        self.live_identity()
            .map_err(|error| self.operation_error(error))?;
        if outcome.outcome == 0 {
            gateway_addresses_from_wire(&outcome.addresses, false)
                .map_err(|error| self.operation_error(error))
        } else {
            let error =
                gateway_compliance_error_from_wire(outcome.outcome, outcome.found, outcome.maximum)
                    .map_err(|error| self.operation_error(error))?;
            Err(error)
        }
    }
    fn fetch(
        &self,
        request: &iroha_torii::sorafs::gateway::GatewayComplianceFetchRequest,
    ) -> Result<
        iroha_torii::sorafs::gateway::GatewayComplianceFetchResponse,
        iroha_torii::sorafs::gateway::GatewayComplianceError,
    > {
        let wire = GatewayComplianceFetchRequestWireV1 {
            url: request.url.as_str().to_owned(),
            pinned_addresses: request
                .pinned_addresses
                .iter()
                .copied()
                .map(IpAddressWireV1::from)
                .collect(),
            connect_timeout: DurationWireV1::from_duration(request.connect_timeout),
            total_timeout: DurationWireV1::from_duration(request.total_timeout),
            max_encoded_bytes: u64::try_from(request.max_encoded_bytes)
                .map_err(|_| self.operation_error(BrokerError::Rejected))?,
        };
        validate_gateway_compliance_fetch_request(&wire)
            .map_err(|error| self.operation_error(error))?;
        let payload = encode_canonical(&wire, MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1)
            .map_err(|error| self.operation_error(error))?;
        let result = provider_call!(
            self,
            call,
            OPERATION_GATEWAY_COMPLIANCE_FETCH_V1,
            payload,
            false,
        )
        .map_err(|error| self.operation_error(error))?;
        let outcome = self
            .session
            .decode_result::<GatewayComplianceFetchOutcomeWireV1>(&result)
            .map_err(|error| self.operation_error(error))?;
        validate_gateway_compliance_fetch_outcome(&outcome, &wire)
            .map_err(|error| self.operation_error(error))?;
        self.live_identity()
            .map_err(|error| self.operation_error(error))?;
        if outcome.outcome != 0 {
            return Err(gateway_compliance_error_from_wire(
                outcome.outcome,
                outcome.found,
                outcome.maximum,
            )
            .map_err(|error| self.operation_error(error))?);
        }
        let connected_address = outcome
            .connected_address
            .as_ref()
            .ok_or_else(|| self.operation_error(BrokerError::Protocol))?
            .to_address()
            .map_err(|error| self.operation_error(error))?;
        let content_encoding = match outcome.content_encoding {
            0 => iroha_torii::sorafs::gateway::GatewayComplianceContentEncoding::Identity,
            1 => iroha_torii::sorafs::gateway::GatewayComplianceContentEncoding::Gzip,
            2 => iroha_torii::sorafs::gateway::GatewayComplianceContentEncoding::Zstd,
            _ => return Err(self.operation_error(BrokerError::Protocol)),
        };
        Ok(
            iroha_torii::sorafs::gateway::GatewayComplianceFetchResponse {
                status: outcome.status,
                redirect_location: outcome.redirect_location.clone(),
                connected_address,
                peer_spki_sha256: outcome.peer_spki_sha256,
                content_encoding,
                body: outcome.body.clone(),
                elapsed: outcome
                    .elapsed
                    .ok_or_else(|| self.operation_error(BrokerError::Protocol))?
                    .to_duration()
                    .map_err(|error| self.operation_error(error))?,
            },
        )
    }
}
fn moderation_signing_error(
    error: BrokerError,
) -> iroha_torii::sorafs::moderation_runtime::ModerationSigningFailureV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            iroha_torii::sorafs::moderation_runtime::ModerationSigningFailureV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            iroha_torii::sorafs::moderation_runtime::ModerationSigningFailureV1::Refused
        }
    }
}
fn moderation_readiness_error(
    error: BrokerError,
) -> sorafs_node::moderation_orchestrator::ModerationRuntimeProviderReadinessErrorV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            sorafs_node::moderation_orchestrator::
                ModerationRuntimeProviderReadinessErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::moderation_orchestrator::
                ModerationRuntimeProviderReadinessErrorV1::Rejected
        }
    }
}
#[derive(Clone)]
struct ModerationTransactionBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl ModerationTransactionBrokerSigner {
    fn live_qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        BrokerError,
    > {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1::new(
                qualification.revision,
                qualification.policy_digest,
            ),
        )
    }
}
impl_broker_debug_fields!(ModerationTransactionBrokerSigner as value {} => finish_non_exhaustive);
impl sorafs_node::moderation_orchestrator::ModerationRuntimeProviderV1
    for ModerationTransactionBrokerSigner
{
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderReadinessErrorV1,
    > {
        self.live_qualification()
            .map_err(moderation_readiness_error)
    }
}
impl iroha_torii::sorafs::moderation_runtime::ModerationSignedTransactionSignerV1
    for ModerationTransactionBrokerSigner
{
    fn sign(
        &self,
        payload: iroha_data_model::transaction::TransactionPayload,
    ) -> Result<
        iroha_data_model::transaction::SignedTransaction,
        iroha_torii::sorafs::moderation_runtime::ModerationSigningFailureV1,
    > {
        ensure_transaction_session_network(&payload, &self.session.network_id)
            .map_err(moderation_signing_error)?;
        self.live_qualification()
            .map_err(moderation_signing_error)?;
        let expected = payload.clone();
        let payload =
            encode_native_transaction_payload(&payload).map_err(moderation_signing_error)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_NATIVE_TRANSACTION_SIGN_V1,
            payload,
            false,
        )
        .map_err(moderation_signing_error)?;
        let signed = decode_scrubbed_canonical::<iroha_data_model::transaction::SignedTransaction>(
            &result,
            MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1,
        )
        .map_err(|_| {
            self.session.poison();
            iroha_torii::sorafs::moderation_runtime::ModerationSigningFailureV1::Refused
        })?;
        if signed.payload() != &expected
            || signed.authority() != expected.authority()
            || signed.verify_signature().is_err()
        {
            self.session.poison();
            return Err(
                iroha_torii::sorafs::moderation_runtime::ModerationSigningFailureV1::Refused,
            );
        }
        self.live_qualification()
            .map_err(moderation_signing_error)?;
        Ok(signed)
    }
}
fn moderation_handoff_error(
    error: BrokerError,
) -> iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1 {
    match error {
        BrokerError::Unavailable | BrokerError::StaleOrRevoked => {
            iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1::NotDelivered
        }
        BrokerError::Ambiguous => {
            iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1::Ambiguous
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1::Permanent
        }
    }
}
fn moderation_panel_notification_error(
    error: BrokerError,
) -> sorafs_node::moderation_orchestrator::ModerationPanelNotificationFailureV1 {
    match error {
        BrokerError::Unavailable | BrokerError::StaleOrRevoked => {
            sorafs_node::moderation_orchestrator::ModerationPanelNotificationFailureV1::NotDelivered
        }
        BrokerError::Ambiguous => {
            sorafs_node::moderation_orchestrator::ModerationPanelNotificationFailureV1::Ambiguous
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::moderation_orchestrator::ModerationPanelNotificationFailureV1::Permanent
        }
    }
}
#[derive(Clone)]
struct ModerationDeliveryBrokerProvider {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl ModerationDeliveryBrokerProvider {
    fn live_qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        BrokerError,
    > {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1::new(
                qualification.revision,
                qualification.policy_digest,
            ),
        )
    }
}
impl_broker_debug_fields!(ModerationDeliveryBrokerProvider as value {} => finish_non_exhaustive);
#[derive(Clone)]
struct ModerationHandoffBrokerBoundary {
    provider: ModerationDeliveryBrokerProvider,
}
impl_broker_debug_fields!(ModerationHandoffBrokerBoundary as value {} => finish_non_exhaustive);
impl sorafs_node::moderation_orchestrator::ModerationRuntimeProviderV1
    for ModerationHandoffBrokerBoundary
{
    fn handle(&self) -> &str {
        &self.provider.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderReadinessErrorV1,
    > {
        self.provider
            .live_qualification()
            .map_err(moderation_readiness_error)
    }
}
impl iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1
    for ModerationHandoffBrokerBoundary
{
    fn deliver_once(
        &self,
        request: &iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffRequestV1,
    ) -> Result<
        iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffOutcomeV1,
        iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1,
    > {
        let wire = moderation_handoff_request_to_wire(request, self.provider.binding.slot)
            .map_err(moderation_handoff_error)?;
        self.provider
            .live_qualification()
            .map_err(moderation_handoff_error)?;
        let payload = encode_canonical(&wire, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
            .map_err(moderation_handoff_error)?;
        let result = self
            .provider
            .session
            .call(
                &self.provider.binding,
                self.provider.metadata_digest,
                OPERATION_MODERATION_HANDOFF_DELIVER_ONCE_V1,
                payload,
                true,
            )
            .map_err(moderation_handoff_error)?;
        let outcome = decode_scrubbed_canonical::<ModerationDurableHandoffOutcomeWireV1>(
            &result,
            MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
        )
        .map_err(|_| {
            self.provider.session.poison();
            iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1::Ambiguous
        })?;
        let outcome = match outcome.outcome {
            1 => iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffOutcomeV1::Delivered,
            2 => iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffOutcomeV1::AlreadyDelivered,
            _ => {
                self.provider.session.poison();
                return Err(
                    iroha_torii::sorafs::moderation_runtime::
                        ModerationDurableHandoffFailureV1::Ambiguous,
                );
            }
        };
        if self.provider.live_qualification().is_err() {
            self.provider.session.poison();
            return Err(
                iroha_torii::sorafs::moderation_runtime::
                    ModerationDurableHandoffFailureV1::Ambiguous,
            );
        }
        Ok(outcome)
    }
    fn publish_archive_head_once(
        &self,
        request: &iroha_torii::sorafs::moderation_runtime::
            ModerationDurableArchiveHeadPublicationRequestV1,
    ) -> Result<
        iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffOutcomeV1,
        iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1,
    > {
        if self.provider.binding.slot
            != IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id()
        {
            return Err(iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffFailureV1::Permanent);
        }
        let wire = moderation_panel_notification_archive_head_publish_request_to_wire(
            request,
            &self.provider.session.network_id,
            &self.provider.session.requested_catalog,
        )
        .map_err(moderation_handoff_error)?;
        self.provider
            .live_qualification()
            .map_err(moderation_handoff_error)?;
        let payload = encode_canonical(&wire, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
            .map_err(moderation_handoff_error)?;
        let result = self
            .provider
            .session
            .call(
                &self.provider.binding,
                self.provider.metadata_digest,
                OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_PUBLISH_V1,
                payload,
                true,
            )
            .map_err(moderation_handoff_error)?;
        let outcome = decode_scrubbed_canonical::<
            ModerationPanelNotificationArchiveHeadPublishResultWireV1,
        >(&result, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
        .map_err(|_| {
            self.provider.session.poison();
            iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1::Ambiguous
        })?;
        if outcome.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
            || outcome.slot != IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id()
            || outcome.operation_id != request.head.operation_id
            || outcome.head_digest != request.head.head_digest
            || outcome.chain_commitment != request.head.chain_commitment
            || !matches!(outcome.outcome, 1 | 2)
        {
            self.provider.session.poison();
            return Err(iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffFailureV1::Ambiguous);
        }
        if self.provider.live_qualification().is_err() {
            self.provider.session.poison();
            return Err(iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffFailureV1::Ambiguous);
        }
        Ok(match outcome.outcome {
            1 => iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffOutcomeV1::Delivered,
            2 => iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffOutcomeV1::AlreadyDelivered,
            _ => unreachable!("validated publication outcome"),
        })
    }
    fn read_published_archive_head(
        &self,
    ) -> Result<
        Option<sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveHeadV1>,
        iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1,
    > {
        if self.provider.binding.slot
            != IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id()
        {
            return Err(iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffFailureV1::Permanent);
        }
        self.provider
            .live_qualification()
            .map_err(moderation_handoff_error)?;
        let payload = encode_canonical(&(), MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
            .map_err(moderation_handoff_error)?;
        let result = self
            .provider
            .session
            .call(
                &self.provider.binding,
                self.provider.metadata_digest,
                OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_READ_V1,
                payload,
                false,
            )
            .map_err(moderation_handoff_error)?;
        let readback = decode_scrubbed_canonical::<
            ModerationPanelNotificationArchiveHeadReadResultWireV1,
        >(&result, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
        .map_err(|_| {
            self.provider.session.poison();
            iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffFailureV1::Permanent
        })?;
        if readback.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
            || readback.slot != IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id()
        {
            self.provider.session.poison();
            return Err(iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffFailureV1::Permanent);
        }
        let head = readback
            .canonical_head
            .as_deref()
            .map(|canonical_head| {
                validate_moderation_panel_notification_archive_public_head_readback_at_broker_boundary(
                    canonical_head,
                    &self.provider.session.network_id,
                )
            })
            .transpose()
            .map_err(|_| {
                self.provider.session.poison();
                iroha_torii::sorafs::moderation_runtime::
                    ModerationDurableHandoffFailureV1::Permanent
            })?;
        if self.provider.live_qualification().is_err() {
            self.provider.session.poison();
            return Err(iroha_torii::sorafs::moderation_runtime::
                ModerationDurableHandoffFailureV1::Ambiguous);
        }
        Ok(head)
    }
}
#[derive(Clone)]
struct ModerationPanelNotificationBrokerBoundary {
    provider: ModerationDeliveryBrokerProvider,
}
impl_broker_debug_fields!(ModerationPanelNotificationBrokerBoundary as value {} => finish_non_exhaustive);
impl sorafs_node::moderation_orchestrator::ModerationRuntimeProviderV1
    for ModerationPanelNotificationBrokerBoundary
{
    fn handle(&self) -> &str {
        &self.provider.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderReadinessErrorV1,
    > {
        self.provider
            .live_qualification()
            .map_err(moderation_readiness_error)
    }
}
impl iroha_torii::sorafs::moderation_runtime::ModerationDurablePanelNotificationBoundaryV1
    for ModerationPanelNotificationBrokerBoundary
{
    fn deliver_once(
        &self,
        request: &iroha_torii::sorafs::moderation_runtime::
            ModerationDurablePanelNotificationRequestV1,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationPanelNotificationDeliveryReceiptV1,
        sorafs_node::moderation_orchestrator::ModerationPanelNotificationFailureV1,
    > {
        let wire = moderation_panel_notification_request_to_wire(request)
            .map_err(moderation_panel_notification_error)?;
        self.provider
            .live_qualification()
            .map_err(moderation_panel_notification_error)?;
        let payload = encode_canonical(&wire, MAX_MODERATION_PANEL_NOTIFICATION_FRAME_BYTES_V1)
            .map_err(moderation_panel_notification_error)?;
        let result = self
            .provider
            .session
            .call(
                &self.provider.binding,
                self.provider.metadata_digest,
                OPERATION_MODERATION_PANEL_NOTIFICATION_DELIVER_ONCE_V1,
                payload,
                true,
            )
            .map_err(moderation_panel_notification_error)?;
        let receipt = decode_scrubbed_canonical::<ModerationPanelNotificationReceiptWireV1>(
            &result,
            MAX_MODERATION_PANEL_NOTIFICATION_FRAME_BYTES_V1,
        )
        .map_err(|_| {
            self.provider.session.poison();
            sorafs_node::moderation_orchestrator::ModerationPanelNotificationFailureV1::Ambiguous
        })?;
        let receipt =
            validate_moderation_panel_notification_receipt(receipt, &wire).map_err(|_| {
                self.provider.session.poison();
                sorafs_node::moderation_orchestrator::
                    ModerationPanelNotificationFailureV1::Ambiguous
            })?;
        if self.provider.live_qualification().is_err() {
            self.provider.session.poison();
            return Err(
                sorafs_node::moderation_orchestrator::
                    ModerationPanelNotificationFailureV1::Ambiguous,
            );
        }
        Ok(receipt)
    }
}
#[derive(Clone)]
struct NativeTransactionBrokerCore {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    exact_binding: iroha_torii::SorafsNativeTransactionSignerBindingV1,
}
impl NativeTransactionBrokerCore {
    fn live_qualification(
        &self,
    ) -> Result<iroha_torii::SorafsNativeTransactionSignerQualificationV1, BrokerError> {
        let payload = encode_canonical(&(), MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = decode_scrubbed_canonical::<QualificationResultWireV1>(
            &result,
            MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1,
        )
        .inspect_err(|_| self.session.poison())?;
        let expected = self.exact_binding.qualification();
        let projected_binding = native_transaction_signer_binding_from_wire(&self.binding)
            .map_err(|_| {
                self.session.poison();
                BrokerError::BindingMismatch
            })?;
        if qualification.revision != expected.revision()
            || qualification.policy_digest != expected.policy_digest()
            || projected_binding != self.exact_binding
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
    fn sign_raw(
        &self,
        payload: &iroha_data_model::transaction::TransactionPayload,
    ) -> Result<iroha_data_model::transaction::SignedTransaction, BrokerError> {
        if payload.authority() != self.exact_binding.authority() {
            return Err(BrokerError::Rejected);
        }
        ensure_transaction_session_network(payload, &self.session.network_id)?;
        let payload = encode_native_transaction_payload(payload)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_NATIVE_TRANSACTION_SIGN_V1,
            payload,
            true,
        )?;
        decode_scrubbed_canonical::<iroha_data_model::transaction::SignedTransaction>(
            &result,
            MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1,
        )
        .map_err(|_| {
            self.session.poison();
            BrokerError::Ambiguous
        })
    }
}
fn native_transaction_probe_error(
    error: BrokerError,
) -> iroha_torii::SorafsNativeTransactionSignerProbeErrorV1 {
    if error == BrokerError::Rejected {
        iroha_torii::SorafsNativeTransactionSignerProbeErrorV1::Refused
    } else {
        iroha_torii::SorafsNativeTransactionSignerProbeErrorV1::Unavailable
    }
}
macro_rules! define_native_transaction_broker_signer {
    (
        $raw:ident,
        $proxy:ident,
        $trait_name:ident,
        $error:ident,
        $role:ident,
        $qualifier:ident
    ) => {
        #[derive(Clone)]
        struct $raw {
            core: NativeTransactionBrokerCore,
        }
        impl iroha_torii::SorafsNativeTransactionSignerProviderV1 for $raw {
            fn role(&self) -> iroha_torii::SorafsNativeTransactionSignerRoleV1 {
                iroha_torii::SorafsNativeTransactionSignerRoleV1::$role
            }
            fn handle(&self) -> &str {
                self.core.exact_binding.handle()
            }
            fn authority(&self) -> iroha_data_model::account::AccountId {
                self.core.exact_binding.authority().clone()
            }
            fn public_key(
                &self,
            ) -> Result<
                iroha_crypto::PublicKey,
                iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
            > {
                self.core
                    .live_qualification()
                    .map_err(native_transaction_probe_error)?;
                Ok(self.core.exact_binding.public_key().clone())
            }
            fn qualification(
                &self,
            ) -> Result<
                iroha_torii::SorafsNativeTransactionSignerQualificationV1,
                iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
            > {
                self.core
                    .live_qualification()
                    .map_err(native_transaction_probe_error)
            }
        }
        impl iroha_torii::$trait_name for $raw {
            fn sign(
                &self,
                payload: iroha_data_model::transaction::TransactionPayload,
            ) -> Result<
                iroha_data_model::transaction::SignedTransaction,
                iroha_torii::$error,
            > {
                self.core.sign_raw(&payload).map_err(|error| match error {
                    BrokerError::Unavailable => iroha_torii::$error::Unavailable,
                    BrokerError::Rejected => iroha_torii::$error::Refused,
                    BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                        self.core.session.poison();
                        iroha_torii::$error::QualificationChanged
                    }
                    BrokerError::Protocol
                    | BrokerError::Conflict
                    | BrokerError::Ambiguous => {
                        self.core.session.poison();
                        iroha_torii::$error::SubstitutedTransaction
                    }
                })
            }
        }
        #[derive(Clone)]
        struct $proxy {
            core: NativeTransactionBrokerCore,
        }
        impl iroha_torii::SorafsNativeTransactionSignerProviderV1 for $proxy {
            fn role(&self) -> iroha_torii::SorafsNativeTransactionSignerRoleV1 {
                iroha_torii::SorafsNativeTransactionSignerRoleV1::$role
            }
            fn handle(&self) -> &str {
                self.core.exact_binding.handle()
            }
            fn authority(&self) -> iroha_data_model::account::AccountId {
                self.core.exact_binding.authority().clone()
            }
            fn public_key(
                &self,
            ) -> Result<
                iroha_crypto::PublicKey,
                iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
            > {
                self.core
                    .live_qualification()
                    .map_err(native_transaction_probe_error)?;
                Ok(self.core.exact_binding.public_key().clone())
            }
            fn qualification(
                &self,
            ) -> Result<
                iroha_torii::SorafsNativeTransactionSignerQualificationV1,
                iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
            > {
                self.core
                    .live_qualification()
                    .map_err(native_transaction_probe_error)
            }
        }
        impl iroha_torii::$trait_name for $proxy {
            fn sign(
                &self,
                payload: iroha_data_model::transaction::TransactionPayload,
            ) -> Result<
                iroha_data_model::transaction::SignedTransaction,
                iroha_torii::$error,
            > {
                if payload.authority() != self.core.exact_binding.authority() {
                    return Err(iroha_torii::$error::InputAuthorityMismatch);
                }
                let raw = Arc::new($raw {
                    core: self.core.clone(),
                });
                let qualified = iroha_torii::$qualifier(
                    self.core.exact_binding.clone(),
                    raw,
                )
                .map_err(|error| {
                    if error
                        == iroha_torii::SorafsNativeTransactionSignerQualificationErrorV1::ProviderUnavailable
                    {
                        iroha_torii::$error::Unavailable
                    } else {
                        self.core.session.poison();
                        iroha_torii::$error::QualificationChanged
                    }
                })?;
                let result = qualified.sign(payload);
                if matches!(
                    &result,
                    Err(
                        iroha_torii::$error::SubstitutedTransaction
                            | iroha_torii::$error::QualificationChanged
                    )
                ) {
                    self.core.session.poison();
                }
                result
            }
        }
    };
}
define_native_transaction_broker_signer!(
    RawProofOutcomeBrokerSigner,
    ProofOutcomeBrokerSigner,
    SoraFsProofOutcomeTransactionSigner,
    SoraFsProofOutcomeSigningError,
    ProofOutcome,
    qualify_sorafs_proof_outcome_transaction_signer_v1
);
define_native_transaction_broker_signer!(
    RawRepairBrokerSigner,
    RepairBrokerSigner,
    SoraFsRepairTransactionSigner,
    SoraFsRepairTransactionSigningError,
    Repair,
    qualify_sorafs_repair_transaction_signer_v1
);
define_native_transaction_broker_signer!(
    RawReserveBrokerSigner,
    ReserveBrokerSigner,
    SoraFsReserveTransactionSigner,
    SoraFsReserveTransactionSigningError,
    Reserve,
    qualify_sorafs_reserve_transaction_signer_v1
);
define_native_transaction_broker_signer!(
    RawOrderbookBrokerSigner,
    OrderbookBrokerSigner,
    SoraFsOrderbookTransactionSigner,
    SoraFsOrderbookTransactionSigningError,
    Orderbook,
    qualify_sorafs_orderbook_transaction_signer_v1
);
#[derive(Clone)]
struct SoracloudRuntimeBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    exact_binding: crate::soracloud_runtime_signer::SoracloudRuntimeSignerBindingV1,
}
impl SoracloudRuntimeBrokerSigner {
    fn live_qualification(
        &self,
    ) -> Result<crate::soracloud_runtime_signer::SoracloudRuntimeSignerQualificationV1, BrokerError>
    {
        let payload = encode_canonical(&(), MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = decode_scrubbed_canonical::<SoracloudSignerQualificationWireV1>(
            &result,
            MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1,
        )
        .inspect_err(|_| self.session.poison())?;
        let expected = self.exact_binding.qualification();
        let projected =
            soracloud_runtime_signer_binding_from_wire(&self.binding).inspect_err(|_| {
                self.session.poison();
            })?;
        if qualification.revision != expected.revision()
            || qualification.policy_digest != expected.policy_digest()
            || !qualification.active
            || qualification.test_only
            || projected != self.exact_binding
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
    fn signing_error(
        &self,
        error: BrokerError,
    ) -> crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1 {
        use crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1 as Error;
        match error {
            BrokerError::Unavailable => Error::Unavailable,
            BrokerError::Rejected => Error::Refused,
            BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                self.session.poison();
                Error::QualificationChanged
            }
            BrokerError::Protocol | BrokerError::Conflict | BrokerError::Ambiguous => {
                self.session.poison();
                Error::SubstitutedTransaction
            }
        }
    }
}
impl crate::soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1
    for SoracloudRuntimeBrokerSigner
{
    fn handle(&self) -> &str {
        self.exact_binding.handle()
    }
    fn authority(&self) -> iroha_data_model::account::AccountId {
        self.exact_binding.authority().clone()
    }
    fn public_key(
        &self,
    ) -> Result<
        iroha_crypto::PublicKey,
        crate::soracloud_runtime_signer::SoracloudRuntimeSignerProbeErrorV1,
    > {
        self.live_qualification().map_err(|error| {
            if error == BrokerError::Rejected {
                crate::soracloud_runtime_signer::SoracloudRuntimeSignerProbeErrorV1::Refused
            } else {
                crate::soracloud_runtime_signer::SoracloudRuntimeSignerProbeErrorV1::Unavailable
            }
        })?;
        Ok(self.exact_binding.public_key().clone())
    }
    fn qualification(
        &self,
    ) -> Result<
        crate::soracloud_runtime_signer::SoracloudRuntimeSignerQualificationV1,
        crate::soracloud_runtime_signer::SoracloudRuntimeSignerProbeErrorV1,
    > {
        self.live_qualification().map_err(|error| {
            if error == BrokerError::Rejected {
                crate::soracloud_runtime_signer::SoracloudRuntimeSignerProbeErrorV1::Refused
            } else {
                crate::soracloud_runtime_signer::SoracloudRuntimeSignerProbeErrorV1::Unavailable
            }
        })
    }
    fn sign_transaction(
        &self,
        payload: iroha_data_model::transaction::TransactionPayload,
    ) -> Result<
        iroha_data_model::transaction::SignedTransaction,
        crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1,
    > {
        if payload.authority() != self.exact_binding.authority() {
            return Err(
                crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1::
                    InputAuthorityMismatch,
            );
        }
        ensure_transaction_session_network(&payload, &self.session.network_id)
            .map_err(|error| self.signing_error(error))?;
        let payload = encode_native_transaction_payload(&payload)
            .map_err(|error| self.signing_error(error))?;
        let result = provider_call!(
            self,
            call,
            OPERATION_NATIVE_TRANSACTION_SIGN_V1,
            payload,
            true,
        )
        .map_err(|error| self.signing_error(error))?;
        decode_scrubbed_canonical::<iroha_data_model::transaction::SignedTransaction>(
            &result,
            MAX_NATIVE_SIGNED_TRANSACTION_BYTES_V1,
        )
        .map_err(|error| self.signing_error(error))
    }
    fn sign_provenance(
        &self,
        purpose: iroha_data_model::soracloud::SoracloudRuntimeProvenancePurposeV1,
        preimage: &[u8],
    ) -> Result<
        iroha_crypto::Signature,
        crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1,
    > {
        if preimage.is_empty() || preimage.len() > MAX_NATIVE_TRANSACTION_PAYLOAD_BYTES_V1 {
            return Err(crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1::Refused);
        }
        iroha_data_model::soracloud::validate_soracloud_runtime_provenance_preimage_v1(
            purpose, preimage,
        )
        .map_err(|_| {
            crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1::
                    InvalidProvenancePreimage
        })?;
        let payload = encode_canonical(
            &SoracloudProvenanceSignRequestWireV1 {
                purpose: purpose.wire_id(),
                preimage: preimage.to_vec(),
            },
            MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1,
        )
        .map_err(|error| self.signing_error(error))?;
        let result = provider_call!(
            self,
            call,
            OPERATION_SORACLOUD_PROVENANCE_SIGN_V1,
            payload,
            true,
        )
        .map_err(|error| self.signing_error(error))?;
        decode_scrubbed_canonical::<iroha_crypto::Signature>(
            &result,
            MAX_NATIVE_TRANSACTION_FRAME_BYTES_V1,
        )
        .map_err(|_| {
            self.session.poison();
            crate::soracloud_runtime_signer::SoracloudRuntimeSigningErrorV1::
                InvalidProvenanceSignature
        })
    }
}
#[derive(Clone)]
struct SoracloudHfCredentialBrokerProvider {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    exact_binding: crate::soracloud_hf_credential::SoracloudHfCredentialProviderBindingV1,
}
impl_broker_debug_fields!(SoracloudHfCredentialBrokerProvider as value {
    "handle" => value.exact_binding.handle(),
} => finish_non_exhaustive);
impl SoracloudHfCredentialBrokerProvider {
    fn live_qualification(
        &self,
    ) -> Result<
        crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationV1,
        BrokerError,
    > {
        let payload = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = decode_scrubbed_canonical::<QualificationResultWireV1>(
            &result,
            MAX_OPERATION_FRAME_BYTES_V1,
        )
        .inspect_err(|_| self.session.poison())?;
        let expected = self.exact_binding.qualification();
        let projected =
            soracloud_hf_credential_binding_from_wire(&self.binding).inspect_err(|_| {
                self.session.poison();
            })?;
        if qualification.revision != expected.revision()
            || qualification.policy_digest != expected.policy_digest()
            || projected != self.exact_binding
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
    fn probe_error(
        &self,
        error: BrokerError,
    ) -> crate::soracloud_hf_credential::SoracloudHfCredentialProviderProbeErrorV1 {
        use crate::soracloud_hf_credential::SoracloudHfCredentialProviderProbeErrorV1 as Error;
        match error {
            BrokerError::Unavailable => Error::Unavailable,
            BrokerError::Rejected => Error::Refused,
            BrokerError::BindingMismatch
            | BrokerError::StaleOrRevoked
            | BrokerError::Protocol
            | BrokerError::Conflict
            | BrokerError::Ambiguous => {
                self.session.poison();
                Error::Refused
            }
        }
    }
    fn operation_error(
        &self,
        error: BrokerError,
    ) -> crate::soracloud_hf_credential::SoracloudHfCredentialProviderOperationErrorV1 {
        use crate::soracloud_hf_credential::SoracloudHfCredentialProviderOperationErrorV1 as Error;
        match error {
            BrokerError::Unavailable => Error::Unavailable,
            BrokerError::Rejected => Error::Refused,
            BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                self.session.poison();
                Error::QualificationChanged
            }
            BrokerError::Protocol | BrokerError::Conflict | BrokerError::Ambiguous => {
                self.session.poison();
                Error::InvalidResponse
            }
        }
    }
}
impl crate::soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1
    for SoracloudHfCredentialBrokerProvider
{
    fn handle(&self) -> &str {
        self.exact_binding.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        crate::soracloud_hf_credential::SoracloudHfCredentialProviderQualificationV1,
        crate::soracloud_hf_credential::SoracloudHfCredentialProviderProbeErrorV1,
    > {
        self.live_qualification()
            .map_err(|error| self.probe_error(error))
    }
    fn check_readiness(
        &self,
    ) -> Result<(), crate::soracloud_hf_credential::SoracloudHfCredentialProviderProbeErrorV1> {
        self.live_qualification()
            .map(|_| ())
            .map_err(|error| self.probe_error(error))
    }
    fn execute_authenticated(
        &self,
        request: &crate::soracloud_hf_credential::SoracloudHfAuthenticatedInferenceRequestV1,
    ) -> Result<
        crate::soracloud_hf_credential::SoracloudHfAuthenticatedInferenceResponseV1,
        crate::soracloud_hf_credential::SoracloudHfCredentialProviderOperationErrorV1,
    > {
        self.live_qualification()
            .map_err(|error| self.operation_error(error))?;
        let wire = SoracloudHfAuthenticatedInferenceRequestWireV1 {
            repo_id: request.repo_id().to_owned(),
            resolved_revision: request.resolved_revision().to_owned(),
            url: request.url().to_owned(),
            content_type: request.content_type().to_owned(),
            accept: request.accept().map(ToOwned::to_owned),
            body: request.body().to_vec(),
            maximum_response_bytes: request.maximum_response_bytes(),
        };
        let payload = encode_canonical(&wire, MAX_SORACLOUD_HF_INFERENCE_FRAME_BYTES_V1)
            .map_err(|error| self.operation_error(error))?;
        let result = provider_call!(
            self,
            call,
            OPERATION_SORACLOUD_HF_AUTHENTICATED_INFERENCE_V1,
            payload,
            false,
        )
        .map_err(|error| self.operation_error(error))?;
        let mut response = decode_scrubbed_canonical::<
            SoracloudHfAuthenticatedInferenceResponseWireV1,
        >(&result, MAX_SORACLOUD_HF_INFERENCE_FRAME_BYTES_V1)
        .map_err(|error| self.operation_error(error))?;
        let response =
            crate::soracloud_hf_credential::SoracloudHfAuthenticatedInferenceResponseV1::try_new(
                std::mem::take(&mut response.served_repo_id),
                std::mem::take(&mut response.served_revision),
                response.status,
                response.content_type.take(),
                response.content_encoding.take(),
                std::mem::take(&mut response.body),
                request.maximum_response_bytes(),
            )
            .map_err(|_| {
                self.session.poison();
                crate::soracloud_hf_credential::
                    SoracloudHfCredentialProviderOperationErrorV1::InvalidResponse
            })?;
        if response.served_repo_id() != request.repo_id()
            || response.served_revision() != request.resolved_revision()
        {
            self.session.poison();
            return Err(crate::soracloud_hf_credential::
                SoracloudHfCredentialProviderOperationErrorV1::InvalidResponse);
        }
        self.live_qualification()
            .map_err(|error| self.operation_error(error))?;
        Ok(response)
    }
}
#[derive(Clone)]
struct GovernanceDagBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    publisher_peer_id: Vec<u8>,
    public_key: [u8; 32],
}
impl_broker_debug_fields!(GovernanceDagBrokerSigner as value {} => finish_non_exhaustive);
impl GovernanceDagBrokerSigner {
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, BrokerError> {
        let payload = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = self
            .session
            .decode_result::<QualificationResultWireV1>(&result)?;
        let expected = qualification_from_binding(&self.binding)?;
        if qualification.revision != expected.revision
            || qualification.policy_digest != expected.policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
}
impl sorafs_node::GovernanceDagRuntimeSigner for GovernanceDagBrokerSigner {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
    fn publisher_peer_id(&self) -> &[u8] {
        &self.publisher_peer_id
    }
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn sign(
        &self,
        purpose: sorafs_node::GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        let request = PurposeSignRequestWireV1 {
            purpose: purpose.wire_id(),
            payload: payload.to_vec(),
        };
        validate_governance_purpose_signing_request(&request, &self.binding)
            .map_err(|_| ERROR_REJECTED.to_owned())?;
        let payload = encode_canonical(&request, MAX_OPERATION_FRAME_BYTES_V1)
            .map_err(redacted_provider_error)?;
        let result = provider_call!(self, call, OPERATION_SIGN_V1, payload, false,)
            .map_err(redacted_provider_error)?;
        let signature = self
            .session
            .decode_result::<SignResultWireV1>(&result)
            .map_err(redacted_provider_error)?
            .signature;
        if signature == [0; 64] {
            self.session.poison();
            return Err(ERROR_REJECTED.to_owned());
        }
        Ok(signature)
    }
}
#[derive(Clone)]
struct GovernanceDagBrokerRequestAuthenticator {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    ingress_binding: sorafs_node::GovernanceDagRequestIngressBindingV1,
    ingress_qualification: sorafs_node::GovernanceDagRequestIngressQualificationV1,
}
impl_broker_debug_fields!(GovernanceDagBrokerRequestAuthenticator as value {
    "slot" => value.binding.slot,
    "endpoint_binding" => hex::encode(value.ingress_binding.endpoint_binding()),
} => finish_non_exhaustive);
impl GovernanceDagBrokerRequestAuthenticator {
    fn live_ingress_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRequestIngressQualificationV1, BrokerError> {
        let payload = encode_canonical(&(), MAX_GOVERNANCE_REQUEST_AUTH_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = governance_request_ingress_qualification_from_wire(
            self.session
                .decode_result::<GovernanceRequestIngressQualificationWireV1>(&result)?,
        )?;
        if qualification != self.ingress_qualification
            || qualification.binding() != self.ingress_binding
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(qualification)
    }
    fn expected_scope(&self) -> Result<sorafs_node::GovernanceDagAuthenticationScope, BrokerError> {
        if self.binding.slot == IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator.wire_id()
        {
            Ok(sorafs_node::GovernanceDagAuthenticationScope::Ipfs)
        } else if self.binding.slot
            == IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator.wire_id()
        {
            Ok(sorafs_node::GovernanceDagAuthenticationScope::SignedHead)
        } else {
            Err(BrokerError::BindingMismatch)
        }
    }
}
impl sorafs_node::GovernanceDagRequestAuthenticator for GovernanceDagBrokerRequestAuthenticator {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn ingress_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRequestIngressQualificationV1, String> {
        self.live_ingress_qualification()
            .map_err(redacted_provider_error)
    }
    fn authenticate(
        &self,
        request: &sorafs_node::GovernanceDagCanonicalRequestV1,
    ) -> Result<sorafs_node::GovernanceDagRequestAuthenticationEnvelopeV1, String> {
        if self.expected_scope().map_err(redacted_provider_error)? != request.scope()
            || self.ingress_binding.scope() != request.scope()
        {
            return Err(ERROR_REJECTED.to_owned());
        }
        if request.body_length() > self.ingress_binding.max_body_bytes() {
            return Err(ERROR_REJECTED.to_owned());
        }
        let qualification = self
            .live_ingress_qualification()
            .map_err(redacted_provider_error)?;
        let payload = encode_canonical(
            &governance_request_auth_to_wire(request),
            MAX_GOVERNANCE_REQUEST_AUTH_FRAME_BYTES_V1,
        )
        .map_err(redacted_provider_error)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_GOVERNANCE_REQUEST_AUTHENTICATE_V1,
            payload,
            false,
        )
        .map_err(redacted_provider_error)?;
        let result = self
            .session
            .decode_result::<GovernanceRequestAuthResultWireV1>(&result)
            .map_err(redacted_provider_error)?;
        let envelope = validate_governance_request_auth_envelope(
            request,
            result,
            self.ingress_binding.public_key(),
        )
        .map_err(|error| {
            self.session.poison();
            redacted_provider_error(error)
        })?;
        let requalification = self
            .live_ingress_qualification()
            .map_err(redacted_provider_error)?;
        if requalification != qualification {
            self.session.poison();
            return Err(ERROR_UNAVAILABLE.to_owned());
        }
        Ok(envelope)
    }
}
#[derive(Clone)]
struct GovernanceDagBrokerCheckpointStore {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(GovernanceDagBrokerCheckpointStore as value {} => finish_non_exhaustive);
impl GovernanceDagBrokerCheckpointStore {
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, BrokerError> {
        let payload = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = self
            .session
            .decode_result::<QualificationResultWireV1>(&result)?;
        let expected = qualification_from_binding(&self.binding)?;
        if qualification.revision != expected.revision
            || qualification.policy_digest != expected.policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
}
impl sorafs_node::GovernanceDagSealedCheckpointStore for GovernanceDagBrokerCheckpointStore {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
        self.live_qualification().map_err(redacted_provider_error)
    }
    fn load(
        &self,
        slot: sorafs_node::GovernanceDagSealedStateSlot,
    ) -> Result<Option<sorafs_node::GovernanceDagSealedStateRecord>, String> {
        let payload = encode_canonical(
            &SealedLoadRequestWireV1 {
                slot: sealed_slot_to_wire(slot),
            },
            MAX_OPERATION_FRAME_BYTES_V1,
        )
        .map_err(redacted_provider_error)?;
        let result = provider_call!(self, call, OPERATION_SEALED_LOAD_V1, payload, false,)
            .map_err(redacted_provider_error)?;
        let record = self
            .session
            .decode_result::<Option<SealedRecordWireV1>>(&result)
            .map_err(redacted_provider_error)?;
        record
            .map(|record| {
                validate_sealed_record_fields(
                    slot,
                    record.generation,
                    record.revision,
                    &record.payload,
                )
                .map_err(|_| {
                    self.session.poison();
                    ERROR_REJECTED.to_owned()
                })?;
                let record = sorafs_node::GovernanceDagSealedStateRecord {
                    generation: record.generation,
                    revision: record.revision,
                    payload: record.payload,
                };
                Ok(record)
            })
            .transpose()
    }
    fn compare_and_swap(
        &self,
        slot: sorafs_node::GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: sorafs_node::GovernanceDagSealedStateRecord,
    ) -> Result<(), String> {
        if expected_revision == Some([0; 32])
            || validate_sealed_record_fields(slot, next.generation, next.revision, &next.payload)
                .is_err()
        {
            return Err(ERROR_REJECTED.to_owned());
        }
        let payload = encode_canonical(
            &SealedCompareAndSwapRequestWireV1 {
                slot: sealed_slot_to_wire(slot),
                expected_revision,
                next: SealedRecordWireV1 {
                    generation: next.generation,
                    revision: next.revision,
                    payload: next.payload,
                },
            },
            MAX_OPERATION_FRAME_BYTES_V1,
        )
        .map_err(redacted_provider_error)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_SEALED_COMPARE_AND_SWAP_V1,
            payload,
            true,
        )
        .map_err(redacted_provider_error)?;
        self.session
            .decode_result::<()>(&result)
            .map_err(|_| ERROR_AMBIGUOUS.to_owned())
    }
    fn delete(
        &self,
        slot: sorafs_node::GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), String> {
        if validate_sealed_delete(slot, expected_revision).is_err() {
            return Err(ERROR_REJECTED.to_owned());
        }
        let payload = encode_canonical(
            &SealedDeleteRequestWireV1 {
                slot: sealed_slot_to_wire(slot),
                expected_revision,
            },
            MAX_OPERATION_FRAME_BYTES_V1,
        )
        .map_err(redacted_provider_error)?;
        let result = provider_call!(self, call, OPERATION_SEALED_DELETE_V1, payload, true,)
            .map_err(redacted_provider_error)?;
        self.session
            .decode_result::<()>(&result)
            .map_err(|_| ERROR_AMBIGUOUS.to_owned())
    }
}
fn evidence_viewer_external_error(
    error: BrokerError,
) -> sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected
        }
    }
}
fn evidence_viewer_readiness_error(
    error: BrokerError,
) -> sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected
        }
    }
}
fn evidence_viewer_checkpoint_error(
    error: BrokerError,
) -> sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1 {
    match error {
        BrokerError::Ambiguous => {
            sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous
        }
        BrokerError::Unavailable => {
            sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1::Rejected
        }
    }
}
fn evidence_viewer_transparency_publisher_error(
    error: BrokerError,
) -> sorafs_node::evidence_viewer::transparency_producer::
    EvidenceViewerTransparencyPublisherExternalErrorV1
{
    match error {
        BrokerError::Ambiguous => {
            sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous
        }
        BrokerError::Unavailable => {
            sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherExternalErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherExternalErrorV1::Rejected
        }
    }
}
#[derive(Clone)]
struct EvidenceViewerBrokerProvider {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(EvidenceViewerBrokerProvider as value {
    "slot" => value.binding.slot,
} => finish_non_exhaustive);
impl EvidenceViewerBrokerProvider {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn live_qualification(
        &self,
    ) -> Result<
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
        BrokerError,
    > {
        let payload = encode_canonical(&(), MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = decode_scrubbed_canonical::<QualificationResultWireV1>(
            &result,
            MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
        )
        .inspect_err(|_| self.session.poison())?;
        let expected_revision = self.binding.revision.ok_or(BrokerError::BindingMismatch)?;
        let expected_policy_digest = self
            .binding
            .policy_digest
            .ok_or(BrokerError::BindingMismatch)?;
        if qualification.revision != expected_revision
            || qualification.policy_digest != expected_policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(
            sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
                expected_revision,
                expected_policy_digest,
            ),
        )
    }
    fn call_sensitive<T: NoritoSerialize>(
        &self,
        operation: u16,
        payload: &T,
        payload_limit: usize,
        mutating: bool,
    ) -> Result<ScrubbedBytes, BrokerError> {
        let payload = encode_sensitive_canonical(payload, payload_limit)?;
        provider_call!(self, call_sensitive, operation, payload, mutating,)
    }
    fn decode_sensitive<T>(&self, result: &ScrubbedBytes, limit: usize) -> Result<T, BrokerError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        let _scope = result.enter_decode_admission();
        decode_canonical::<T>(result, limit).inspect_err(|_| self.session.poison())
    }
}
macro_rules! impl_evidence_viewer_runtime_provider {
    ($provider:ty) => {
        impl sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderV1 for $provider {
            fn handle(&self) -> &str {
                self.provider.handle()
            }
            fn qualification(
                &self,
            ) -> Result<
                sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
                sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1,
            > {
                self.provider
                    .live_qualification()
                    .map_err(evidence_viewer_readiness_error)
            }
        }
    };
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerWebAuthn {
    provider: EvidenceViewerBrokerProvider,
}
impl_evidence_viewer_runtime_provider!(EvidenceViewerBrokerWebAuthn);
impl sorafs_node::evidence_viewer::EvidenceViewerWebAuthnBoundaryV1
    for EvidenceViewerBrokerWebAuthn
{
    fn issue_challenge(
        &self,
        binding_digest: [u8; 32],
        expires_at_unix_ms: u64,
    ) -> Result<
        sorafs_node::evidence_viewer::OpaqueEvidenceViewerSecretV1,
        sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1,
    > {
        let challenge_ttl_ms = self
            .provider
            .binding
            .evidence_viewer_webauthn_binding
            .as_ref()
            .ok_or(sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected)?
            .challenge_ttl_ms;
        let issued_at_unix_ms = expires_at_unix_ms
            .checked_sub(challenge_ttl_ms)
            .ok_or(sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected)?;
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_ISSUE_CHALLENGE_V1,
                &EvidenceViewerIssueChallengeRequestWireV1 {
                    binding_digest,
                    issued_at_unix_ms,
                    expires_at_unix_ms,
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_external_error)?;
        let mut secret = self
            .provider
            .decode_sensitive::<EvidenceViewerSecretResultWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .map_err(evidence_viewer_external_error)?;
        let secret = String::from_utf8(std::mem::take(&mut secret.secret)).map_err(|_| {
            self.provider.session.poison();
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected
        })?;
        sorafs_node::evidence_viewer::OpaqueEvidenceViewerSecretV1::new(secret)
    }
    fn verify_and_consume(
        &self,
        challenge: &str,
        assertion: &[u8],
        binding_digest: [u8; 32],
        rp_id: &str,
        allowed_origins: &[String],
        now_unix_ms: u64,
    ) -> Result<
        sorafs_node::evidence_viewer::EvidenceViewerWebAuthnResultV1,
        sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1,
    > {
        let configured = self
            .provider
            .binding
            .evidence_viewer_webauthn_binding
            .as_ref()
            .ok_or(sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected)?;
        if rp_id != configured.rp_id || allowed_origins != configured.allowed_origins.as_slice() {
            return Err(sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected);
        }
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_VERIFY_AND_CONSUME_V1,
                &EvidenceViewerVerifyAndConsumeRequestWireV1 {
                    challenge: challenge.as_bytes().to_vec(),
                    assertion: assertion.to_vec(),
                    binding_digest,
                    rp_id: rp_id.to_owned(),
                    allowed_origins: allowed_origins.to_vec(),
                    now_unix_ms,
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_external_error)?;
        let verified = self
            .provider
            .decode_sensitive::<EvidenceViewerWebAuthnResultWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .map_err(evidence_viewer_external_error)?;
        Ok(
            sorafs_node::evidence_viewer::EvidenceViewerWebAuthnResultV1 {
                attestation_digest: verified.attestation_digest,
                credential_id_digest: verified.credential_id_digest,
                authenticator_counter: verified.authenticator_counter,
            },
        )
    }
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerGrants {
    provider: EvidenceViewerBrokerProvider,
}
impl_evidence_viewer_runtime_provider!(EvidenceViewerBrokerGrants);
impl sorafs_node::evidence_viewer::EvidenceViewerGrantBoundaryV1 for EvidenceViewerBrokerGrants {
    fn issue(
        &self,
        claims: &sorafs_node::evidence_viewer::EvidenceViewerGrantClaimsV1,
    ) -> Result<
        sorafs_node::evidence_viewer::OpaqueEvidenceViewerSecretV1,
        sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1,
    > {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_GRANT_ISSUE_V1,
                &EvidenceViewerGrantIssueRequestWireV1 {
                    claims: claims.clone(),
                },
                MAX_EVIDENCE_VIEWER_CLAIMS_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_external_error)?;
        let mut secret = self
            .provider
            .decode_sensitive::<EvidenceViewerSecretResultWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .map_err(evidence_viewer_external_error)?;
        let secret = String::from_utf8(std::mem::take(&mut secret.secret)).map_err(|_| {
            self.provider.session.poison();
            sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1::Rejected
        })?;
        sorafs_node::evidence_viewer::OpaqueEvidenceViewerSecretV1::new(secret)
    }
    fn verify(
        &self,
        token: &str,
        claims: &sorafs_node::evidence_viewer::EvidenceViewerGrantClaimsV1,
        now_unix_ms: u64,
    ) -> Result<(), sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_GRANT_VERIFY_V1,
                &EvidenceViewerGrantVerifyRequestWireV1 {
                    token: token.as_bytes().to_vec(),
                    claims: claims.clone(),
                    now_unix_ms,
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
            .map_err(evidence_viewer_external_error)?;
        self.provider
            .decode_sensitive::<()>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(evidence_viewer_external_error)
    }
    fn revoke(
        &self,
        token_digest: [u8; 32],
    ) -> Result<(), sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_GRANT_REVOKE_V1,
                &EvidenceViewerGrantRevokeRequestWireV1 { token_digest },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_external_error)?;
        self.provider
            .decode_sensitive::<()>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(evidence_viewer_external_error)
    }
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerReceiptSigner {
    provider: EvidenceViewerBrokerProvider,
    public_key: [u8; 32],
}
impl_evidence_viewer_runtime_provider!(EvidenceViewerBrokerReceiptSigner);
impl sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1
    for EvidenceViewerBrokerReceiptSigner
{
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn sign(
        &self,
        purpose: sorafs_node::evidence_viewer::EvidenceViewerSigningPurposeV1,
        message: &[u8],
    ) -> Result<[u8; 64], sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
        let sign = PurposeSignRequestWireV1 {
            purpose: purpose.wire_id(),
            payload: message.to_vec(),
        };
        validate_evidence_purpose_signing_request(&sign, &self.provider.binding)
            .map_err(evidence_viewer_external_error)?;
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_RECEIPT_SIGN_V1,
                &sign,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
            .map_err(evidence_viewer_external_error)?;
        let signed = self
            .provider
            .decode_sensitive::<SignResultWireV1>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(evidence_viewer_external_error)?;
        verify_evidence_viewer_ed25519_signature(self.public_key, signed.signature, message)
            .map_err(evidence_viewer_external_error)?;
        Ok(signed.signature)
    }
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerErasure {
    provider: EvidenceViewerBrokerProvider,
}
impl_evidence_viewer_runtime_provider!(EvidenceViewerBrokerErasure);
impl sorafs_node::evidence_viewer::EvidenceViewerErasureBoundaryV1 for EvidenceViewerBrokerErasure {
    fn erase(
        &self,
        operation_id: [u8; 32],
        quarantine_id: [u8; 16],
        object_id: [u8; 16],
        evidence_digest: [u8; 32],
    ) -> Result<[u8; 32], sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_ERASE_V1,
                &EvidenceViewerEraseRequestWireV1 {
                    operation_id,
                    quarantine_id,
                    object_id,
                    evidence_digest,
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_external_error)?;
        let erased = self
            .provider
            .decode_sensitive::<EvidenceViewerEraseResultWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .map_err(evidence_viewer_external_error)?;
        Ok(erased.commit_digest)
    }
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerCheckpointStore {
    provider: EvidenceViewerBrokerProvider,
}
impl_evidence_viewer_runtime_provider!(EvidenceViewerBrokerCheckpointStore);
impl sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1
    for EvidenceViewerBrokerCheckpointStore
{
    fn load_latest(
        &self,
    ) -> Result<
        Option<sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1>,
        sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1,
    > {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_CHECKPOINT_LOAD_V1,
                &(),
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
            .map_err(evidence_viewer_checkpoint_error)?;
        let record = self
            .provider
            .decode_sensitive::<Option<Vec<u8>>>(&result, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
            .map_err(evidence_viewer_checkpoint_error)?;
        record
            .map(|record| {
                decode_evidence_viewer_checkpoint_record(&record, &self.provider.binding)
                    .map_err(evidence_viewer_checkpoint_error)
            })
            .transpose()
    }
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1,
    ) -> Result<(), sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreExternalErrorV1>
    {
        let next_record = encode_canonical(
            next,
            evidence_viewer_checkpoint_record_limit(&self.provider.binding)
                .map_err(evidence_viewer_checkpoint_error)?,
        )
        .map_err(evidence_viewer_checkpoint_error)?;
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_CHECKPOINT_COMPARE_AND_SWAP_V1,
                &EvidenceViewerCheckpointCompareAndSwapRequestWireV1 {
                    expected_revision,
                    next_record,
                },
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_checkpoint_error)?;
        self.provider
            .decode_sensitive::<()>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(evidence_viewer_checkpoint_error)
    }
}
fn moderation_checkpoint_error(
    error: BrokerError,
) -> sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1 {
    match error {
        BrokerError::Ambiguous => sorafs_node::moderation_orchestrator::
            ModerationCheckpointStoreExternalErrorV1::Ambiguous,
        BrokerError::Unavailable => sorafs_node::moderation_orchestrator::
            ModerationCheckpointStoreExternalErrorV1::Unavailable,
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => sorafs_node::moderation_orchestrator::
            ModerationCheckpointStoreExternalErrorV1::Rejected,
    }
}
fn moderation_panel_notification_archive_error(
    error: BrokerError,
) -> sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveExternalErrorV1 {
    match error {
        BrokerError::Ambiguous => sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationArchiveExternalErrorV1::Ambiguous,
        BrokerError::Unavailable => sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationArchiveExternalErrorV1::Unavailable,
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationArchiveExternalErrorV1::Rejected,
    }
}
#[derive(Clone, Debug)]
struct ModerationCheckpointBrokerStore {
    provider: EvidenceViewerBrokerProvider,
}
impl sorafs_node::moderation_orchestrator::ModerationRuntimeProviderV1
    for ModerationCheckpointBrokerStore
{
    fn handle(&self) -> &str {
        self.provider.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderReadinessErrorV1,
    > {
        self.provider
            .live_qualification()
            .map(|qualification| {
                sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1::new(
                    qualification.revision(),
                    qualification.policy_digest(),
                )
            })
            .map_err(|error| {
                match error {
                BrokerError::Unavailable | BrokerError::Ambiguous => {
                    sorafs_node::moderation_orchestrator::
                        ModerationRuntimeProviderReadinessErrorV1::Unavailable
                }
                BrokerError::Rejected
                | BrokerError::Conflict
                | BrokerError::StaleOrRevoked
                | BrokerError::Protocol
                | BrokerError::BindingMismatch => {
                    sorafs_node::moderation_orchestrator::
                        ModerationRuntimeProviderReadinessErrorV1::Rejected
                }
            }
            })
    }
}
impl sorafs_node::moderation_orchestrator::ModerationCheckpointStoreV1
    for ModerationCheckpointBrokerStore
{
    fn attestation_public_key(&self) -> [u8; 32] {
        self.provider
            .binding
            .moderation_checkpoint_attestation_public_key
            .unwrap_or([0; 32])
    }
    fn load_latest(
        &self,
    ) -> Result<
        Option<sorafs_node::moderation_orchestrator::ModerationCheckpointStoreRecordV1>,
        sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1,
    > {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_MODERATION_CHECKPOINT_LOAD_V1,
                &(),
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
            .map_err(moderation_checkpoint_error)?;
        let record = self
            .provider
            .decode_sensitive::<Option<Vec<u8>>>(&result, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
            .map_err(moderation_checkpoint_error)?;
        record
            .map(|record| {
                decode_moderation_checkpoint_record(
                    &record,
                    &self.provider.binding,
                    Some(&self.provider.session.network_id),
                )
                .map_err(moderation_checkpoint_error)
            })
            .transpose()
    }
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &sorafs_node::moderation_orchestrator::ModerationCheckpointStoreRecordV1,
    ) -> Result<(), sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1>
    {
        let record_limit = moderation_checkpoint_record_limit(&self.provider.binding)
            .map_err(moderation_checkpoint_error)?;
        let next_record =
            encode_canonical(next, record_limit).map_err(moderation_checkpoint_error)?;
        let result = self
            .provider
            .call_sensitive(
                OPERATION_MODERATION_CHECKPOINT_COMPARE_AND_SWAP_V1,
                &EvidenceViewerCheckpointCompareAndSwapRequestWireV1 {
                    expected_revision,
                    next_record,
                },
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
                true,
            )
            .map_err(moderation_checkpoint_error)?;
        self.provider
            .decode_sensitive::<()>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(moderation_checkpoint_error)
    }
    fn attest_terminal_set(
        &self,
        statement: &sorafs_node::moderation_orchestrator::
            ModerationPanelNotificationSourceAttestationV1,
    ) -> Result<
        [u8; 64],
        sorafs_node::moderation_orchestrator::ModerationCheckpointStoreExternalErrorV1,
    > {
        if statement.network_id != self.provider.session.network_id
            || statement.attestor_slot
                != IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id()
            || statement.attestor_handle != self.provider.binding.handle
            || Some(statement.attestor_revision) != self.provider.binding.revision
            || Some(statement.attestor_policy_digest) != self.provider.binding.policy_digest
            || Some(statement.attestor_public_key)
                != self
                    .provider
                    .binding
                    .moderation_checkpoint_attestation_public_key
        {
            return Err(sorafs_node::moderation_orchestrator::
                ModerationCheckpointStoreExternalErrorV1::Rejected);
        }
        let result = self
            .provider
            .call_sensitive(
                OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1,
                &ModerationPanelNotificationSourceAttestRequestWireV1 {
                    version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                    slot: IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id(),
                    network_id: self.provider.session.network_id,
                    statement: statement.clone(),
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                true,
            )
            .map_err(moderation_checkpoint_error)?;
        let signed = self
            .provider
            .decode_sensitive::<ModerationPanelNotificationSourceAttestResultWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .map_err(moderation_checkpoint_error)?;
        if signed.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
            || signed.slot != IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id()
            || signed.statement_digest == [0; 32]
            || statement.verify(signed.signature).is_err()
        {
            self.provider.session.poison();
            return Err(sorafs_node::moderation_orchestrator::
                ModerationCheckpointStoreExternalErrorV1::Rejected);
        }
        Ok(signed.signature)
    }
}
#[derive(Clone, Debug)]
struct ModerationPanelNotificationBrokerArchive {
    provider: EvidenceViewerBrokerProvider,
    archive_id: [u8; 32],
    public_key: [u8; 32],
}
impl ModerationPanelNotificationBrokerArchive {
    fn live_qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        BrokerError,
    > {
        let result = self.provider.call_sensitive(
            OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_QUALIFY_V1,
            &ModerationPanelNotificationArchiveQualifyRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                network_id: self.provider.session.network_id,
            },
            MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            false,
        )?;
        let observed = self
            .provider
            .decode_sensitive::<ModerationPanelNotificationArchiveQualificationWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )?;
        let expected_revision = self
            .provider
            .binding
            .revision
            .ok_or(BrokerError::BindingMismatch)?;
        let expected_policy_digest = self
            .provider
            .binding
            .policy_digest
            .ok_or(BrokerError::BindingMismatch)?;
        if observed.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
            || observed.slot
                != IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id()
            || observed.revision != expected_revision
            || observed.policy_digest != expected_policy_digest
            || observed.archive_id != self.archive_id
            || observed.public_key != self.public_key
        {
            self.provider.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(
            sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1::new(
                expected_revision,
                expected_policy_digest,
            ),
        )
    }
}
impl sorafs_node::moderation_orchestrator::ModerationRuntimeProviderV1
    for ModerationPanelNotificationBrokerArchive
{
    fn handle(&self) -> &str {
        self.provider.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderQualificationV1,
        sorafs_node::moderation_orchestrator::ModerationRuntimeProviderReadinessErrorV1,
    > {
        self.live_qualification().map_err(|error| {
            match error {
                BrokerError::Unavailable | BrokerError::Ambiguous => {
                    sorafs_node::moderation_orchestrator::
                        ModerationRuntimeProviderReadinessErrorV1::Unavailable
                }
                BrokerError::Rejected
                | BrokerError::Conflict
                | BrokerError::StaleOrRevoked
                | BrokerError::Protocol
                | BrokerError::BindingMismatch => {
                    sorafs_node::moderation_orchestrator::
                        ModerationRuntimeProviderReadinessErrorV1::Rejected
                }
            }
        })
    }
}
impl sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveV1
    for ModerationPanelNotificationBrokerArchive
{
    fn archive_id(&self) -> [u8; 32] {
        self.archive_id
    }
    fn signing_public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<
        [u8; 64],
        sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveExternalErrorV1,
    > {
        let validated = validate_moderation_panel_notification_archive_artifact_at_broker_boundary(
            canonical_artifact,
            &self.provider.session.network_id,
            &self.provider.binding,
            &self.provider.session.requested_catalog,
        )
        .map_err(moderation_panel_notification_archive_error)?;
        if operation_id != validated.operation_id || receipt_message != validated.receipt_message {
            return Err(sorafs_node::moderation_orchestrator::
                ModerationPanelNotificationArchiveExternalErrorV1::Rejected);
        }
        let result = self
            .provider
            .call_sensitive(
                OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
                &ModerationPanelNotificationArchiveInstallRequestWireV1 {
                    version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                    slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                    network_id: self.provider.session.network_id,
                    operation_id,
                    receipt_message,
                    canonical_artifact: canonical_artifact.to_vec(),
                },
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
                true,
            )
            .map_err(moderation_panel_notification_archive_error)?;
        let signed = self
            .provider
            .decode_sensitive::<ModerationPanelNotificationArchiveInstallResultWireV1>(
                &result,
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .map_err(moderation_panel_notification_archive_error)?;
        if signed.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
            || signed.slot
                != IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id()
        {
            self.provider.session.poison();
            return Err(sorafs_node::moderation_orchestrator::
                ModerationPanelNotificationArchiveExternalErrorV1::Rejected);
        }
        verify_evidence_viewer_ed25519_signature(
            self.public_key,
            signed.signature,
            &validated.receipt_message,
        )
        .map_err(moderation_panel_notification_archive_error)?;
        Ok(signed.signature)
    }
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<
        Option<sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveReadbackV1>,
        sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveExternalErrorV1,
    > {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_READ_V1,
                &ModerationPanelNotificationArchiveReadRequestWireV1 {
                    version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                    slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                    network_id: self.provider.session.network_id,
                    operation_id,
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
            .map_err(moderation_panel_notification_archive_error)?;
        let readback = self
            .provider
            .decode_sensitive::<Option<ModerationPanelNotificationArchiveReadbackWireV1>>(
                &result,
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
            )
            .map_err(moderation_panel_notification_archive_error)?;
        readback
            .map(|mut readback| {
                if readback.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
                    || readback.slot
                        != IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id()
                {
                    self.provider.session.poison();
                    return Err(sorafs_node::moderation_orchestrator::
                        ModerationPanelNotificationArchiveExternalErrorV1::Rejected);
                }
                let validated =
                    validate_moderation_panel_notification_archive_readback_at_broker_boundary(
                        &readback.canonical_artifact,
                        &self.provider.session.network_id,
                        &self.provider.binding,
                        &self.provider.session.requested_catalog,
                    )
                    .map_err(moderation_panel_notification_archive_error)?;
                if validated.operation_id != operation_id
                    || verify_evidence_viewer_ed25519_signature(
                        validated.archive_public_key,
                        readback.signature,
                        &validated.receipt_message,
                    )
                    .is_err()
                {
                    self.provider.session.poison();
                    return Err(sorafs_node::moderation_orchestrator::
                        ModerationPanelNotificationArchiveExternalErrorV1::Rejected);
                }
                Ok(sorafs_node::moderation_orchestrator::
                    ModerationPanelNotificationArchiveReadbackV1 {
                        canonical_artifact: std::mem::take(
                            &mut readback.canonical_artifact,
                        ),
                        signature: readback.signature,
                    })
            })
            .transpose()
    }
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerCompactionArchive {
    provider: EvidenceViewerBrokerProvider,
    archive_id: [u8; 32],
    public_key: [u8; 32],
}
impl_evidence_viewer_runtime_provider!(EvidenceViewerBrokerCompactionArchive);
impl sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1
    for EvidenceViewerBrokerCompactionArchive
{
    fn archive_id(&self) -> [u8; 32] {
        self.archive_id
    }
    fn signing_public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1> {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_ARCHIVE_INSTALL_V1,
                &EvidenceViewerArchiveInstallRequestWireV1 {
                    operation_id,
                    receipt_message,
                    canonical_artifact: canonical_artifact.to_vec(),
                },
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
                true,
            )
            .map_err(evidence_viewer_external_error)?;
        let signed = self
            .provider
            .decode_sensitive::<SignResultWireV1>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(evidence_viewer_external_error)?;
        verify_evidence_viewer_ed25519_signature(
            self.public_key,
            signed.signature,
            &receipt_message,
        )
        .map_err(evidence_viewer_external_error)?;
        Ok(signed.signature)
    }
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<
        Option<sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveReadbackV1>,
        sorafs_node::evidence_viewer::EvidenceViewerExternalErrorV1,
    > {
        let result = self
            .provider
            .call_sensitive(
                OPERATION_EVIDENCE_VIEWER_ARCHIVE_READ_V1,
                &EvidenceViewerArchiveReadRequestWireV1 { operation_id },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
            .map_err(evidence_viewer_external_error)?;
        self.provider
            .decode_sensitive::<Option<EvidenceViewerArchiveReadbackWireV1>>(
                &result,
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
            )
            .map_err(evidence_viewer_external_error)
            .map(|readback| {
                readback.map(|mut readback| {
                    sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveReadbackV1 {
                        canonical_artifact: std::mem::take(&mut readback.canonical_artifact),
                        signature: readback.signature,
                    }
                })
            })
    }
}
#[derive(Clone, Debug)]
struct EvidenceViewerBrokerTransparencyPublisher {
    provider: EvidenceViewerBrokerProvider,
    public_key: [u8; 32],
}
impl sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderV1
    for EvidenceViewerBrokerTransparencyPublisher
{
    fn handle(&self) -> &str {
        self.provider.handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1,
        sorafs_node::evidence_viewer::EvidenceViewerRuntimeProviderReadinessErrorV1,
    > {
        match self.provider.live_qualification() {
            Ok(qualification) => Ok(qualification),
            Err(BrokerError::Unavailable) => {
                self.provider
                    .session
                    .reconnect()
                    .map_err(evidence_viewer_readiness_error)?;
                self.provider
                    .live_qualification()
                    .map_err(evidence_viewer_readiness_error)
            }
            Err(error) => Err(evidence_viewer_readiness_error(error)),
        }
    }
}
impl sorafs_node::evidence_viewer::transparency_producer::EvidenceViewerTransparencyPublisherV1
    for EvidenceViewerBrokerTransparencyPublisher
{
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn load_head(
        &self,
    ) -> Result<
        Option<
            sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerSignedTransparencyHeadV1,
        >,
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1,
    >{
        let call = || {
            self.provider.call_sensitive(
                OPERATION_EVIDENCE_VIEWER_TRANSPARENCY_LOAD_V1,
                &(),
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
                false,
            )
        };
        let result = match call() {
            Ok(result) => result,
            Err(BrokerError::Unavailable) => {
                self.provider
                    .session
                    .reconnect()
                    .map_err(evidence_viewer_transparency_publisher_error)?;
                call().map_err(evidence_viewer_transparency_publisher_error)?
            }
            Err(error) => return Err(evidence_viewer_transparency_publisher_error(error)),
        };
        self.provider
            .decode_sensitive::<
                Option<
                    sorafs_node::evidence_viewer::transparency_producer::
                        EvidenceViewerSignedTransparencyHeadV1,
                >,
            >(&result, MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1)
            .map_err(evidence_viewer_transparency_publisher_error)
    }
    fn compare_and_publish(
        &self,
        body: &sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyHeadBodyV1,
    ) -> Result<
        (),
        sorafs_node::evidence_viewer::transparency_producer::
            EvidenceViewerTransparencyPublisherExternalErrorV1,
    >{
        let result = match self.provider.call_sensitive(
            OPERATION_EVIDENCE_VIEWER_TRANSPARENCY_COMPARE_AND_PUBLISH_V1,
            body,
            MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
            true,
        ) {
            Ok(result) => result,
            Err(error @ (BrokerError::Ambiguous | BrokerError::Unavailable)) => {
                // Never replay a mutation whose delivery is uncertain.
                // A fresh authenticated session is established only so
                // the producer can requalify and load the authoritative
                // signed head.
                let _reconnect_result = self.provider.session.reconnect();
                return Err(evidence_viewer_transparency_publisher_error(error));
            }
            Err(error) => {
                return Err(evidence_viewer_transparency_publisher_error(error));
            }
        };
        self.provider
            .decode_sensitive::<()>(&result, MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1)
            .map_err(evidence_viewer_transparency_publisher_error)
    }
}
