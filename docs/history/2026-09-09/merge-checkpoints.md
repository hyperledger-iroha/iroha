# Merge checkpoint history — 2026-09-09

These pre-merge observations retain their original scopes and limitations. They
do not qualify the merged candidate. The excerpt below is preserved verbatim
from the conflict-resolved status source before its current-view compaction.

Source status SHA-256: `3367c37deb44f449cf04f1a315e319cb275e5f34b02c7c492b15c0ed8ca04d6a`.
Excerpt SHA-256: `bf44a3cfd9864a1a133b6e0d8ecbfdf82e70fc5ee0bb912fab5e9df5a3a8ce93`.

[Current status](../../../status.md) · [History index](../README.md)

<!-- BEGIN EXACT PRE-MERGE CHECKPOINTS -->
The prior branch merge integrated the validated seven-field
Native AMX participant control, shared Torii configuration DTO, Kotlin-owned
Java API and explicit Norito identities. FASTPQ retains the exact-integer and
quotient/degree checks alongside the six-lane digest implementation; its current
raw transcript was regenerated and replayed successfully. Focused SDK, OpenAPI,
Norito and source-closure checks pass; the Rust workspace and Core/network
matrix have not been rerun. Swift execution still requires the ABI-23 bridge.
Receipt replay now disables bytecode writes into sealed inputs; the remaining
macOS Python framework-copy size/hash assertion fails in its isolated test.
The three OpenAPI schema copies agree, but release manifest provenance and
artifact digests require the clean-candidate replay before release. Those checks
do not qualify a release.

Combined privacy retry 22 passes the selected native build and ordinary daemon
build with all 8,821 captured inputs unchanged. All 61 CLI and 62 SoraFS daemon
tests pass against retained binaries. Manifest's 959 and Torii's 33 passing tests
remain retry-20 executions; Core's other 206 checks remain scoped to retry 19.
The four direct BFV-mode and Kaigi controls pass in retry 21; the original full
BFV bootstrap fixture remains unqualified. Kaigi's retry-22 network starts four
validators but times out before genesis commits, so lifecycle qualification is
still open. Fresh captured SDK source controls pass 284 tests, and its actual
JNI build passes 159 selected JVM tests plus the standalone Java utility.
The shipping JAR contains no retired privacy producers or test fixtures. Full
Swift execution and signed-source qualification remain pending. Scoped results
are recorded in the [privacy ledger](specs/privacy_first_release_closure.md).
Production proof size, full GPU proofs, remaining engines and audits remain open.

Kaigi's final model passes 33 tests, its proof circuits pass 20, and the fresh
JS-host native proof suite passes 21, including every 31/25-row mutation. All
three real Core Kaigi integration tests pass; its unit selection passes 96/97,
with a typed capacity-error assertion corrected for the next capture. Strict
Rust account decoding passes 42 cases across all ten Norito layouts. Updated
Kotlin/Java sources compile with JDK 8 API enforcement; three decoder/absence
tests pass. Earlier managed full-controller passes predate mandatory native
key admission. Python's separate native-owner test-target check passes, but
installed SDK wheels/addon/JNI/XCFramework execution remains pending. Fresh
Metal digest/Merkle/runtime tests pass 68 cases; complete GPU proofs, relay
transport and four-validator deployment remain unqualified.
See the [current privacy evidence](specs/privacy_first_release_closure.md).

<!-- END EXACT PRE-MERGE CHECKPOINTS -->

## Earlier Taira startup checkpoint

Retained verbatim from the incoming branch status at commit
`3cac1dc8d93ae0a3f5c5970001ce2131adc35263`.
This checkpoint predates the later native preparation and QueuePlan
application findings in [current status](../../../status.md).

Source status SHA-256: `de2d2bdbc0260843ee5991d8f8676fb3617d68bc1035473caefcb7aa17069ba0`.
Excerpt SHA-256 (without trailing newline): `218483edabb1a34f1c1286f309e5190d85eaeb6eef6ad36569ba61a242d23527`.

The latest Taira rollout failed before public cutover: daemon startup treated the QEMU executable as a directory during minimal-root attestation, so Torii never bound. A service stop during initialization also left a live nested QEMU outside its systemd unit cgroup. The exact orphan was stopped through a pidfd and native rollback completed with all four validators stopped. The directory/file fix and exact daemon-pidfd/cgroup watchdog are written. Host topology confirms bubblewrap’s retained PID1 reaper and QEMU share the private root and namespaces; compiled startup and supervisor-death validation remain pending. Prior standalone host probes did not exercise the complete daemon attestation path. No live finality, workload canary or application rollout is qualified.
