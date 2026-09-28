# Version diagnostic wire fixtures

`version_identity_frames.json` contains 20 canonical frames for RawVersioned,
UnsupportedVersion and their Option/Vec containers. Its SHA-256 is
`7e2771ac63826b0a67e2d1b1da196d5e6ac8967090c47885bbc9974f9961de14`.
The records retain compiler-observed names, both codec-direction hashes,
complete bare/frame bytes, header flags, lengths and alignment padding.

Versioned containers are Norito-only, so RawVersioned has one variant: opaque
Norito bytes under the explicit u32 tag 1. The retired JSON variant (tag 0) was
removed together with its rows; the retained Norito-byte frames are unchanged.
Empty, short and all-byte values remain valid diagnostic content.
UnsupportedVersion fixtures preserve version bytes 0, 1, 2 and 255; representing
diagnostic input does not make a version supported.
The two root declarations preserve the observed names `iroha_version::RawVersioned`
and `iroha_version::UnsupportedVersion`.

The public `wire_contract` suite checks complete decoded values, exact bytes,
wrong-owner/truncated/trailing frame rejection and recovery. Direct slice tests
also cover both advertised length layouts, every payload truncation, unknown
tags, canonical V1 rejection of alternate archives, typed field/depth/allocation
limits and caller-context restoration. The slice decoder reconstructs the
complete derived enum payload; it no longer substitutes a one-byte tag parser.

The rows are produced by `frame_rows()` in `wire_contract.rs`; the permanent
suite has no capture writer. These results do not qualify active identity
cutover or the full workspace/release candidate.

```sh
cargo test -p iroha_version --locked
```
