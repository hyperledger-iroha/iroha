# Kotlin KAGEMUSHA protocol tests — 2026-09-22

The fresh `kotlin-core-kagemusha-r1` run **failed: 57 tests executed, 33 passed,
24 failed, zero skipped**, in 106.953 seconds (Gradle exit 1). Kotlin and test
compilation completed. Independent inspection of all eight saved JUnit XML files
matches every case in `result.json`; all 24 failures are
`AccountAddressException` with the same message:

> Account addresses require the complete ABI-23 Rust address validator from connect_norito_bridge

Every failure stack enters `AccountAddressNative.unavailable` from
`validateCanonical`. These results establish an unavailable native address
validation boundary in this runtime, not 24 distinct KAGEMUSHA assertion defects.
The wrapper maps both link and state failures to that message; the reports do not
establish whether a native file was absent, incompatible or otherwise unusable.
The affected test behaviors remain unqualified until rerun successfully.

| Test class (prefix `Kagemusha`, suffix `V1Test`) | Passed | Failed |
| --- | ---: | ---: |
| CoreCoordinatorArchive | 5 | 1 |
| CoreCoordinatorFrame | 6 | 0 |
| DeviceMintReplyCodec | 2 | 0 |
| EnrolledOpenChallenge | 1 | 10 |
| EnrolledOpenSelector | 0 | 8 |
| OperationIntent | 13 | 0 |
| OperationReservation | 1 | 4 |
| ThreeMessage | 5 | 1 |

## Command and source boundary

Run from `/Users/takemiyamakoto/dev/iroha/kotlin`, branch `optimizations`, using
the recorded JDK 21 home
`/opt/homebrew/Cellar/openjdk@21/21.0.12.1/libexec/openjdk.jdk/Contents/Home`:

```sh
./gradlew :core-jvm:test --tests 'org.hyperledger.iroha.sdk.offline.Kagemusha*' --rerun-tasks --offline --console=plain
```

The wrapper configuration selects Gradle 9.3.0. The runner resolves JDK 21 via
`/usr/libexec/java_home -v 21`, sets `JAVA_HOME`, and removes
`MOBILE_SDK_ANDROID_ARTIFACT_DIR` from its inherited environment. No Android test,
native rebuild, Cargo feature selection or physical-device run is part of this
command. Its saved context describes protocol-test intent; account validation in
this selection nevertheless requires the native bridge.

The two captured source maps are byte-identical: **699 inputs** (690 Kotlin-tree
source/build files, one Gradle configuration and eight offline fixtures). Their
hashes also matched the corresponding checkout files when this note was checked.
The map contains no Rust sources, Cargo manifests or Cargo lockfiles, and excludes
build directories. It therefore binds this SDK source window, **not the Rust
native dependency graph or a native binary**. No native artifact hash or native
build-feature evidence is recorded here. The recorded Git HEAD is
`8209033deae7409d800fcc576a63609a0c2c5f9d`; that value alone does not identify
the complete working-tree candidate.

## Receipts

Receipt root: `target/kagemusha-validation/20260922/kotlin-core-kagemusha-r1/`.
`junit-sha256.json` was added during this independent report check and records
the eight saved XML hashes; it is not an original runner output.

| Artifact | SHA-256 |
| --- | --- |
| `result.json` | `94c86710c2d6e1924d5c851070741950eeb33abb18863f98ced74bc9ffd12c67` |
| `context.json` | `5722821690582dee926d44d3d9ea7fe52b4324c1974501a1a9353178eba76880` |
| Both source maps | `8adf7c4fe9545ce2a9dd968b081f06787fd69aa53751c83b46e2c5455efabebb` |
| `stdout` | `5b0a14dd83925779d6ced17c084283a7f58d103a7bbb48f7521b0e27799745ad` |
| `stderr` | `19a46394e930950a227de0c3d668d14d885f493a9a338b415a15bbb99b05619d` |
| `junit-sha256.json` | `ca19a01a3a1b20b1d88c4710ed0970a4672dc26811d5851b7ca9a74600a63165` |
| Parent `run_kotlin_kagemusha.py` | `c41bd3960bacda7a45e65ad40ccb7064e4096247cecd9245540481371c3c91eb` |

The next gate is a current-source `connect_norito_bridge` rebuild with its exact
Rust dependency/feature graph and artifact identity retained, followed by this
same selection using that isolated artifact via `IROHA_NATIVE_LIBRARY_PATH`.
The SDK requires ABI 23 and native signer contract revision 5
(`NativeSignerBridge.kt:13`, `:134`); `core-jvm/build.gradle.kts:117` configures
the library directory. Preserve mandatory Rust address validation; do not bypass
it, suppress these failures or mark the affected tests skipped. A successful host
rerun would still leave Android/JNI consumer, native release-provenance,
performance and physical-device qualification as separate gates.

See [current readiness](../../../../specs/kagemusha_v1_production_readiness.md).
