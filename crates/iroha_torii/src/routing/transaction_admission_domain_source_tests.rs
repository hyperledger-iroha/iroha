#[cfg(test)]
mod transaction_admission_domain_source_tests {
    fn function_signature<'a>(source: &'a str, name: &str) -> &'a str {
        let marker = format!("fn {name}(");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("missing Torii function `{name}`"));
        let signature = &source[start..];
        let end = signature
            .find(") ->")
            .unwrap_or_else(|| panic!("missing signature terminator for `{name}`"));
        &signature[..=end]
    }
    #[test]
    fn ordinary_transaction_admission_has_no_chain_id_parameter_axis() {
        let source = include_str!("../routing.rs");
        let network_scoped_functions = [
            "handle_transaction_inner_sync",
            "handle_transaction_inner",
            "handle_transaction",
            "handle_transaction_with_metrics",
            "handle_transaction_with_metrics_and_routing_plan",
            "handle_transaction_with_metrics_and_routing_plan_sync",
            "submit_contract_call_request",
            "handle_post_contract_call",
            "handle_post_contract_call_multisig_propose",
            "handle_post_contract_call_multisig_approve",
            "handle_post_multisig_cancel",
            "handle_post_multisig_propose",
            "handle_post_multisig_approve",
            "execute_account_recovery_mutation",
            "handle_post_account_recovery_policy_set",
            "handle_post_account_recovery_propose",
            "handle_post_account_recovery_approve",
            "handle_post_account_recovery_finalize",
            "handle_post_vk_register",
            "handle_post_vk_update",
            "handle_post_contract_alias_set",
            "handle_post_sorafs_register_manifest",
            "handle_post_sorafs_register_capacity_declaration",
            "handle_post_sorafs_record_capacity_telemetry",
            "handle_post_space_directory_manifest_publish",
            "handle_post_space_directory_manifest_revoke",
            "handle_post_v1_subscription_plan",
            "handle_post_v1_subscription_usage",
        ];
        for name in network_scoped_functions {
            let signature = function_signature(source, name);
            assert!(
                !signature.contains("ChainId"),
                "`{name}` must derive transaction security domains from CoreState NetworkId: {signature}"
            );
        }
        let dead_underscore_parameter = source.lines().find(|line| {
            let line = line.trim_start();
            line.starts_with("_chain_id:") && line.contains("Arc<ChainId>")
        });
        assert!(
            dead_underscore_parameter.is_none(),
            "dead underscore-prefixed ChainId parameter returned: {dead_underscore_parameter:?}"
        );
    }
}
