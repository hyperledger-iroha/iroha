// Independently selected reserve policy facts; no activation or namespace eligibility authority.

impl Client {
    /// Read reserve policy presence or absence at an independently selected Global decision.
    ///
    /// The manager authenticates the HTTP request with this client's account. The response
    /// must prove its direct policy permission, every selected account and asset, and the
    /// exact selected policy when present. The caller supplies its qualified native schema
    /// and establishes fresh finality before this read. Singleton absence does not prove
    /// initial activation eligibility or absence of other reserve records.
    ///
    /// # Errors
    /// Wrong account, chain or Global network, invalid policy, expired deadline, bounded
    /// transport/codec failure, or substituted, concealed or stale native state.
    pub fn get_reserve_policy_state(
        &self,
        expected_manager: &iroha_data_model::account::AccountId,
        expected_policy: &iroha_data_model::sorafs::reserve::ReserveAuthorityPolicyV1,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
    ) -> Result<iroha_data_model::sorafs::reserve::proof::VerifiedReservePolicyStateV1> {
        use iroha_data_model::sorafs::reserve::proof::{
            MAX_RESERVE_POLICY_PROOF_BYTES_V1, ReservePolicyProofV1,
        };
        expected_policy.validate()?;
        block.verify_global_scope(self.network_id, self.chain.as_ref())?;
        if expected_manager != &self.account || block.height() < 2 {
            return Err(eyre!(
                "reserve policy state requires this client's manager account and a non-genesis Global decision"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::contracts_and_verification_keys::SORAFS_RESERVE_POLICY_PROOF_GET
            .path()
            .replace("{height}", &block.height().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_RESERVE_POLICY_PROOF_BYTES_V1,
            None,
            ActivationEvidenceReadAuth::Account,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_RESERVE_POLICY_PROOF_BYTES_V1,
            "Failed to get native reserve policy state",
        )?;
        let proof = ReservePolicyProofV1::decode_frame(body)?;
        let verified = proof.verify(
            self.chain.as_ref(),
            self.network_id,
            expected_manager,
            expected_policy,
            native_schema,
            block,
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }
}
