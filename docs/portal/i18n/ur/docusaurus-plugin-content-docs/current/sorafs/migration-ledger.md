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
| M0 | ہفتے 1–6 | Chunker fixtures شائع ہوئے؛ pipelines CAR + manifest bundles legacy artefacts کے ساتھ emit کرتے ہیں؛ مائیگریشن لیجر اندراجات بنائے گئے۔ | Docs, DevRel, SDKs | `sorafs_manifest_stub` کو expectation flags کے ساتھ اپنائیں، اس لیجر میں اندراجات ریکارڈ کریں، legacy CDN برقرار رکھیں۔ | ✅ فعال |
| M1 | ہفتے 7–12 | CI deterministic fixtures نافذ کرتا ہے؛ alias proofs staging میں دستیاب ہیں؛ tooling explicit expectation flags دیتا ہے۔ | Docs, Storage, Governance | Fixtures کو signed رکھیں، staging registry میں aliases رجسٹر کریں، release checklists کو `--car-digest/--root-cid` enforcement کے ساتھ اپڈیٹ کریں۔ | ⏳ زیر التوا |
| M2 | ہفتے 13–20 | Registry-backed pinning بنیادی راستہ بن جاتا ہے؛ legacy artefacts read-only ہو جاتے ہیں؛ gateways registry proofs کو ترجیح دیتے ہیں۔ | Storage, Ops, Governance | Pinning کو registry کے ذریعے چلائیں، legacy hosts freeze کریں، operators کے لیے migration notices شائع کریں۔ | ⏳ زیر التوا |
| M3 | ہفتہ 21+ | صرف alias-based رسائی نافذ؛ observability registry parity پر الرٹ کرتی ہے؛ legacy CDN بند کیا جاتا ہے۔ | Ops, Networking, SDKs | legacy DNS ہٹائیں، cached URLs rotate کریں، parity dashboards مانیٹر کریں، SDK defaults اپڈیٹ کریں۔ | ⏳ زیر التوا |
| R0–R3 | 2025-03-31 → 2025-07-01 | Provider advert enforcement phases: R0 observe, R1 warn, R2 canonical handles/capabilities نافذ، R3 legacy payloads purge۔ | Observability, Ops, SDKs, DevRel | `grafana_sorafs_admission.json` امپورٹ کریں، `provider_advert_rollout.md` کی operator checklist فالو کریں، R2 گیٹ سے 30+ دن پہلے advert renewals stage کریں۔ | ⏳ زیر التوا |

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
