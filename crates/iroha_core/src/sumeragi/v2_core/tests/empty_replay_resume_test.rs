// Empty-WAL replay coverage for single-application and duplicate-resume semantics.

#[test]
fn empty_replay_resume_is_one_applied_stutter_then_duplicate() {
    let context = context();
    let mut recovered = Reducer::recover(
        context,
        Some(id(1)),
        Generation::new(21),
        std::iter::empty(),
    )
    .unwrap();
    let resumed = resume_after_replay(&mut recovered);
    assert_eq!(resumed.disposition(), StepDisposition::Applied);
    assert!(resumed.effects().is_empty());

    let duplicate = resume_after_replay(&mut recovered);
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(duplicate.effects().is_empty());
}
