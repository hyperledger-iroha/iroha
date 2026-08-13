#[test]
fn moderation_snapshot_defaults_equal_native_query_ceilings() {
    let config = SorafsModerationOrchestrator::default();
    assert_eq!(config.max_cases, MODERATION_QUERY_MAX_CASES_V1);
    assert_eq!(config.max_events, MODERATION_QUERY_MAX_EVENTS_V1);
}
#[test]
fn moderation_snapshot_native_query_ceilings_parse_exactly() {
    let mut config = valid_config();
    config.max_cases = MODERATION_QUERY_MAX_CASES_V1;
    config.max_events = MODERATION_QUERY_MAX_EVENTS_V1;
    let mut emitter = Emitter::new();
    assert!(config.parse(true, &mut emitter).is_some());
    assert!(emitter.into_result().is_ok());
}
#[test]
fn moderation_snapshot_bounds_reject_each_native_ceiling_plus_one() {
    for configure in [
        |config: &mut SorafsModerationOrchestrator| {
            config.max_cases = MODERATION_QUERY_MAX_CASES_V1 + 1;
        },
        |config: &mut SorafsModerationOrchestrator| {
            config.max_events = MODERATION_QUERY_MAX_EVENTS_V1 + 1;
        },
    ] {
        let mut config = valid_config();
        configure(&mut config);
        let mut emitter = Emitter::new();
        let _ = config.parse(true, &mut emitter);
        let diagnostic = format!(
            "{:?}",
            emitter
                .into_result()
                .expect_err("native query ceiling overflow must fail")
        );
        assert!(diagnostic.contains("must be within 1..="));
    }
}
#[test]
fn moderation_snapshot_bounds_reject_each_zero_independently() {
    for configure in [
        |config: &mut SorafsModerationOrchestrator| config.max_cases = 0,
        |config: &mut SorafsModerationOrchestrator| config.max_events = 0,
    ] {
        let mut config = valid_config();
        configure(&mut config);
        let mut emitter = Emitter::new();
        let _ = config.parse(true, &mut emitter);
        assert!(emitter.into_result().is_err());
    }
}
