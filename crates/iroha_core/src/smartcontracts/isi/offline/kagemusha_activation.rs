// Consensus execution for atomic Kagemusha V4 release activation.

impl Execute for ActivateKagemushaRecursiveReleaseV4 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        kagemusha_release_lifecycle::require_direct_stage(&self, state_transaction)?;
        ensure_kagemusha_recursive_release_v4_activation_authorized(state_transaction, authority)?;
        let runtime_effective_config_sha256 = self.runtime_effective_config_sha256;
        let promotion_binding = self.promotion_binding;
        let policy = self.device_attestation_policy;
        validate_offline_attestation_policy_for_release_activation(
            &policy,
            state_transaction.block_unix_timestamp_ms(),
        )?;
        validate_offline_attestation_policy_transition_from_state(&policy, state_transaction)?;
        let policy_bytes = norito::encode_canonical(&policy).map_err(|error| {
            labeled_invariant(
                "invalid_attestation_policy",
                format!("failed to encode atomic Offline device attestation policy: {error}"),
            )
        })?;
        let activation = self.activation;
        let binding = kagemusha_v4_release_binding(&activation.release_record)?;
        let (promotion_marker, promotion_binding_marker, release_record_bytes) =
            plan_kagemusha_v4_activation_binding(
                &promotion_binding,
                &activation,
                &binding,
                &policy_bytes,
                state_transaction,
            )?;
        let cached = state_transaction
            .kagemusha_release_catalog
            .resolve_binding(&binding)
            .map_err(|error| labeled_invariant("recursive_release_invalid", error))?
            .clone();
        if activation.configured_policy_sha256
            != state_transaction
                .kagemusha_release_catalog
                .configured_policy_sha256()
                .ok_or_else(|| {
                    labeled_invariant(
                        "recursive_release_invalid",
                        "this validator has no configured Kagemusha V4 release policy",
                    )
                })?
            || cached.release_record() != &activation.release_record
        {
            return Err(labeled_invariant(
                "recursive_release_invalid",
                "activation release or configured-policy digest differs from the local authenticated catalog",
            )
            .into());
        }
        let manifest = &activation.release_record.manifest;
        if &manifest.network_id != state_transaction.network_id() {
            return Err(labeled_invariant(
                "wrong_network",
                "Kagemusha V4 activation manifest targets a different network",
            )
            .into());
        }
        let spec = state_transaction.numeric_spec_for(&manifest.asset)?;
        if spec.scale() != Some(manifest.asset_scale) {
            return Err(labeled_invariant(
                "amount_scale_mismatch",
                "Kagemusha V4 activation manifest scale differs from the live asset definition",
            )
            .into());
        }
        let current_height = state_transaction.block_height();
        if manifest.activation_height <= current_height {
            return Err(labeled_invariant(
                "recursive_release_invalid",
                "Kagemusha V4 activation height must be in the future",
            )
            .into());
        }
        ensure_kagemusha_v4_non_overlapping_issuance(&binding, manifest, state_transaction)?;
        let expected_eq_id =
            iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
                binding.manifest_sha256,
            );
        let expected_ep_id =
            iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
                binding.manifest_sha256,
            );
        if activation.step_eq_verifier_key_id != expected_eq_id
            || activation.step_ep_verifier_key_id != expected_ep_id
            || activation.step_eq_verifier_key_id == activation.step_ep_verifier_key_id
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V4 activation verifier ids do not match the release-bound Eq/Ep identities",
            )
            .into());
        }
        let expected_version = kagemusha_v4_next_verifier_version(state_transaction)?;
        if activation.step_eq_verifier_record.version != expected_version
            || activation.step_ep_verifier_record.version != expected_version
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha V4 Eq/Ep verifier records do not use the next atomic version",
            )
            .into());
        }
        cached
            .validate_verifier_records(
                &activation.step_eq_verifier_record,
                &activation.step_ep_verifier_record,
            )
            .map_err(|error| labeled_invariant("verifier_key_invalid", error))?;
        if state_transaction
            .world
            .verifying_keys
            .get(&expected_eq_id)
            .is_some()
            || state_transaction
                .world
                .verifying_keys
                .get(&expected_ep_id)
                .is_some()
        {
            return Err(labeled_invariant(
                "recursive_release_overlap",
                "Kagemusha V4 release verifier ids are already registered",
            )
            .into());
        }
        let release_key = kagemusha_terminal_registry_v4::release_state_key(&binding)
            .map_err(|error| labeled_invariant("recursive_release_invalid", error))?;
        if state_transaction
            .world
            .smart_contract_state
            .get(&release_key)
            .is_some()
        {
            return Err(labeled_invariant(
                "recursive_release_overlap",
                "Kagemusha V4 release record is already activated",
            )
            .into());
        }
        let lifecycle_plan = kagemusha_release_lifecycle::plan_staged(
            authority,
            promotion_binding,
            binding.clone(),
            policy,
            &release_record_bytes,
            runtime_effective_config_sha256,
            expected_eq_id.clone(),
            expected_ep_id.clone(),
            expected_version,
            state_transaction,
        )?;
        // Stage the release, verifier records, and promotion atomically. The
        // bound device policy becomes globally active only with the later
        // evidence-backed issuance-enable transition.
        state_transaction
            .world
            .smart_contract_state
            .insert(release_key, release_record_bytes);
        state_transaction.world.verifying_keys.insert(
            expected_eq_id.clone(),
            activation.step_eq_verifier_record.clone(),
        );
        state_transaction.world.verifying_keys.insert(
            expected_ep_id.clone(),
            activation.step_ep_verifier_record.clone(),
        );
        state_transaction.world.verifying_keys_by_circuit.insert(
            (
                activation.step_eq_verifier_record.circuit_id.clone(),
                expected_version,
            ),
            expected_eq_id,
        );
        state_transaction.world.verifying_keys_by_circuit.insert(
            (
                activation.step_ep_verifier_record.circuit_id.clone(),
                expected_version,
            ),
            expected_ep_id,
        );
        kagemusha_release_lifecycle::commit_staged(lifecycle_plan, state_transaction);
        commit_v4_promotion_binding(
            promotion_marker,
            promotion_binding_marker,
            state_transaction,
        );
        Ok(())
    }
}
