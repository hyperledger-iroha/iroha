fn runtime_identity(kind: u8, byte: u8) -> CanonicalIdentityProjection {
    successor_identity(IDENTITY_DOMAIN_PROCESS_LOCAL, kind, byte)
}
fn valid_effect_candidate_projection() -> ProductionEffectToCandidateTraceProjection {
    let owner = runtime_identity(IDENTITY_KIND_RUNTIME_LIFECYCLE_OWNER, 0x11);
    ProductionEffectToCandidateTraceProjection {
        incoming_effect_kind: RUNTIME_EFFECT_KIND_STORE_BODY,
        stored_effect_kind: RUNTIME_EFFECT_KIND_STORE_BODY,
        incoming_candidate_kind: RUNTIME_CANDIDATE_KIND_STORE_BODY,
        stored_candidate_kind: RUNTIME_CANDIDATE_KIND_STORE_BODY,
        causality: RUNTIME_EFFECT_CAUSALITY_INHERIT,
        fresh_root_kind: 0,
        incoming_effect_position: 2,
        stored_effect_position: 2,
        incoming_effect_count: 3,
        stored_effect_count: 3,
        incoming_candidate_position: 1,
        stored_candidate_position: 1,
        incoming_candidate_count: 2,
        stored_candidate_count: 2,
        incoming_lifecycle_ordinal: 17,
        stored_lifecycle_ordinal: 17,
        incoming_effect_identity: runtime_identity(IDENTITY_KIND_RUNTIME_EFFECT, 0x22),
        stored_effect_identity: runtime_identity(IDENTITY_KIND_RUNTIME_EFFECT, 0x22),
        incoming_owner_identity: owner,
        stored_owner_identity: owner,
        parent_owner_identity: owner,
        incoming_candidate_semantic_identity: runtime_identity(
            IDENTITY_KIND_RUNTIME_CANDIDATE_SEMANTIC,
            0x33,
        ),
        stored_candidate_semantic_identity: runtime_identity(
            IDENTITY_KIND_RUNTIME_CANDIDATE_SEMANTIC,
            0x33,
        ),
        incoming_candidate_identity: runtime_identity(IDENTITY_KIND_RUNTIME_CAUSAL_CANDIDATE, 0x44),
        stored_candidate_identity: runtime_identity(IDENTITY_KIND_RUNTIME_CAUSAL_CANDIDATE, 0x44),
        candidate_owner_count_before: 0,
        candidate_owner_count_after: 1,
        candidate_owner_admitted: true,
        producer_episode_retained: true,
    }
}
#[test]
fn effect_to_candidate_kernel_rejects_identity_rank_and_owner_weakening() {
    let valid = valid_effect_candidate_projection();
    assert!(production_effect_to_candidate_refines_async_ownership_kernel(valid));
    assert_eq!(
        check_production_effect_to_candidate_transition(valid)
            .expect("exact first-owner candidate must mint evidence")
            .into_projection(),
        valid
    );
    let coalesced_retry = ProductionEffectToCandidateTraceProjection {
        candidate_owner_count_before: 1,
        candidate_owner_admitted: false,
        ..valid
    };
    assert!(production_effect_to_candidate_refines_async_ownership_kernel(coalesced_retry));
    for mutant in [
        ProductionEffectToCandidateTraceProjection {
            stored_effect_kind: RUNTIME_EFFECT_KIND_FETCH_BODY,
            ..valid
        },
        ProductionEffectToCandidateTraceProjection {
            stored_effect_identity: runtime_identity(IDENTITY_KIND_RUNTIME_EFFECT, 0x23),
            ..valid
        },
        ProductionEffectToCandidateTraceProjection {
            stored_candidate_kind: RUNTIME_CANDIDATE_KIND_FETCH_BODY,
            ..valid
        },
        ProductionEffectToCandidateTraceProjection {
            parent_owner_identity: CanonicalIdentityProjection::zero(),
            ..valid
        },
        ProductionEffectToCandidateTraceProjection {
            incoming_candidate_count: 4,
            stored_candidate_count: 4,
            ..valid
        },
        ProductionEffectToCandidateTraceProjection {
            candidate_owner_count_before: 1,
            candidate_owner_admitted: true,
            ..valid
        },
        ProductionEffectToCandidateTraceProjection {
            producer_episode_retained: false,
            ..valid
        },
    ] {
        assert!(!production_effect_to_candidate_refines_async_ownership_kernel(mutant));
        assert!(check_production_effect_to_candidate_transition(mutant).is_none());
    }
    let zero = CanonicalIdentityProjection::zero();
    let diagnostic = ProductionEffectToCandidateTraceProjection {
        incoming_effect_kind: RUNTIME_EFFECT_KIND_BROADCAST,
        stored_effect_kind: RUNTIME_EFFECT_KIND_BROADCAST,
        incoming_candidate_kind: RUNTIME_CANDIDATE_KIND_NONE,
        stored_candidate_kind: RUNTIME_CANDIDATE_KIND_NONE,
        incoming_candidate_position: 0,
        stored_candidate_position: 0,
        incoming_candidate_count: 0,
        stored_candidate_count: 0,
        incoming_candidate_semantic_identity: zero,
        stored_candidate_semantic_identity: zero,
        incoming_candidate_identity: zero,
        stored_candidate_identity: zero,
        candidate_owner_count_before: 0,
        candidate_owner_count_after: 0,
        candidate_owner_admitted: false,
        ..valid
    };
    assert!(production_effect_to_candidate_refines_async_ownership_kernel(diagnostic));
    let forged_diagnostic_owner = ProductionEffectToCandidateTraceProjection {
        candidate_owner_count_after: 1,
        ..diagnostic
    };
    assert!(
        !production_effect_to_candidate_refines_async_ownership_kernel(forged_diagnostic_owner)
    );
}
