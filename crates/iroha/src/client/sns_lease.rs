// Independently selected current SNS lease projection; no endpoint-selected trust inputs.

impl Client {
    /// Authenticate an active dataspace lease against an independently selected native block.
    ///
    /// `native_schema` comes from qualified installation authority. The caller establishes fresh
    /// quorum and selects `block` before this read; the response carries no checkpoint or schema
    /// authorization. `now_unix_ms` additionally enforces current lease expiry.
    /// # Errors
    /// Noncanonical alias, wrong network/schema/cut/owner, inactive lease, finite transport or
    /// codec limits, or an elapsed request deadline.
    pub fn get_dataspace_lease(
        &self,
        alias: &str,
        owner: &iroha_data_model::account::AccountId,
        native_schema: iroha_crypto::Hash,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
        now_unix_ms: u64,
    ) -> Result<iroha_data_model::sns::lease::VerifiedSnsLeaseV1> {
        use iroha_data_model::sns::{
            DATASPACE_ALIAS_SUFFIX_ID, NameSelectorV1,
            lease::{MAX_SNS_LEASE_PROOF_BYTES_V1, SnsLeaseProofV1},
        };
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, alias)?;
        if selector.label != alias
            || alias == "universal"
            || alias.contains('.')
            || alias.contains('/')
            || block.height() < 2
            || block.commitment().schedule.current.network_id != self.network_id
        {
            return Err(eyre!(
                "SNS lease requires a canonical private dataspace and independently verified block on this network"
            ));
        }
        self.ensure_data_model_compatibility()?;
        let path = iroha_torii_shared::route_catalog::sumeragi::SNS_DATASPACE_LEASE
            .path()
            .replace("{alias}", alias)
            .replace("{height}", &block.height().to_string());
        let response = self.send_activation_evidence_read(
            &path,
            MAX_SNS_LEASE_PROOF_BYTES_V1,
            None,
            ActivationEvidenceReadAuth::Public,
        )?;
        let body = Self::bounded_norito_response_body(
            &response,
            StatusCode::OK,
            MAX_SNS_LEASE_PROOF_BYTES_V1,
            "Failed to get authenticated SNS lease",
        )?;
        let proof = SnsLeaseProofV1::decode_frame(body)?;
        let verified = proof.verify(
            self.network_id,
            &selector,
            owner,
            native_schema,
            block,
            now_unix_ms,
        )?;
        self.ensure_activation_evidence_deadline()?;
        Ok(verified)
    }
}
