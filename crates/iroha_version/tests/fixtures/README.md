# Version diagnostic wire fixtures

`version_identity_frames.json` contains 26 actual pre-declaration canonical
frames for RawVersioned, UnsupportedVersion and their Option/Vec containers.
Its SHA-256 is
`53a756884c0be09642986acc5b014010fe34b2ad93942689d68329d1e18769c4`.
The records retain compiler-observed names, both codec-direction hashes,
complete bare/frame bytes, header flags, lengths and alignment padding.

RawVersioned keeps explicit u32 tags: 0 for opaque JSON text and 1 for opaque
Norito bytes. Empty, Unicode/NUL-containing and all-byte values remain valid
diagnostic content. UnsupportedVersion fixtures preserve version bytes 0, 1,
2 and 255; representing diagnostic input does not make a version supported.
The two root declarations preserve the observed names `iroha_version::RawVersioned`
and `iroha_version::UnsupportedVersion`.

The public `wire_contract` suite checks complete decoded values, exact bytes,
wrong-owner/truncated/trailing frame rejection and recovery. Direct slice tests
also cover both advertised length layouts, every payload truncation, unknown
tags, canonical V1 rejection of alternate archives, typed field/depth/allocation
limits and caller-context restoration. The slice decoder reconstructs the
complete derived enum payload; it no longer substitutes a one-byte tag parser.

The capture producer SHA-256 is
`d1fd0f67a543046d5fa2f897a6a73b920fff9c0e7ac96e5ef96950e6df6bb83e`.
Exact producer/source and the genuine pre-fix failure are retained under local
`target/architecture-redesign/owned-storage-identity/model-identity-closure/version-frame-closure/`.
The permanent suite has no capture writer. All 16 default version/derive test
executions and all 11 minimal-version executions pass on 19,343 unchanged
inputs without a stack override; shared tests run in both selections. Strict
all-target Clippy passes for both version selections. These results do not
qualify active identity cutover or the full workspace/release candidate.

```sh
cargo test -p iroha_version -p iroha_version_derive --locked
cargo test -p iroha_version --no-default-features --lib --test wire_contract --locked
```
