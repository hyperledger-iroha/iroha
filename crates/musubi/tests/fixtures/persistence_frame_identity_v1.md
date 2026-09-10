# Musubi persistence frame identities

The thirteen explicit owners retain the names measured with Rust 1.93.1 and the
original Norito serializer and decoder. Their fields, variant tags, signing,
validation, resource limits, filesystem behavior and journal transitions are
unchanged. Nested payload-only records do not acquire frame identities.

`persistence_frame_identity_v1.json` has SHA-256
`b8272df8c7f1a7d63ccb4300cc52956222c9463cf7a3b7cf26a90a206549865f`.
It preserves both directions for thirteen roots and their Option/Vec identities
(39 distinct identities, 78 directional observations), plus 32 complete frames.
All owners have Option(None) and Vec(empty) frames. The existing
`registry_cache::tests::image("apps.sora", 10, 10)` snapshot and the production
empty catalog default also retain complete root, Option(Some), and Vec(two)
frames. The cache fixture constructors and all 39 local codec declarations are
copied exactly into the bounded original probe, including 26 payload-only
children. The snapshot's model anchor and page validation pass in the probe.

The untracked capture is
`target/architecture-redesign/sdk-musubi-capability/persistence-frame-reference-v1`.
Its seal is `6e7c6ff1c04a77515cbbfb1a60681235bff139ed07f573ed39f3e3718f47f51e`;
the successful executable is
`2ed805c71efe17a64caf87d7b4595cab838ba2bcd8067de056be26ac7693d66c`.
The probe retains the exact `musubi::{publish,publication_runtime,registry_cache}`
module scopes and uses copied original dependencies from the separately sealed
original build of source
`e76dbf9a351009aaf0a7f398cb9001f01ed7f0f9544d49a5e847eee75ae537d3`.
Compiler events, dependency/native artifacts, source spans and runtime output are
retained. Four unused-import warnings belong only to the bounded reproduction.
An initial fixture-selector preparation failure and repeated preliminary capture
remain recorded; neither adds coverage to the final result.

This is codec preservation evidence, not execution of the full publication
workflow. Populated publication roots are exercised by the existing consumer
suite, not represented as newly captured populated root goldens. Permanent tests
check both root hashes, exact container composition and bytes under alternate
ambient flags, cache fixture roundtrips, truncation, suffix and substituted-root
rejection. Full Musubi runtime qualification remains a separate consumer result.
