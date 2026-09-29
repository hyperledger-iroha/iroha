// Embedded-certificate finality transport; no alternate decoding or conversion.

const SUMERAGI_FINALITY_RESPONSE_MAX_BYTES: usize =
    2 * iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES + 4 * 1024 * 1024;

impl Client {
    /// Probe the original genesis tip under a caller-authenticated current genesis verifier.
    /// A successful node signature authenticates that node's observed genesis execution;
    /// callers requiring quorum readiness must collect independently selected nodes.
    ///
    /// # Errors
    /// Invalid independent inputs, elapsed deadline, transport, signature, root or instance mismatch.
    pub fn poll_sumeragi_genesis_readiness(
        &self,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
        trusted: &iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
        deadline: std::time::Instant,
    ) -> Result<GenesisFinalityReadiness> {
        if challenge == [0; 32]
            || expected_node.public_key().try_algorithm()? != iroha_crypto::Algorithm::BlsNormal
        {
            return Err(eyre!(
                "genesis readiness requires a nonzero challenge and selected BLS node"
            ));
        }
        let deadline = self
            .http_transport
            .deadline()
            .map_or(deadline, |prior| prior.min(deadline));
        Self::ensure_genesis_readiness_deadline(deadline)?;
        let client = self.with_request_deadline(deadline);
        client.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
            .path()
            .replace("{height}", "1");
        let response = client.send_activation_evidence_read(
            &path,
            SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
            Some(challenge),
            ActivationEvidenceReadAuth::Public,
        )?;
        Self::ensure_genesis_readiness_deadline(deadline)?;
        if response.status() != StatusCode::OK {
            let failure = Self::decode_finality_attestation_failure(
                &response,
                1,
                challenge,
                expected_node,
                self.network_id,
            )?;
            Self::ensure_genesis_readiness_deadline(deadline)?;
            return Ok(GenesisFinalityReadiness::NotReady(failure.reason));
        }
        let attestation: iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation =
            Self::decode_canonical_norito_response(
                &response,
                SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                "Failed to get current genesis readiness",
            )?;
        attestation.verify()?;
        let body = &attestation.body;
        if body.challenge != challenge
            || body.node_id != *expected_node
            || body.network_id != self.network_id
            || body.status.instance != trusted.instance().0
            || body.finality_proof.height() != 1
        {
            return Err(eyre!(
                "genesis readiness differs from selected request, network or instance"
            ));
        }
        let mut candidate = trusted.clone();
        candidate.verify(&body.genesis_finality_proof)?;
        candidate.verify_same_decision(&body.genesis_finality_proof, &body.finality_proof)?;
        Self::ensure_genesis_readiness_deadline(deadline)?;
        Ok(GenesisFinalityReadiness::Ready(Box::new(attestation)))
    }
    /// Read and authenticate the selected node's fresh current-consensus tip statement.
    /// The caller's independently anchored verifier must authenticate the embedded chain.
    ///
    /// # Errors
    /// Bounded transport/codec failure, malformed proof, invalid node signature or wrong request binding.
    pub fn get_sumeragi_finality_attestation(
        &self,
        height: NonZeroU64,
        challenge: [u8; 32],
        expected_node: &iroha_model_base::peer::PeerId,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation> {
        if challenge == [0; 32] {
            return Err(eyre!("finality challenge must be nonzero"));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY_ATTESTATION
            .path()
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
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
                "current finality attestation unavailable: {}",
                failure.reason.as_str()
            ));
        }
        let attestation: iroha_data_model::sumeragi_finality::SumeragiFinalityAttestation =
            Self::decode_canonical_norito_response(
                &response,
                SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                "Failed to get current finality attestation",
            )?;
        attestation.verify()?;
        if attestation.body.challenge != challenge
            || attestation.body.node_id != *expected_node
            || attestation.body.network_id != self.network_id
            || attestation.body.status.committed_height != height.get()
        {
            return Err(eyre!(
                "current finality attestation differs from exact request bindings"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(attestation)
    }

    /// Fetch one canonical current proof without choosing a trust root from the response.
    /// A structural certificate check does not authenticate its candidate committee.
    ///
    /// # Errors
    /// Bounded transport/codec error, wrong height or malformed current certificate.
    pub fn get_sumeragi_finality_proof(
        &self,
        height: NonZeroU64,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
            .path()
            .replace("{height}", &height.get().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof =
            Self::decode_canonical_norito_response(
                &response,
                SUMERAGI_FINALITY_RESPONSE_MAX_BYTES,
                "Failed to get current finality proof",
            )?;
        if proof.height() != height.get() {
            return Err(eyre!(
                "current finality proof differs from requested height"
            ));
        }
        proof.decode_checked()?;
        self.ensure_activation_evidence_deadline()?;
        Ok(proof)
    }

    /// Fetch and admit the immediate successor into the caller's independently anchored prefix.
    ///
    /// # Errors
    /// Fetch failure, invalid certificate, wrong committee or discontinuous certified chain.
    pub fn get_next_sumeragi_finality_proof(
        &self,
        height: NonZeroU64,
        verifier: &mut iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier,
    ) -> Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        let proof = self.get_sumeragi_finality_proof(height)?;
        let mut candidate = verifier.clone();
        candidate.verify(&proof)?;
        self.ensure_activation_evidence_deadline()?;
        *verifier = candidate;
        Ok(proof)
    }
}
