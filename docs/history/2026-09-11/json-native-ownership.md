# Native JSON ownership and terminal resource errors

This checkpoint belongs to the isolated composed candidate under
`target/architecture-redesign/model-base-extraction-v1/norito-context-asset-composed-v1/`.
It has not been cut over to the live workspace.

`Value::json_from_value` previously called the recursively derived `clone()`.
It now traverses borrowed values iteratively, retaining only the active ancestry
in a fixed-size inline array. The canonical 33-level JSON bound and any stricter
caller decode-depth limit are checked before descending. Arrays, object nodes,
keys and strings are admitted against decode budgets; vectors and string buffers
use the existing fallible allocation owners. Object allocation uses the existing
conservative standard-library B-tree node budget.

Reverse-order depth-guard cleanup restores the caller's scope on failure.
Partial owned values use ordinary iterative destruction. The implementation
preserves numeric variants and floating-point bits without serializing an
intermediate JSON document. Shared borrowed string copying also checks the
configured field-byte limit before allocation. Plain and escaped streamed strings
use that same limit on decoded UTF-8 bytes, including surrogate pairs and keys.

Core's existing JSON error wrapper now preserves terminal resource classification.
The canonical conversion back to JSON retains allocation failures and nesting
depth, limit and context. Syntax errors remain distinct. The ordinary derived
`Value::Clone`, `Debug` and equality operations remain recursive; untrusted
borrowed retention must use the fallible native decoding owner.

## Executed qualification

At source
`91a491fd6834b99202677d46432b5125052eb5b16db30e46597435342516a18f`:

- All 556 Norito library tests pass, with one existing ignored test.
- All four actual allocator/destructor tests pass. A 4,096-string native array
  allocates exactly its 4,097 retained buffers, with no temporary traversal
  storage. Empty containers and scalar copies allocate nothing.
- Four 32,768-level first/last-child array/object shapes return a bounded error
  and release every partial owned buffer. These run on four ordinary test
  workers, without increasing stack sizes.
- Strict Norito all-target Clippy passes.

The build profiler seals source inputs and compiler-selected artifact identities.
The runtime runner verifies source and executable hashes before and after
execution. These are focused codec results, not aggregate or release qualification.

Earlier checkpoints are retained accurately: build 1 rejected three private
test-field accesses; runtime 2 passed 553 library tests but failed the stricter
nesting-error assertion. Both failures have source-bound reports. Runtime 3
passed 554 library tests and four allocator tests before the additional nested
error regression was added. No failing assertion was waived.

A separate no-default-features source
`906e1118bcfb6a037fd2395800214e0cf0dff239a973a9c00c03c3a36b7e6c2a`
passes its library build and all 544 library tests, with one existing ignored
case. Its preceding check exposed five inconsistent JSON feature gates; the
already-unconditional JSON and telemetry interfaces now have unconditional
error/metrics owners as well. That selection predates the streamed-string
parity regression and does not qualify later source changes.

Independent read-only review
`query-native-copy-owner-review-v1` found no blocking defect in the active-frame
bound, guard cleanup, allocation ownership or typed error conversions. It records
the standard-library B-tree allocator limitation without treating budget
admission as fallible operating-system allocation. Review is separate from
runtime qualification.

TODO: Complete aggregate/context consumers and reconcile the qualified candidate
into the live source. Workspace, memory, native/device and four-validator release
qualification remain open.
