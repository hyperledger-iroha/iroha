---
lang: ru
direction: ltr
source: docs/source/norito_streaming_legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4e240e9e4df82db87414f6934ee9510bfdb5e9378c6a704ee24dd979214780b
source_last_modified: "2026-01-04T10:50:53.631469+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Norito Streaming – Codec Patent & Export Appendix
summary: Patent pools, licensing posture, and export considerations for rANS/CABAC features.
---

# Norito Streaming — Codec Patent & Export Appendix

> **Status (2026‑03‑14):** Legal reviewed the rANS + CABAC hybrid profile described in NSC‑42/NSC‑55.
> This appendix captures the current patent posture, licensing checkpoints, and export guidance so
> engineering and release teams can gate features confidently. Update the matrix whenever codec
> features or covered jurisdictions change.

## Compliance Artefacts

- `NOTICE` — entropy-coding patent notice (CABAC, trellis, rANS) and integrator obligations.
- `PATENTS.md` — standing patent posture for each codec feature, including claim-avoidance notes.
- `EXPORT.md` — EAR99 classification statement for CABAC/trellis/rANS deliverables.
- `tools/rans/gen_tables.py` + `codec/rans/tables/rans_seed0.toml` — deterministic, CC0-licensed tables referenced by NSC‑55.

## 1. Patent Landscape

### 1.1 Range Asymmetric Numeral Systems (rANS)
- **Primary licence:** Alliance for Open Media Patent License 1.0 (covers libaom-style implementations).  
  Source: <https://aomedia.org/license/patent/>
- **Status:** Royalty-free in the jurisdictions evaluated (US, EU, JP, SG). No active patent encumbrances were
  identified for our current rANS usage (initial state tables + adaptive modelling). Counsel confirmed that
  the licence obligations are satisfied by retaining copyright and patent notices in binary distributions.
- **Obligations:**  
  - Include the AOM patent notice in binary packages (`LICENSE.aom-patent`).  
  - Track derivative works: changes to the canonical rANS tables must be documented (see NSC‑55 artefacts).

### 1.2 Context-Based Adaptive Binary Arithmetic Coding (CABAC)
- **Patent pool:** MPEG-LA VVC programme (CABAC remains encumbered in multiple jurisdictions).  
  Reference: <https://www.mpegla.com/programs/vvc/>
- **Status:** CABAC is **optional** in NSC builds. Enterprise binaries may ship CABAC subject to commercial
  arrangements; community builds compile it out. Patent coverage is required for production deployments in the
  US, EU, JP, and SG until the relevant claims expire (earliest expiries in 2030–2032 depending on jurisdiction).
- **Obligations:**  
  - Maintain a feature manifest (`CODEC_FEATURES.md`) for each release build.  
  - Ensure CABAC is gated behind the `ENABLE_CABAC=1` build flag (`norito_enable_cabac` cfg) and disabled by default in open-source artefacts.
  - Provide downstream operators with patent notice language supplied by Legal (see Appendix B).

### 1.3 Trellis Optimisation & SIMD Paths
- Trellis search (32×32 adaptive) maps to ISO/IEC 23090‑3 Annex B. Existing patents are either expired or covered
  by AOM’s licence perimeter.  
- SIMD optimisations (AVX2, NEON) reuse public-domain primitives; no new patent exposure identified. Maintain
  attribution for third-party intrinsics where applicable.

## 2. Export Classification

| Jurisdiction | Classification | Notes |
|--------------|----------------|-------|
| United States | EAR99 (community builds); ECCN 5D992.c (enterprise/CABAC builds) | Community builds omit CABAC and fall under EAR99. Enterprise artefacts enabling CABAC are export-controlled; record end-user statements. |
| European Union | Non-dual-use (Regulation 2021/821 Annex I) | No additional controls, but retain audit trail for CABAC-enabled deliveries. |
| Japan | Category 5 Part 2 (encryption) non-applicable; codec falls under general export | Document CABAC feature state for METI filings if requested. |
| Singapore | Strategic Goods (Control) Order — Schedule 2 Category 5 excluded | Sufficient to file internal memo referencing this appendix. |

**Operational guardrails:**
1. Release Engineering tags CABAC-enabled artefacts with `EXPORT_CONTROLLED=1` in build metadata.
2. Distribution portal requires operator acknowledgement before providing CABAC builds.
3. Audit logs record SHA256 of distributed binaries + recipient org IDs for seven years.

## 3. Feature Gating Matrix

| Build profile | Features | Default jurisdictions | Prerequisites |
|---------------|----------|-----------------------|---------------|
| Community (`ENABLE_CABAC` unset) | rANS, trellis disabled, SIMD | All | MIT/Apache 2.0 + AOM patent notice in package. |
| Enterprise (Adaptive CABAC) | rANS, (future) trellis, CABAC (runtime toggle) | US/EU/JP/SG | Active MPEG-LA licence; export flag set; build with `ENABLE_CABAC=1`. |
| Staging (CABAC forced) | rANS, CABAC forced ON | Internal nets only | Legal sign-off per release; not for external distribution. |

Runtime gates are exposed via `[streaming.codec]`:


`entropy_mode = "rans_bundled"` only parses when the binary was compiled with
`ENABLE_RANS_BUNDLES=1` (mirrors the CABAC gate); community builds leave the flag
unset so the bundled profile cannot be enabled accidentally.

CI matrix additions (implemented in `ci/nightly-codec-matrix.yml`):

- `cargo test --no-default-features --features codec-cabac`
- `cargo bench --features codec-simd-avx2`
- Manifest upload step that writes the feature set to `${artifact}/CODEC_FEATURES.md`.

## 4. Distribution Checklist

1. Verify patent notices are bundled (`LICENSE.aom-patent`, MPEG-LA rider when CABAC enabled).
2. Ensure `CODEC_FEATURES.md` and `EXPORT_ATTESTATION.md` accompany every enterprise build.
3. Update `docs/source/norito_streaming.md` (§7.7 & §19) with any jurisdictional changes.
4. Log distribution events in the compliance registry (`compliance/distribution.log`).

## 5. Change Log

| Date | Change | Owner |
|------|--------|-------|
| 2026‑03‑14 | Initial legal/export appendix drafted from NSC‑42 opinion. | Legal & Standards, Codec TL |

## Appendix A — Reference Documents

- NSC‑42 Legal Brief: `docs/source/project_tracker/nsc42_codec_legal_brief.md`
- NSC‑55 Validation Plan: `docs/source/project_tracker/nsc55_rans_validation_plan.md`
- Norito Streaming Overview: `docs/source/norito_streaming.md`
