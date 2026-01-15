# Norito Java Library Design (JDK 25)

## Goals
- Provide a pure-Java reference implementation of the Norito serialization
  codec that mirrors the Rust implementation semantics and the Python port.
- Target JDK 25 without requiring third-party dependencies, so compilation with
  `javac` works in constrained environments.
- Offer composable adapters for encoding/decoding common types (primitives,
  strings, byte slices, options, results, sequences, packed structs) and expose
  high-level helpers for typical usage.
- Maintain deterministic behaviour and identical header/CRC handling to Rust and
  Python implementations.

## Scope for v0.1.0
- Header support: encode/decode `NoritoHeader` with validation of magic, version
  (major 0, minor 0x00), payload length, checksum (CRC64-XZ), flags, and
  compression byte.
- Compression: support `COMPRESSION_NONE` and Zstandard. The codec reflects against
  `com.github.luben.zstd.Zstd` at runtime; when present (e.g., via `zstd-jni`) the encoder can emit
  compressed payloads and the decoder transparently inflates them. When absent, requesting Zstd
  raises an `UnsupportedOperationException`, keeping the pure-Java build dependency-free.
- Flag support: `PACKED_SEQ`, `COMPACT_LEN`, `PACKED_STRUCT`, `VARINT_OFFSETS`,
  `COMPACT_SEQ_LEN`, `FIELD_BITSET` mirroring the Rust flag byte values. The Java
  defaults now mirror Rust by keeping all optional flags disabled (`DEFAULT_FLAGS = 0`)
  so sequential layouts are emitted unless a caller opts in explicitly.
- CRC64 implementation: table-driven CRC64-XZ (reflected ECMA polynomial) matching Rust/Python.
- Varint helpers: 7-bit LEB128 encoding/decoding for lengths and offsets.
- Type adapters: generic interface `TypeAdapter<T>` with concrete adapters for
  unsigned/signed integers (8–64 bit), booleans, UTF-8 strings, byte arrays
  (variable and fixed-length), optional values, result values, sequences
  (packed/compat layouts), maps (sequence of key-value tuples), and packed
  structs using the hybrid bitset layout.
- Struct support: `StructAdapter` supports dataclass-style factories (record
  builder) and Map-backed values; size calculations follow the Python
  implementation (bitset + varint sizes for non-self-delimiting fields).
- High-level API: `NoritoCodec.encode(value, schema, adapter, flags)` and
  `NoritoCodec.decode(bytes, adapter, schema)` with builder helpers exposed via
  `NoritoAdapters` (static factory methods).
- Streaming helpers: `NoritoStreaming` provides enums, records, and adapters for
  NSC manifests, telemetry, and control frames, sharing discriminant ordering
  and fixed-length hash/signature handling with the Rust codec.
- Schema hashing: FNV-1a 64-bit hashed canonical string, duplicated to 16 bytes, plus
  structural schema hashing via Norito's native JSON canonicalisation.
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
- Provide `run_tests.sh` that uses `javac --release 21` (fallback to default if
  unavailable) to compile both main and test sources into `build/classes`, then
  runs `java -ea org.hyperledger.iroha.norito.NoritoTests`.
- Tests assert roundtrips for signed/unsigned ints, strings, sequences (fixed
  offsets by default with varint offsets gated by the flag), options, results,
  and struct adapter behaviours; verify
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
- Packaging guidance for the JNI backend:
  1. Add `implementation("com.github.luben:zstd-jni:1.5.6-9")` to the Gradle build (or the
     equivalent Maven dependency).
  2. For Android, align the application ABI filters with the shipped native
     libraries and keep debug symbols for `libzstd-jni.so` when useful for
     crash triage.
  3. Gate optional compression paths with `NoritoCompression.hasZstd()` so
     environments without the native dependency fall back gracefully.

## Future Work / TODOs
- Extend columnar (NCB/AoS) helpers to cover enum/discriminant columns; `(u64, String, boolean)` and `(u64, bytes)` layouts (including optional bytes) are now implemented.
- Explore Java foreign memory acceleration (Panama) for large payloads.
- Provide gradle/maven coordinates once the repository is open sourced.

## Maintenance Notes
- The Norito Rust crate's `build.rs` invokes `scripts/check_norito_bindings_sync.sh`
  (a thin shell wrapper around the Python helper) to ensure updates to the Rust codec are mirrored in both the Python and Java
  bindings. Packaged builds without the sync script automatically skip the
  guard; manual bypass hooks are intentionally unavailable.
