# Generated query identity capture

`query_generated_identity_frames.json` records the current 127 query types and
171 typed payloads. Each case retains canonical JSON and complete root, `Vec`,
`Option` and `BTreeMap<u8, T>` frames (684 frames in total), together with the
actual native compiler name and both directional codec identity hashes.

Fixture SHA-256: `78e23cd91a3a2db7d5c10d71faf21d1568f174719accc1345638c5f9c5b67e35`.

The scoped contract-manifest query is `FindContractManifestByArtifactId`. Its
native capture binds `DataSpaceId::new(u64::MAX)` and the canonical contract hash
in every frame; the retired hash-only query has no alias or fallback decoder.
The capture uses the same typed JSON and four-context decode/re-encode checks as
the permanent fixture test, without constructing bytes from nominal names.

The ignored `print_scoped_manifest_query_identity_frame` maintenance test prints
the native scoped row for explicit review. The permanent
`generated_queries_preserve_captured_frames` test compares the current catalog's
exact identity, both codec hashes, canonical JSON and all four frame contexts
against this fixture. These codec checks do not establish live query execution
or release qualification.
