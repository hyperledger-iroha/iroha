# Java consumer cleanup for the ZK SDK checks

Date: 2026-09-29. The retiring Java settlement client now calls Kotlin-owned
`BlsNormalPublicKeyAdmission.isCanonicalBlsNormalPeerId` for both responder
attestations, matching the canonical Kotlin client. The three stale
`NativeAmxV2` references are removed without restoring the retired implementation.
Two removed V2 fixture files remained declared as Gradle inputs; independent
source inventories found no remaining Java or Kotlin consumer. Only those dead
input declarations are removed. Required current Kotlin native-execution
fixtures remain untouched.

Normal JDK-21 `:core:compileJava :core:compileTestJava` passes, including the
explicit identifier program/opening-key constructor changes. Normal `:core:test`
runs the complete selected HTTP transport and atomic-settlement harnesses:
2/2 fresh JUnit results, no failures, errors or skips. The task enables Java
assertions. The 1,609-file before/after source inventory is unchanged in each run.
Transport tests include the new required-policy metadata negatives, retained
BFV diagnostic vectors and public encryption-refusal controls. Settlement tests
exercise request/response handling with an injected verifier; these passes do
not qualify a native cryptographic verifier or a live network.

Ignored receipts are in `dist/zk-remediation/2026-09-29/` under
`java-bls-canonical-consumer` and `java-retired-consensus-inputs`. They retain
before/after patches, ordinary Gradle logs, source hashes and fresh JUnit output.
Canonical Kotlin runtime validation still requires real Kagami-generated native
execution fixtures; no input was dropped to bypass that requirement.
