---
lang: ar
direction: rtl
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sorafs_ci_sample/README.md -->

# عينات Fixtures لـ SoraFS CI

هذا الدليل يحزم artefacts حتمية تم توليدها من payload العينة تحت `fixtures/sorafs_manifest/ci_sample/`. توضح الحزمة مسار التعبئة والتوقيع من البداية للنهاية في SoraFS الذي تشغله مهام CI.

## جرد القطع

| الملف | الوصف |
|------|-------|
| `payload.txt` | payload المصدر المستخدم بواسطة سكربتات fixture (عينة نصية). |
| `payload.car` | ارشيف CAR الناتج عن `sorafs_cli car pack`. |
| `car_summary.json` | ملخص من `car pack` يلتقط digests للchunks والبيانات الوصفية. |
| `chunk_plan.json` | JSON لخطة fetch يصف نطاقات chunks وتوقعات المزودين. |
| `manifest.to` | manifest من Norito ناتج عن `sorafs_cli manifest build`. |
| `manifest.json` | تمثيل قابل للقراءة للmanifest لاستخدام التصحيح. |
| `proof.json` | ملخص PoR صادر عن `sorafs_cli proof verify`. |
| `manifest.bundle.json` | bundle توقيع keyless صادر عن `sorafs_cli manifest sign`. |
| `manifest.sig` | توقيع Ed25519 منفصل مرتبط بالmanifest. |
| `manifest.sign.summary.json` | ملخص CLI صادر اثناء التوقيع (hashes، بيانات bundle). |
| `manifest.verify.summary.json` | ملخص CLI من `manifest verify-signature`. |

كل digests المشار اليها في ملاحظات الاصدار والوثائق مصدرها هذه الملفات. يقوم مسار `ci/check_sorafs_cli_release.sh` باعادة توليد نفس artefacts ومقارنتها بالنسخ الملتزمة.

## اعادة توليد fixtures

شغّل الاوامر ادناه من جذر المستودع لاعادة توليد مجموعة fixtures. وهي تماثل الخطوات المستخدمة في مسار `sorafs-cli-fixture`:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

اذا انتجت اي خطوة hashes مختلفة، تحقق قبل تحديث fixtures. تعتمد مسارات CI على المخرجات الحتمية لاكتشاف التراجعات.

## تغطية مستقبلية

مع انتقال ملفات chunker الاضافية وصيغ الاثبات من خارطة الطريق، سيتم اضافة fixtures قياسية تحت هذا الدليل (مثل
`sorafs.sf2@1.0.0` (انظر `fixtures/sorafs_manifest/ci_sample_sf2/`) او اثباتات بث PDP). كل ملف جديد سيتبع نفس
البنية — payload و CAR وخطة و manifest واثباتات وartefacts توقيع — حتى تتمكن الاليات اللاحقة من مقارنة الاصدارات
دون سكربتات مخصصة.

</div>
