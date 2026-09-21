# SCCP Java source consumer integration

The reviewed 19-file migration is applied with exact before/after hashes in
`target/first-release-sccp-java-consumer-applied-20260921/identity.json`.
The original Git index and the independent native build's recorded inputs are
unchanged. The source uses `java-source-kotlin` without compatibility dispatch.

Kotlin owns one shared Java test source compiled by both JVM and Android host
tests. All 31 original SCCP assertion groups remain; the two public-write API
absence assertions move to a compiled-class checker that traverses inherited
members and validates the selected JDK's actual Java 8 terminal signatures.
The superseded Java suites and sponsor wrapper are removed. Other Java-only
capabilities remain in the consolidation inventory until their own migration.

The canonical Python run passes **472 tests** in 15.29 seconds. Its **23 skipped
tests** originally lacked the native SCCP release validator; that failed
prerequisite is preserved separately from the subsequent source-matched run.
All observed applied inputs remain unchanged. The packet's earlier 31-group
Java run uses captured Kotlin classes and a prior JNI binary; it is diagnostic
evidence, not current JVM/Android package qualification.

Inspection after integration found a required consumer missing from the initial
packet: `sccp_release_evidence.rs` still requires `java-android` in its exact
phase inventory. The reviewed successor
`target/first-release-sccp-native-phase-join-20260921` updates that native owner,
adds retired-name/duplicate-evidence rejection controls and checks agreement
between the Rust and Python inventories. Its source agreement control passes;
the exact two-file successor is now applied as part of
`target/first-release-s-stream-event-storage-applied-20260921`. All 31 corridor
controls pass, including source agreement. The separate production-feature native
build now passes both exact-envelope tests, including the Java-source phase and
retired/duplicate phase rejection. Its rebuilt validator then executes the full
release-tooling/corridor selection: **297 passed, zero skipped**, including the
previously unavailable native-backed controls. The combined packet is
`target/first-release-s-stream-event-native-20260921`; its observed source remains
unchanged. This joins the native phase migration, not final release evidence.

Fresh source-matched JNI, both actual Gradle runs and their exact reports,
physical-device execution, the distinct Java-consumer qualification artifact
and complete signed twelve-phase release evidence remain open.

## Actual-package producer and bounded evidence ownership

The eleven-path Java-source producer and its four-path custody successor are
integrated, with dependency-ordered exact images, preserved originals and an
unchanged Git index. All **384 canonical tooling controls pass**, with no skips;
`target/first-release-java-consumer-applied-20260921` retains the receipt.
The shared Java test source preserves all 25 original assertion bodies for both
JVM and Android host consumers. The producer consumes actual JAR/AAR and native
ABI23 artifacts, checks loaded class/library origins, and retains exact inputs.

Independent review reproduced two prior failures: captured manifests were
reopened before use, and direct JVM file logging allowed rotation and output
outside the live capture cap. The successor parses original captured bytes and
uses one bounded stdout/stderr owner for diagnostics, class/library logs and the
fixed runner's report. Missing, repeated, truncated or oversized report frames
and unknown output files reject. The independent review reruns all 33 new
custody controls successfully; original failures are preserved in their target
review packet.

The actual JDK runner diagnostics use explicit synthetic test groups and verify
complete/failed/skipped/missing/extra report behavior. They do not execute the
final SDK candidate. Matching final JAR/AAR/JNI packages, actual full producer
execution, physical devices, five remaining package producer joins, the strict
six-consumer inventory and its signed aggregate remain release requirements.
