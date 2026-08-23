// Strict activation-evidence readers kept out of the main client module's source budget.

use crate::data_model::block::{
    BlockHeader, decode_framed_signed_block,
    proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
};
use iroha_data_model::{
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeFinalityProof, BridgeFinalityVerifier,
        verify_bridge_finality_proof,
    },
    query::CommittedTransaction,
};

const BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;

impl Client {
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
            !name.eq_ignore_ascii_case("accept") && !name.eq_ignore_ascii_case("content-type")
        });
        let mut builder =
            DefaultRequestBuilder::new(HttpMethod::GET, join_torii_url(&self.torii_url, path))
                .headers(headers)
                .header("Accept", APPLICATION_NORITO)
                .max_response_bytes(maximum);
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        builder
    }

    /// Fetch the exact canonical, result-bearing executed block wire containing `committed`.
    ///
    /// The returned bytes are accepted only when the route yields bounded Norito, the block
    /// round-trips to the byte-identical canonical [`SignedBlock`] wire, its requested height and
    /// block hash match, its entrypoint/result Merkle material is internally consistent, and the
    /// supplied successful committed transaction verifies against that exact carrier block.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, status, media-type, size, decode, canonicality, height,
    /// hash, result-shape, Merkle-cache, transaction-result, or inclusion-proof failures.
    pub fn get_canonical_executed_block_wire(
        &self,
        height: NonZeroU64,
        committed: &CommittedTransaction,
    ) -> Result<Vec<u8>> {
        self.ensure_data_model_compatibility()?;
        let path =
            torii_uri::LEDGER_EXECUTED_BLOCK_WIRE.replace("{height}", &height.get().to_string());
        let response = self.send_builder(self.canonical_norito_get_request(
            &path,
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
        ))?;
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
        if !committed.verify_inclusion_in_block(&block) {
            return Err(eyre!(
                "committed transaction does not verify against the exact executed block"
            ));
        }
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
        let response = self.send_builder(
            self.canonical_norito_get_request(&path, BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES),
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
        verifier
            .verify(&proof)
            .map_err(|error| eyre!("bridge finality proof verification failed: {error}"))?;
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
        verifier
            .verify(&proof)
            .map_err(|error| eyre!("bridge finality proof verification failed: {error}"))?;
        Ok(proof)
    }
}
