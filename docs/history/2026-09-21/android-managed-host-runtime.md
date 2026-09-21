# Android managed and host JNI validation

The September 21 integration executes all three maintained Android consumer
tasks with JDK 21, the JDK 8 API restriction intact, and the retained freshly
built macOS ARM64 JNI bridge. The earlier ordinary JVM and attestation-tool
results are recorded in [the runtime checkpoint](runtime-and-governance-integration.md).

The initial Android run collects 108 client tests: 79 pass and 29 fail because
their account-address validation now requires the Rust ABI-23 bridge. The wallet
and explicit host-native tasks are not reached. These failures expose stale test
classification, not permission to add a managed validator or skip assertions.

The coordinator adapter and Java explicit-chain-context suites now carry the
existing `host-native` tag. The explicit task discovers all tagged SDK tests,
instead of restricting discovery to the software key-manager class. It requires
the supplied native library and executes without cached results. Every original
test assertion is retained; the adapter's scripted endpoints remain mapping
controls and do not qualify native coordinator or hardware execution.

The fresh run uses:

```text
./gradlew :client-android:testDebugUnitTest \
  :kagemusha-wallet-android:testDebugUnitTest \
  :client-android:testDebugHostNative --console=plain --no-daemon \
  --max-workers=1 --offline --rerun-tasks --continue
```

All 133 tests pass in 57.97 seconds: 79 client managed tests, 24 wallet managed
tests and 30 host-JNI Kotlin/Java tests. Each task produces current, nonempty XML
results, with zero failures, errors or skips. The observed 1,410 Kotlin/shared
fixture inputs remain unchanged. Native bridge SHA256 is
`1f709bb8a1c87cda7525be9c329644c753ee6a6bed00a662819aada3c8d87518`;
the retained Rust fixture generator SHA256 is
`e2766a18f38c63a434fed3331b8dc9a0be3578bbfe5f7902cfc472d0d6a9607c`.

Failure and successful packets are
`target/first-release-kotlin-android-managed-runtime-20260921` and
`target/first-release-kotlin-android-managed-runtime-2-20260921`. The exact source
repair and unchanged index identities are retained under
`target/first-release-android-native-task-repair-20260921`.
An additional invocation with `IROHA_NATIVE_LIBRARY_PATH` absent exits 1 in
8.87 seconds at the task's explicit missing-library check. Its diagnostic is
retained in the same repair packet; it is an expected refusal, not a skipped
native test or a successful qualification run.

This is host consumer evidence. Android native packages, physical devices,
StrongBox, complete proof/hardware parity, signed reproducible artifacts and the
final immutable candidate remain unqualified. Later shared Rust integration
requires rebuilding native artifacts for that candidate.
