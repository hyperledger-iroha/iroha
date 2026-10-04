// Original reserve partition evidence against independently selected Global finality.

impl Client {
    /// Read one provider's reserve, credit, capacity and governed pricing facts before service activation.
    ///
    /// The current client signs as the independently selected operations account. The caller
    /// selects the provider owner, complete active policy, native schema and verified Global
    /// block separately. A partition may retain an older policy digest after policy rotation.
    /// Absence means only the exact provider key is absent, never permission to register or
    /// proof of collateral, provider admission, successful submission or readiness. The optional
    /// credit row is independently authenticated at the same cut; its absence is separate from
    /// partition absence and its amounts do not establish current spending eligibility. Capacity
    /// may be absent, future or expired; its exact original row does not grant active capacity.
    /// Required pricing is the same-cut governed cell; arithmetic consumers validate it when
    /// selecting economics. This read creates no capacity/pricing CAS or funding authority.
    ///
    /// # Errors
    /// Invalid signer/provider/policy/scope, elapsed original deadline, bounded transport or
    /// codec refusal, HTTP failures, hidden or substituted originals, or stale World evidence.
    pub fn get_reserve_account_state(
        &self,
        expected_operator: &iroha_data_model::account::AccountId,
        expected_provider: iroha_data_model::sorafs::capacity::ProviderId,
        expected_owner: &iroha_data_model::account::AccountId,
        expected_policy: &iroha_data_model::sorafs::reserve::ReserveAuthorityPolicyV1,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
    ) -> Result<iroha_data_model::sorafs::reserve::account_proof::VerifiedReserveAccountStateV1>
    {
        use iroha_data_model::sorafs::reserve::{
            account_proof::{
                MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1, RESERVE_ACCOUNT_PROOF_LIMITS_V1,
                ReserveAccountProofExpectedV1, ReserveAccountProofV1,
            },
            history::STATE_MAX_BYTES,
        };
        // All independent request selections are admitted before compatibility/network I/O.
        block.verify_global_scope(self.network_id, self.chain.as_str())?;
        if expected_operator != &self.account
            || expected_provider.as_bytes() == &[0; 32]
            || expected_policy.operations_authority != *expected_operator
            || block.height() < 2
            || norito::canonical_frame_len(expected_operator)? > STATE_MAX_BYTES
            || norito::canonical_frame_len(expected_owner)? > STATE_MAX_BYTES
            || norito::canonical_frame_len(expected_policy)? > STATE_MAX_BYTES
        {
            return Err(eyre!(
                "reserve account read requires this client's selected operations account, nonzero provider and Global successor"
            ));
        }
        expected_policy.validate()?;
        self.ensure_activation_evidence_deadline()?;
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::contracts_and_verification_keys::SORAFS_RESERVE_ACCOUNT_PROOF_GET
            .path()
            .replace("{provider_id}", &hex::encode(expected_provider.as_bytes()))
            .replace("{height}", &block.height().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1,
            None,
            ActivationEvidenceReadAuth::Account,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1,
            "Failed to get native reserve account state",
        )?;
        let verified = norito::core::with_decode_limits_scope(
            RESERVE_ACCOUNT_PROOF_LIMITS_V1,
            || -> Result<_> {
                let proof = ReserveAccountProofV1::decode_frame(body)?;
                Ok(proof.verify(
                    &ReserveAccountProofExpectedV1 {
                        chain: self.chain.as_str(),
                        network_id: self.network_id,
                        operator: expected_operator,
                        provider_id: expected_provider,
                        owner: expected_owner,
                        policy: expected_policy,
                        schema: native_schema,
                    },
                    block,
                )?)
            },
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }
}
