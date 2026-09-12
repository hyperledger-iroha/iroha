# IPA verification-envelope frame fixture

`open_verify_frame.v1.json` captures a valid two-coefficient Pallas polynomial
opening with transcript label `iroha-ipa-frame-v1`. The original prover and
verifier both accepted the proof before the codec declaration changed.

The fixture retains complete root, empty/populated Option, and empty/two-item Vec
frames, both original directional schema hashes, and all three nested bare
payloads. `OpenVerifyEnvelope` owns the frame identity;
`IpaParams`, `PolyOpenPublic`, and `IpaProofData` use their bare payload codecs.
No proof fields, transcript bindings or curve validation rules changed.

The capture used Rust 1.93.1 and source
`421b7af496538a6c1163abac477707ab0ed353a7416e26c2e8891ccd984e2567`.
All 341 original proof-crate sources matched the working tree before adding the
capture-only test. The canonical frame hash is
`75441f03503001a4618e52d70f1e388d`.

The tests in `src/norito_types/tests.rs` compare every captured frame, verify the
roundtripped proof and batch, and reject wrong identities and truncation. The
payload-only regression separately checks exact prefix consumption without a
frame-identity declaration. These focused fixtures do not qualify the complete
proof engine, Core/Torii execution, native backends or release artifacts.
