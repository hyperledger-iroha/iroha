# F02 streamed exact ingress comparison, 2026-09-24

Scope: the existing `optimizations` checkout. Fair-ingress ownership already
retains the original bounded Norito bytes. `matches_message` previously
allocated a second message-sized `Vec` to compare the current decoded message
with those bytes. It now streams canonical serialization through a writer that
compares each write to the retained slice, rejects changed/overlong bytes, and
requires the complete original length. Global V2 uses its inner consensus
encoding; lane-local and auxiliary messages use their outer `BlockMessage`
encoding, matching the admission cut. No alternate decoder or wire layout was
added.

The combined-source Core test binary passes the two new exact-byte tests,
including first/middle/last byte changes and short/extended frames. The
adjacent fair-ingress owner test passes 1/1 and the canonical executed-body
worker selector passes 3/3. Signed-ballot and Native changes were present in
that same binary. Scoped formatting and `git diff --check` pass.

This removes the second output buffer from exact message comparison. Nested
Norito serialization scratch and work still need original-owner admission;
the canonical request frame-size check also still clones and encodes a request.
The worker's live ingress/output handoff is not connected by this cut. F02 and
F03 release gates remain open.

The first direct `cargo test --offline --locked` invocation failed while
compiling DataModel because it did not expose `MerkleMapProof` symbols in that
build configuration. The repository `scripts/cargo_fast.sh
--stable-local-metadata --incremental` test invocation built Core and passed
the focused tests above; no source change was made to accommodate the failed
direct invocation.
