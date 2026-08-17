use crate::governance::manifest::GovernanceRules;
use iroha_telemetry::metrics::{
    NexusLaneManifestValidatorBindingStatus, NexusLaneRuntimeUpgradeHookStatus, NexusLaneTeuStatus,
};

pub(super) fn populate_manifest_rules(entry: &mut NexusLaneTeuStatus, rules: &GovernanceRules) {
    entry.manifest_validators = rules.validators.iter().map(ToString::to_string).collect();
    let mut manifest_validator_bindings = rules
        .validator_bindings
        .iter()
        .map(|binding| NexusLaneManifestValidatorBindingStatus {
            validator: binding.validator.to_string(),
            peer_id: binding.peer_id.to_string(),
            torii_url: binding.torii_url.clone(),
        })
        .collect::<Vec<_>>();
    manifest_validator_bindings.sort();
    entry.manifest_validator_bindings = manifest_validator_bindings;
    entry.manifest_quorum = rules.quorum;
    entry.manifest_protected_namespaces = rules
        .protected_namespaces
        .iter()
        .map(ToString::to_string)
        .collect();
    entry.manifest_runtime_upgrade = rules.hooks.runtime_upgrade.as_ref().map(|hook| {
        let allowed_ids = hook
            .allowed_ids
            .as_ref()
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default();
        NexusLaneRuntimeUpgradeHookStatus {
            allow: hook.allow,
            require_metadata: hook.require_metadata,
            metadata_key: hook.metadata_key.as_ref().map(ToString::to_string),
            allowed_ids,
        }
    });
}
