    #[test]
    fn primitive_projection_cannot_hide_a_safety_violation() {
        let before = reducer();
        let after = before.clone();
        let event = Event::RetransmitElapsed {
            tag: before.current_tag(),
        };
        let mut projection = before.transition_projection(&event, &after, &[]);
        assert!(refinement::accepts(projection));

        projection.safety_before.invalid_pending_append = 1;
        assert!(!refinement::accepts(projection));
    }

