use super::*;

impl GenesisMaterial {
    pub(super) fn finalize_kagami_stub_signature(
        &self,
        config_path: &Path,
        consensus_mode: SumeragiConsensusMode,
    ) -> Result<()> {
        let manifest = RawGenesisTransaction::from_path(&self.manifest_path)?.with_consensus_meta();
        if manifest.consensus_mode() != consensus_mode {
            return Err(SupervisorError::KagamiInvocation(format!(
                "test Kagami stub manifest mode {} differs from requested mode {consensus_mode}",
                manifest.consensus_mode()
            )));
        }
        if manifest.consensus_fingerprint().is_none() {
            return Err(SupervisorError::KagamiInvocation(
                "test Kagami stub manifest has no consensus fingerprint after normalization"
                    .to_owned(),
            ));
        }
        fs::write(
            &self.manifest_path,
            norito::json::to_vec_pretty(&manifest).map_err(|error| {
                SupervisorError::KagamiInvocation(format!(
                    "test Kagami stub failed encoding bound manifest: {error}"
                ))
            })?,
        )?;
        let mut source = TomlSource::from_file(config_path).map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "test Kagami stub failed reading config `{}`: {error:?}",
                config_path.display()
            ))
        })?;
        if let Some(expected_hash) = source
            .table_mut()
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .and_then(|genesis| genesis.get_mut("expected_hash"))
            && expected_hash.as_str() == Some(GENESIS_EXPECTED_HASH_PLACEHOLDER)
        {
            let hash_body = Hash::new(b"Mochi unit-test Kagami unresolved genesis hash")
                .to_string()
                .to_ascii_uppercase();
            *expected_hash =
                toml::Value::String(norito::literal::format("hash", hash_body.as_str()));
        }
        let config = actual::Root::from_toml_source(source).map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "test Kagami stub failed parsing config `{}`: {error:?}",
                config_path.display()
            ))
        })?;
        if config.common.chain != *manifest.chain_id()
            || *config.common.chain_discriminant.value() != manifest.chain_discriminant()
            || config.genesis.public_key != *self.public_key()
        {
            return Err(SupervisorError::KagamiInvocation(
                "test Kagami stub config differs from its manifest or signing key".to_owned(),
            ));
        }
        let block = manifest
            .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
                &self.key_pair,
                Some(iroha_core::da::proof_policy_bundle(
                    &config.nexus.lane_config,
                )),
                Some(iroha_core::state::compute_genesis_confidential_policy_hash(
                    &config.zk,
                )),
            )
            .map_err(|error| {
                SupervisorError::KagamiInvocation(format!(
                    "test Kagami stub failed signing canonical genesis: {error:#}"
                ))
            })?
            .0;
        let wire = block.encode_wire().map_err(|error| {
            SupervisorError::KagamiInvocation(format!(
                "test Kagami stub failed encoding canonical genesis: {error}"
            ))
        })?;
        fs::write(&self.block_path, wire)?;
        fs::write(&self.expected_hash_path, format!("{}\n", block.hash()))?;
        Ok(())
    }
}
