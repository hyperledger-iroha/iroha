use super::{PIPELINE_AUTO_WORKER_MAX, PIPELINE_AUTO_WORKER_MIN, resolve_pipeline_worker_threads};
#[test]
fn pipeline_parallelism_auto_is_bounded() {
    let expected = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .clamp(PIPELINE_AUTO_WORKER_MIN, PIPELINE_AUTO_WORKER_MAX);
    assert_eq!(resolve_pipeline_worker_threads(0), expected);
    assert!(resolve_pipeline_worker_threads(0) <= PIPELINE_AUTO_WORKER_MAX);
}
#[test]
fn pipeline_parallelism_preserves_explicit_workers() {
    assert_eq!(resolve_pipeline_worker_threads(32), 32);
}
