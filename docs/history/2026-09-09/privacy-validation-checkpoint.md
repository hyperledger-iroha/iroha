# Privacy validation checkpoint — 2026-09-09

These are bounded local observations from separate captured sources. The merge
changed 1,335 of the 8,821 native23 source inputs; its earlier receipts do not
qualify the merged checkout. No complete privacy release, independent signed
cryptographic audit, full GPU proof or deployment qualification is claimed.

## Native23 and the four-validator Kaigi attempt

The selected native build and optimized daemon build passed. Three separate
Kaigi preflight controls passed, including real governed proof fixtures and
proof corruption with preserved framing and instances. The network test
`four_validator_kaigi_private_lifecycle_replay_and_restart` then failed after
168.533 seconds: zero tests passed, one failed, none were ignored at execution.

All four validators observed height one with a nonempty block. The authoritative
status-height barrier then exhausted its existing 120-second deadline because
the blocking SDK facade rejected execution from a Tokio multi-thread runtime.
No governed-key registration, funding, Kaigi create/join/usage/leave, replay or
restart phase was reached. Test-owned cleanup completed for all four peers.
The logs also retain missing-proposal warnings; genesis alone is not a liveness
or workload qualification.

The completed merge uses the canonical async SDK status API at this barrier.
The minimum height, deadline and polling interval remain one, 120 seconds and
100 milliseconds. This source correction still needs fresh compiled regression
and four-validator execution.

## SDK20 Apple build and whole Swift failure

The official local integration builder completed all five optimized targets:
device arm64, simulator arm64 and x86_64, and macOS arm64 and x86_64. All five
thin-archive C links passed, along with the builder's host runtime control.
The two universal archives and device archive were published as one verified
ABI-23 XCFramework. The retained archives have these SHA-256 digests:

| Archive | SHA-256 |
| --- | --- |
| iOS arm64 | `c988f5ea1040480d7c1b89592ac14560640203abd13cbec5142ccb4ed88f8e61` |
| iOS simulator universal | `f1adcaa10d2c1a3605fefaac22cbae8af63cc48a53632a9c684847edccc771fa` |
| macOS universal | `3b0273bdfaeeeed0a6d4a15049a60df896068af7fca3a056921df7a88221a511` |

The official pin projection changed exactly three loader hashes in a separate
consumer capture; the other 19,461 captured entries were unchanged. Installed
artifact validation passed. The clean-source gate rejected the dirty candidate.
The separately authorized dirty-source whole Swift gate built the full package,
then started 48 XCTest cases: 46 passed, one failed, and one crashed unfinished.
None of the six privacy-witness tests ran.

The completed failure expected the lower-level missing-sentinel error from a
native entry point whose Rust owner returns unsupported address format. Four
multisig fixture assertions then failed, and the 256-member fixture trapped when
the Swift encoder converted the member count to `UInt8`. The final decoder and
Rust encoder both require a big-endian `u16` count. The actual OS crash report
and stack are retained with the failed whole-suite logs.

The coordinator started the dirty-source gate before the clean gate finished.
Their 164.507-second overlap is retained as a sequencing failure. Both actual
terminal results remain separate; this is not a passed sequential release gate.

## Integrated corrections and remaining checks

The reviewed Swift correction emits a checked, nonzero big-endian `u16` count,
preserves all existing controller fixture assertions, and adds boundary and
malformed-count regressions. The missing-sentinel assertion remains on the
syntax inspection API; native admission assertions match their Rust owner.
All three changed Swift files pass Xcode syntax parsing. Type checking and the
full unfiltered suite against a fresh source-matched framework remain pending.

Five BFV fixtures now use the canonical 64-slot identifier format, retaining
their existing assertions and checking complete output coordinates. Two new
material-only tests reach the real Core preflight without constructing bootstrap
outputs or proofs. One requires rejection at the unavailable production
qualification gate; the other retains all eleven malformed-material cases.
The earlier failed end-to-end test and the closed production gate are unchanged.
These new Rust tests have not yet executed.

The merged Cargo lock must be reviewed and provisioned through the canonical SDK
owner before the next Apple build. Old SDK20 source, lock, archives, failures and
consumer remain retained; they cannot supply provenance for changed source.

## Retained evidence

The ignored evidence root is
`target/privacy-release-evidence/2026-09-07-recovery/`. The independent Apple
review capsule is `sdk20-apple-postbuild-independent-review/review-capsule.json`
(`4a0f6d8ac6f498402d9217435ab9e8a59c72960348c81164b783cac4fc582e38`).
The Swift and BFV publication receipts are respectively
`merge24-swift-root-publication/result.json`
(`c4792e4601671e8f05e503896676d8e3e8eaad1fecd8358e77f79aa8b2710fa9`)
and `merge24-bfv-root-publication/result.json`
(`803b25a97f12cfaece26901ccf58eb88f519fec06040eb59c3a40b3f31e0b0c8`).
The independent native23 terminal review is
`dist/multilane-validation-20260907/kaigi-r23-terminal-runtime-independent-review/result.json`
(`ad6c5d2fb441e2cffe654d473f610d193fe9f33629e060ec6c0d71aaf0f2e4bc`).
These local receipts are not independently signed release attestations.
