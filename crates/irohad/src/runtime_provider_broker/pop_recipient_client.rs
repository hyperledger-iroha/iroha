fn pop_registry_error(
    error: BrokerError,
) -> iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1 {
    use iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryErrorV1 as Error;
    match error {
        BrokerError::StaleOrRevoked => Error::StaleOrRevoked,
        BrokerError::Rejected | BrokerError::BindingMismatch | BrokerError::Conflict => {
            Error::RejectedBindings
        }
        BrokerError::Unavailable | BrokerError::Ambiguous | BrokerError::Protocol => {
            Error::Unavailable
        }
    }
}

fn pop_exact_bindings_match(
    supplied: &iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderBindingsV1,
    exact: &PopCredentialRuntimeBindingWireV1,
) -> bool {
    supplied.issuer_policy_digest() == exact.issuer_policy_digest
        && supplied.issuer_id() == exact.issuer_id
        && supplied.issuer_signer_handle() == exact.issuer_signer_handle
        && supplied.issuer_public_key() == exact.issuer_public_key
        && supplied.enrollment_recipient_key_id() == exact.enrollment_recipient_key_id
        && supplied.enrollment_recipient_public_key_digest()
            == exact.enrollment_recipient_public_key_digest
        && supplied.wallet_recipient_key_id() == exact.wallet_recipient_key_id
        && supplied.wallet_recipient_public_key_digest() == exact.wallet_recipient_public_key_digest
        && supplied.wallet_wrapping_key_id() == exact.wallet_wrapping_key_id
}

#[derive(Clone)]
struct PopBrokerEnrollmentRecipient {
    provider: PopBrokerProvider,
    key_id: String,
    public_key_digest: [u8; 32],
}

impl fmt::Debug for PopBrokerEnrollmentRecipient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopBrokerEnrollmentRecipient")
            .field("key_id", &self.key_id)
            .field("private_recipient", &"[REMOTE]")
            .finish_non_exhaustive()
    }
}

impl sorafs_node::pop_credentials::PopEnrollmentRecipientV1 for PopBrokerEnrollmentRecipient {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn public_key_digest(&self) -> [u8; 32] {
        self.public_key_digest
    }

    fn open_enrollment(
        &self,
        encrypted_payload: &sorafs_manifest::hybrid_envelope::HybridPayloadEnvelopeV1,
        aad: &[u8],
    ) -> Result<Vec<u8>, sorafs_node::pop_credentials::PopRecipientOpenErrorV1> {
        open_pop_recipient(
            &self.provider,
            OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1,
            encrypted_payload,
            aad,
        )
    }
}

#[derive(Clone)]
struct PopBrokerWalletRecipient {
    provider: PopBrokerProvider,
    key_id: String,
    public_key_digest: [u8; 32],
}

impl fmt::Debug for PopBrokerWalletRecipient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PopBrokerWalletRecipient")
            .field("key_id", &self.key_id)
            .field("private_recipient", &"[REMOTE]")
            .finish_non_exhaustive()
    }
}

impl sorafs_node::pop_credentials::PopWalletRecipientV1 for PopBrokerWalletRecipient {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn public_key_digest(&self) -> [u8; 32] {
        self.public_key_digest
    }

    fn open_wallet_delivery(
        &self,
        encrypted_payload: &sorafs_manifest::hybrid_envelope::HybridPayloadEnvelopeV1,
        aad: &[u8],
    ) -> Result<Vec<u8>, sorafs_node::pop_credentials::PopRecipientOpenErrorV1> {
        open_pop_recipient(
            &self.provider,
            OPERATION_POP_WALLET_RECIPIENT_OPEN_V1,
            encrypted_payload,
            aad,
        )
    }
}

fn open_pop_recipient(
    provider: &PopBrokerProvider,
    operation: u16,
    encrypted_payload: &sorafs_manifest::hybrid_envelope::HybridPayloadEnvelopeV1,
    aad: &[u8],
) -> Result<Vec<u8>, sorafs_node::pop_credentials::PopRecipientOpenErrorV1> {
    use sorafs_node::pop_credentials::PopRecipientOpenErrorV1 as Error;

    if !matches!(
        operation,
        OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1 | OPERATION_POP_WALLET_RECIPIENT_OPEN_V1
    ) {
        return Err(Error::Rejected);
    }
    let wire = PopRecipientOpenRequestWireV1 {
        encrypted_payload: encrypted_payload.clone(),
        aad: aad.to_vec(),
    };
    validate_pop_recipient_open_request(&wire, operation).map_err(|_| Error::Rejected)?;
    let payload =
        encode_canonical(&wire, MAX_POP_RUNTIME_FRAME_BYTES_V1).map_err(|_| Error::Unavailable)?;
    let result = provider
        .call(operation, payload, false)
        .map_err(|error| match error {
            BrokerError::Rejected => Error::Rejected,
            BrokerError::Unavailable
            | BrokerError::StaleOrRevoked
            | BrokerError::Protocol
            | BrokerError::BindingMismatch
            | BrokerError::Conflict
            | BrokerError::Ambiguous => Error::Unavailable,
        })?;
    let mut opened = provider
        .decode::<PopRecipientOpenResultWireV1>(&result, MAX_POP_RUNTIME_FRAME_BYTES_V1)
        .map_err(|_| Error::Unavailable)?;
    if validate_pop_recipient_open_result(&opened, operation).is_err() {
        provider.session.poison();
        return Err(Error::Unavailable);
    }
    Ok(opened.take_plaintext())
}
