# Python Norito Library Design

## Goals
- Provide a pure-Python reference implementation of the Norito binary codec that mirrors the Rust implementation used across the workspace.
- Focus on deterministic encoding/decoding for a curated set of core Norito types to enable interoperability tests and tooling.
- Expose both low-level building blocks (header parsing, CRC64, varint helpers) and a high-level encoding surface suitable for Python applications.
- Stay dependency-light (standard library only) so the library can be embedded in constrained environments and easily audited.

## Scope in v0.1.0
- Header handling: encode/decode `NoritoHeader` with magic `NRT0`, version (major=0, minor=0), schema hash (16 bytes), compression byte, payload length, CRC64, flag byte.
- CRC64-ECMA implementation with table-driven fast path and portable fallback.
- Flag support: `PACKED_SEQ`, `COMPACT_LEN`, `VARINT_OFFSETS`, `COMPACT_SEQ_LEN`, `PACKED_STRUCT`, and `FIELD_BITSET` as documented in `norito.md`.
- Compression: optional Zstandard backend via the `zstandard` Python module. When the dependency is missing, attempts to use compression raise `UnsupportedCompressionError`.
- Encoding/decoding primitives:
  - Unsigned integers up to 64 bits (`u8`, `u16`, `u32`, `u64`).
  - Signed integers up to 64 bits (`i8`, `i16`, `i32`, `i64`).
  - `bool`, IEEE-754 floats (`f32`, `f64`).
  - Byte sequences (`bytes`, `bytearray`, `memoryview`) with both variable-length and fixed-length adapters (`fixed_bytes()`).
  - UTF-8 strings (`str`).
  - Optional values (Python `None` mapped to `Option::None`).
  - Results represented as `Ok(value)`/`Err(error)` helpers in Python.
  - Sequences (`list`, `tuple`) encoded as Norito sequences honoring packed/unpacked layouts.
  - Mappings (`dict`, `OrderedDict`) encoded as `Vec<(K,V)>` respecting packed sequence rules.
- Streaming surface (module `norito.streaming`):
  - Data classes and adapters for the NSC manifest, telemetry, and control frames mirroring the Rust codec.
  - Enumerations and helper adapters for HPKE suites, capability roles, error codes, and telemetry events.
  - Control frame encoder that validates variant/payload pairings and reuses the same discriminant order as Rust.
- Schema description: callers provide a `SchemaDescriptor` when encoding to produce the schema hash. The helper supports both dotted type-path hashing and structural descriptors that are canonicalised using Norito's JSON rules so hashes match the Rust implementation.
- Error model: create `NoritoError` hierarchy with categories paralleling the Rust variant names for easier cross-language comparison.
- Public API surface:
  - `norito.encode(value, schema, adapter, *, flags=None, compression=False, compression_level=3) -> bytes`
  - `norito.decode(data, adapter, schema=None) -> Any`
  - `NoritoEncoder`/`NoritoDecoder` classes for incremental usage.
  - Utility modules: `header`, `crc64`, `varint`, `schema`, `errors`, `types`.

## Type Adaptation Strategy
- Introduce `TypeAdapter` protocol with methods `encode(encoder, value)` and `decode(decoder)`. Built-in adapters cover primitives/collections.
- Adapter registration API is deferred; the initial release focuses on explicit adapter composition.
- `StructAdapter` optionally accepts a factory callable, enabling ergonomic roundtrips for dataclasses or simple structs.
- For results: define lightweight `Ok`/`Err` wrapper classes to disambiguate from plain tuples.

## Determinism & Flags
- Encoder accepts `flags` set; when `PACKED_SEQ` and `COMPACT_LEN` are present simultaneously, default to the hybrid layout described in `norito.md` for sequences and struct-like adapters.
- Packed sequence implementation writes len header followed by either varint sizes or `(len+1)` u64 offsets depending on `VARINT_OFFSETS`. Provide deterministic decision tree identical to Rust defaults (varint only when the flag is explicitly enabled; defaults keep u64 offsets for determinism).
- Non-packed sequences fall back to compat layout (per-element header + payload).

## Testing Plan
- Unit tests using the standard library `unittest` module covering:
  - Roundtrip encoding/decoding for primitives, options, sequences, maps.
  - Header encode/decode parity including checksum failures.
  - CRC64 test vectors (cross-checked with known values from Rust implementation).
  - Flag-specific behaviors: packed vs compat sequences, varint offsets.
  - Deterministic outputs compared against golden byte arrays produced by minimal Rust fixtures (add static reference bytes sampled from the repo's tests).
- Property-based fuzzing remains deferred until a lightweight pure-Python subset of the Rust harness is extracted (tracked in NORITO-PY#12); current unit tests cover the canonical encoders/decoders.

## Documentation
- Provide `README.md` with overview, install instructions, quick start.
- Generate API reference via docstrings; include usage examples in README.
- Add changelog entry for v0.1.0 summarising features.

## Maintenance
- CI check `scripts/check_norito_bindings_sync.sh` (Python-backed, so it runs fine from any shell) enforces that any changes to the
  Rust Norito sources (`crates/norito`, `norito.md`) are accompanied by updates
  to `python/norito_py`.
- The `norito` crate's `build.rs` runs the same guard during `cargo build` so
  developers catch drift early. Packaged source distributions that omit VCS
  metadata automatically skip the guard because the sync script is not
  available there.

## Packaging & Distribution
- Structure as standard Python package with `pyproject.toml` (PEP 621, setuptools backend) to stay dependency-light.
- Publish artifact named `iroha-norito`. Provide CLI entry point `norito-dump` for inspecting Norito payload headers.
- Include `LICENSE` (Apache-2.0 to match project) and `MANIFEST.in` to ship docs/tests.
- Document manual publishing steps with `twine`:
  1. Install the tooling (`python -m pip install --upgrade build twine`).
  2. Build the artifacts from the repository root: `python -m build python/norito_py`.
  3. Inspect the generated wheels/sdist under `python/norito_py/dist/`; run `twine check` for sanity.
  4. Upload to PyPI (or TestPyPI) with `twine upload python/norito_py/dist/*` using API tokens stored in `~/.pypirc`.
  5. Record the published version/tag so the release notes and changelog stay in sync.

## Runtime Notes
- CRC64 acceleration review (2026-05-14): re-ran `python -m python.norito_py.benchmarks.bench_crc64 --size 8 --repeat 5` on an Apple M2 Pro (macOS 14.6). The canonical pure-Python table loop sustains ~6.2 MiB/s on an 8 MiB payload (≈41 µs for a 256 KiB frame), meeting the Norito admission target (≤ 250 µs). Prototypes that used `struct.unpack_from` and slicing-by-8 tables regressed to ~4.7 MiB/s, and while a NumPy/Numba variant reached ~28 MiB/s, it regularly fails to import on minimal/embedded deployments that lack BLAS runtimes. We therefore keep the dependency-free baseline, document the benchmark entry point, and added `tests/test_crc64.py` to guard correctness (including a reference implementation comparison).
- Streaming relay registry helpers (NSC-30): `norito.relay_registry.StreamingRelayRegistry` models bond calculation, reward accrual, EMA reputation scoring, and slashing tiers. The CLI entrypoint (`python -m norito.cli relay-registry …`) lets operators and SDKs exercise the logic offline; `tests/test_relay_registry.py` covers the registration, performance window, slashing progression, treasury fee, and persistence flows.
- Columnar helpers now mirror the Rust surface: adaptive helpers cover `(u64, str, bool)`, `(u64, Option<str>, bool)`, `(u64, Option<u32>, bool)`, `(u64, bytes, bool)`, and `(u64, enum{Name|Code}, bool)` with identical heuristics. Python parity tests exercise the new encoders/decoders and lock their hex output against Rust goldens.
- NSC coverage: streaming control frames presently cover manifest negotiation, telemetry, and error paths. Add audio frame adapters only once the Java binding reaches parity and streaming audio stabilises.
- Compression heuristics: the encoder now accepts ``compression_profile`` (`"fast"`, `"balanced"`, `"compact"`) to pick Zstandard levels dynamically based on payload size. Unit tests cover multi-megabyte fixtures to ensure round-trips succeed and profiles remain deterministic.
