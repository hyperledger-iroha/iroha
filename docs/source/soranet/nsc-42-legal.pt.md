---
lang: pt
direction: ltr
source: docs/source/soranet/nsc-42-legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 977ba8e208b2dddb63f558f5e20c97a894d85c930e63d2c805e12d320be58b78
source_last_modified: "2026-01-03T18:08:01.774319+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: NSC-42 — Codec Legal Sign-off
summary: Deliverables that unblock CABAC/rANS legal gating for the Norito Streaming backlog.
---

# NSC-42 — Codec Legal Sign-off

**Status:** Completed — the repository now ships the legal scaffolding required by the NSC-42 opinion.

- `NOTICE` documents the entropy-coding patent posture (CABAC, trellis, rANS) and explicitly warns that CABAC deployments require pool licensing (Via Licensing Alliance / Access Advance).
- `PATENTS.md` summarizes the standing SEP obligations, the trellis claim-avoidance posture, and the rANS baseline constraints.
- `EXPORT.md` captures the EAR99 classification for CABAC/trellis/rANS, documenting that these compression tools fall outside Category 5 Part 2 “encryption items”.
- `tools/rans/gen_tables.py` and `codec/rans/tables/rans_seed0.toml` provide the deterministic, CC0-licensed rANS tables referenced by NSC-55 while preserving provenance data (seed, commit, checksum).
- Build gating: CABAC is disabled unless the builder sets `ENABLE_CABAC=1` in the environment, which adds the `norito_enable_cabac` cfg to the `norito` crate. Trellis scans remain disabled until a claim-avoidance design is published.
- Runtime gating: `[streaming.codec]` enforces CABAC/trellis preconditions (non-`disabled` cabac modes require the build cfg; trellis lists must remain empty until the claim-avoidance profile lands) and now captures the entropy toggles (`entropy_mode`, `bundle_width`, `bundle_accel`, `rans_tables_path`) with validation so bundled rANS can be rolled out safely once NSC-55 closes; the bundled profile also requires `ENABLE_RANS_BUNDLES=1` at build time, mirroring the CABAC gate. Bundled entropy is pinned to table widths 2–3; out-of-range widths are rejected at parse time.

**Follow-up:** None — NSC-55 inherits these artefacts and focuses on deterministic-table validation.
