# SDK frame identity observations

The JSON records preserve 70 independently observed directions for 20 Rust
SDK and 15 executor-model owners. They are the records for those owners from
the original compiler capture (SHA-256
`59502ec34866cf345f33507ccd644c887a1f1331177fbf920eca6ff04168589a`), unchanged
byte for byte.
The committed array's SHA-256 is `4b9a2a72ee5ddeadd6aa3b571274b489c9bb69830e673af5cf7d33ea0d839d34`.

The original source fingerprint is
`2442fa4b5930be33511cd9bbc1ce83cc9c5d8c1bf13a71cd2ab9caf28742ef4f`.
All 70 selected identity probes pass. The original build retains three
instrumentation-only warnings for function-local probes. One production local
request-witness signing payload has separate same-scope capture and a shared
encoder candidate awaiting runtime qualification.

These records capture nominal names, frame roots and directional hashes, not
canonical frame bytes or container hashes. Existing signing, frame and rejection
fixtures remain independent checks. The shared Rust assertion module is compiled
as an ordinary test module by each consuming crate and remains in source-size
measurement. Package runtime and full-release qualification are separate.
