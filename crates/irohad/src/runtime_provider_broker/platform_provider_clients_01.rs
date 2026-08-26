#[derive(Clone)]
struct BootleLanternBrokerProvider {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    exact_bindings:
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderBindingsV1,
}
impl_broker_debug_fields!(BootleLanternBrokerProvider as value {
    "handle" => value.binding.handle,
    "revision" => value.binding.revision,
    "policy_digest" => value.binding.policy_digest,
    "issuer_id" => value.exact_bindings.issuer_id(),
    "policy_id" => value.exact_bindings.policy_id(),
    "authorization_lifetime_blocks" => value.exact_bindings.authorization_lifetime_blocks(),
} => finish_non_exhaustive);
impl BootleLanternBrokerProvider {
    fn live_qualification(
        &self,
    ) -> Result<
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderQualificationV1,
        BrokerError,
    > {
        let payload = encode_canonical(&(), MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = self
            .session
            .decode_operation_result::<QualificationResultWireV1>(&result, OPERATION_QUALIFY_V1)?;
        if Some(qualification.revision) != self.binding.revision
            || Some(qualification.policy_digest) != self.binding.policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(iroha_torii::privacy_issuance_api::
            BootleLanternIssuanceRuntimeProviderQualificationV1::new(
                qualification.revision,
                qualification.policy_digest,
            ))
    }
    fn call_sensitive_requalified(
        &self,
        operation: u16,
        payload: ScrubbedBytes,
    ) -> Result<ScrubbedBytes, BrokerError> {
        self.live_qualification()?;
        let outcome = provider_call!(self, call_sensitive, operation, payload, false,);
        match outcome {
            Ok(result) => {
                self.live_qualification()
                    .inspect_err(|_| self.session.poison())?;
                Ok(result)
            }
            Err(error @ (BrokerError::Rejected | BrokerError::Conflict)) => {
                self.live_qualification()
                    .inspect_err(|_| self.session.poison())?;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
    fn registry_error(
        error: BrokerError,
    ) -> iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryErrorV1
    {
        match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuanceRuntimeProviderRegistryErrorV1::Unavailable
            }
            BrokerError::StaleOrRevoked => {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuanceRuntimeProviderRegistryErrorV1::StaleOrRevoked
            }
            BrokerError::BindingMismatch
            | BrokerError::Protocol
            | BrokerError::Rejected
            | BrokerError::Conflict => {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuanceRuntimeProviderRegistryErrorV1::RejectedBindings
            }
        }
    }
    fn crypto_error(
        error: BrokerError,
    ) -> iroha_torii::privacy_issuance_api::BootleLanternIssuerCryptoProviderErrorV1 {
        match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuerCryptoProviderErrorV1::Unavailable
            }
            BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuerCryptoProviderErrorV1::PolicyMismatch
            }
            BrokerError::Protocol | BrokerError::Rejected | BrokerError::Conflict => {
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest
            }
        }
    }
}
impl iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryV1
    for BootleLanternBrokerProvider
{
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderQualificationV1,
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryErrorV1,
    > {
        self.live_qualification().map_err(Self::registry_error)
    }
    fn resolve(
        &self,
        bindings: &iroha_torii::privacy_issuance_api::
            BootleLanternIssuanceRuntimeProviderBindingsV1,
    ) -> Result<
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeSecretsV1,
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryErrorV1,
    > {
        if bindings != &self.exact_bindings {
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuanceRuntimeProviderRegistryErrorV1::RejectedBindings);
        }
        self.live_qualification().map_err(Self::registry_error)?;
        let provider = Arc::new(self.clone());
        let issuer_provider: Arc<
            dyn iroha_torii::privacy_issuance_api::BootleLanternIssuerCryptoProviderV1,
        > = provider.clone();
        let authenticator: Arc<
            dyn iroha_torii::privacy_issuance_api::BootleLanternIssuanceAuthenticatorV1,
        > = provider;
        Ok(
            iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeSecretsV1 {
                issuer_provider,
                authenticator,
            },
        )
    }
}
impl iroha_torii::privacy_issuance_api::BootleLanternIssuanceAuthenticatorV1
    for BootleLanternBrokerProvider
{
    fn authenticate(
        &self,
        opaque_credential: &[u8],
        action: iroha_torii::privacy_issuance_api::BootleLanternIssuanceActionV1,
        request_binding: [u8; 32],
        committed_height: u64,
    ) -> Result<
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceAuthenticatedPrincipalV1,
        iroha_torii::privacy_issuance_api::BootleLanternIssuanceAuthenticationErrorV1,
    > {
        if opaque_credential.is_empty()
            || opaque_credential.len() > MAX_BOOTLE_LANTERN_AUTH_CREDENTIAL_BYTES_V1
            || request_binding == [0; 32]
            || committed_height == 0
        {
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuanceAuthenticationErrorV1::Denied);
        }
        let payload = encode_sensitive_canonical(
            &BootleLanternAuthenticateRequestWireV1 {
                opaque_credential: opaque_credential.to_vec(),
                action: bootle_lantern_action_to_wire(action),
                request_binding,
                committed_height,
            },
            MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
        )
        .map_err(|_| {
            iroha_torii::privacy_issuance_api::
            BootleLanternIssuanceAuthenticationErrorV1::Unavailable
        })?;
        let result = self
            .call_sensitive_requalified(OPERATION_BOOTLE_LANTERN_ISSUANCE_AUTHENTICATE_V1, payload)
            .map_err(|error| {
                match error {
                BrokerError::Rejected | BrokerError::Conflict => {
                    iroha_torii::privacy_issuance_api::
                        BootleLanternIssuanceAuthenticationErrorV1::Denied
                }
                BrokerError::Unavailable
                | BrokerError::Ambiguous
                | BrokerError::BindingMismatch
                | BrokerError::StaleOrRevoked
                | BrokerError::Protocol => {
                    iroha_torii::privacy_issuance_api::
                        BootleLanternIssuanceAuthenticationErrorV1::Unavailable
                }
            }
            })?;
        let principal = self
            .session
            .decode_operation_result::<BootleLanternAuthenticatedPrincipalWireV1>(
                &result,
                OPERATION_BOOTLE_LANTERN_ISSUANCE_AUTHENTICATE_V1,
            )
            .map_err(|_| {
                iroha_torii::privacy_issuance_api::
                BootleLanternIssuanceAuthenticationErrorV1::Unavailable
            })?;
        if principal.principal_digest == [0; 32]
            || principal.issued_at_height == 0
            || principal.issued_at_height > committed_height
            || principal.expires_at_height < committed_height
            || principal.expires_at_height < principal.issued_at_height
        {
            self.session.poison();
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuanceAuthenticationErrorV1::Unavailable);
        }
        Ok(
            iroha_torii::privacy_issuance_api::BootleLanternIssuanceAuthenticatedPrincipalV1 {
                principal_digest: principal.principal_digest,
                issued_at_height: principal.issued_at_height,
                expires_at_height: principal.expires_at_height,
            },
        )
    }
}
impl iroha_torii::privacy_issuance_api::BootleLanternIssuerCryptoProviderV1
    for BootleLanternBrokerProvider
{
    fn issuer_id(&self) -> iroha_data_model::privacy::PrivacyIssuerIdV1 {
        self.exact_bindings.issuer_id()
    }
    fn policy_id(&self) -> iroha_data_model::privacy::PrivacyPolicyIdV1 {
        self.exact_bindings.policy_id()
    }
    fn prepare_authorization(
        &self,
        context: &iroha_data_model::privacy::PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &iroha_data_model::privacy::BootleLanternIssuerPolicyV1,
        requester_authorization_digest: [u8; 32],
        issued_at_height: u64,
        expires_at_height: u64,
    ) -> Result<
        iroha_core::privacy_engines::bootle_lantern::issuer::BootleLanternIssuanceAuthorizationV1,
        iroha_torii::privacy_issuance_api::BootleLanternIssuerCryptoProviderErrorV1,
    > {
        let request = BootleLanternPrepareAuthorizationRequestWireV1 {
            context: *context,
            canonical_genesis_hash,
            policy: policy.clone(),
            requester_authorization_digest,
            issued_at_height,
            expires_at_height,
        };
        validate_bootle_lantern_prepare_request(&request, &self.binding, &self.session.network_id)
            .map_err(Self::crypto_error)?;
        let payload =
            encode_sensitive_canonical(&request, MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1)
                .map_err(Self::crypto_error)?;
        let result = self
            .call_sensitive_requalified(
                OPERATION_BOOTLE_LANTERN_ISSUANCE_PREPARE_AUTHORIZATION_V1,
                payload,
            )
            .map_err(Self::crypto_error)?;
        let mut authorization = self
            .session
            .decode_operation_result::<BootleLanternAuthorizationWireV1>(
                &result,
                OPERATION_BOOTLE_LANTERN_ISSUANCE_PREPARE_AUTHORIZATION_V1,
            )
            .map_err(Self::crypto_error)?;
        if authorization.authorization.len() != BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 {
            self.session.poison();
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::Unavailable);
        }
        let authorization_bytes =
            ScrubbedBytes::new(std::mem::take(&mut authorization.authorization));
        let authorization = iroha_core::privacy_engines::bootle_lantern::issuer::
            BootleLanternIssuanceAuthorizationV1::decode_exact(&authorization_bytes)
            .map_err(|_| {
                self.session.poison();
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuerCryptoProviderErrorV1::Unavailable
            })?;
        iroha_core::privacy_engines::bootle_lantern::issuer::
            issuer_validate_prepared_blind_issuance_authorization_v1(
                context,
                canonical_genesis_hash,
                policy,
                &authorization,
            )
            .map_err(|_| {
                self.session.poison();
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuerCryptoProviderErrorV1::Unavailable
            })?;
        if authorization.requester_authorization_digest() != requester_authorization_digest
            || authorization.issued_at_height() != issued_at_height
            || authorization.expires_at_height() != expires_at_height
        {
            self.session.poison();
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::Unavailable);
        }
        Ok(authorization)
    }
    fn validate_request(
        &self,
        context: &iroha_data_model::privacy::PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &iroha_data_model::privacy::BootleLanternIssuerPolicyV1,
        authorization: &iroha_core::privacy_engines::bootle_lantern::issuer::
            BootleLanternIssuanceAuthorizationV1,
        request_bytes: &[u8],
        current_height: u64,
    ) -> Result<[u8; 32], iroha_torii::privacy_issuance_api::BootleLanternIssuerCryptoProviderErrorV1>
    {
        if context.network_id != self.session.network_id {
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest);
        }
        let expected = iroha_core::privacy_engines::bootle_lantern::issuer::
            issuer_validate_blind_issuance_request_encoded_v1(
                context,
                canonical_genesis_hash,
                policy,
                authorization,
                request_bytes,
                current_height,
            )
            .map_err(|_| iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest)?;
        let authorization_bytes = authorization.encode().map_err(|_| {
            iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest
        })?;
        let payload = encode_sensitive_canonical(
            &BootleLanternIssueRequestWireV1 {
                context: *context,
                canonical_genesis_hash,
                policy: policy.clone(),
                authorization: authorization_bytes,
                request: request_bytes.to_vec(),
                current_height,
            },
            MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
        )
        .map_err(Self::crypto_error)?;
        let result = self
            .call_sensitive_requalified(
                OPERATION_BOOTLE_LANTERN_ISSUANCE_VALIDATE_REQUEST_V1,
                payload,
            )
            .map_err(Self::crypto_error)?;
        let actual = self
            .session
            .decode_operation_result::<[u8; 32]>(
                &result,
                OPERATION_BOOTLE_LANTERN_ISSUANCE_VALIDATE_REQUEST_V1,
            )
            .map_err(Self::crypto_error)?;
        if actual == [0; 32] || actual != expected {
            self.session.poison();
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::Unavailable);
        }
        Ok(actual)
    }
    fn issue_validated(
        &self,
        context: &iroha_data_model::privacy::PrivacyStatementContextV1,
        canonical_genesis_hash: [u8; 32],
        policy: &iroha_data_model::privacy::BootleLanternIssuerPolicyV1,
        authorization: &iroha_core::privacy_engines::bootle_lantern::issuer::
            BootleLanternIssuanceAuthorizationV1,
        request_bytes: &[u8],
        current_height: u64,
    ) -> Result<
        iroha_core::privacy_engines::bootle_lantern::issuer::BootleLanternBlindIssuanceResponseV1,
        iroha_torii::privacy_issuance_api::BootleLanternIssuerCryptoProviderErrorV1,
    > {
        if context.network_id != self.session.network_id {
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest);
        }
        iroha_core::privacy_engines::bootle_lantern::issuer::
            issuer_validate_blind_issuance_request_encoded_v1(
                context,
                canonical_genesis_hash,
                policy,
                authorization,
                request_bytes,
                current_height,
            )
            .map_err(|_| iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest)?;
        let authorization_bytes = authorization.encode().map_err(|_| {
            iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::InvalidRequest
        })?;
        let payload = encode_sensitive_canonical(
            &BootleLanternIssueRequestWireV1 {
                context: *context,
                canonical_genesis_hash,
                policy: policy.clone(),
                authorization: authorization_bytes,
                request: request_bytes.to_vec(),
                current_height,
            },
            MAX_BOOTLE_LANTERN_ISSUANCE_FRAME_BYTES_V1,
        )
        .map_err(Self::crypto_error)?;
        let result = self
            .call_sensitive_requalified(
                OPERATION_BOOTLE_LANTERN_ISSUANCE_ISSUE_VALIDATED_V1,
                payload,
            )
            .map_err(Self::crypto_error)?;
        let mut response = self
            .session
            .decode_operation_result::<BootleLanternIssuanceResponseWireV1>(
                &result,
                OPERATION_BOOTLE_LANTERN_ISSUANCE_ISSUE_VALIDATED_V1,
            )
            .map_err(Self::crypto_error)?;
        if response.response.len() != BOOTLE_LANTERN_RESPONSE_BYTES_V1 {
            self.session.poison();
            return Err(iroha_torii::privacy_issuance_api::
                BootleLanternIssuerCryptoProviderErrorV1::Unavailable);
        }
        let response_bytes = ScrubbedBytes::new(std::mem::take(&mut response.response));
        let response = iroha_core::privacy_engines::bootle_lantern::issuer::
            issuer_validate_cached_blind_issuance_response_encoded_v1(
                context,
                canonical_genesis_hash,
                policy,
                authorization,
                request_bytes,
                &response_bytes,
            )
            .map_err(|_| {
                self.session.poison();
                iroha_torii::privacy_issuance_api::
                    BootleLanternIssuerCryptoProviderErrorV1::Unavailable
            })?;
        Ok(response)
    }
}
fn source_reader_io_error(kind: std::io::ErrorKind) -> std::io::Error {
    std::io::Error::new(kind, "authenticated provider source stream failed")
}
struct ProviderIngestBrokerSourceReader {
    stream: UnixStream,
    deadline: std::time::Instant,
    content_length: u64,
    remaining: u64,
    frame_count: u64,
    next_sequence: u64,
    pending: Vec<u8>,
    pending_offset: usize,
    expected_payload_digest: [u8; 32],
    expected_provider_metadata_digest: [u8; 32],
    payload_hasher: blake3::Hasher,
    transcript: blake3::Hasher,
    finished: bool,
    poisoned: bool,
    _retained_memory: Option<DecodeResourcePoolPermitV1>,
}
impl_broker_debug_fields!(ProviderIngestBrokerSourceReader as value {
    "content_length" => value.content_length,
    "remaining" => value.remaining,
    "frame_count" => value.frame_count,
    "finished" => value.finished,
} => finish_non_exhaustive);
impl ProviderIngestBrokerSourceReader {
    fn poison(&mut self, kind: std::io::ErrorKind) -> std::io::Error {
        self.poisoned = true;
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        source_reader_io_error(kind)
    }
    fn apply_deadline(&mut self) -> std::io::Result<()> {
        apply_source_socket_deadline(&self.stream, self.deadline)
            .map_err(|_| self.poison(std::io::ErrorKind::TimedOut))
    }
    fn transport_failure(&mut self) -> std::io::Error {
        let kind = if std::time::Instant::now() >= self.deadline {
            std::io::ErrorKind::TimedOut
        } else {
            std::io::ErrorKind::UnexpectedEof
        };
        self.poison(kind)
    }
    fn load_chunk(&mut self) -> std::io::Result<()> {
        if self.remaining == 0 || self.next_sequence >= self.frame_count {
            return Err(self.poison(std::io::ErrorKind::InvalidData));
        }
        self.apply_deadline()?;
        let decode_admission =
            DecodeResourceAdmissionV1::acquire(None, SOURCE_STREAM_FRAME_DECODE_POLICY_V1)
                .map_err(|_| self.poison(std::io::ErrorKind::OutOfMemory))?;
        let frame = read_length_prefixed_with_decode_admission(
            &mut self.stream,
            MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1,
            &decode_admission,
        )
        .map_err(|_| self.transport_failure())?;
        let _decode_scope = decode_admission.enter();
        let chunk = decode_frame_with_policy::<ProviderIngestSourceChunkWireV1>(
            &frame,
            FRAME_KIND_PROVIDER_INGEST_SOURCE_CHUNK_V1,
            MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1,
            SOURCE_STREAM_FRAME_DECODE_POLICY_V1,
        )
        .map_err(|_| self.poison(std::io::ErrorKind::InvalidData))?;
        let expected_len = usize::try_from(
            self.remaining.min(
                u64::try_from(MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1)
                    .map_err(|_| self.poison(std::io::ErrorKind::InvalidData))?,
            ),
        )
        .map_err(|_| self.poison(std::io::ErrorKind::InvalidData))?;
        let expected_offset = self
            .content_length
            .checked_sub(self.remaining)
            .ok_or_else(|| self.poison(std::io::ErrorKind::InvalidData))?;
        if chunk.sequence != self.next_sequence
            || chunk.offset != expected_offset
            || chunk.bytes.len() != expected_len
            || chunk.bytes.is_empty()
        {
            return Err(self.poison(std::io::ErrorKind::InvalidData));
        }
        self.payload_hasher.update(&chunk.bytes);
        update_source_stream_transcript(&mut self.transcript, &chunk);
        self.remaining = self
            .remaining
            .checked_sub(
                u64::try_from(chunk.bytes.len())
                    .map_err(|_| self.poison(std::io::ErrorKind::InvalidData))?,
            )
            .ok_or_else(|| self.poison(std::io::ErrorKind::InvalidData))?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| self.poison(std::io::ErrorKind::InvalidData))?;
        self.pending = chunk.bytes;
        self.pending_offset = 0;
        Ok(())
    }
    fn finish(&mut self) -> std::io::Result<()> {
        if self.finished {
            return Ok(());
        }
        if self.remaining != 0
            || self.pending_offset != self.pending.len()
            || self.next_sequence != self.frame_count
        {
            return Err(self.poison(std::io::ErrorKind::UnexpectedEof));
        }
        self.apply_deadline()?;
        let decode_admission =
            DecodeResourceAdmissionV1::acquire(None, SOURCE_STREAM_FRAME_DECODE_POLICY_V1)
                .map_err(|_| self.poison(std::io::ErrorKind::OutOfMemory))?;
        let frame = read_length_prefixed_with_decode_admission(
            &mut self.stream,
            MAX_PROVIDER_INGEST_SOURCE_TRAILER_FRAME_BYTES_V1,
            &decode_admission,
        )
        .map_err(|_| self.transport_failure())?;
        let _decode_scope = decode_admission.enter();
        let trailer = decode_frame_with_policy::<ProviderIngestSourceTrailerWireV1>(
            &frame,
            FRAME_KIND_PROVIDER_INGEST_SOURCE_TRAILER_V1,
            MAX_PROVIDER_INGEST_SOURCE_TRAILER_FRAME_BYTES_V1,
            SOURCE_STREAM_FRAME_DECODE_POLICY_V1,
        )
        .map_err(|_| self.poison(std::io::ErrorKind::InvalidData))?;
        let payload_digest = *self.payload_hasher.clone().finalize().as_bytes();
        let transcript_digest = *self.transcript.clone().finalize().as_bytes();
        if trailer.status != STATUS_OK_V1
            || trailer.content_length != self.content_length
            || trailer.frame_count != self.frame_count
            || trailer.payload_digest != self.expected_payload_digest
            || trailer.payload_digest != payload_digest
            || trailer.transcript_digest != transcript_digest
            || trailer.provider_metadata_digest != self.expected_provider_metadata_digest
        {
            return Err(self.poison(std::io::ErrorKind::InvalidData));
        }
        self.apply_deadline()?;
        let mut trailing = [0_u8; 1];
        match std::io::Read::read(&mut self.stream, &mut trailing) {
            Ok(0) => {
                self.finished = true;
                Ok(())
            }
            Ok(_) => Err(self.poison(std::io::ErrorKind::InvalidData)),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Err(self.poison(std::io::ErrorKind::TimedOut))
            }
            Err(_) => Err(self.poison(std::io::ErrorKind::UnexpectedEof)),
        }
    }
}
impl std::io::Read for ProviderIngestBrokerSourceReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        if self.poisoned {
            return Err(source_reader_io_error(std::io::ErrorKind::InvalidData));
        }
        loop {
            if self.pending_offset < self.pending.len() {
                let available = &self.pending[self.pending_offset..];
                let copied = available.len().min(output.len());
                output[..copied].copy_from_slice(&available[..copied]);
                self.pending_offset += copied;
                return Ok(copied);
            }
            if self.remaining == 0 {
                self.finish()?;
                return Ok(0);
            }
            self.load_chunk()?;
        }
    }
}
impl Drop for ProviderIngestBrokerSourceReader {
    fn drop(&mut self) {
        if !self.finished {
            self.poisoned = true;
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
    }
}
#[derive(Clone)]
struct ProviderIngestBrokerAuthenticatedSource {
    session: Arc<BrokerSession>,
    endpoint: EndpointPolicy,
    chain_id: String,
    requested_catalog: Vec<ProviderBindingWireV1>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    source_provider_ids: Vec<[u8; 32]>,
}
impl_broker_debug_fields!(ProviderIngestBrokerAuthenticatedSource as value {
    "source_provider_count" => value.source_provider_ids.len(),
} => finish_non_exhaustive);
impl ProviderIngestBrokerAuthenticatedSource {
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::ProviderIngestRuntimeProviderQualificationV1, BrokerError> {
        let payload = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = self
            .session
            .decode_result::<ProviderIngestRuntimeQualificationWireV1>(&result)?;
        let expected = qualification_from_binding(&self.binding)?;
        if qualification.revision != expected.revision
            || qualification.policy_digest != expected.policy_digest
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(
            sorafs_node::ProviderIngestRuntimeProviderQualificationV1::new(
                expected.revision,
                expected.policy_digest,
            ),
        )
    }
    #[expect(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "the blocking stream owns all authenticated connection inputs"
    )]
    fn open_stream(
        endpoint: EndpointPolicy,
        chain_id: String,
        network_id: NetworkId,
        requested_catalog: Vec<ProviderBindingWireV1>,
        binding: ProviderBindingWireV1,
        metadata_digest: [u8; 32],
        source_provider_ids: Vec<[u8; 32]>,
        request: sorafs_node::ProviderIngestSourceRequestV1,
    ) -> Result<crate::sorafs_provider_ingest_runtime::VerifiedProviderIngestPayloadV1, BrokerError>
    {
        let limits = required_binding_value!(binding, provider_ingest_source_limits);
        let deadline = std::time::Instant::now()
            .checked_add(Duration::from_millis(limits.operation_timeout_ms))
            .ok_or(BrokerError::Rejected)?;
        let fetch = source_request_to_wire(request)?;
        validate_source_fetch_request(&fetch, &binding, Some(&source_provider_ids), &network_id)?;
        let (mut connection, observations) = connect_broker_connection(
            &endpoint,
            &chain_id,
            network_id,
            requested_catalog,
            Some(source_deadline_remaining(deadline)?),
        )?;
        let observed = observations
            .iter()
            .find(|observation| observation.binding == binding)
            .ok_or(BrokerError::BindingMismatch)?;
        if observed.metadata_digest != metadata_digest
            || observed.provider_ingest_source_provider_ids != source_provider_ids
        {
            return Err(BrokerError::StaleOrRevoked);
        }
        apply_source_socket_deadline(&connection.stream, deadline)?;
        let decode_admission = DecodeResourceAdmissionV1::acquire_operation(
            OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V1,
        )?;
        let decode_scope = decode_admission.enter();
        let payload = encode_canonical(&fetch, MAX_PROVIDER_INGEST_SOURCE_REQUEST_BYTES_V1)?;
        let operation_request = make_operation_request(
            connection.session_id,
            connection.next_request_id,
            binding,
            metadata_digest,
            OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V1,
            payload,
        )?;
        let request_frame = encode_frame(
            FRAME_KIND_OPERATION_REQUEST_V1,
            &operation_request,
            MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
        )?;
        write_operation_request_frame(&mut connection.stream, &operation_request, &request_frame)?;
        drop(request_frame);
        let response_frame = read_length_prefixed_with_decode_admission(
            &mut connection.stream,
            MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
            &decode_admission,
        )?;
        let response = decode_operation_frame::<OperationResponseV1>(
            &response_frame,
            FRAME_KIND_OPERATION_RESPONSE_V1,
            OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V1,
        )?;
        validate_operation_response(&operation_request, &response, &network_id)?;
        match response.status {
            STATUS_OK_V1 => {}
            STATUS_REJECTED_V1 => return Err(BrokerError::Rejected),
            STATUS_STALE_OR_REVOKED_V1 => return Err(BrokerError::StaleOrRevoked),
            _ => return Err(BrokerError::Protocol),
        }
        let header = decode_canonical_with_policy::<ProviderIngestSourceHeaderWireV1>(
            &response.result,
            MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
            SOURCE_PLAN_DECODE_POLICY_V1,
        )?;
        validate_source_metadata_lengths(header.manifest.len(), header.plan.len())?;
        if header.content_length != fetch.authorization.content_length()
            || header.frame_count != source_stream_frame_count(header.content_length)?
        {
            return Err(BrokerError::Rejected);
        }
        let manifest = sorafs_manifest::decode_manifest_v1_canonical(&header.manifest)
            .map_err(|_| BrokerError::Rejected)?;
        let plan = decode_source_plan(&header.plan)?;
        validate_source_payload_metadata(&fetch.authorization, &manifest, &plan)?;
        let retained_memory = acquire_source_retained_memory(&plan)?;
        let content_length = header.content_length;
        let frame_count = header.frame_count;
        let transcript = source_stream_transcript(&operation_request, &response);
        drop(decode_scope);
        drop(decode_admission);
        drop(response_frame);
        drop(response);
        drop(header);
        let expected_payload_digest = *plan.payload_digest.as_bytes();
        let reader = ProviderIngestBrokerSourceReader {
            stream: connection.stream,
            deadline,
            content_length,
            remaining: content_length,
            frame_count,
            next_sequence: 0,
            pending: Vec::new(),
            pending_offset: 0,
            expected_payload_digest,
            expected_provider_metadata_digest: metadata_digest,
            payload_hasher: blake3::Hasher::new(),
            transcript,
            finished: false,
            poisoned: false,
            _retained_memory: Some(retained_memory),
        };
        Ok(
            crate::sorafs_provider_ingest_runtime::VerifiedProviderIngestPayloadV1::new(
                manifest,
                plan,
                Box::new(reader),
            ),
        )
    }
}
impl sorafs_node::ProviderIngestAuthenticatedSourceFetchV1
    for ProviderIngestBrokerAuthenticatedSource
{
    type Fetched = crate::sorafs_provider_ingest_runtime::VerifiedProviderIngestPayloadV1;
    fn fetch(
        &self,
        request: sorafs_node::ProviderIngestSourceRequestV1,
    ) -> sorafs_node::ProviderIngestFutureV1<
        '_,
        Result<Self::Fetched, sorafs_node::ProviderIngestSourceFetchErrorV1>,
    > {
        let endpoint = self.endpoint.clone();
        let chain_id = self.chain_id.clone();
        let network_id = self.session.network_id;
        let requested_catalog = self.requested_catalog.clone();
        let binding = self.binding.clone();
        let metadata_digest = self.metadata_digest;
        let source_provider_ids = self.source_provider_ids.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                Self::open_stream(
                    endpoint,
                    chain_id,
                    network_id,
                    requested_catalog,
                    binding,
                    metadata_digest,
                    source_provider_ids,
                    request,
                )
            })
            .await
            .map_err(|_| sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable)?
            .map_err(|error| match error {
                BrokerError::Unavailable | BrokerError::Ambiguous => {
                    sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable
                }
                BrokerError::Rejected | BrokerError::Protocol | BrokerError::Conflict => {
                    sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected
                }
                BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                    sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected
                }
            })
        })
    }
}
impl crate::sorafs_provider_ingest_runtime::ProviderIngestAuthenticatedSourceRuntimeV1
    for ProviderIngestBrokerAuthenticatedSource
{
    fn runtime_handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::ProviderIngestRuntimeProviderQualificationV1,
        sorafs_node::ProviderIngestSourceFetchErrorV1,
    > {
        self.live_qualification().map_err(|error| match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable
            }
            BrokerError::Rejected
            | BrokerError::Protocol
            | BrokerError::Conflict
            | BrokerError::BindingMismatch
            | BrokerError::StaleOrRevoked => {
                sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected
            }
        })
    }
    fn source_provider_ids(&self) -> &[[u8; 32]] {
        &self.source_provider_ids
    }
    fn check_readiness(&self) -> Result<(), sorafs_node::ProviderIngestSourceFetchErrorV1> {
        let payload = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
            .map_err(|_| sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_PROVIDER_INGEST_SOURCE_READINESS_V1,
            payload,
            false,
        )
        .map_err(|error| match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable
            }
            BrokerError::Rejected | BrokerError::Protocol | BrokerError::Conflict => {
                sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected
            }
            BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected
            }
        })?;
        self.session
            .decode_result::<()>(&result)
            .map_err(|_| sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected)
    }
}
#[derive(Clone)]
struct ModerationQuarantineBrokerKeyWrapper {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    active_key_id: String,
}
impl_broker_debug_fields!(ModerationQuarantineBrokerKeyWrapper as value {} => finish_non_exhaustive);
impl ModerationQuarantineBrokerKeyWrapper {
    fn live_qualification(
        &self,
    ) -> Result<sorafs_node::ModerationQuarantineKeyProviderQualificationV1, BrokerError> {
        let payload = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)?;
        let result = provider_call!(self, call, OPERATION_QUALIFY_V1, payload, false,)?;
        let qualification = self
            .session
            .decode_result::<QualificationResultWireV1>(&result)?;
        let expected = moderation_quarantine_qualification_from_binding(&self.binding)?;
        if qualification.revision != expected.revision()
            || qualification.policy_digest != expected.policy_digest()
        {
            self.session.poison();
            return Err(BrokerError::StaleOrRevoked);
        }
        Ok(expected)
    }
    fn readiness_error(
        error: BrokerError,
    ) -> sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1 {
        match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1::Unavailable
            }
            BrokerError::Protocol
            | BrokerError::BindingMismatch
            | BrokerError::StaleOrRevoked
            | BrokerError::Rejected
            | BrokerError::Conflict => {
                sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1::Rejected
            }
        }
    }
    fn operation_error(
        error: BrokerError,
        wrap_may_have_dispatched: bool,
    ) -> sorafs_node::ModerationQuarantineKeyOperationErrorV1 {
        match error {
            BrokerError::BindingMismatch | BrokerError::StaleOrRevoked => {
                sorafs_node::ModerationQuarantineKeyOperationErrorV1::StaleOrRevoked
            }
            BrokerError::Ambiguous if wrap_may_have_dispatched => {
                sorafs_node::ModerationQuarantineKeyOperationErrorV1::Ambiguous
            }
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                sorafs_node::ModerationQuarantineKeyOperationErrorV1::Unavailable
            }
            BrokerError::Protocol | BrokerError::Rejected | BrokerError::Conflict => {
                sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected
            }
        }
    }
}
impl sorafs_node::ModerationQuarantineKeyWrapper for ModerationQuarantineBrokerKeyWrapper {
    fn provider_handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::ModerationQuarantineKeyProviderQualificationV1,
        sorafs_node::ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        self.live_qualification().map_err(Self::readiness_error)
    }
    fn active_key_id(&self) -> &str {
        &self.active_key_id
    }
    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, sorafs_node::ModerationQuarantineKeyOperationErrorV1> {
        validate_moderation_quarantine_context_and_dek(context_digest, *dek)
            .map_err(|_| sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected)?;
        let payload = encode_sensitive_canonical(
            &ModerationQuarantineWrapDekRequestWireV1 {
                context_digest,
                dek: *dek,
            },
            MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1,
        )
        .map_err(|error| Self::operation_error(error, false))?;
        let result = provider_call!(
            self,
            call_sensitive,
            OPERATION_MODERATION_QUARANTINE_WRAP_DEK_V1,
            payload,
            true,
        )
        .map_err(|error| Self::operation_error(error, true))?;
        let mut wrapped = self
            .session
            .decode_nested_result::<ModerationQuarantineWrapDekResultWireV1>(
                &result,
                MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1,
            )
            .map_err(|_| sorafs_node::ModerationQuarantineKeyOperationErrorV1::Ambiguous)?;
        let mut wrapped = ScrubbedBytes::new(std::mem::take(&mut wrapped.wrapped_dek));
        if validate_moderation_quarantine_wrapped_dek(&wrapped).is_err() {
            self.session.poison();
            return Err(sorafs_node::ModerationQuarantineKeyOperationErrorV1::Ambiguous);
        }
        Ok(wrapped.take())
    }
    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], sorafs_node::ModerationQuarantineKeyOperationErrorV1> {
        validate_moderation_quarantine_key_id(key_id)
            .map_err(|_| sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected)?;
        if context_digest == [0; 32] {
            return Err(sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected);
        }
        validate_moderation_quarantine_wrapped_dek(wrapped_dek)
            .map_err(|_| sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected)?;
        let payload = encode_sensitive_canonical(
            &ModerationQuarantineUnwrapDekRequestWireV1 {
                key_id: key_id.to_owned(),
                context_digest,
                wrapped_dek: wrapped_dek.to_vec(),
            },
            MAX_MODERATION_QUARANTINE_OPERATION_BYTES_V1,
        )
        .map_err(|error| Self::operation_error(error, false))?;
        let result = provider_call!(
            self,
            call_sensitive,
            OPERATION_MODERATION_QUARANTINE_UNWRAP_DEK_V1,
            payload,
            false,
        )
        .map_err(|error| Self::operation_error(error, false))?;
        let dek = self
            .session
            .decode_result::<ModerationQuarantineUnwrapDekResultWireV1>(&result)
            .map_err(|error| Self::operation_error(error, false))?
            .dek;
        if dek == [0; 32] {
            self.session.poison();
            return Err(sorafs_node::ModerationQuarantineKeyOperationErrorV1::Rejected);
        }
        Ok(dek)
    }
}
fn live_exact_qualification(
    session: &BrokerSession,
    binding: &ProviderBindingWireV1,
    metadata_digest: [u8; 32],
) -> Result<QualificationResultWireV1, BrokerError> {
    let payload = encode_canonical(&(), MAX_STREAM_TOKEN_FRAME_BYTES_V1)?;
    let result = session.call(
        binding,
        metadata_digest,
        OPERATION_QUALIFY_V1,
        payload,
        false,
    )?;
    let qualification = session.decode_result::<QualificationResultWireV1>(&result)?;
    if binding.revision != Some(qualification.revision)
        || binding.policy_digest != Some(qualification.policy_digest)
    {
        session.poison();
        return Err(BrokerError::StaleOrRevoked);
    }
    Ok(qualification)
}
fn map_stream_token_error(error: BrokerError) -> iroha_torii::sorafs::StreamTokenSigningError {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            iroha_torii::sorafs::StreamTokenSigningError::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => iroha_torii::sorafs::StreamTokenSigningError::Refused,
    }
}
fn map_stream_token_probe_error(
    error: BrokerError,
) -> iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1 {
    match error {
        BrokerError::Unavailable | BrokerError::Ambiguous => {
            iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::Unavailable
        }
        BrokerError::Rejected
        | BrokerError::Conflict
        | BrokerError::StaleOrRevoked
        | BrokerError::Protocol
        | BrokerError::BindingMismatch => {
            iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::StaleOrRevoked
        }
    }
}
#[derive(Clone)]
struct StreamTokenBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    public_key: [u8; 32],
}
impl iroha_torii::sorafs::StreamTokenRuntimeSigner for StreamTokenBrokerSigner {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenRuntimeSignerQualificationV1,
        iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1,
    > {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)
                .map_err(map_stream_token_probe_error)?;
        let qualification = iroha_torii::sorafs::StreamTokenRuntimeSignerQualificationV1::new(
            qualification.revision,
            qualification.policy_digest,
        );
        qualification.validate().map_err(|_| {
            self.session.poison();
            iroha_torii::sorafs::StreamTokenRuntimeSignerProbeErrorV1::StaleOrRevoked
        })?;
        Ok(qualification)
    }
    fn sign(
        &self,
        signing_payload: &[u8],
    ) -> Result<[u8; 64], iroha_torii::sorafs::StreamTokenSigningError> {
        validate_stream_token_signing_payload(signing_payload).map_err(map_stream_token_error)?;
        live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)
            .map_err(map_stream_token_error)?;
        let payload = encode_canonical(
            &SignRequestWireV1 {
                payload: signing_payload.to_vec(),
            },
            MAX_STREAM_TOKEN_FRAME_BYTES_V1,
        )
        .map_err(map_stream_token_error)?;
        let result = provider_call!(self, call, OPERATION_STREAM_TOKEN_SIGN_V1, payload, false,)
            .map_err(map_stream_token_error)?;
        let signature = self
            .session
            .decode_result::<SignResultWireV1>(&result)
            .map_err(map_stream_token_error)?
            .signature;
        live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)
            .map_err(map_stream_token_error)?;
        if verify_evidence_viewer_ed25519_signature(self.public_key, signature, signing_payload)
            .is_err()
        {
            self.session.poison();
            return Err(iroha_torii::sorafs::StreamTokenSigningError::Refused);
        }
        Ok(signature)
    }
}
#[derive(Clone)]
struct AppealFinanceBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    authority: iroha_data_model::account::AccountId,
    public_key: iroha_crypto::PublicKey,
}
impl AppealFinanceBrokerSigner {
    fn live_qualification(
        &self,
    ) -> Result<
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceRuntimeProviderQualificationV1,
        BrokerError,
    >{
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceRuntimeProviderQualificationV1::new(
                    qualification.revision,
                    qualification.policy_digest,
                ),
        )
    }
}
impl iroha_torii::SoraFsAppealFinanceTransactionSigner for AppealFinanceBrokerSigner {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn public_key(
        &self,
    ) -> Result<iroha_crypto::PublicKey, iroha_torii::SoraFsAppealFinanceSigningError> {
        self.live_qualification().map_err(|error| match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                iroha_torii::SoraFsAppealFinanceSigningError::Unavailable
            }
            _ => iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged,
        })?;
        Ok(self.public_key.clone())
    }
    fn qualification(
        &self,
    ) -> Result<
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceRuntimeProviderQualificationV1,
        iroha_torii::SoraFsAppealFinanceSigningError,
    >{
        self.live_qualification().map_err(|error| match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                iroha_torii::SoraFsAppealFinanceSigningError::Unavailable
            }
            _ => iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged,
        })
    }
    fn sign(
        &self,
        payload: iroha_data_model::transaction::TransactionPayload,
    ) -> Result<
        iroha_data_model::transaction::SignedTransaction,
        iroha_torii::SoraFsAppealFinanceSigningError,
    > {
        if payload.authority() != &self.authority
            || ensure_transaction_session_network(&payload, &self.session.network_id).is_err()
        {
            return Err(iroha_torii::SoraFsAppealFinanceSigningError::Refused);
        }
        self.live_qualification()
            .map_err(|_| iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged)?;
        let expected = payload.clone();
        let payload =
            encode_transaction_payload_bounded(&payload, MAX_APPEAL_FINANCE_TRANSACTION_BYTES_V1)
                .map_err(|_| iroha_torii::SoraFsAppealFinanceSigningError::Refused)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_APPEAL_FINANCE_TRANSACTION_SIGN_V1,
            payload,
            false,
        )
        .map_err(|error| match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                iroha_torii::SoraFsAppealFinanceSigningError::Unavailable
            }
            BrokerError::StaleOrRevoked | BrokerError::BindingMismatch => {
                self.session.poison();
                iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged
            }
            _ => iroha_torii::SoraFsAppealFinanceSigningError::Refused,
        })?;
        let signed = self
            .session
            .decode_result::<iroha_data_model::transaction::SignedTransaction>(&result)
            .map_err(|_| {
                self.session.poison();
                iroha_torii::SoraFsAppealFinanceSigningError::Refused
            })?;
        if signed.payload() != &expected
            || signed.authority() != &self.authority
            || signed.verify_signature().is_err()
        {
            self.session.poison();
            return Err(iroha_torii::SoraFsAppealFinanceSigningError::Refused);
        }
        self.live_qualification().map_err(|_| {
            self.session.poison();
            iroha_torii::SoraFsAppealFinanceSigningError::QualificationChanged
        })?;
        Ok(signed)
    }
}
#[derive(Clone)]
struct AppealFinanceBrokerCheckpoint {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    public_key: [u8; 32],
    checkpoint_max_bytes: u64,
}
impl_broker_debug_fields!(AppealFinanceBrokerCheckpoint as value {} => finish_non_exhaustive);
impl AppealFinanceBrokerCheckpoint {
    fn live_identity(
        &self,
    ) -> Result<
        sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntimeIdentityV1,
        BrokerError,
    > {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointRuntimeIdentityV1 {
                    provider_handle: self.binding.handle.clone(),
                    public_key: self.public_key,
                    qualification:
                        sorafs_node::appeal_finance_transaction_forwarder::
                            AppealFinanceRuntimeProviderQualificationV1::new(
                                qualification.revision,
                                qualification.policy_digest,
                            ),
                },
        )
    }
}
impl sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntime
    for AppealFinanceBrokerCheckpoint
{
    fn identity(
        &self,
    ) -> Result<
        sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntimeIdentityV1,
        sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointExternalError,
    > {
        self.live_identity().map_err(|error| {
            match error {
            BrokerError::Unavailable | BrokerError::Ambiguous => {
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Unavailable
            }
            _ => sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Rejected,
        }
        })
    }
    fn sign_digest(
        &self,
        digest: [u8; 32],
    ) -> Result<
        [u8; 64],
        sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointExternalError,
    > {
        if digest == [0; 32] {
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected,
            );
        }
        let payload = encode_canonical(&digest, MAX_STREAM_TOKEN_FRAME_BYTES_V1).map_err(|_| {
            sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected
        })?;
        let result = provider_call!(
            self,
            call,
            OPERATION_APPEAL_FINANCE_CHECKPOINT_SIGN_V1,
            payload,
            false,
        )
        .map_err(appeal_checkpoint_external_error)?;
        let signature = self
            .session
            .decode_result::<SignResultWireV1>(&result)
            .map_err(appeal_checkpoint_external_error)?
            .signature;
        if verify_evidence_viewer_ed25519_signature(self.public_key, signature, &digest).is_err() {
            self.session.poison();
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected,
            );
        }
        Ok(signature)
    }
    fn load_latest(
        &self,
    ) -> Result<
        Option<
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceSealedCheckpointRecordV1,
        >,
        sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError,
    >{
        let payload = encode_canonical(&(), MAX_STREAM_TOKEN_FRAME_BYTES_V1)
            .map_err(appeal_checkpoint_external_error)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_APPEAL_FINANCE_CHECKPOINT_LOAD_V1,
            payload,
            false,
        )
        .map_err(appeal_checkpoint_external_error)?;
        let record = self
            .session
            .decode_result::<Option<
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceSealedCheckpointRecordV1,
            >>(&result)
            .map_err(appeal_checkpoint_external_error)?;
        if record
            .as_ref()
            .is_some_and(|record| record.validate(self.checkpoint_max_bytes).is_err())
        {
            self.session.poison();
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected,
            );
        }
        Ok(record)
    }
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceSealedCheckpointRecordV1,
    ) -> Result<
        (),
        sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointExternalError,
    > {
        if expected_revision == Some([0; 32]) || next.validate(self.checkpoint_max_bytes).is_err() {
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Rejected,
            );
        }
        self.live_identity()
            .map_err(appeal_checkpoint_external_error)?;
        let current = self.load_latest()?;
        if current.as_ref().map(|record| record.revision) != expected_revision {
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected,
            );
        }
        let monotonic = current.as_ref().map_or_else(
            || expected_revision.is_none() && next.checkpoint_sequence == 1,
            |record| {
                record
                    .checkpoint_sequence
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == next.checkpoint_sequence)
            },
        );
        if !monotonic {
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected,
            );
        }
        let payload = encode_canonical(
            &AppealFinanceCheckpointCompareAndSwapWireV1 {
                expected_revision,
                next: next.clone(),
            },
            MAX_APPEAL_FINANCE_CHECKPOINT_FRAME_BYTES_V1,
        )
        .map_err(appeal_checkpoint_external_error)?;
        let result = provider_call!(
            self,
            call,
            OPERATION_APPEAL_FINANCE_CHECKPOINT_COMPARE_AND_SWAP_V1,
            payload,
            true,
        )
        .map_err(appeal_checkpoint_external_error)?;
        self.session
            .decode_result::<()>(&result)
            .map_err(appeal_checkpoint_external_error)?;
        let readback = self.load_latest().map_err(|error| {
            self.session.poison();
            match error {
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected => {
                        sorafs_node::appeal_finance_transaction_forwarder::
                            AppealFinanceCheckpointExternalError::Rejected
                    }
                _ => sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Ambiguous,
            }
        })?;
        if readback.as_ref() != Some(next) {
            self.session.poison();
            return Err(
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Ambiguous,
            );
        }
        self.live_identity().map_err(|error| {
            self.session.poison();
            match appeal_checkpoint_external_error(error) {
                sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Rejected => {
                        sorafs_node::appeal_finance_transaction_forwarder::
                            AppealFinanceCheckpointExternalError::Rejected
                    }
                _ => sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointExternalError::Ambiguous,
            }
        })?;
        Ok(())
    }
}
fn appeal_checkpoint_external_error(
    error: BrokerError,
) -> sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointExternalError {
    match error {
        BrokerError::Ambiguous | BrokerError::Conflict => {
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Ambiguous
        }
        BrokerError::Unavailable => {
            sorafs_node::appeal_finance_transaction_forwarder::
                AppealFinanceCheckpointExternalError::Unavailable
        }
        _ => sorafs_node::appeal_finance_transaction_forwarder::
            AppealFinanceCheckpointExternalError::Rejected,
    }
}
#[derive(Clone)]
struct PotrGatewayBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    public_key: [u8; 32],
    signer_id: [u8; 32],
}
#[derive(Clone)]
struct PotrProviderBrokerSigner {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    public_key: Vec<u8>,
    signer_id: [u8; 32],
    provider_id: [u8; 32],
}
fn potr_qualification(
    session: &BrokerSession,
    binding: &ProviderBindingWireV1,
    metadata_digest: [u8; 32],
) -> Result<iroha_torii::sorafs::PotrRuntimeProviderQualificationV1, BrokerError> {
    let qualification = live_exact_qualification(session, binding, metadata_digest)?;
    Ok(
        iroha_torii::sorafs::PotrRuntimeProviderQualificationV1::new(
            qualification.revision,
            qualification.policy_digest,
        ),
    )
}
fn potr_service_error(error: BrokerError) -> iroha_torii::sorafs::PotrSignerServiceError {
    if error == BrokerError::Unavailable {
        iroha_torii::sorafs::PotrSignerServiceError::Unavailable
    } else {
        iroha_torii::sorafs::PotrSignerServiceError::Refused
    }
}
fn potr_sign(
    session: &BrokerSession,
    binding: &ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    public_key: &[u8],
    payload: &[u8],
    role: &'static str,
    algorithm: sorafs_manifest::potr::PotrSignatureAlgorithm,
) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
    let runtime = binding
        .potr_runtime_binding
        .as_ref()
        .ok_or(iroha_torii::sorafs::PotrSignerServiceError::Refused)?;
    validate_potr_signing_payload(payload, runtime.baseline_admission_policy.provider_id)
        .map_err(potr_service_error)?;
    let request = encode_canonical(
        &PotrSignRequestWireV1 {
            payload: payload.to_vec(),
            expected_public_key: public_key.to_vec(),
        },
        MAX_POTR_FRAME_BYTES_V1,
    )
    .map_err(potr_service_error)?;
    let result = session
        .call(
            binding,
            metadata_digest,
            OPERATION_POTR_SIGN_V1,
            request,
            false,
        )
        .map_err(potr_service_error)?;
    let signature = session
        .decode_result::<VariableSignatureResultWireV1>(&result)
        .map_err(potr_service_error)?
        .signature
        .clone();
    if (sorafs_manifest::potr::PotrSignatureV1 {
        algorithm,
        public_key: public_key.to_vec(),
        signature: signature.clone(),
    })
    .verify(role, payload)
    .is_err()
    {
        session.poison();
        return Err(iroha_torii::sorafs::PotrSignerServiceError::Refused);
    }
    Ok(signature)
}
impl iroha_torii::sorafs::PotrGatewaySignerV1 for PotrGatewayBrokerSigner {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn signer_id(&self) -> [u8; 32] {
        self.signer_id
    }
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::PotrRuntimeProviderQualificationV1,
        iroha_torii::sorafs::PotrSignerServiceError,
    > {
        potr_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)
            .map_err(potr_service_error)
    }
    fn public_key(&self) -> Result<[u8; 32], iroha_torii::sorafs::PotrSignerServiceError> {
        self.qualification()?;
        Ok(self.public_key)
    }
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
        potr_sign(
            self.session.as_ref(),
            &self.binding,
            self.metadata_digest,
            &self.public_key,
            payload,
            "gateway",
            sorafs_manifest::potr::PotrSignatureAlgorithm::Ed25519,
        )
    }
}
impl iroha_torii::sorafs::PotrProviderSignerV1 for PotrProviderBrokerSigner {
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn signer_id(&self) -> [u8; 32] {
        self.signer_id
    }
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::PotrRuntimeProviderQualificationV1,
        iroha_torii::sorafs::PotrSignerServiceError,
    > {
        potr_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)
            .map_err(potr_service_error)
    }
    fn provider_id(&self) -> Result<[u8; 32], iroha_torii::sorafs::PotrSignerServiceError> {
        self.qualification()?;
        Ok(self.provider_id)
    }
    fn public_key(&self) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
        self.qualification()?;
        Ok(self.public_key.clone())
    }
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, iroha_torii::sorafs::PotrSignerServiceError> {
        potr_sign(
            self.session.as_ref(),
            &self.binding,
            self.metadata_digest,
            &self.public_key,
            payload,
            "provider",
            sorafs_manifest::potr::PotrSignatureAlgorithm::MlDsa65,
        )
    }
}
#[derive(Clone)]
struct PopBrokerProvider {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
}
impl_broker_debug_fields!(PopBrokerProvider as value {
    "handle" => value.binding.handle,
    "private_provider" => "[REDACTED]",
} => finish_non_exhaustive);
impl PopBrokerProvider {
    fn live_qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderQualificationV1,
        BrokerError,
    > {
        let qualification =
            live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)?;
        Ok(
            iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderQualificationV1::new(
                qualification.revision,
                qualification.policy_digest,
            ),
        )
    }
    fn call(
        &self,
        operation: u16,
        payload: Vec<u8>,
        mutating: bool,
    ) -> Result<ScrubbedBytes, BrokerError> {
        self.live_qualification()?;
        let result = provider_call!(
            self,
            call_sensitive,
            operation,
            ScrubbedBytes::new(payload),
            mutating,
        );
        if matches!(
            &result,
            Err(BrokerError::Unavailable
                | BrokerError::Ambiguous
                | BrokerError::StaleOrRevoked
                | BrokerError::Protocol
                | BrokerError::BindingMismatch)
        ) {
            return result;
        }
        if self.live_qualification().is_err() {
            self.session.poison();
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::StaleOrRevoked
            });
        }
        result
    }
    fn decode<T>(&self, bytes: &ScrubbedBytes, max_bytes: usize) -> Result<T, BrokerError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        let _scope = bytes.enter_decode_admission();
        decode_canonical::<T>(bytes, max_bytes).inspect_err(|_| {
            self.session.poison();
        })
    }
    fn redacted_string_error(&self, error: BrokerError) -> String {
        if matches!(
            error,
            BrokerError::Protocol
                | BrokerError::BindingMismatch
                | BrokerError::StaleOrRevoked
                | BrokerError::Ambiguous
        ) {
            self.session.poison();
        }
        "PoP runtime provider unavailable".to_owned()
    }
}
#[derive(Clone)]
struct PopCredentialBrokerRegistry {
    provider: PopBrokerProvider,
}
impl_broker_debug_fields!(PopCredentialBrokerRegistry as value {
    "handle" => value.provider.binding.handle,
    "private_registry" => "[REDACTED]",
} => finish_non_exhaustive);
impl iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1
    for PopCredentialBrokerRegistry
{
    fn handle(&self) -> &str {
        &self.provider.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderQualificationV1,
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1,
    > {
        self.provider
            .live_qualification()
            .map_err(pop_registry_error)
    }
    fn resolve(
        &self,
        bindings: &iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderBindingsV1,
    ) -> Result<
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProvidersV1,
        iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1,
    > {
        let exact = self
            .provider
            .binding
            .pop_credential_runtime_binding
            .as_ref()
            .ok_or(
                iroha_torii::sorafs::pop_api::
                    PopCredentialRuntimeProviderRegistryErrorV1::RejectedBindings,
            )?;
        if !pop_exact_bindings_match(bindings, exact) {
            return Err(
                iroha_torii::sorafs::pop_api::
                    PopCredentialRuntimeProviderRegistryErrorV1::RejectedBindings,
            );
        }
        let payload =
            encode_canonical(exact, MAX_POP_RUNTIME_FRAME_BYTES_V1).map_err(pop_registry_error)?;
        let result = self
            .provider
            .call(OPERATION_POP_RUNTIME_OPEN_V1, payload, true)
            .map_err(pop_registry_error)?;
        let outcome = self
            .provider
            .decode::<PopRuntimeOpenResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
            .map_err(pop_registry_error)?;
        validate_pop_open_result(&outcome, exact).map_err(pop_registry_error)?;
        Ok(
            iroha_torii::sorafs::pop_api::PopCredentialRuntimeProvidersV1 {
                enrollment_recipient: Arc::new(PopBrokerEnrollmentRecipient {
                    provider: self.provider.clone(),
                    key_id: exact.enrollment_recipient_key_id.clone(),
                    public_key_digest: exact.enrollment_recipient_public_key_digest,
                }),
                issuer_signer: Arc::new(PopBrokerIssuerSigner {
                    provider: self.provider.clone(),
                    key_id: exact.issuer_signer_handle.clone(),
                    public_key: exact.issuer_public_key,
                }),
                authenticator: Arc::new(PopBrokerAuthenticator {
                    provider: self.provider.clone(),
                }),
                registry_submitter: Arc::new(PopBrokerRegistrySubmitter {
                    provider: self.provider.clone(),
                }),
                registry_reader: Arc::new(PopBrokerRegistryReader {
                    provider: self.provider.clone(),
                }),
                issuance_draft_provider: Arc::new(PopBrokerIssuanceDraftProvider {
                    provider: self.provider.clone(),
                }),
                wallet_recipient: Arc::new(PopBrokerWalletRecipient {
                    provider: self.provider.clone(),
                    key_id: exact.wallet_recipient_key_id.clone(),
                    public_key_digest: exact.wallet_recipient_public_key_digest,
                }),
                wallet_key_wrapper: Arc::new(PopBrokerWalletKeyWrapper {
                    provider: self.provider.clone(),
                    active_key_id: exact.wallet_wrapping_key_id.clone(),
                }),
                wallet_witness_provider: Arc::new(PopBrokerWalletWitnessProvider {
                    provider: self.provider.clone(),
                }),
                finalized_time_provider: Arc::new(PopBrokerFinalizedTimeProvider {
                    provider: self.provider.clone(),
                }),
            },
        )
    }
}
