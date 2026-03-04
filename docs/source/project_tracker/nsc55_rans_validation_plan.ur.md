---
lang: ur
direction: rtl
source: docs/source/project_tracker/nsc55_rans_validation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e0166534dccc2163bab37239e95a9d13e8569b7b9bdd8a2e94ef560da7a3bb82
source_last_modified: "2026-01-03T18:08:02.047266+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Validation plan for NSC-55 rANS table verification. -->

# NSC-55 — rANS Initialization Table Validation

## Objectives
- Generate deterministic rANS initialization tables shared by encoders/decoders.
- Validate parity against reference implementations and record performance deltas.
- Document patent/export posture for CABAC-optional deployments leveraging rANS.

## Work Items
- [x] Implement table generation workflow producing a signed artefact committed to the repo.  
      *Design:* extend `xtask` with `xtask codec rans-tables` that:
        1. Accepts `--seed`, `--output`, `--format {json,toml}` and optional `--signing-key` (ed25519) flags.  
        2. Generates canonical init tables for symbol group sizes 4, 8, 16 (matching encoder presets).  
        3. Emits a manifest `{ version, generated_at, generator_commit, checksum_sha256 }` and optionally signs the payload (Norito `SignedRansTablesV1`).  
        4. Writes results to `artifacts/nsc/rans_tables.json` and `artifacts/nsc/rans_tables.toml`.  
        5. Validates determinism via unit tests (`cargo test -p xtask rans_tables_deterministic`) comparing two invocations with identical seeds.  
        6. Includes `--verify` mode to check an existing artefact against the current generator.  
      *Dependencies:* use `norito::json` for JSON outputs (no direct `serde_json`), use `rand_chacha` for reproducible RNG, and reuse the existing Norito hashing utilities for signature payloads.
      *Status:* `cargo xtask codec rans-tables` now produces JSON/TOML/CSV outputs and validates existing artefacts via `--verify`. The helper `tools/rans/gen_tables.py` wraps that command, writes CC0 headers, and drops the canonical TOML under `codec/rans/tables/rans_seed0.toml` with the seed/commit/checksum recorded in the manifest. New unit tests (`verify_payload_accepts_generated_body`, `verify_signature_roundtrip`, `write_and_parse_artifacts`, etc.) cover determinism, checksum validation, signature verification, and CSV export. See `xtask/src/codec.rs` and the new script for regeneration instructions.
- [x] Cross-check outputs with reference encoders (libaom, libdav1d, SVT-AV1) using canonical clips.  
      *Status:* The harness now validates clip integrity against `reference_clips.json` (SHA-256 + byte count) before running runners, records the clip checks in `report.json`, and ships a README pointer to the manifest so governance packets capture evidence of the input set. The preset-driven runners still target the packaged synthetic clip set, with manual workflows available for heavier datasets; reports/logs land under `artifacts/nsc/rans_compare/<timestamp>/` with regression tests covering CSV generation, table metadata extraction, runner plumbing, and clip verification.【benchmarks/nsc/rans_compare.py:1】【benchmarks/nsc/rans_compare_presets.toml:1】【benchmarks/nsc/run_reference_codec.sh:1】【scripts/tests/rans_compare_test.py:1】【.github/workflows/rans-compare.yml:1】
- [x] Integrate rANS mode tests into `integration_tests/tests/norito_streaming_roundtrip.rs` and FEC harness.  
      *Status:* Baseline/bundled vectors now live under `integration_tests/fixtures/norito_streaming/rans/{baseline,bundled}.json`, round-trip checks decode rANS payloads alongside bundled manifests, and the FEC harness reconstructs rANS chunks to their committed hashes so both entropy modes stay under CI. A dedicated xtask regression exercises bundled round-trips with the repo tables to ensure the SignedRansTables artefact stays decodable.【integration_tests/tests/norito_streaming_roundtrip.rs:1】【integration_tests/tests/norito_streaming_fec.rs:1】【integration_tests/fixtures/norito_streaming/rans/baseline.json:1】【integration_tests/fixtures/norito_streaming/rans/bundled.json:1】【xtask/tests/codec_rans_tables.rs:1】
- [x] Profile performance impact (CPU cycles/frame) across representative hardware.  
      *Status:* The Criterion bench `crates/norito/benches/streaming_rans.rs` compares RANS vs bundled encode/decode (multiple frame counts) using the repo tables, complementing the xtask entropy bench and CI helper (`ci/check_streaming_entropy.sh`) so perf deltas are tracked without bespoke scripts.【crates/norito/benches/streaming_rans.rs:1】【xtask/src/streaming_bench.rs:1】【xtask/tests/streaming_entropy_bench.rs:1】【ci/check_streaming_entropy.sh:1】
- [x] Draft patent posture note and export considerations for repository docs.  
      *Status:* `docs/source/norito_streaming_legal.md` now maps CABAC/rANS patent coverage, export classification (EAR99 vs ECCN 5D992), and feature gating guidance per jurisdiction, referencing the NSC‑42 legal brief. The appendix is linked from `norito_streaming.md` and tracked by Release Engineering for build manifests.

## Artefacts
- `codec/rans/tables/rans_seed0.toml` (SignedRansTablesV1 TOML with CC0 header, checksum, generation seed, and commit hash).
- Comparison report summarising mismatch cases, linked to benchmark data.
- Updated documentation (`norito_streaming.md`, legal appendix, `docs/source/soranet/nsc-55-legal.md`).

## Timeline
- Script + initial tables: **2026-03-12**.
- Bench/parity report: **2026-03-26**.
- Patent memo draft: **2026-04-05**.

## Risks & Mitigations
- Differences in reference implementations → mitigate by using common test suite and multiple decoders.
- Performance regression → track via Criterion benches; keep baseline fallback.
- Patent uncertainty → coordinate with NSC-42 legal review results.

## Coordination
- Codec Team leads implementation.
- Overlap with Legal/Standards for patent memo.
- Notify Streaming Runtime before changing default entropy settings.
