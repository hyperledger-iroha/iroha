//! Inspect exact unpublished transition bytes without inventing retained-history authority.
use super::*;

fn unique_staged_bytes<'a>(writes: &'a [(StatePath, Vec<u8>)], path: &StatePath) -> &'a [u8] {
    let mut matching = writes.iter().filter(|(candidate, _)| candidate == path);
    let bytes = &matching.next().expect("required staged row").1;
    assert!(matching.next().is_none(), "staged row must be unique");
    bytes
}

/// Decode only the exact staged head, immutable row and state for test assertions.
///
/// This does not validate or install a complete history, including in capacity-tail fixtures.
pub(crate) fn staged_control<P: CustodyPurpose>(
    writes: &[(StatePath, Vec<u8>)],
    deployment: &str,
) -> NativeControl<P> {
    let index: ControlIndexV1 = decode(unique_staged_bytes(
        writes,
        &control_head_key::<P>(deployment).unwrap(),
    ))
    .unwrap();
    let record = decode(unique_staged_bytes(
        writes,
        &control_record_key::<P>(deployment, index.revision).unwrap(),
    ))
    .unwrap();
    let fields = P::record_view(&record);
    assert_eq!(fields.deployment, deployment);
    assert_eq!(fields.revision, index.revision);
    assert_eq!(control_digest::<P>(&record).unwrap(), index.digest);
    assert_eq!(fields.execution.view().height, index.height);
    assert_eq!(fields.execution.view().ordinal, index.ordinal);
    let height_index: ControlIndexV1 = decode(unique_staged_bytes(
        writes,
        &control_height_key::<P>(deployment, index.height, index.ordinal).unwrap(),
    ))
    .unwrap();
    assert_eq!(height_index, index);
    let state = decode(fields.control_state).unwrap();
    NativeControl {
        record,
        state,
        index,
    }
}
