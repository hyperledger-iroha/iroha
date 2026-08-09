/// Hard entry-count limit for the exact certified-artifact lookup used by
/// autonomous reservation reconciliation.
///
/// The lookup reads one bounded index without recovering or rewriting it. Its
/// cap deliberately matches the already-reviewed autonomous namespace file
/// limit so hostile sparse indices cannot turn startup classification into an
/// unbounded scan.
const MAX_AUTONOMOUS_RESERVATION_CERTIFIED_INDEX_ENTRIES: usize = 65_536;
