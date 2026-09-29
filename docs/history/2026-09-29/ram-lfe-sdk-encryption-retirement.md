# RAM-LFE SDK encryption retirement

Date: 2026-09-29. Scope: current Kotlin implementation and maintenance of the
retiring Java duplicate. Secure replacement encryption and complete execution
proofs remain open under [ZK03/ZK07](../../../specs/zk_first_release_goals.md).

The server's refusal did not protect callers using local plaintext-encryption
helpers: those helpers still generated the insecure exact-lift ciphertext before
any request. Production Kotlin and Java arithmetic builders are now removed.
The existing high-level calls refuse before normalization, parameter decoding or
private arithmetic; public deterministic seed overloads are removed. Kotlin and
Java consumers of its canonical API receive `RamLfeEncryptionUnavailableException`
with stable code `ram_lfe_encryption_unavailable`. The retiring Java duplicate
uses `UnsupportedOperationException` with that code in its fixed message.
Neither diagnostic ciphertext nor a refusal qualifies a replacement algorithm.

`DiagnosticIdentifierBfvEnvelopeBuilder` exists only in each test source set.
Existing exact ciphertext, shared-vector and adversarial-parameter assertions
call it explicitly. The tests distinguish historical arithmetic from public
availability. New controls exercise all three plaintext entry points, empty and
oversized input, a fixed error without input disclosure, and Java-source calls
to Kotlin. Ciphertext DTO constructors remain separate from encryption and
production admission.

Kotlin's JSON response parsers and canonical Norito receipt codec share one exact
backend/mode table. Five response surfaces reject retired lowercase names and
unknown modes as well as casing and whitespace aliases. The Java duplicate
reuses its canonical codec mapping and no longer substitutes the resolver key
when `output_opening_public_key` is absent. An obsolete Kotlin test expecting a
plaintext opening from execute was replaced with rejection of a null retired
field; independent identifier-opening signature controls remain.

Normal Gradle `:core-jvm:jar :core-jvm:compileTestKotlin :core-jvm:compileTestJava`
passes on JDK 21 with JDK 8 API restrictions, 1,557 captured inputs and no drift.
The 5,525,551-byte production JAR has SHA-256
`0993b1b91e6e56f8e8e2977b3b437e081f5ba2be7db82b2c8bdf382c2aba8655`.
Its 2,389 classes contain neither the diagnostic/retired encryptor nor the old
PRG domain. `javap` confirms no public encryption seed overload. These are
compilation/artifact checks, **not runtime test passes**. The normal test task
still requires two canonical Sumeragi execution fixtures; their generator must
run rather than dropping Gradle inputs or inventing captures. The Java duplicate
has not received a new native/runtime qualification.

Evidence lives under
`dist/zk-remediation/2026-09-29/ram-lfe-production-boundary/`:
`sdk-encryption-retirement-source` retains the exact source amendment, while
`sdk-encryption-retirement-compile` retains the normal task log, source manifest,
JAR, exported API and separate result records. Later cross-language changes and
test results require their own receipts.
