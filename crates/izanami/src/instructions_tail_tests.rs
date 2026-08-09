    #[test]
    fn revoke_manifest_without_entry_sets_failure() {
        let PreparedChaos { mut state, .. } =
            prepare_state(2, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let mut rng = StdRng::seed_from_u64(3);
        let plan = state
            .plan_revoke_space_manifest(&mut rng)
            .expect("revoke plan builds");
        assert!(
            !plan.expect_success,
            "revocation without manifest should fail"
        );
    }

    #[test]
    fn domain_metadata_roundtrip_clears_tracking() {
        let PreparedChaos { mut state, .. } =
            prepare_state(3, None, None, WorkloadProfile::Stable, false).expect("state prepared");
        let set_plan = state.plan_set_domain_key().expect("set domain metadata");
        assert_eq!(set_plan.label, "set_domain_kv");
        assert!(
            !state.domain_metadata.is_empty(),
            "domain metadata should be tracked after setting"
        );
        let remove_plan = state
            .plan_remove_domain_key()
            .expect("remove domain metadata");
        assert_eq!(remove_plan.label, "remove_domain_kv");
        // Removal path may leave map empty or retain other keys, but it must not panic.
    }
