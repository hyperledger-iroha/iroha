# Norito Java Library Design (JDK 21)

## Goals
- Provide a pure-Java reference implementation of the Norito serialization
  codec that mirrors the Rust implementation semantics and the Python port.
- Target JDK 21 with one pinned, direct `zstd-jni` dependency for the release compression codec.
- Offer composable adapters for encoding/decoding common types (primitives,
  strings, byte slices, options, results, sequences, packed structs) and expose
  high-level helpers for typical usage.
- Maintain deterministic behaviour and identical header/CRC handling to Rust and
  Python implementations.

## Scope for v0.1.0
- Header support: encode/decode `NoritoHeader` with validation of magic, version
  (major 0, minor 0x00), payload length, checksum (CRC64-XZ), flags, and
  compression byte.
- Compression: support `COMPRESSION_NONE` and Zstandard through the direct, pinned
  `com.github.luben.zstd.Zstd` dependency. The encoder and decoder have no reflective discovery or
  optional-backend branch.
- Flag support: `PACKED_SEQ`, `COMPACT_LEN`, `PACKED_STRUCT`, and `FIELD_BITSET`
  mirroring the Rust flag byte values. Reserved layout bits are rejected. The
  Java defaults mirror Rust by enabling compact per-value lengths
  (`DEFAULT_FLAGS = COMPACT_LEN`, `0x02`); callers may explicitly select the
  fixed-width V1 layout with flags `0x00`.
- CRC64 implementation: table-driven CRC64-XZ (reflected ECMA polynomial) matching Rust/Python.
- Varint helpers: 7-bit LEB128 encoding/decoding for compact length prefixes.
- Type adapters: generic interface `TypeAdapter<T>` with concrete adapters for
  unsigned/signed integers (8–64 bit), booleans, UTF-8 strings, byte arrays
  (variable and fixed-length), optional values, result values, sequences
  (packed/delimited layouts), maps (sequence of key-value tuples), and packed
  structs using the hybrid bitset layout.
- Struct support: `StructAdapter` encodes exact Map-backed values and supports typed decode
  factories. Missing fields and object-property discovery are rejected; size calculations follow
  the Python implementation (bitset + varint sizes for non-self-delimiting fields).
- High-level API: `NoritoCodec.encode(value, schema, adapter, flags)` and
  `NoritoCodec.decode(bytes, adapter, schema)` with builder helpers exposed via
  `NoritoAdapters` (static factory methods).
- Streaming helpers: `NoritoStreaming` provides enums, records, and adapters for
  NSC manifests, telemetry, and control frames, sharing discriminant ordering
  and fixed-length hash/signature handling with the Rust codec.
- Streaming resume parity: `KeyUpdateState`/`ContentKeyState` snapshot helpers
  plus baseline RLE block decoding with explicit end-of-block validation.
- Columnar helpers: NCB/AoS layouts for `(u64, String, boolean)`,
  `(u64, Optional<String>, boolean)`, `(u64, Optional<u32>, boolean)`,
  `(u64, bytes)`, `(u64, bytes, boolean)` (including optional bytes), and
  `(u64, enum(Name|Code), boolean)` rows.
- Schema hashing: first 16 bytes of domain-separated SHA-256 over the canonical type name.
- CLI utility: `NoritoDump` prints header fields for inspection.
- Tests: standalone harness under `src/test/java` covering header roundtrips,
  encode/decode for primitives, sequences, options, results, struct adapters,
  checksum mismatch, packed sequence layout, streaming telemetry, and control
  frame roundtrips.

## Directory Layout
- `java/norito_java/src/main/java/org/hyperledger/iroha/norito/`
  - `NoritoHeader`, `CRC64`, `Varint`, `NoritoEncoder`, `NoritoDecoder`,
    `TypeAdapter`, adapter classes, `NoritoCodec`, `NoritoAdapters`, `SchemaHash`,
    `Result`, and CLI `NoritoDump`.
- `java/norito_java/src/test/java/org/hyperledger/iroha/norito/`
  - `NoritoTests` main class (assert-based tests executed via `java`).
- `java/norito_java/README.md` – usage, build, testing, roadmap.
- `java/norito_java/run_tests.sh` – compiles and runs the test harness.
- `java/norito_java/LICENSE` – Apache-2.0 to match the workspace.
- `java/norito_java/CHANGELOG.md` – release notes.
- `java/norito_java/BUILDING.md` (alias README build section) if needed.

## Testing Strategy
- Provide `run_tests.sh` as a small wrapper around the Gradle `runNoritoTests` task so compilation,
  assertions, and the pinned Zstandard runtime use the published dependency graph.
- Tests assert roundtrips for signed/unsigned ints, strings, sequences (packed
  offsets are fixed u64 in v1), options, results, and struct adapter behaviours; verify
  header validation and CRC mismatch detection.
- No external testing frameworks to avoid network/build dependencies.

## Compression Profiles & Packaging
- `CompressionConfig.zstdProfile(profile, payloadLen)` exposes the `"fast"`,
  `"balanced"`, and `"compact"` heuristics. The levels mirror the Python
  binding so tests remain aligned across languages:

  | Profile   | Payload buckets (bytes)                                            | Level |
  |-----------|--------------------------------------------------------------------|-------|
  | `FAST`    | `[0, 64 KiB) → 1`, `[64 KiB, 512 KiB) → 2`, `[512 KiB, 4 MiB) → 3`, `≥ 4 MiB → 4` |
  | `BALANCED`| `[0, 64 KiB) → 3`, `[64 KiB, 512 KiB) → 5`, `[512 KiB, 4 MiB) → 7`, `≥ 4 MiB → 9` |
  | `COMPACT` | `[0, 64 KiB) → 7`, `[64 KiB, 512 KiB) → 11`, `[512 KiB, 4 MiB) → 15`, `≥ 4 MiB → 19` |

  The helper clamps the final level to the canonical Zstandard range `[1, 22]`
  and rejects negative payload lengths or unknown profiles.
- `CompressionConfig.zstdProfile(String, int)` normalises the profile name via
  `Locale.ROOT` so configuration files can rely on case-insensitive strings.
- Publishing: Gradle includes `maven-publish`; use `./gradlew publishToMavenLocal`
  with `-PnoritoJavaVersion=...` to publish the `org.hyperledger.iroha:norito-java`
  artifact for local consumption.
- Packaging guidance for the JNI backend:
  1. Consume the transitive `com.github.luben:zstd-jni:1.5.7-7` dependency from `norito-java` and
     keep any centrally constrained version aligned.
  2. For Android, align the application ABI filters with the shipped native
     libraries and keep debug symbols for `libzstd-jni.so` when useful for
     crash triage.
  3. Treat missing or broken JNI linkage as a packaging error; there is no optional codec path.

## Panama Acceleration Notes
- norito-java currently avoids the Foreign Function & Memory API to keep the
  artifact usable with Android and Java 21 toolchains; buffer paths operate
  on `ByteBuffer` without preview or Panama dependencies.
- Additional vectorized CRC/varint paths can build on these entrypoints later,
  but the pure-Java implementation remains the source of truth.

## Maintenance Notes
- Run `scripts/check_norito_bindings_sync.sh` (a thin shell wrapper around the
  Python helper) to ensure updates to the Rust codec are mirrored in the Python,
  Java, and Kotlin bindings. CI should invoke the script directly. The Norito
  Rust crate's `build.rs` keeps ordinary Cargo builds lightweight and only runs
  the sync guard when `NORITO_CHECK_BINDINGS_SYNC=1` is set.
