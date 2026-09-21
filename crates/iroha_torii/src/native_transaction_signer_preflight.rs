// Startup qualification of all four native signer roles against one verified State network.

#[cfg(feature = "app_api")]
fn qualify_configured_sorafs_native_transaction_signer_for_startup<S>(
    network_id: NetworkId,
    role: SorafsNativeTransactionSignerRoleV1,
    required: bool,
    configured: Option<&iroha_config::parameters::actual::SorafsNativeTransactionSignerBinding>,
    provider: Option<Arc<S>>,
    qualify: impl FnOnce(
        NetworkId,
        SorafsNativeTransactionSignerBindingV1,
        Arc<S>,
    ) -> Result<Arc<S>, SorafsNativeTransactionSignerQualificationErrorV1>,
) -> Result<Option<Arc<S>>, String>
where
    S: SorafsNativeTransactionSignerProviderV1 + ?Sized,
{
    let role_label = role.as_str();
    match (required, configured.is_some()) {
        (true, false) => {
            return Err(format!(
                "required SoraFS {role_label} signer role is missing its configured binding for storage-enabled durable drain or role generation"
            ));
        }
        (false, true) => {
            return Err(format!(
                "inactive SoraFS {role_label} signer role rejects a configured binding without storage-enabled durable drain or role generation"
            ));
        }
        _ => {}
    }
    match (configured.is_some(), provider.is_some()) {
        (true, false) => {
            return Err(format!(
                "configured SoraFS {role_label} signer role is missing its runtime provider"
            ));
        }
        (false, true) => {
            return Err(format!(
                "unconfigured SoraFS {role_label} signer role rejects an injected runtime provider"
            ));
        }
        _ => {}
    }
    let (Some(configured), Some(provider)) = (configured, provider) else {
        return Ok(None);
    };
    if configured.public_key.try_algorithm() != Ok(configured.algorithm) {
        return Err(format!(
            "configured SoraFS {role_label} signer binding has a substituted key algorithm"
        ));
    }
    let binding = SorafsNativeTransactionSignerBindingV1::try_new(
        role,
        configured.handle.clone(),
        configured.authority.clone(),
        configured.public_key.clone(),
        SorafsNativeTransactionSignerQualificationV1::new(
            configured.revision,
            configured.policy_digest,
        ),
    )
    .map_err(|_| format!("configured SoraFS {role_label} signer binding is invalid"))?;
    qualify(network_id, binding, provider).map(Some).map_err(|_| {
        format!(
            "SoraFS {role_label} signer provider is substituted, stale, test-marked, unavailable, or unstable"
        )
    })
}
#[cfg(feature = "app_api")]
const fn sorafs_native_signer_role_required(
    storage_enabled: bool,
    role_generation_enabled: bool,
) -> bool {
    storage_enabled || role_generation_enabled
}
#[cfg(feature = "app_api")]
fn preflight_sorafs_native_transaction_signers(
    network_id: NetworkId,
    config: &Config,
    runtime_deps: &mut ToriiRuntimeDeps,
) -> Result<(), String> {
    let configured = &config.sorafs_storage.native_transaction_signers;
    let proof_provider = runtime_deps.sorafs_proof_outcome_signer.take();
    runtime_deps.sorafs_proof_outcome_signer =
        qualify_configured_sorafs_native_transaction_signer_for_startup(
            network_id,
            SorafsNativeTransactionSignerRoleV1::ProofOutcome,
            sorafs_native_signer_role_required(config.sorafs_storage.enabled, false),
            configured.proof_outcome.as_ref(),
            proof_provider,
            qualify_sorafs_proof_outcome_transaction_signer_v1,
        )?;
    let repair_provider = runtime_deps.sorafs_repair_transaction_signer.take();
    runtime_deps.sorafs_repair_transaction_signer =
        qualify_configured_sorafs_native_transaction_signer_for_startup(
            network_id,
            SorafsNativeTransactionSignerRoleV1::Repair,
            sorafs_native_signer_role_required(
                config.sorafs_storage.enabled,
                config.sorafs_repair.enabled,
            ),
            configured.repair.as_ref(),
            repair_provider,
            qualify_sorafs_repair_transaction_signer_v1,
        )?;
    let reserve_provider = runtime_deps.sorafs_reserve_transaction_signer.take();
    runtime_deps.sorafs_reserve_transaction_signer =
        qualify_configured_sorafs_native_transaction_signer_for_startup(
            network_id,
            SorafsNativeTransactionSignerRoleV1::Reserve,
            sorafs_native_signer_role_required(
                config.sorafs_storage.enabled,
                config.sorafs_storage.reserve_worker.enabled,
            ),
            configured.reserve.as_ref(),
            reserve_provider,
            qualify_sorafs_reserve_transaction_signer_v1,
        )?;
    let orderbook_provider = runtime_deps.sorafs_orderbook_transaction_signer.take();
    runtime_deps.sorafs_orderbook_transaction_signer =
        qualify_configured_sorafs_native_transaction_signer_for_startup(
            network_id,
            SorafsNativeTransactionSignerRoleV1::Orderbook,
            sorafs_native_signer_role_required(
                config.sorafs_storage.enabled,
                config.sorafs_storage.orderbook_worker.enabled,
            ),
            configured.orderbook.as_ref(),
            orderbook_provider,
            qualify_sorafs_orderbook_transaction_signer_v1,
        )?;
    Ok(())
}
