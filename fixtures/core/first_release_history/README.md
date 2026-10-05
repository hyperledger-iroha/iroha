# Pinned first-release history

These canonical frames contain the node-test genesis and two certified `Log`
blocks from four validators. The manifest pins their bytes, block hashes,
complete World root and execution tip. Strict replay must reproduce every
stored certificate and the pinned State.

The 2026-10-05 merge cutover regenerated this local history through
`capture_pinned_first_release_history` because the previous pin's genesis
certificate differed from current execution. This is a fresh first-release
history; no earlier layout or execution fallback is accepted. Its digest is
recorded in `specs/first_release_history_cutover.json`. No live network was
reset or changed.

The producer, replay check and regeneration procedure are defined in
[`specs/first_release_history_cutover.md`](../../../specs/first_release_history_cutover.md).
