# norito-java

`norito-java` is a pure-Java (JDK 25-ready) reference implementation of the
Norito serialization codec. It mirrors the behaviour of the Rust and Python
implementations so tooling can construct and inspect deterministic payloads
without native dependencies.

## Features
- Header parsing/serialization (`NRT0`, major 0 with fixed v1 minor `0x00`) with CRC64-XZ validation
- Flag support: packed sequences, compact lengths, packed structs, field bitset
- Adapters for signed/unsigned integers, booleans, UTF-8 strings, byte arrays,
  options (`Optional`), results (`Result.Ok`/`Result.Err`), sequences, maps, and
  packed structs with hybrid bitset layout
- Schema hashing (type-name and structural descriptors using Norito's canonical JSON rules)
- Columnar helpers and adaptive AoS layouts for `(u64, String, boolean)`,
  `(u64, bytes)` (including optional bytes), and `(u64, enum(Name|Code), boolean)` rows
- Compression profiles (`CompressionConfig.zstdProfile`) that choose Zstandard levels
  automatically for `"fast"`, `"balanced"`, or `"compact"` workloads.
- CLI inspector `NoritoDump` for quick header introspection
- Streaming resume helpers (`KeyUpdateState`/`ContentKeyState`) plus baseline RLE
  block decoding with explicit end-of-block validation.

## Rust trait parity
The JVM binding follows the Rust `NoritoSerialize`/`NoritoDeserialize`
contracts and the ergonomics exposed through `norito::codec::Encode`/`Decode`.
`NoritoCodec.encode(...)` and `NoritoCodec.decode(...)` therefore align with
the trait guidance refreshed in `norito.md`, keeping high-level callers in sync
across Rust, Python, and Java.

### Limitations (v0.1.0)
- Optional Zstandard compression requires `com.github.luben:zstd-jni` on the classpath; when
  absent, compression requests raise a clear `UnsupportedOperationException`.
- Columnar helpers currently cover the `(u64, String, boolean)`, `(u64, bytes)` (optional bytes),
  and `(u64, enum(Name|Code), boolean)` shapes; optional string/u32 and bytes+bool combos remain
  to be implemented in the Java binding.

## Build & Test
This module avoids external build tools. Compile with `javac` and run the
assert-based test harness:

```bash
cd java/norito_java
./run_tests.sh
```

By default the script uses `javac --release 21` when available, falling back to
`javac` defaults. Enable assertions (`-ea`) to ensure test checks run.

## Usage Example
```java
import java.util.List;
import org.hyperledger.iroha.norito.*;

TypeAdapter<List<Long>> adapter = NoritoAdapters.sequence(NoritoAdapters.uint(64));
byte[] payload = NoritoCodec.encode(List.of(1L, 2L, 3L), "iroha.demo.Numbers", adapter);
List<Long> decoded = NoritoCodec.decode(payload, adapter, "iroha.demo.Numbers");
```

Run the CLI to inspect a payload header:

```bash
javac -cp src/main/java src/main/java/org/hyperledger/iroha/norito/NoritoDump.java
java -cp src/main/java org.hyperledger.iroha.norito.NoritoDump contract.to
```

## Packaging
Publish the artifact locally via Gradle and reference the Maven coordinates:

```bash
cd java/norito_java
./gradlew publishToMavenLocal -PnoritoJavaVersion=0.1.0
```

Use the published coordinates (group `org.hyperledger.iroha`, artifact `norito-java`):

```kotlin
repositories {
    mavenLocal()
}

dependencies {
    implementation("org.hyperledger.iroha:norito-java:0.1.0")
}
```

```xml
<dependency>
  <groupId>org.hyperledger.iroha</groupId>
  <artifactId>norito-java</artifactId>
  <version>0.1.0</version>
</dependency>
```

### Bundling `zstd-jni`
`CompressionConfig.zstdProfile` and `CompressionConfig.zstd(level)` call into
`com.github.luben:zstd-jni` when Zstandard compression is requested. Add the
dependency to your build so the native library is on the classpath:

```kotlin
// build.gradle.kts
dependencies {
    implementation("com.github.luben:zstd-jni:1.5.6-9")
}
```

On Android, ensure the required ABIs are packaged. For example:

```kotlin
android {
    defaultConfig {
        ndk {
            abiFilters += setOf("arm64-v8a", "armeabi-v7a")
        }
    }
    packaging {
        jniLibs.keepDebugSymbols += "**/libzstd-jni.so"
    }
}
```

Server-side or desktop JVM deployments only need the dependency on the classpath; no additional
steps are required. `NoritoCompression.hasZstd()` returns `true` when the native library is
available and can be used for smoke tests or feature gating.

## Maintenance & Sync
Changes to the Rust Norito crate require corresponding updates in both
`python/norito_py` and `java/norito_java`. CI and `cargo build -p norito` run
`scripts/check_norito_bindings_sync.sh` (which forwards to a cross-platform Python helper) to enforce this by executing schema-hash
and columnar parity checks across bindings. Trimmed source distributions that
omit the sync script automatically skip the guard; no manual override is
available.

> Streaming manifests, control frames, and telemetry adapters now ship under
> `NoritoStreaming`, while baseline RLE decoding and resume state helpers live
> in `NoritoStreamingCodec` and `NoritoStreaming` respectively.

### October 2025

- Reviewed parity after Rust Norito refactored ChaCha20/XChaCha20 key and
  nonce helpers. No updates were necessary in the Java bindings; this note
  documents the sync point for the binding check.

### December 2026

- Rust Norito restricted debug-only environment toggles (`NORITO_TRACE`,
  `NORITO_DISABLE_PACKED_STRUCT`, GPU/parallel stage1 cutover shims) to
  debug/test builds and now uses fixed defaults in release mode. The Java
  binding exposes no environment shims, so behaviour remains aligned; this
  note records the parity check for the release-mode change.

### December 2027

- Revalidated parity after the Rust Norito bare decode fallback for exact-length
  drift; Java bindings required no code changes beyond rerunning the parity
  suite.

## Installation (JDK 25 on macOS)
Homebrew’s Temurin cask ships the current GA JDK 25 release:

```bash
brew update
brew install --cask temurin
```

Select the installed JDK and verify:

```bash
export JAVA_HOME=$(/usr/libexec/java_home -v 25)
java -version
```

Alternatively, download Oracle’s macOS bundle from
https://www.oracle.com/java/technologies/downloads/ and set `JAVA_HOME` via
`/usr/libexec/java_home -v 25` after installation.

## License
Licensed under the Apache License, Version 2.0. See `LICENSE` for details.
