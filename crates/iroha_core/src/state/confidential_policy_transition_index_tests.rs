    #[test]
    fn rebuild_uses_only_authoritative_pending_transitions() {
        let policy = policy_with_transition(
            ConfidentialPolicyMode::Convertible,
            ConfidentialPolicyMode::ShieldedOnly,
            41,
            Some(7),
            b"policy-index-rebuild",
        );
        let (definition_id, definition) = definition_with_policy("coin", policy);
        let mut world = World::default();
        world
            .asset_definitions
            .insert(definition_id.clone(), definition);
        world
            .confidential_policy_transition_index
            .insert((99, definition_id.clone()), ());
        world.confidential_policy_transition_counts.insert(99, 1);
        world
            .rebuild_confidential_policy_transition_index()
            .expect("valid authoritative transition rebuilds");
        let transition_index = world.confidential_policy_transition_index.view();
        assert!(transition_index.get(&(99, definition_id.clone())).is_none());
        assert_eq!(
            transition_index.get(&(41, definition_id.clone())),
            Some(&())
        );
        let transition_counts = world.confidential_policy_transition_counts.view();
        assert!(transition_counts.get(&99).is_none());
        assert_eq!(transition_counts.get(&41), Some(&1));
    }
