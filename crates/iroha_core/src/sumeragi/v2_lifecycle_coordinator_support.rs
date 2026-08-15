//! Small owner and source-review helpers kept outside the coordinator reducer.

use super::ProductionLifecycleOwnerV1;

impl ProductionLifecycleOwnerV1 {
    /// Attach the exact interrupted-tip replay to the storage-only adapter startup.
    ///
    /// Only the pending-Kura authenticated factory calls this after the normal
    /// owner constructor proved there is no live recovered WAL carrier. The
    /// replay remains nested in the adapter startup and cannot become an
    /// independently scheduled Decision-Fetch row.
    pub(in crate::sumeragi) fn with_pending_kura_apply_replay(
        mut self,
        replay: crate::sumeragi::v2::RecoveredPendingKuraApplyReplayV1,
    ) -> Self {
        let startup = self
            .adapter_startup
            .take()
            .expect("storage-only pending Kura owner retains adapter startup");
        self.adapter_startup = Some(startup.with_pending_kura_apply_replay(replay));
        self
    }
}

#[cfg(test)]
/// Reconstruct the ledger source exactly as Rust expands its reviewed providers.
pub(crate) fn reviewed_lifecycle_ledger_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let tests = include_str!("v2_lifecycle_ledger_tests.rs")
                .replacen(
                    "        include!(\"v2_lifecycle_ledger_tests_durable_recovery_01.rs\");\n",
                    include_str!("v2_lifecycle_ledger_tests_durable_recovery_01.rs"),
                    1,
                )
                .replacen(
                    "        include!(\"v2_lifecycle_ledger_tests_durable_recovery_02.rs\");\n",
                    include_str!("v2_lifecycle_ledger_tests_durable_recovery_02.rs"),
                    1,
                )
                .replacen(
                    "    include!(\"v2_lifecycle_ledger_tests_frame_and_store.rs\");\n",
                    include_str!("v2_lifecycle_ledger_tests_frame_and_store.rs"),
                    1,
                );
            include_str!("v2_lifecycle_ledger.rs")
                .replacen(
                    "include!(\"v2_lifecycle_ledger_operations.rs\");\n",
                    include_str!("v2_lifecycle_ledger_operations.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_ledger_store.rs\");\n",
                    include_str!("v2_lifecycle_ledger_store.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_ledger_tests.rs\");\n",
                    tests.as_str(),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the registry source exactly as Rust expands its reviewed provider.
pub(crate) fn reviewed_lifecycle_work_registry_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let recovered_wal =
                include_str!("v2_lifecycle_work_registry_recovered_wal.rs").replacen(
                    "include!(\"v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs\");\n",
                    include_str!(
                        "v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs"
                    ),
                    1,
                );
            let recovery = include_str!("v2_lifecycle_work_registry_validate_recovery.rs")
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_recovery_registry_impl.rs\");\n",
                    include_str!(
                        "v2_lifecycle_work_registry_validate_recovery_registry_impl.rs"
                    ),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_recovery_parent.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_validate_recovery_parent.rs"),
                    1,
                );
            include_str!("v2_lifecycle_work_registry.rs")
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_recovered_wal.rs\");\n",
                    recovered_wal.as_str(),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_recovery.rs\");\n",
                    recovery.as_str(),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_execution.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_validate_execution.rs"),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the adapter source exactly as Rust expands its reviewed provider.
pub(crate) fn reviewed_v2_adapter_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            include_str!("v2.rs").replacen(
                "include!(\"v2_authenticated_recovered_adapter_startup_impl.rs\");\n",
                include_str!("v2_authenticated_recovered_adapter_startup_impl.rs"),
                1,
            )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the runtime source exactly as Rust expands its reviewed providers.
pub(crate) fn reviewed_v2_runtime_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            include_str!("v2_runtime.rs")
                .replacen(
                    "include!(\"v2_runtime_effect_ownership_core_impl.rs\");\n",
                    include_str!("v2_runtime_effect_ownership_core_impl.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_runtime_effect_ownership_rebind_impl.rs\");\n",
                    include_str!("v2_runtime_effect_ownership_rebind_impl.rs"),
                    1,
                )
        })
        .as_str()
}
