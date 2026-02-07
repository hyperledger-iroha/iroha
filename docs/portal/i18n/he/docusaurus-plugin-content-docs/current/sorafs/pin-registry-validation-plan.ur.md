---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-validation-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: pin-registry-validation-plan
title: Pin Registry کے manifests کی توثیقی منصوبہ بندی
sidebar_label: Pin Registry توثیق
description: SF-4 Pin Registry rollout سے پہلے ManifestV1 gating کے لیے توثیقی منصوبہ۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_validation_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں مقامات کو ہم آہنگ رکھیں۔
:::

# תוכנית אימות מניפסט רישום סיכות (הכנה SF-4)

یہ منصوبہ وہ اقدامات بیان کرتا ہے جو `sorafs_manifest::ManifestV1` کی توثیق کو
آنے والے Pin Registry کنٹریکٹ میں جوڑنے کے لیے درکار ہیں تاکہ SF-4 کا کام
موجودہ tooling پر استوار ہو اور encode/decode منطق کی نقل نہ بنے۔

## מידע

1. Host-side submission راستے manifest کی ساخت، chunking profile، اور governance
   envelopes کو proposals قبول کرنے سے پہلے verify کرتے ہیں۔
2. Torii اور gateway سروسز وہی validation routines دوبارہ استعمال کرتی ہیں تاکہ
   hosts کے درمیان deterministic behavior برقرار رہے۔
3. מבחני אינטגרציה: קבלה מניפסט,
   policy enforcement، اور error telemetry شامل ہیں۔

## אדריכלות

```mermaid
flowchart LR
    cli["sorafs_pin CLI"] --> torii["Torii Manifest Service"]
    torii --> validator["ManifestValidator (new)"]
    validator --> manifest["sorafs_manifest::ManifestV1"]
    validator --> registry["Pin Registry Contract"]
    validator --> policy["Governance Policy Checks"]
    registry --> torii
```

### רכיבים

- `ManifestValidator` (`sorafs_manifest` یا `sorafs_pin` crate میں نیا ماڈیول)
  ساختی چیکس اور policy gates کو encapsulate کرتا ہے۔
- Torii ایک gRPC endpoint `SubmitManifest` expose کرتا ہے جو کنٹریکٹ کو forward
  کرنے سے پہلے `ManifestValidator` کو کال کرتا ہے۔
- Gateway fetch path optionally وہی validator استعمال کرتا ہے جب registry سے
  نئے manifests cache کیے جائیں۔

## פירוט משימות

| משימה | תיאור | בעלים | סטטוס |
|------|-------------|-------|--------|
| שלד API V1 | `sorafs_manifest` میں `validate_manifest(manifest: &ManifestV1, policy: &PinPolicyInputs) -> Result<(), ValidationError>` شامل کریں۔ BLAKE3 digest verification اور chunker registry lookup شامل کریں۔ | אינפרא ליבה | ✅ בוצע | مشترکہ helpers (`validate_chunker_handle`, `validate_pin_policy`, `validate_manifest`) اب `sorafs_manifest::validation` میں ہیں۔ |
| חיווט מדיניות | registry policy config (`min_replicas`, expiry windows, allowed chunker handles) کو validation inputs سے map کریں۔ | ממשל / Infra Core | Pending — SORAFS-215 میں ٹریکڈ |
| אינטגרציה Torii | Torii submission path کے اندر validator کال کریں؛ failure پر structured Norito errors واپس کریں۔ | צוות Torii | Planned — SORAFS-216 میں ٹریکڈ |
| בדל חוזה מארח | یقینی بنائیں کہ contract entrypoint وہ manifests reject کرے جو validation hash میں fail ہوں؛ metrics counters ظاہر کریں۔ | צוות חוזה חכם | ✅ בוצע | `RegisterPinManifest` اب state mutate کرنے سے پہلے shared validator (`ensure_chunker_handle`/`ensure_pin_policy`) چلاتا ہے اور unit tests failure cases کور کرتے ہیں۔ |
| בדיקות | validator کے لیے unit tests + invalid manifests کے لیے trybuild cases شامل کریں؛ `crates/iroha_core/tests/pin_registry.rs` میں integration tests شامل کریں۔ | QA Guild | 🠠 בתהליך | validator unit tests on-chain rejection tests کے ساتھ آ گئے ہیں؛ مکمل integration suite ابھی باقی ہے۔ |
| מסמכים | validator آنے کے بعد `docs/source/sorafs_architecture_rfc.md` اور `migration_roadmap.md` اپڈیٹ کریں؛ CLI استعمال `docs/source/sorafs/manifest_pipeline.md` میں لکھیں۔ | צוות Docs | בהמתנה — DOCS-489 חדשות |

## תלות- Pin Registry Norito schema کی تکمیل (ref: roadmap میں SF-4 آئٹم)۔
- Council-signed chunker registry envelopes (validator mapping کو deterministic بناتے ہیں)۔
- Manifest submission کے لیے Torii authentication فیصلے۔

## סיכונים והפחתות

| סיכון | השפעה | הקלה |
|------|--------|--------|
| Torii اور کنٹریکٹ کے درمیان policy interpretation میں فرق | קבלה לא דטרמיניסטית. | validation crate شیئر کریں + host vs on-chain فیصلوں کا موازنہ کرنے والی integration tests شامل کریں۔ |
| بڑے manifests کے لیے performance regression | Submission سست | cargo criterion سے benchmark کریں؛ manifest digest results cache کرنے پر غور کریں۔ |
| סחיפה של הודעות שגיאה | Operators میں کنفیوژن | Norito error codes define کریں؛ `manifest_pipeline.md` میں document کریں۔ |

## יעדי ציר זמן

- Week 1: `ManifestValidator` skeleton + unit tests لینڈ کریں۔
- Week 2: Torii submission path wire کریں اور CLI کو validation errors دکھانے کے لیے اپڈیٹ کریں۔
- Week 3: contract hooks implement کریں، integration tests شامل کریں، docs اپڈیٹ کریں۔
- Week 4: migration ledger entry کے ساتھ end-to-end rehearsal چلائیں اور council sign-off حاصل کریں۔

یہ منصوبہ validator کام شروع ہونے کے بعد roadmap میں حوالہ دیا جائے گا۔