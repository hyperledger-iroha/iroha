#[cfg(test)]
/// Ledger-local behavior and source-surface regressions.
pub(crate) mod tests {
    use super::*;
    #[test]
    fn production_owner_parent_surface_is_declaration_only() {
        let source = include_str!("v2_lifecycle_coordinator.rs");
        let declaration = source
            .split_once("// PRODUCTION_LIFECYCLE_OWNER_DECLARATION_BEGIN")
            .expect("production owner declaration begin marker")
            .1
            .split_once("// PRODUCTION_LIFECYCLE_OWNER_DECLARATION_END")
            .expect("production owner declaration end marker")
            .0;
        assert!(declaration.lines().count() <= 25);
        assert!(!declaration.contains("fn "));
        for retained in [
            "VerifiedHeightContext",
            "LifecycleCoordinator",
            "LifecycleWorkRegistryHolder",
            "CertifiedServePayloadStoreV1",
            "AuthenticatedCertifiedServePayloadRecoveryCut",
            "V2BodyStore",
            "V2ApplyService",
            "ProductionLifecycleAdapterStartupV1",
        ] {
            assert!(declaration.contains(retained), "owner dropped {retained}");
        }
    }
    #[cfg(feature = "bls")]
    /// BLS-backed storage-recovery and CompleteTip retirement regressions.
    pub(crate) mod durable_ready_fetch_recovery {
        include!("v2_lifecycle_ledger_tests_durable_recovery_01.rs");
        include!("v2_lifecycle_ledger_tests_durable_recovery_02.rs");
    }
    include!("v2_lifecycle_ledger_tests_frame_and_store.rs");
}
