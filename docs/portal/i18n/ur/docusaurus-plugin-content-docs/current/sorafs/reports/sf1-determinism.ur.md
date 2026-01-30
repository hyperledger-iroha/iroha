---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: SoraFS SF1 Determinism Dry-Run
summary: canonical `sorafs.sf1@1.0.0` chunker profile کو validate کرنے کے لئے checklist اور expected digests.
---

# SoraFS SF1 Determinism Dry-Run

یہ رپورٹ canonical `sorafs.sf1@1.0.0` chunker profile کے baseline dry-run کو
capture کرتی ہے۔ Tooling WG کو fixtures refreshes یا نئے consumer pipelines کی
validation کے وقت نیچے والا checklist دوبارہ چلانا چاہیے۔ ہر کمانڈ کا نتیجہ
ٹیبل میں ریکارڈ کریں تاکہ auditable trail برقرار رہے۔

## Checklist

| Step | Command | Expected Outcome | Notes |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تمام tests پاس ہوں؛ `vectors` parity test کامیاب ہو۔ | تصدیق کرتا ہے کہ canonical fixtures compile ہوتے ہیں اور Rust implementation سے match کرتے ہیں۔ |
| 2 | `ci/check_sorafs_fixtures.sh` | اسکرپٹ 0 کے ساتھ exit کرے؛ نیچے والے manifest digests رپورٹ کرے۔ | verify کرتا ہے کہ fixtures صاف طور پر regenerate ہوں اور signatures attached رہیں۔ |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` کا entry registry descriptor (`profile_id=1`) سے match کرے۔ | یقینی بناتا ہے کہ registry metadata sync رہے۔ |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | regeneration `--allow-unsigned` کے بغیر succeed کرے؛ manifest اور signature فائلیں unchanged رہیں۔ | chunk boundaries اور manifests کے لئے determinism proof فراہم کرتا ہے۔ |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript fixtures اور Rust JSON کے درمیان کوئی diff report نہ کرے۔ | optional helper؛ runtimes کے درمیان parity یقینی بنائیں (script Tooling WG maintain کرتا ہے)۔ |

## Expected Digests

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Sign-Off Log

| Date | Engineer | Checklist Result | Notes |
|------|----------|------------------|-------|
| 2026-02-12 | Tooling (LLM) | ✅ Passed | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` کے ذریعے fixtures regenerate ہوئیں، canonical handle + alias lists اور نیا manifest digest `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` بنا۔ `cargo test -p sorafs_chunker` اور صاف `ci/check_sorafs_fixtures.sh` run سے verify کیا (fixtures check کے لئے staged تھیں). Step 5 تب تک pending ہے جب تک Node parity helper نہ آ جائے۔ |
| 2026-02-20 | Storage Tooling CI | ✅ Passed | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` کے ذریعے حاصل ہوا؛ اسکرپٹ نے fixtures regenerate کیے، manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` confirm کیا، اور Rust harness دوبارہ چلایا (Go/Node steps دستیاب ہوں تو چلتے ہیں) بغیر diffs کے۔ |

Tooling WG کو checklist چلانے کے بعد تاریخ کے ساتھ قطار شامل کرنی چاہیے۔ اگر
کوئی قدم fail ہو تو یہاں linked issue فائل کریں اور remediation تفصیلات شامل
کریں، پھر نئے fixtures یا profiles approve کریں۔
