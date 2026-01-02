---
lang: ar
direction: rtl
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/taikai_anchor_lineage_packet.md -->

# حزمة نسب مرساة Taikai (قالب) (SN13-C)

يتطلب بند خارطة الطريق **SN13-C - Manifests & SoraNS anchors** ان يقوم كل تدوير alias
بارسال حزمة ادلة حتمية. انسخ هذا القالب الى دليل artefacts الخاص بال rollout (على سبيل
المثال
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) واستبدل القوالب قبل
ارسال الحزمة الى الحوكمة.

## 1. البيانات الوصفية

| الحقل | القيمة |
|-------|--------|
| معرف الحدث | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| مساحة الاسم / اسم alias | `<sora / docs>` |
| دليل الادلة | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| جهة اتصال المشغل | `<name + email>` |
| تذكرة GAR / RPT | `<governance ticket or GAR digest>` |

## مساعد الحزمة (اختياري)

انسخ artefacts من spool واصدر ملخص JSON (اختياري التوقيع) قبل اكمال الاقسام المتبقية:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

يقوم المساعد بسحب `taikai-anchor-request-*` و `taikai-trm-state-*` و `taikai-lineage-*` و
envelopes و sentinels من دليل spool الخاص بـ Taikai
(`config.da_ingest.manifest_store_dir/taikai`) لكي يحتوي مجلد الادلة مسبقا على الملفات
الدقيقة المشار اليها ادناه.

## 2. سجل النسب والـ hint

ارفق سجل النسب الموجود على القرص وملف hint JSON الذي كتبته Torii لهذه النافذة. يتم اخذها
مباشرة من
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` و
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| artefact | الملف | SHA-256 | ملاحظات |
|----------|-------|---------|---------|
| سجل النسب | `taikai-trm-state-docs.json` | `<sha256>` | يثبت digest/النافذة للبيان السابق. |
| hint النسب | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | تم الالتقاط قبل الرفع الى مرساة SoraNS. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. التقاط حمولة المرساة

سجل حمولة POST التي ارسلها Torii الى خدمة المرساة. تتضمن الحمولة `envelope_base64` و
`ssm_base64` و `trm_base64` والكائن `lineage_hint` inline؛ تعتمد عمليات التدقيق على هذا
الالتقاط لاثبات hint الذي تم ارساله الى SoraNS. يقوم Torii الان بكتابة هذا JSON تلقائيا
كـ
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
داخل دليل spool الخاص بـ Taikai (`config.da_ingest.manifest_store_dir/taikai/`)، لذا يمكن
للمشغلين نسخه مباشرة بدلا من استخراج سجلات HTTP.

| artefact | الملف | SHA-256 | ملاحظات |
|----------|-------|---------|---------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | طلب خام منسوخ من `taikai-anchor-request-*.json` (Taikai spool). |

## 4. اقرار digest البيان

| الحقل | القيمة |
|-------|--------|
| digest البيان الجديد | `<hex digest>` |
| digest البيان السابق (من hint) | `<hex digest>` |
| بداية/نهاية النافذة | `<start seq> / <end seq>` |
| طابع وقت القبول | `<ISO8601>` |

ارجع الى hashes سجل النسب/hint المسجلة اعلاه حتى يتمكن reviewers من التحقق من النافذة التي
تم استبدالها.

## 5. المقاييس / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (لكل alias): `<file path + hash>`

قدّم تصدير Prometheus/Grafana او مخرجات `curl` التي تظهر زيادة العداد ومصفوفة `/status`
لهذا alias.

## 6. بيان لدليل الادلة

انشئ بيانا حتميا لدليل الادلة (ملفات spool، التقاط الحمولة، لقطات المقاييس) حتى تتمكن
الحوكمة من التحقق من كل hash دون فك الارشيف.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| artefact | الملف | SHA-256 | ملاحظات |
|----------|-------|---------|---------|
| بيان الادلة | `manifest.json` | `<sha256>` | ارفق هذا بحزمة الحوكمة / GAR. |

## 7. قائمة تحقق

- [ ] تم نسخ سجل النسب + عمل hash.
- [ ] تم نسخ hint النسب + عمل hash.
- [ ] تم التقاط حمولة Anchor POST وعمل hash.
- [ ] تم تعبئة جدول digest البيان.
- [ ] تم تصدير لقطات المقاييس (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] تم توليد البيان عبر `scripts/repo_evidence_manifest.py`.
- [ ] تم رفع الحزمة الى الحوكمة مع hashes + معلومات الاتصال.

الالتزام بهذا القالب لكل تدوير alias يبقي حزمة الحوكمة الخاصة بـ SoraNS قابلة لاعادة
الانتاج ويربط hints النسب مباشرة بادلة GAR/RPT.

</div>
