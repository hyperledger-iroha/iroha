# SDK frame identity observations

The immutable JSON records preserve 271 independently observed directions for
28 Rust SDK, 93 SCCP and 15 executor-model owners. They are the exact `records`
array from the original compiler capture (SHA-256
`59502ec34866cf345f33507ccd644c887a1f1331177fbf920eca6ff04168589a`).
The committed array's SHA-256 is `389cf8f47123437d56bc608f5989936bb7fdb1000da104e404dddc3cc73d6631`.

The original source fingerprint is
`2442fa4b5930be33511cd9bbc1ce83cc9c5d8c1bf13a71cd2ab9caf28742ef4f`.
All 271 selected identity probes pass. The original build retains three
instrumentation-only warnings for function-local probes. One production local
request-witness signing payload has separate same-scope capture and a shared
encoder candidate awaiting runtime qualification; two SCCP shortened test fixtures are not production
owners. The replay checkpoint inventory is serialization-only.

These records capture nominal names, frame roots and directional hashes, not
canonical frame bytes or container hashes. Existing signing, frame and rejection
fixtures remain independent checks. The shared Rust assertion module is compiled
as an ordinary test module by each consuming crate and remains in source-size
measurement. Package runtime and full-release qualification are separate.
