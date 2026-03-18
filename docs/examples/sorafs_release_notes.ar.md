---
lang: ar
direction: rtl
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/sorafs_release_notes.md -->

# ملاحظات اصدار SoraFS CLI و SDK (v0.1.0)

## ابرز النقاط
- `sorafs_cli` يغطي الان خط تغليف الحزم بالكامل (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) بحيث تستدعي مشغلات CI
  ثنائيا واحدا بدلا من مساعدات مخصصة. مسار التوقيع بدون مفاتيح يعتمد افتراضيا على
  `SIGSTORE_ID_TOKEN`، ويفهم موفري OIDC في GitHub Actions، ويصدر ملخص JSON حتمي
  بجانب حزمة التواقيع.
- لوحة *scoreboard* لجلب متعدد المصادر تأتي كجزء من `sorafs_car`: تطبع تليمترية
  المزوّدين، تفرض عقوبات القدرات، تحفظ تقارير JSON/Norito، وتغذي محاكي المنسق
  (`sorafs_fetch`) عبر registry handle المشترك. توضّح fixtures تحت
  `fixtures/sorafs_manifest/ci_sample/` المدخلات والمخرجات الحتمية التي يجب على
  CI/CD مقارنتها.
- أتمتة الاصدار موثقة في `ci/check_sorafs_cli_release.sh` و
  `scripts/release_sorafs_cli.sh`. كل اصدار يؤرشف bundle البيان، التوقيع، ملخصات
  `manifest.sign/verify` ولقطة scoreboard كي يتمكن reviewers الحوكمة من تتبع
  artefacts دون اعادة تشغيل خط الانابيب.

## التوافق
- تغييرات كاسرة: **لا يوجد.** جميع اضافات CLI هي رايات/اوامر فرعية اضافية؛ الاستدعاءات
  الحالية تعمل دون تعديل.
- الحد الادنى لاصدارات البوابة/العقد: يتطلب Torii `2.0.0-rc.2.0` (او احدث) حتى تتوفر
  واجهة chunk-range، حصص stream-token، وترويسات القدرات التي يوفرها
  `crates/iroha_torii`. يجب ان تعمل عقد التخزين على حزمة SoraFS host من commit
  `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` (يشمل مدخلات scoreboard الجديدة وربط التليمترية).
- اعتمادات upstream: لا توجد ترقيات طرف ثالث خارج خط الاساس للـ workspace؛ الاصدار
  يعيد استخدام النسخ المثبتة من `blake3`, `reqwest`, و `sigstore` في `Cargo.lock`.

## خطوات الترقية
1. حدّث crates المتوافقة في workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. اعِد تشغيل بوابة الاصدار محليا (او في CI) لتأكيد تغطية fmt/clippy/tests:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. اعِد توليد artefacts الموقعة والملخصات باستخدام الاعدادات المعتمدة:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   انسخ bundles/proofs المحدثة الى `fixtures/sorafs_manifest/ci_sample/` اذا كان الاصدار
   يحدّث fixtures القياسية.

## التحقق
- Commit بوابة الاصدار: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` مباشرة بعد نجاح البوابة).
- مخرجات `ci/check_sorafs_cli_release.sh`: مؤرشفة في
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (مرفقة مع bundle الاصدار).
- ملخص bundle البيان: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- ملخص proof: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- ملخص البيان (لتحقق attestation downstream):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (من `manifest.sign.summary.json`).

## ملاحظات للمشغلين
- بوابة Torii تفرض الان ترويسة القدرة `X-Sora-Chunk-Range`. حدّث allowlists حتى يتم قبول
  العملاء الذين يقدمون نطاقات stream token الجديدة؛ الرموز القديمة التي تفتقد claim النطاق
  سيتم تقييدها.
- `scripts/sorafs_gateway_self_cert.sh` يدمج التحقق من البيان. عند تشغيل harness self-cert،
  زوّد bundle البيان الذي تم توليده حديثا حتى يفشل wrapper بسرعة عند انحراف التوقيع.
- يجب ان تستوعب لوحات التليمترية تصدير scoreboard الجديد (`scoreboard.json`) لمواءمة اهلية
  المزوّدين، تخصيص الاوزان، واسباب الرفض.
- ارشف الملخصات الاربعة القياسية مع كل rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. تشير تذاكر الحوكمة الى هذه الملفات بدقة خلال الموافقة.

## شكر وتقدير
- Storage Team - توحيد CLI من البداية للنهاية، renderer لـ chunk-plan، وربط تليمترية scoreboard.
- Tooling WG - release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) وحزمة fixtures حتمية.
- Gateway Operations - capability gating، مراجعة سياسة stream-token، و playbooks محدثة
  لـ self-cert.

</div>
