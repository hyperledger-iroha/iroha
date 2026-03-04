---
lang: am
direction: ltr
source: docs/source/project_tracker/nsc42_codec_legal_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0e1b9a1e9688e09d0dd2f9cacd16f926d0ee6d0e8077c44e123ab21b9f0bff4a
source_last_modified: "2026-01-05T09:28:12.036977+00:00"
translation_last_reviewed: 2026-02-07
---

<!-- Preparation notes for NSC-42 legal/patent review. -->

# NSC-42 — Block-Based Codec Legal & Patent Review

## Review Objectives
- Assess CABAC, trellis scan, and rANS usage for patent exposure in all target jurisdictions (US, EU, JP, CN, SG).
- Confirm export controls, licensing obligations, and redistribution requirements for advanced codec features.
- Produce guidance for feature gating (open-source vs enterprise builds) and operator compliance.

## Current Feature Inventory
- Baseline profile (rANS, trellis optional).
- Hybrid CABAC fallback planned for high-end profiles.
- SIMD optimisations (x86 AVX2, aarch64 NEON) with potential patent dependencies.
- Adaptive block tools (32×32 trellis, delta quant tables).

## Pre-Review Checklist
- [x] Assemble prior-art documentation and existing licence references (AV1, VVC, MPEG-LA statements).
- [x] Inventory jurisdictions/operators planning to ship NSC advanced profiles.
- [x] Summarise planned feature set and gating strategy for counsel.
- [x] Prepare question list for external counsel (patent expiry timelines, indemnification expectations, sublicensing).
- [x] Coordinate with Release Engineering for feature flag documentation and build-time toggles.
- [x] Collect current Norito docs/benchmarks referencing CABAC/rANS for appendix.

### Prior-Art & Licence References
- [Alliance for Open Media Patent License 1.0](https://aomedia.org/license/patent/) — governs AV1 rANS usage; include in briefing packet.
- [Apple VVC Patent Package (MPEG-LA)](https://www.mpegla.com/programs/vvc/) — confirm coverage for CABAC fallback deployments.
- [ISO/IEC 23090-3 Annex B] — codifies trellis scan; cite alongside our implementation note in `norito_streaming.md`.
- Internal: `docs/source/norito_streaming.md` (§7 Encoder Profiles) and `docs/source/norito_streaming_legal.md` (once opinion delivered).

### Deployment Jurisdictions & Operators
| Jurisdiction | Target operators | Notes |
|--------------|------------------|-------|
| United States | SoraNet staging, streaming PoC partners | Requires CABAC feature flag disabled by default; enterprise builds gated behind legal approval. |
| European Union (DE, FR, NL) | SoraFS CDN nodes, research consortium | GDPR alignment already covered; patent posture mirrors US. |
| Japan | SoraNet JP validators | Local counsel confirmed no additional rANS obligations; follow up on CABAC once legal opinion received. |
| Singapore | Enterprise streaming pilot | Export review pending; ensure optional CABAC module can be stripped during build. |
| Mainland China | Not planned for NSC launch | Document this explicitly to avoid accidental enablement. |

### Feature Set & Gating Summary
- **All builds:** Bundled rANS is compiled in and required; `entropy_mode` only accepts `rans_bundled`, trellis scans remain disabled, and SIMD features stay optional (`codec-simd-avx2`, `codec-simd-neon`). CABAC is still optional and compiles out unless explicitly enabled.
- **Operator guidance:** Captured in `docs/source/norito_streaming.md` (§19) and `docs/source/soranet/nsc-42-legal.md`; mirrors the jurisdictional matrix posted in `docs/source/norito_streaming_legal.md`.
- **Testing:** CI matrix adds a CABAC-enabled job (`ENABLE_CABAC=1 cargo test`) plus SIMD benches to ensure fallback remains deterministic when CABAC is disabled.

### Counsel Question Bank (Draft)
1. Confirm CABAC patent expiry timelines for the jurisdictions listed above.
2. Are there sublicensing or notice requirements when redistributing binaries with CABAC enabled?
3. Does rANS (as implemented in AV1/libaom) require additional licensing beyond AOM Patent License 1.0?
4. Guidance on export classification for binaries shipping CABAC/rANS (especially for Singapore deployments).
5. Indemnification expectations for enterprise customers toggling CABAC; should contracts include specific language?
6. Any obligations when shipping SIMD-accelerated trellis (AVX2/NEON) code paths?

**Ownership:** Questions 1–3 sit with Legal & Standards; question 4 with Export Compliance; question 5 with Release Engineering + Legal; question 6 with the Codec TL (inputs) and Legal (assessment).

### Build-Time Toggle Coordination
- Release Engineering to add `codec-cabac`, `codec-trellis`, and `codec-simd-*` features to build matrix (`.github/workflows/codec-build-matrix.yml` placeholder) with documentation in `docs/source/references/build_flags.md` (draft).
- Build artefacts include `CODEC_FEATURES.md` manifest enumerating enabled codec modules for traceability.
- Nightly CI job `ci/nightly-codec-matrix` will produce three binaries (community, enterprise, perf) and attach feature manifest.

### Supporting Documentation & Benchmarks
- `NOTICE`, `PATENTS.md`, `EXPORT.md` — legal/export scaffolding recorded at repo root.
- `docs/source/norito_streaming.md` — encoder profiles, CABAC/rANS integration notes (see §§7 & 19).
- `benchmarks/nsc/rans_compare.py` — harness for validating deterministic rANS tables.
- `docs/source/project_tracker/nsc55_rans_validation_plan.md` — cross-reference table generation/verification tasks.
- `docs/source/norito_streaming_legal.md` — counsel opinion + export posture.

## Meeting Prep
- Target counsel outreach by **2026-02-24** (Legal & Standards owner).
- Internal prep meeting: **2026-02-26** with Codec TL, Legal, Release Engineering.
- Reference materials: VVC licence overview, RFCs on rANS, internal design docs.

## Deliverables
1. Written legal opinion filed in repo (`docs/source/norito_streaming_legal.md`).
2. Updated licensing guidance in `norito_streaming.md` and operator docs.
3. Decision matrix for feature enablement per jurisdiction.
4. Risk register updates (new patent or compliance risks).

## Follow-Ups
- [x] Capture open questions from counsel and assign owners (Codec TL, Legal, Release).
- [ ] Update roadmap/status once opinion received.
- [ ] Coordinate with NSC-55 team if table/patent guidance overlap emerges.
