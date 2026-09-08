# Frame owner identity observations

`frame_owner_identity_observations.json` contains 54 independently observed
serializer/deserializer identities for 28 model frame owners. The two private
commitment-material owners implement only serialization; no decoder was added.

The observations came from the original codec before the frame-identity cutover,
source SHA-256 `728f117b6a49428099460043e0aa152947073d4581e773938d5f29b53328ecfe`.
All 54 selected original-code probes passed. The instrumented model test build
reported 91 `unnameable_test_items` warnings from unrelated function-local probes;
these warnings remain in the capture evidence and are not production qualification.

Fixture SHA-256:
`8d13cc2d1f69fe26ed544345faa2293b22d029685d124e574903cbd6dc89f4c5`.

Before adding the declarations, every owner item body was compared byte-for-byte
with that reference source. Co-located tests check nominal names, frame roots and
each captured direction. Existing payload roundtrips and signed-byte fixtures
remain separate checks; these observations do not establish release readiness.

`additional_frame_owner_identity_observations.json` preserves another 48
serializer/deserializer observations for 24 production owners whose typed frames
are exercised by the model test suite. They were collected from the same original
source and executable, and all selected probes passed. Two additional test-only
receipt probes were observed but are excluded from this production fixture.
Every production owner body again matched the reference byte-for-byte.

Additional fixture SHA-256:
`8c2d12849206a580b07fde3a25a5d78fdfa3a5091cb39a9144e6007787beb21e`.

`http_frame_owner_identity_observations.json` adds six original compiler
observations for the SDK's framed `DaIngestRequest`, `DaIngestReceipt` and
`ValidationFail` operations. All six retained original-code probes pass; the
item bodies and attributes match the original source. The validation error keeps
its actual `iroha_data_model::executor::model::ValidationFail` identity despite
its public reexport. HTTP fixture SHA-256:
`ed7393ee2bb6487a1f390c911770db49430a885af8ab39a3e60953f5b016570f`.

The shared test checker verifies all three immutable files, all 108 direction
records and all 55 distinct production owners. Malformed test-only frame producers use
explicit test identities and are qualified by their rejection tests separately.
