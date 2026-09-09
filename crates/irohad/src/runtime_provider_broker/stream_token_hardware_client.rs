// Actual bounded hardware and observer transports. Metadata confirms routing only; the issuer
// separately verifies independent challenged custody/completion evidence before releasing tokens.
use iroha_torii::sorafs::{StreamTokenHardwareClientV1, StreamTokenStateObserverClientV1};

#[derive(Clone, Copy)]
#[expect(
    variant_size_differences,
    reason = "one fixed 32-byte operation ID keeps post-dispatch fencing allocation-free"
)]
enum StreamTokenSignLatchV1 {
    Idle,
    Ambiguous([u8; 32]),
    Terminal(BrokerError),
}

#[derive(Clone)]
struct StreamTokenHardwareBrokerClient {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    // One serialized in-flight Sign lifecycle, never an unbounded pending-operation map. An
    // ambiguous operation permanently fences this client even after its read-only recovery.
    latch: Arc<Mutex<StreamTokenSignLatchV1>>,
}

impl StreamTokenHardwareBrokerClient {
    fn metadata(&self) -> Result<(), BrokerError> {
        let result = self.session.call(
            &self.binding,
            self.metadata_digest,
            OPERATION_QUALIFY_V1,
            encode_canonical(&(), MAX_STREAM_TOKEN_METADATA_BYTES_V1)?,
            false,
        )?;
        let _scope = result.enter_decode_admission();
        validate_stream_token_metadata_result(&self.binding, &result)
            .inspect_err(|error| self.session.poison_with_reason(*error))
    }

    fn prepared_payload(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &sorafs_manifest::StreamTokenBodyV1,
    ) -> Result<ScrubbedBytes, StreamTokenHardwareCallErrorV1> {
        let binding = self
            .binding
            .stream_token_hardware_binding
            .as_ref()
            .ok_or(StreamTokenHardwareCallErrorV1::Refused)?;
        let derived = SignerStreamTokenExpectedV1::new(body, binding.custody())
            .map_err(|_| StreamTokenHardwareCallErrorV1::Refused)?;
        if &derived != expected {
            return Err(StreamTokenHardwareCallErrorV1::Refused);
        }
        body.signing_payload_bytes()
            .map(ScrubbedBytes::new)
            .map_err(|_| StreamTokenHardwareCallErrorV1::Refused)
    }

    fn fence_after_sign(
        &self,
        latch: &mut StreamTokenSignLatchV1,
        operation: [u8; 32],
        error: BrokerError,
    ) -> StreamTokenHardwareCallErrorV1 {
        if matches!(error, BrokerError::Unavailable | BrokerError::Ambiguous) {
            self.session.poison_with_reason(BrokerError::Ambiguous);
            *latch = StreamTokenSignLatchV1::Ambiguous(operation);
            StreamTokenHardwareCallErrorV1::AmbiguousCompletion
        } else {
            self.session.poison_with_reason(error);
            *latch = StreamTokenSignLatchV1::Terminal(error);
            stream_token_transport_error(error)
        }
    }
}

impl StreamTokenHardwareClientV1 for StreamTokenHardwareBrokerClient {
    fn handle(&self) -> &str {
        &self.binding.handle
    }

    fn sign(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &sorafs_manifest::StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, StreamTokenHardwareCallErrorV1> {
        let payload = self.prepared_payload(expected, body)?;
        let mut latch = self
            .latch
            .lock()
            .map_err(|_| StreamTokenHardwareCallErrorV1::Unavailable)?;
        match *latch {
            StreamTokenSignLatchV1::Idle => {}
            StreamTokenSignLatchV1::Ambiguous(_) => {
                return Err(StreamTokenHardwareCallErrorV1::AmbiguousCompletion);
            }
            StreamTokenSignLatchV1::Terminal(error) => {
                return Err(stream_token_transport_error(error));
            }
        }
        if let Err(error) = self.metadata() {
            if error != BrokerError::Unavailable {
                *latch = StreamTokenSignLatchV1::Terminal(error);
            }
            return Err(stream_token_transport_error(error));
        }
        let receipt = match self.session.call_sensitive(
            &self.binding,
            self.metadata_digest,
            OPERATION_STREAM_TOKEN_SIGN_V1,
            payload,
            true,
        ) {
            Ok(result) => result,
            Err(error) => {
                return Err(self.fence_after_sign(&mut latch, expected.operation_id(), error));
            }
        };
        if let Err(error) = self.metadata() {
            return Err(self.fence_after_sign(&mut latch, expected.operation_id(), error));
        }
        StreamTokenHardwareReceiptV1::new(receipt.to_vec()).map_err(|_| {
            self.fence_after_sign(&mut latch, expected.operation_id(), BrokerError::Ambiguous)
        })
    }

    fn recover(
        &self,
        expected: &SignerStreamTokenExpectedV1,
        body: &sorafs_manifest::StreamTokenBodyV1,
    ) -> Result<StreamTokenHardwareReceiptV1, StreamTokenHardwareCallErrorV1> {
        let deadline =
            BrokerDeadlineV1::new(BROKER_IO_TIMEOUT_V1).map_err(stream_token_transport_error)?;
        let payload = self.prepared_payload(expected, body)?;
        let latch = deadline
            .lock(&self.latch)
            .map_err(stream_token_transport_error)?;
        if !matches!(*latch, StreamTokenSignLatchV1::Ambiguous(operation) if operation == expected.operation_id())
        {
            return Err(StreamTokenHardwareCallErrorV1::Refused);
        }
        // This temporary independently authenticated connection executes only the exact read.
        // It never replaces the signed session, resets its poison, or makes another Sign.
        let session =
            stream_token_read_session(&self.session, &self.binding, self.metadata_digest, deadline)
                .map_err(stream_token_transport_error)?;
        let receipt = session
            .call_before(
                &self.binding,
                self.metadata_digest,
                OPERATION_STREAM_TOKEN_RECOVER_V1,
                payload,
                false,
                deadline,
            )
            .map_err(stream_token_transport_error)?;
        let receipt = StreamTokenHardwareReceiptV1::new(receipt.to_vec())?;
        deadline.remaining().map_err(stream_token_transport_error)?;
        Ok(receipt)
    }
}

#[derive(Clone)]
struct StreamTokenObserverBrokerClient {
    session: Arc<BrokerSession>,
    binding: ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    observer_handle: String,
}

impl StreamTokenStateObserverClientV1 for StreamTokenObserverBrokerClient {
    fn handle(&self) -> &str {
        &self.observer_handle
    }

    fn observe(
        &self,
        request: &SignerStreamTokenObservationRequestV1,
    ) -> Result<StreamTokenObserverReplyV1, StreamTokenHardwareCallErrorV1> {
        let deadline =
            BrokerDeadlineV1::new(BROKER_IO_TIMEOUT_V1).map_err(stream_token_transport_error)?;
        let payload = request
            .encode_canonical()
            .map_err(|_| StreamTokenHardwareCallErrorV1::Refused)?;
        decode_stream_token_observer_request(&self.binding, &payload)
            .map_err(stream_token_transport_error)?;
        // The observer remains separately routed and usable after the Sign connection is
        // poisoned. Its signed evidence never obtains authority from this shared endpoint.
        let session =
            stream_token_read_session(&self.session, &self.binding, self.metadata_digest, deadline)
                .map_err(stream_token_transport_error)?;
        let result = session
            .call_before(
                &self.binding,
                self.metadata_digest,
                OPERATION_STREAM_TOKEN_OBSERVE_V1,
                ScrubbedBytes::new(payload.clone()),
                false,
                deadline,
            )
            .map_err(stream_token_transport_error)?;
        // Typed result validation already ran under the result-owned decode admission. This
        // second conversion only transfers the bounded untrusted wire leaves to Torii's owner.
        let _scope = result.enter_decode_admission();
        let reply = decode_stream_token_observer_reply(&self.binding, &payload, &result)
            .map_err(stream_token_transport_error)?;
        deadline.remaining().map_err(stream_token_transport_error)?;
        Ok(reply)
    }
}

fn stream_token_read_session(
    original: &BrokerSession,
    binding: &ProviderBindingWireV1,
    metadata_digest: [u8; 32],
    deadline: BrokerDeadlineV1,
) -> Result<Arc<BrokerSession>, BrokerError> {
    let (session, observations) = BrokerSession::connect_before(
        &original.endpoint,
        &original.chain_id,
        original.network_id,
        vec![binding.clone()],
        deadline,
    )?;
    if observations.len() != 1
        || observations[0].binding != *binding
        || observations[0].metadata_digest != metadata_digest
    {
        session.poison();
        return Err(BrokerError::StaleOrRevoked);
    }
    Ok(session)
}
