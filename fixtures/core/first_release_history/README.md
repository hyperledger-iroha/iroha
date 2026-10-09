# Pinned first-release history

These canonical frames contain the node-test genesis and two certified `Log`
blocks from four validators. The manifest pins their bytes, block hashes,
complete World root and execution tip. Strict replay must reproduce every
stored certificate and the pinned State.

The genesis cutover removes the retired `kagemusha_mint_finality` parameter.
This history was regenerated through `capture_pinned_first_release_history`
with the current strict genesis model and four real validators. Its digest is
recorded in `specs/first_release_history_cutover.json`. Earlier signed frames
containing that retired field are rejected; no compatibility decoder is kept.
This local fixture cutover does not reset or change a live network.

The producer, replay check and regeneration procedure are defined in
[`specs/first_release_history_cutover.md`](../../../specs/first_release_history_cutover.md).
