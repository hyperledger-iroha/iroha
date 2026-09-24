// Strict activation-evidence readers kept out of the main client module's source budget.

use crate::data_model::block::{
    decode_framed_signed_block, proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
};
use iroha_data_model::{
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityAttestationV1, BridgeFinalityProof,
        BridgeFinalityVerifier, verify_bridge_finality_proof,
    },
    query::CommittedTransaction,
};

const BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;
// A strict envelope cap, not a promise to accept two maximum-size independent proofs.
const GENESIS_FINALITY_ATTESTATION_RESPONSE_MAX_BYTES: usize = 16 * 1024 * 1024;
const GENESIS_FINALITY_CHALLENGE_HEADER: &str = "x-iroha-finality-challenge";

#[derive(Clone, Copy)]
enum ActivationEvidenceReadAuth {
    Public,
    Account,
}

/// One authenticated-ready result or an unsigned non-success observation.
#[derive(Debug)]
#[expect(
    variant_size_differences,
    reason = "The attestation is already boxed; keep the small failure reason allocation-free."
)]
pub enum GenesisFinalityReadiness {
    /// The original genesis and node independently authenticate this fresh statement.
    Ready(Box<iroha_data_model::bridge::BridgeFinalityAttestationV1>),
    /// A closed failure reason. Only explicit startup reasons permit bounded retries.
    NotReady(iroha_torii_shared::bridge_attestation::FinalityAttestationFailureReason),
}

/// A request-bound observation that the selected finality tip is still changing.
///
/// This is progress information, not a finality proof. A bounded caller may take a
/// fresh snapshot; unrelated HTTP errors and invalid attestations never produce it.
#[derive(Debug, Clone)]
pub struct BridgeFinalityAttestationTipMismatch {
    response: iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1,
}

impl BridgeFinalityAttestationTipMismatch {
    /// Validate progress against the exact request and independently selected identity.
    ///
    /// # Errors
    /// Rejects malformed progress or a different height, challenge, node, or network.
    pub fn from_response(
        response: iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1,
        height: NonZeroU64,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
        network: NetworkId,
    ) -> Result<Self> {
        if !response.matches(height.get(), challenge, expected_node, network) {
            return Err(eyre!(
                "finality tip progress differs from exact request bindings"
            ));
        }
        Ok(Self { response })
    }

    /// Return the exact validated request and observed height bindings.
    #[must_use]
    pub const fn response(
        &self,
    ) -> &iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
        &self.response
    }
}

impl std::fmt::Display for BridgeFinalityAttestationTipMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "finality tip is changing: requested {}, applied {}, consensus status {}",
            self.response.requested_height,
            self.response.applied_height,
            self.response.status_height
        )
    }
}

impl std::error::Error for BridgeFinalityAttestationTipMismatch {}

impl Client {
    /// Fetch a canonical challenge-bound statement for an exact durable tip.
    ///
    /// With an explicit request deadline, HTTP 429 reads honor Retry-After within that
    /// original budget. Other response or verification failures are never retried.
    /// Verifies the reporting node signature and request bindings. Callers must
    /// independently anchor and verify both embedded finality proofs before
    /// treating this statement as chain finality.
    ///
    /// # Errors
    /// Returns a downcastable [`BridgeFinalityAttestationTipMismatch`] only for the
    /// canonical HTTP 409 progress envelope bound to this exact request.
    /// Rejects transport/codec failures, zero challenges, wrong node/network/height,
    /// and inconsistent or invalid node signatures.
    pub fn get_bridge_finality_attestation(
        &self,
        height: NonZeroU64,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
    ) -> Result<iroha_data_model::bridge::BridgeFinalityAttestationV1> {
        if challenge == [0; 32] {
            return Err(eyre!("finality challenge must be nonzero"));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
            .path()
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES,
            Some(challenge),
            ActivationEvidenceReadAuth::Public,
        )?;
        if response.status() != StatusCode::OK {
            let failure = Self::decode_finality_attestation_failure(
                &response,
                height.get(),
                challenge,
                expected_node,
                self.network_id,
            )?;
            if let Some(progress) = failure.tip_mismatch {
                return Err(BridgeFinalityAttestationTipMismatch::from_response(
                    progress,
                    height,
                    challenge,
                    expected_node,
                    self.network_id,
                )?
                .into());
            }
            return Err(eyre!(
                "finality attestation unavailable: {}",
                failure.reason.as_str()
            ));
        }
        let attestation: iroha_data_model::bridge::BridgeFinalityAttestationV1 =
            Self::decode_canonical_norito_response(
                &response,
                BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES,
                "Failed to get finality attestation",
            )?;
        attestation
            .verify()
            .map_err(|error| eyre!("invalid finality attestation: {error}"))?;
        if attestation.body.challenge != challenge
            || attestation.body.node_id != *expected_node
            || attestation.body.network_id != self.network_id
            || attestation.body.status.last_committed_height != height.get()
        {
            return Err(eyre!(
                "finality attestation differs from exact request bindings"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(attestation)
    }

    /// Decode the sole bounded failure schema with exact HTTP and request bindings.
    fn decode_finality_attestation_failure(
        response: &Response<Vec<u8>>,
        height: u64,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
        network_id: NetworkId,
    ) -> Result<iroha_torii_shared::bridge_attestation::FinalityAttestationFailure> {
        use iroha_torii_shared::bridge_attestation::{
            FINALITY_ATTESTATION_FAILURE_CODE, FINALITY_ATTESTATION_FAILURE_MAX_BYTES,
        };
        let status = response.status();
        if !matches!(
            status,
            StatusCode::CONFLICT
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::INTERNAL_SERVER_ERROR
        ) {
            return Err(eyre!(
                "unexpected finality attestation HTTP status {status}"
            ));
        }
        let content_type = exact_single_response_header(response, "content-type")?;
        if !content_type.eq_ignore_ascii_case(APPLICATION_NORITO) {
            return Err(eyre!(
                "finality failure requires exact application/x-norito"
            ));
        }
        let bytes = Self::bounded_norito_response_body(
            response,
            status,
            FINALITY_ATTESTATION_FAILURE_MAX_BYTES,
            "Failed to get finality attestation",
        )?;
        let envelope: iroha_torii_shared::ErrorEnvelope = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .wrap_err("failed to decode canonical finality failure envelope")?;
        if envelope.code() != FINALITY_ATTESTATION_FAILURE_CODE {
            return Err(eyre!(
                "finality error code does not establish a typed observation"
            ));
        }
        let mut details = envelope
            .details
            .ok_or_else(|| eyre!("finality failure omitted its detail"))?;
        let failure = details
            .finality_attestation_failure
            .take()
            .ok_or_else(|| eyre!("finality failure omitted its exact request bindings"))?;
        if !details.is_empty() {
            return Err(eyre!("finality failure carries conflicting error details"));
        }
        if failure.reason.http_status_code() != status.as_u16()
            || !failure.matches(height, challenge, expected_node, network_id)
        {
            return Err(eyre!(
                "finality failure differs from exact request/status bindings"
            ));
        }
        Ok(failure)
    }

    fn bounded_norito_response_body<'a>(
        response: &'a Response<Vec<u8>>,
        expected_status: StatusCode,
        maximum: usize,
        context: &'static str,
    ) -> Result<&'a [u8]> {
        if response.body().len() > maximum {
            return Err(eyre!(
                "{context}: response exceeds the {maximum}-byte limit"
            ));
        }
        if response.status() != expected_status {
            return Err(ResponseReport::with_msg(context, response)
                .unwrap_or_else(core::convert::identity)
                .into());
        }
        let content_type_values = response.headers().get_all(http::header::CONTENT_TYPE);
        let mut content_types = content_type_values.iter();
        let content_type = content_types
            .next()
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if content_types.next().is_some() {
            return Err(eyre!(
                "{context}: response carries multiple Content-Type headers"
            ));
        }
        if !Self::is_norito_content_type(content_type) {
            return Err(eyre!(
                "{context}: invalid content-type `{content_type}` (expected application/x-norito)"
            ));
        }
        if response.body().is_empty() {
            return Err(eyre!("{context}: response body is empty"));
        }
        Ok(response.body())
    }

    /// Decode one bounded, exact canonical Norito success response.
    pub(crate) fn decode_canonical_norito_response<T>(
        response: &Response<Vec<u8>>,
        maximum: usize,
        context: &'static str,
    ) -> Result<T>
    where
        T: norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let body = Self::bounded_norito_response_body(response, StatusCode::OK, maximum, context)?;
        norito::decode_canonical_with_limits(body, norito::canonical_decode_limits(body.len()))
            .map_err(|error| eyre!("{context}: failed to decode canonical Norito payload: {error}"))
    }

    fn canonical_norito_get_request(
        &self,
        path: &str,
        maximum: usize,
        auth: ActivationEvidenceReadAuth,
    ) -> Result<DefaultRequestBuilder> {
        let url = join_torii_url(&self.torii_url, path);
        let mut headers = match auth {
            ActivationEvidenceReadAuth::Public => self.headers_without_canonical_account_auth(),
            ActivationEvidenceReadAuth::Account => {
                self.account_signed_headers(&HttpMethod::GET, &url, &[])?
            }
        };
        headers.retain(|name, _| {
            !name.eq_ignore_ascii_case("accept")
                && !name.eq_ignore_ascii_case("content-type")
                && !name.eq_ignore_ascii_case("x-iroha-finality-challenge")
        });
        let mut builder = DefaultRequestBuilder::new(HttpMethod::GET, url)
            .with_transport(self.http_transport.clone())
            .headers(headers)
            .header("Accept", APPLICATION_NORITO)
            .max_response_bytes(maximum);
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        Ok(builder)
    }

    // Only an explicit operation deadline authorizes repeated reads. A one-shot
    // client still exposes backpressure immediately. Never retry transport,
    // authentication, codec or proof failures, and never resend a transaction.
    fn send_activation_evidence_read(
        &self,
        path: &str,
        maximum: usize,
        challenge: Option<[u8; 32]>,
        auth: ActivationEvidenceReadAuth,
    ) -> Result<Response<Vec<u8>>> {
        loop {
            self.ensure_activation_evidence_deadline()?;
            // Account authentication is constructed inside the retry loop: a 429
            // retry must carry a fresh nonce for the same exact-network GET.
            let mut request = self.canonical_norito_get_request(path, maximum, auth)?;
            if let Some(challenge) = challenge {
                request = request.header("X-Iroha-Finality-Challenge", &hex::encode(challenge));
            }
            let response = self.send_builder(request)?;
            if response.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(response);
            }
            let Some(deadline) = self.http_transport.deadline() else {
                return Ok(response);
            };
            let minimum_delay = Duration::from_millis(100);
            let delay = transaction_wait::retry_after_delay(&response)?
                .unwrap_or(minimum_delay)
                .max(minimum_delay);
            if deadline.saturating_duration_since(std::time::Instant::now()) <= delay {
                return Err(eyre!(
                    "activation evidence deadline cannot accommodate HTTP 429 Retry-After"
                ));
            }
            std::thread::sleep(delay);
        }
    }

    fn ensure_activation_evidence_deadline(&self) -> Result<()> {
        if self
            .http_transport
            .deadline()
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(eyre!("activation evidence deadline elapsed"));
        }
        Ok(())
    }

    // Verify on a candidate so CPU work that exceeds the caller's deadline cannot
    // advance its retained chain anchor. The original deadline is never renewed.
    fn verify_activation_evidence_successor(
        &self,
        proof: &BridgeFinalityProof,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<()> {
        self.ensure_activation_evidence_deadline()?;
        let mut candidate = verifier.clone();
        candidate
            .verify(proof)
            .map_err(|error| eyre!("bridge finality proof verification failed: {error}"))?;
        self.ensure_activation_evidence_deadline()?;
        *verifier = candidate;
        Ok(())
    }

    /// Fetch canonical block wire bound to an independently authenticated execution commitment.
    ///
    /// The returned bytes are accepted only when the route yields bounded Norito, the block
    /// round-trips to the byte-identical canonical [`SignedBlock`] wire, its requested height and
    /// block hash match, its proposal inputs and execution context match the header commitments, its full typed output
    /// cache is consistent, and the supplied successful transaction verifies through its exact
    /// Network input-index join and separate input/output proofs.
    ///
    /// The required commitment must come from an independently verified, externally anchored
    /// native finality proof for this carrier. Its exact wire hash and length authenticate results
    /// and internal invocation outputs, which the consensus header hash alone does not bind. This reader verifies
    /// that binding; it does not establish finality or trust in a caller-supplied commitment.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, status, media-type, size, decode, canonicality, height,
    /// hash, result-shape, Merkle-cache, execution-commitment, transaction-result, or inclusion-proof
    /// failures.
    pub fn get_canonical_executed_block_wire(
        &self,
        height: NonZeroU64,
        committed: &CommittedTransaction,
        execution_commitment: &iroha_data_model::block::consensus_v2::ExecutionCommitment,
    ) -> Result<Vec<u8>> {
        self.ensure_data_model_compatibility()?;
        let path =
            torii_uri::LEDGER_EXECUTED_BLOCK_WIRE.replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            None,
            ActivationEvidenceReadAuth::Account,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
            "Failed to get canonical executed block wire",
        )?;
        let block = norito::core::with_decode_limits_scope(
            norito::canonical_decode_limits(body.len()),
            || decode_framed_signed_block(body),
        )
        .map_err(|error| eyre!("Failed to decode canonical executed block wire: {error}"))?;
        let canonical = block
            .encode_wire()
            .map_err(|error| eyre!("Failed to re-encode canonical executed block wire: {error}"))?;
        if canonical.as_slice() != body {
            return Err(eyre!(
                "executed block response is not the exact canonical SignedBlock wire"
            ));
        }
        if block.header().height() != height {
            return Err(eyre!(
                "executed block height {} does not match requested height {height}",
                block.header().height()
            ));
        }
        let block_hash = block.hash();
        if &block_hash != committed.block_hash() {
            return Err(eyre!(
                "executed block hash does not match the committed transaction carrier hash"
            ));
        }
        if !block.has_results() {
            return Err(eyre!("executed block response has no execution results"));
        }
        block
            .validate_proposal_commitments()
            .map_err(|error| eyre!("executed block proposal commitments are invalid: {error}"))?;
        block
            .validate_output_merkle_cache()
            .map_err(|error| eyre!("executed block output Merkle cache is invalid: {error}"))?;
        if committed.result().is_err() {
            return Err(eyre!(
                "committed transaction carries a rejected execution result"
            ));
        }
        if !committed.verify_inclusion_in_authenticated_execution(&block, execution_commitment) {
            return Err(eyre!(
                "committed transaction does not verify against the authenticated execution commitment"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(canonical)
    }

    fn fetch_bridge_finality_proof_at_height(
        &self,
        height: NonZeroU64,
    ) -> Result<BridgeFinalityProof> {
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
            .path()
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let proof: BridgeFinalityProof = Self::decode_canonical_norito_response(
            &response,
            BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES,
            "Failed to get bridge finality proof",
        )?;
        if proof.version != BRIDGE_FINALITY_PROOF_VERSION_V2 {
            return Err(eyre!(
                "bridge finality proof version {} does not match required version {}",
                proof.version,
                BRIDGE_FINALITY_PROOF_VERSION_V2
            ));
        }
        let artifact = &proof.finality_artifact;
        if proof.block_header.height() != height
            || artifact.height != height.get()
            || artifact.height_context.height != height.get()
            || artifact.commit_qc.round.height != height.get()
            || artifact.commit_qc.proposal_round.height != height.get()
        {
            return Err(eyre!(
                "bridge finality proof does not match requested height {height}"
            ));
        }
        Ok(proof)
    }

    fn ensure_genesis_readiness_deadline(deadline: std::time::Instant) -> Result<()> {
        if std::time::Instant::now() >= deadline {
            return Err(eyre!("genesis readiness deadline elapsed"));
        }
        Ok(())
    }

    /// Poll once for a fresh node statement whose exact durable tip is the original genesis.
    ///
    /// Only `Ready` authenticates readiness. `NotReady` is an unsigned bounded observation;
    /// only the explicit uninitialized/uncommitted reasons permit retries under a caller-owned
    /// deadline. Restart, wrong tip, conflicting state and absent evidence never mean pending.
    /// Unknown statuses, malformed/oversized errors and transport failures return errors.
    /// The absolute deadline bounds compatibility waiting and all HTTP requests together;
    /// it can only shorten a deadline already attached to this client. Results completing
    /// after it expire. The caller must separately bound CPU work in its process owner.
    ///
    /// All expected identities must come from independently retained generated inputs. The
    /// caller supplies a fresh unpredictable nonzero challenge and the original role's consensus
    /// key. This checks that node's signature and the complete genesis certificate under the
    /// original context; it never selects an anchor from the response. A node already beyond
    /// height one is rejected. The reducer's active height may be greater than one.
    ///
    /// The canonical response is capped at 16 MiB and decoded under the existing cumulative
    /// Norito limits. The caller still owns the overall retry deadline and original input/process
    /// custody. This statement does not prove runtime-provider readiness or future progress.
    ///
    /// # Errors
    ///
    /// Returns an error before any request for a zero challenge, a non-BLS expected node key,
    /// or inconsistent client/network/genesis expectations. Transport, canonical framing,
    /// independent identity binding, tip, node signature or anchored finality failures also fail.
    pub fn poll_genesis_finality_attestation(
        &self,
        challenge: [u8; 32],
        expected_peer: &iroha_model_base::peer::PeerId,
        expected_network_id: NetworkId,
        expected_genesis_hash: HashOf<BlockHeader>,
        expected_context_id: iroha_data_model::block::consensus_v2::HeightContextId,
        deadline: std::time::Instant,
    ) -> Result<GenesisFinalityReadiness> {
        if challenge.iter().all(|byte| *byte == 0) {
            return Err(eyre!("genesis finality challenge must be non-zero"));
        }
        if !matches!(
            expected_peer.public_key().try_algorithm(),
            Ok(iroha_crypto::Algorithm::BlsNormal)
        ) {
            return Err(eyre!("expected genesis node key must be BLS-normal"));
        }
        if expected_network_id != self.network_id
            || expected_network_id.as_genesis_hash() != &expected_genesis_hash
        {
            return Err(eyre!("inconsistent client/network/genesis expectations"));
        }
        let deadline = self
            .http_transport
            .deadline()
            .map_or(deadline, |existing| existing.min(deadline));
        Self::ensure_genesis_readiness_deadline(deadline)?;
        let client = self.with_request_deadline(deadline);
        norito::with_decode_limits_scope(
            norito::canonical_decode_limits(GENESIS_FINALITY_ATTESTATION_RESPONSE_MAX_BYTES),
            || {
                client.ensure_data_model_compatibility()?;
                let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
                    .path()
                    .replace("{height}", "1");
                let response = client.send_builder(
                    client
                        .canonical_norito_get_request(
                            &path,
                            GENESIS_FINALITY_ATTESTATION_RESPONSE_MAX_BYTES,
                            ActivationEvidenceReadAuth::Public,
                        )?
                        .replace_header(GENESIS_FINALITY_CHALLENGE_HEADER, &hex::encode(challenge)),
                )?;
                Self::ensure_genesis_readiness_deadline(deadline)?;
                if response.status() != StatusCode::OK {
                    let failure = Self::decode_finality_attestation_failure(
                        &response,
                        1,
                        challenge,
                        expected_peer,
                        expected_network_id,
                    )?;
                    Self::ensure_genesis_readiness_deadline(deadline)?;
                    return Ok(GenesisFinalityReadiness::NotReady(failure.reason));
                }
                let attestation: BridgeFinalityAttestationV1 =
                    Self::decode_canonical_norito_response(
                        &response,
                        GENESIS_FINALITY_ATTESTATION_RESPONSE_MAX_BYTES,
                        "Failed to get genesis finality attestation",
                    )?;
                let body = &attestation.body;
                if body.challenge != challenge {
                    return Err(eyre!("genesis attestation challenge mismatch"));
                }
                if &body.node_id != expected_peer {
                    return Err(eyre!("genesis attestation node mismatch"));
                }
                if body.network_id != expected_network_id
                    || body.genesis_block_hash != expected_genesis_hash
                {
                    return Err(eyre!("genesis attestation network/genesis mismatch"));
                }
                if body.finality_proof.finality_artifact.height != 1
                    || body.status.last_committed_height != 1
                    || body.genesis_finality_proof != body.finality_proof
                {
                    return Err(eyre!(
                        "genesis attestation is not the exact height-one durable tip"
                    ));
                }
                attestation
                    .verify()
                    .map_err(|error| eyre!("genesis attestation verification failed: {error}"))?;
                let mut verifier =
                    BridgeFinalityVerifier::with_context(expected_network_id, expected_context_id);
                verifier
                    .verify(&body.genesis_finality_proof)
                    .map_err(|error| eyre!("genesis finality verification failed: {error}"))?;
                Self::ensure_genesis_readiness_deadline(deadline)?;
                Ok(GenesisFinalityReadiness::Ready(Box::new(attestation)))
            },
        )
    }

    /// Fetch and independently verify a bridge-finality checkpoint candidate.
    ///
    /// The returned tuple is ordered as `(proof, verified_block_hash)`. The proof must be
    /// canonical, match the exact requested height and network, and pass standalone certificate
    /// verification before either value is returned. Standalone verification proves
    /// self-consistency under the proof's frozen roster; callers must still authenticate and pin
    /// the returned context id through governance or another trusted channel before treating this
    /// candidate as a chain anchor.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, status, media-type, size, canonical decode, requested
    /// height or network mismatch, unsupported proof version, malformed proof structure, invalid
    /// validator proofs of possession, or invalid aggregate signature.
    pub fn get_bridge_finality_anchor(
        &self,
        height: NonZeroU64,
        expected_network_id: NetworkId,
    ) -> Result<(BridgeFinalityProof, HashOf<BlockHeader>)> {
        let proof = self.fetch_bridge_finality_proof_at_height(height)?;
        verify_bridge_finality_proof(&proof, &expected_network_id)
            .map_err(|error| eyre!("bridge finality anchor verification failed: {error}"))?;
        let block_hash = proof.block_header.hash();
        self.ensure_activation_evidence_deadline()?;
        Ok((proof, block_hash))
    }

    /// Fetch and verify the next proof when its block hash is not known in advance.
    ///
    /// Reuse the same externally anchored `verifier` for every immediate successor. The response
    /// contract and every encoded height binding are checked before stateful verification, and the
    /// verifier itself advances only after the complete successor proof verifies.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, status, media-type, size, canonical decode, requested
    /// height mismatch, unsupported proof version, or stateful finality verification failure.
    pub fn get_next_bridge_finality_proof(
        &self,
        height: NonZeroU64,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<BridgeFinalityProof> {
        let proof = self.fetch_bridge_finality_proof_at_height(height)?;
        self.verify_activation_evidence_successor(&proof, verifier)?;
        Ok(proof)
    }

    /// Fetch and verify one exact bridge-finality proof at the next expected chain height.
    ///
    /// Initialize `verifier` from an externally trusted pre-submission height-context anchor and
    /// reuse that same verifier for every immediate successor. Requested height/hash bindings are
    /// checked before verification can advance the verifier. The verifier itself advances only
    /// after complete network, context-transition, quorum, proof-of-possession, and aggregate
    /// signature verification succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, status, media-type, size, canonical decode, requested
    /// height/hash mismatch, unsupported proof version, or stateful finality verification failure.
    pub fn get_bridge_finality_proof(
        &self,
        height: NonZeroU64,
        expected_block_hash: HashOf<BlockHeader>,
        verifier: &mut BridgeFinalityVerifier,
    ) -> Result<BridgeFinalityProof> {
        let proof = self.fetch_bridge_finality_proof_at_height(height)?;
        let artifact = &proof.finality_artifact;
        let header_hash = proof.block_header.hash();
        if header_hash != expected_block_hash
            || artifact.block_hash != expected_block_hash
            || artifact.subject.block_hash != expected_block_hash
            || artifact.commit_qc.subject.block_hash != expected_block_hash
        {
            return Err(eyre!(
                "bridge finality proof does not match the requested block hash"
            ));
        }
        self.verify_activation_evidence_successor(&proof, verifier)?;
        Ok(proof)
    }
}
