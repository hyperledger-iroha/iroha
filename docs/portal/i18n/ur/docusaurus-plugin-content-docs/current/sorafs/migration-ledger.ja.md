---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 52f3ae2e2190647099003cf291ff2af381352f5cd11648946f184a6d8ef86851
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: migration-ledger
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md) سے ماخوذ۔

# SoraFS مائیگریشن لیجر

یہ لیجر SoraFS Architecture RFC میں ریکارڈ کردہ مائیگریشن چینج لاگ کی عکاسی کرتا ہے۔ اندراجات
سنگ میل کے حساب سے گروپ ہوتی ہیں اور effective window، متاثرہ ٹیمیں، اور مطلوبہ actions درج
کرتی ہیں۔ مائیگریشن پلان میں اپڈیٹس لازمی طور پر اس صفحے اور RFC
(`docs/source/sorafs_architecture_rfc.md`) دونوں میں تبدیلی کریں تاکہ downstream consumers
ہم آہنگ رہیں۔

| سنگ میل | موثر مدت | تبدیلی کا خلاصہ | متاثرہ ٹیمیں | ایکشن آئٹمز | حیثیت |
|---------|----------|-----------------|--------------|-------------|-------|
| M1 | ہفتے 7–12 | CI deterministic fixtures نافذ کرتا ہے؛ alias proofs staging میں دستیاب ہیں؛ tooling explicit expectation flags دیتا ہے۔ | Docs, Storage, Governance | Fixtures کو signed رکھیں، staging registry میں aliases رجسٹر کریں، release checklists کو `--car-digest/--root-cid` enforcement کے ساتھ اپڈیٹ کریں۔ | ⏳ زیر التوا |

گورننس control plane کی minutes جو ان milestones کو حوالہ دیتی ہیں `docs/source/sorafs/` کے تحت
موجود ہیں۔ ٹیموں کو ہر قطار کے نیچے dated bullet points شامل کرنے چاہئیں جب نمایاں واقعات
ہوئیں (مثلا نئے alias registrations یا registry incident retrospectives) تاکہ قابلِ آڈٹ
ریکارڈ فراہم ہو۔

## تازہ ترین اپڈیٹس

- 2025-11-01 — `migration_roadmap.md` کو governance council اور operator lists میں review کے
  لیے بھیجا گیا؛ اگلی council session میں sign-off کا انتظار ہے (ref:
  `docs/source/sorafs/council_minutes_2025-10-29.md` follow-up).
- 2025-11-02 — Pin Registry register ISI اب `sorafs_manifest` helpers کے ذریعے shared chunker/
  policy validation نافذ کرتا ہے، جس سے on-chain paths Torii checks کے ساتھ aligned رہتے ہیں۔
- 2026-02-13 — Provider advert rollout phases (R0–R3) لیجر میں شامل کی گئیں اور متعلقہ dashboards
  اور operator guidance شائع کی گئی
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
