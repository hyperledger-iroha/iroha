#[test]
fn standard_launcher_qualifies_and_supervises_governance_dag_service_adapters() {
    let compact_source: String = include_str!("../main.rs")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let qualification = compact_source
        .find("letsorafs_governance_dag_service_launch=resolve_governance_dag_service_launch(")
        .expect("launcher qualifies Governance DAG providers");
    let supervisor = compact_source
        .find("sorafs_node::prepare_governance_dag_service_from_view(view,providers)")
        .expect("launcher prepares the Governance DAG service");
    let install = compact_source
        .find("sorafs_node.install_governance_dag_mirror_read_handle(runner.mirror_read_handle())")
        .expect("launcher installs the service-owned Governance DAG mirror reader");
    let service_spawn = compact_source[install..]
        .find("tokio::spawn(asyncmove")
        .map(|offset| install + offset)
        .expect("launcher spawns the prepared Governance DAG service");
    let node_construction = compact_source
        .find("letmutsorafs_node=sorafs_node::NodeHandle::try_new_with_policies_and_runtime_deps(")
        .expect("launcher constructs the embedded SoraFS node");
    let first_node_clone = compact_source[node_construction..]
        .find("sorafs_node.clone()")
        .map(|offset| node_construction + offset)
        .expect("launcher eventually shares the embedded SoraFS node");
    let state_open = compact_source
        .find("Kura::new_with_configured_lane_catalog_hash_and_snapshot_bootstrap_and_sumeragi_limits(")
        .expect("launcher contains the persistent-state startup corridor");
    assert!(
        qualification < state_open
            && state_open < supervisor
            && supervisor < install
            && install < service_spawn
            && install < first_node_clone,
        "provider qualification, state opening, service preparation, mirror-reader installation, service spawn, and first node sharing must retain their startup-fatal order"
    );
    assert_eq!(
        compact_source[node_construction..first_node_clone]
            .matches("install_governance_dag_mirror_read_handle(")
            .count(),
        1,
        "the service-owned mirror reader must be installed exactly once"
    );
    assert!(
        compact_source
            .contains("runner.run_until(asyncmove{service_shutdown.receive().await}).await"),
        "the embedded service must receive the existing supervisor shutdown signal"
    );
    assert!(
        compact_source.contains("panic!(\"supervisedGovernanceDAGservicefailed:{error}\")"),
        "a runner error must remain fatal to the supervisor"
    );
    for (field, builder) in [
        (
            "sorafs_governance_dag_ipfs_authenticator",
            "with_ipfs_authenticator",
        ),
        (
            "sorafs_governance_dag_head_authenticator",
            "with_head_authenticator",
        ),
        (
            "sorafs_governance_dag_checkpoint_store",
            "with_checkpoint_store",
        ),
    ] {
        assert!(
            compact_source.contains(&["runtime_deps.", field, ".as_ref()"].concat()),
            "launcher must consume registry dependency `{field}`"
        );
        assert!(
            compact_source.contains(&["providers=providers.", builder, "("].concat()),
            "launcher must forward `{field}` through `{builder}`"
        );
    }
}
