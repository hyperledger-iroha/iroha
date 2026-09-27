// Current embedded-certificate finality transport. No V2 decoding or conversion.

const SUMERAGI_FINALITY_RESPONSE_MAX_BYTES: usize =
    2 * iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES + 4 * 1024 * 1024;

impl Client {
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
        verifier.verify(&proof)?;
        self.ensure_activation_evidence_deadline()?;
        Ok(proof)
    }
}
