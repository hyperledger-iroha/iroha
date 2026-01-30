---
lang: ur
direction: rtl
source: docs/source/soranet/nsc-55-legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b72f94767de6c92f28503526ea37756d4ca5a7c4ca007b3d31939e78296eb7de
source_last_modified: "2026-01-03T18:08:01.744540+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: NSC-55 — rANS Validation & Patent Posture
summary: Deterministic tables, parity tests, and claim-avoidance notes for NSC-55.
---

# NSC-55 — rANS Validation & Patent Posture

**Status:** Completed — deterministic tables, generator tooling, and policy notes now live in-repo.

- Deterministic tables: `codec/rans/tables/rans_seed0.toml` stores the canonical `SignedRansTablesV1` artefact with CC0-1.0 dedication, checksum, seed, and commit hash. The script `tools/rans/gen_tables.py` wraps `cargo run -p xtask -- codec rans-tables` so future refreshes remain reproducible.
- Patent posture: `PATENTS.md` records the baseline rANS scope and explicitly excludes patented variants (Markov-model table switching, escape-code mechanisms, Microsoft’s US 11,234,023 B2 features). Publication of the tables does not include those claim elements.
- Export memo: `EXPORT.md` confirms the EAR99 (NLR) classification for the deterministic tables because they do not implement cryptography.
- Validation hooks: the xtask command now emits TOML artefacts and `tools/rans/gen_tables.py --verify` reuses the built-in verifier to check signatures/checksums. Benchmark harnesses (see `benchmarks/nsc/rans_compare.py`) load either JSON or TOML tables and compare encoders against the published vectors.
- Config wiring: `[streaming.codec]` now captures the deterministic table path plus the entropy toggles (`rans_tables_path`, `entropy_mode`, `bundle_width`, `bundle_accel`). Operators overriding the path must provide artefacts with the same manifest structure, and parsing rejects invalid entropy settings or missing files. Bundled entropy currently ships tables for widths 2–3; configs outside that range are rejected.

**Next steps:** Add additional seeds/artefacts only after regenerating through `tools/rans/gen_tables.py` and committing the resulting TOML with the CC0 header intact.
