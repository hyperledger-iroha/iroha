// Genesis commit-time coverage for mock-clock advancement.
#[test]
fn genesis_commit_time_is_zero() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1500));
    let creation_time_ms =
        u64::try_from(time_source.get_unix_time().as_millis()).unwrap_or(u64::MAX);
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        creation_time_ms,
        0,
    );
    time_handle.advance(Duration::from_secs(12));
    let report = BlockCommitReport::new(&header, &time_source);
    assert_eq!(report.commit_time, Duration::ZERO)
}
