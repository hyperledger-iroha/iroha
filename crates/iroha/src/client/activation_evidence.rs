// Strict activation-evidence readers kept out of the main client module's source budget.

use crate::data_model::block::{
    decode_framed_signed_block, proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
};
use iroha_data_model::{
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof, BridgeFinalityVerifier,
        verify_bridge_finality_proof,
    },
    query::CommittedTransaction,
};

const BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;

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
        )?;
        if response.status() == StatusCode::CONFLICT {
            use iroha_torii_shared::bridge_finality::{
                BRIDGE_FINALITY_ATTESTATION_TIP_MISMATCH_CODE,
                BRIDGE_FINALITY_ATTESTATION_TIP_MISMATCH_MAX_BYTES,
            };
            if response.body().len() > BRIDGE_FINALITY_ATTESTATION_TIP_MISMATCH_MAX_BYTES {
                return Err(eyre!("finality tip progress exceeds its response bound"));
            }
            let content_type = exact_single_response_header(&response, "content-type")?;
            if !content_type.eq_ignore_ascii_case(APPLICATION_NORITO) {
                return Err(eyre!("finality tip progress requires application/x-norito"));
            }
            let envelope: iroha_torii_shared::ErrorEnvelope = norito::decode_canonical_with_limits(
                response.body(),
                norito::canonical_decode_limits(response.body().len()),
            )
            .wrap_err("failed to decode canonical finality tip progress envelope")?;
            if envelope.code() != BRIDGE_FINALITY_ATTESTATION_TIP_MISMATCH_CODE {
                return Err(eyre!(
                    "finality attestation HTTP 409 code `{}` does not establish tip progress",
                    envelope.code()
                ));
            }
            let mut details = envelope
                .details
                .ok_or_else(|| eyre!("finality tip progress omitted exact request bindings"))?;
            let progress = details
                .bridge_finality_attestation_tip_mismatch
                .take()
                .ok_or_else(|| eyre!("finality tip progress omitted exact request bindings"))?;
            if !details.is_empty() {
                return Err(eyre!(
                    "finality tip progress carries conflicting error details"
                ));
            }
            return Err(BridgeFinalityAttestationTipMismatch::from_response(
                progress,
                height,
                challenge,
                expected_node,
                self.network_id,
            )?
            .into());
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

    fn bounded_norito_response_body<'a>(
        response: &'a Response<Vec<u8>>,
        maximum: usize,
        context: &'static str,
    ) -> Result<&'a [u8]> {
        if response.body().len() > maximum {
            return Err(eyre!(
                "{context}: response exceeds the {maximum}-byte limit"
            ));
        }
        if response.status() != StatusCode::OK {
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
        let body = Self::bounded_norito_response_body(response, maximum, context)?;
        norito::decode_canonical_with_limits(body, norito::canonical_decode_limits(body.len()))
            .map_err(|error| eyre!("{context}: failed to decode canonical Norito payload: {error}"))
    }

    fn canonical_norito_get_request(&self, path: &str, maximum: usize) -> DefaultRequestBuilder {
        let mut headers = self.headers.clone();
        headers.retain(|name, _| {
            !name.eq_ignore_ascii_case("accept")
                && !name.eq_ignore_ascii_case("content-type")
                && !name.eq_ignore_ascii_case("x-iroha-finality-challenge")
        });
        let mut builder =
            DefaultRequestBuilder::new(HttpMethod::GET, join_torii_url(&self.torii_url, path))
                .with_transport(self.http_transport.clone())
                .headers(headers)
                .header("Accept", APPLICATION_NORITO)
                .max_response_bytes(maximum);
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        builder
    }

    // Only an explicit operation deadline authorizes repeated reads. A one-shot
    // client still exposes backpressure immediately. Never retry transport,
    // authentication, codec or proof failures, and never resend a transaction.
    fn send_activation_evidence_read(
        &self,
        path: &str,
        maximum: usize,
        challenge: Option<[u8; 32]>,
    ) -> Result<Response<Vec<u8>>> {
        loop {
            self.ensure_activation_evidence_deadline()?;
            let mut request = self.canonical_norito_get_request(path, maximum);
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
    /// block hash match, its external-entrypoint/result roots and execution context match the
    /// header commitments, its Merkle caches/counts are consistent, and the supplied successful
    /// transaction verifies at its exact ordinary index or certified-merge reference.
    ///
    /// The required commitment must come from an independently verified, externally anchored
    /// native finality proof for this carrier. Its exact wire hash and length authenticate results
    /// and time triggers, which the consensus header hash alone does not bind. This reader verifies
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
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
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
            .validate_entrypoint_merkle_cache()
            .map_err(|error| eyre!("executed block entrypoint Merkle cache is invalid: {error}"))?;
        block
            .validate_result_merkle_cache()
            .map_err(|error| eyre!("executed block result Merkle cache is invalid: {error}"))?;
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
