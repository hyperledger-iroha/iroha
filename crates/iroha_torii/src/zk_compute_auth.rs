// Exact-network account authentication shared by expensive ZK tooling routes.
macro_rules! mount_authenticated_zk_compute_routes {
    ($builder:ident, $app_state:ident, $proof_body_limit:ident) => {
        #[cfg(feature = "zk-verify-batch")]
        $builder.route(
            &route_catalog::runtime_governance::ZK_VERIFY_BATCH,
            catalog_post(handler_zk_verify_batch)
                .authenticated_canonical_account_proof_body($app_state.clone(), $proof_body_limit),
        );
    };
}
