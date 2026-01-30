---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 53f53c285b0d39dca239bcf89ff727d453e14af83e8104eb4cf61c8a075fda25
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
id: chunker-registry-rollout-checklist
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
تعكس `docs/source/sorafs/chunker_registry_rollout_checklist.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# قائمة تحقق لإطلاق سجل SoraFS

تجمع هذه القائمة الخطوات المطلوبة لترقية ملف chunker جديد أو حزمة قبول مزوّد
من مرحلة المراجعة إلى الإنتاج بعد التصديق على ميثاق الحوكمة.

> **النطاق:** ينطبق على كل الإصدارات التي تعدل
> `sorafs_manifest::chunker_registry` أو أظرف قبول المزوّدين أو حزم الـ fixtures
> المعتمدة (`fixtures/sorafs_chunker/*`).

## 1. تحقق ما قبل الإطلاق

1. أعد توليد الـ fixtures وتحقق من الحتمية:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تأكد من أن بصمات الحتمية في
   `docs/source/sorafs/reports/sf1_determinism.md` (أو تقرير الملف المعني)
   تتطابق مع الآثار المُعاد توليدها.
3. تأكد من أن `sorafs_manifest::chunker_registry` يبنى مع
   `ensure_charter_compliance()` عبر:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. حدّث ملف اقتراح الحزمة:
   - `docs/source/sorafs/proposals/<profile>.json`
   - إدخال محاضر المجلس في `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحتمية

## 2. اعتماد الحوكمة

1. قدّم تقرير Tooling Working Group وdigest الاقتراح إلى Sora Parliament Infrastructure Panel.
2. سجّل تفاصيل الموافقة في
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. انشر الظرف الموقّع من البرلمان بجوار الـ fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. تحقق من إمكانية الوصول إلى الظرف عبر مساعد جلب الحوكمة:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. إطلاق staging

ارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح مفصل.

1. انشر Torii مع تفعيل discovery الخاص بـ `torii.sorafs` وتشغيل enforcement للقبول
   (`enforce_admission = true`).
2. ادفع أظرف قبول المزوّدين المعتمدة إلى دليل سجل staging المشار إليه في
   `torii.sorafs.discovery.admission.envelopes_dir`.
3. تحقق من انتشار إعلانات المزوّد عبر واجهة discovery:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. اختبر نقاط manifest/plan مع رؤوس الحوكمة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن لوحات التليمترية (`torii_sorafs_*`) وقواعد التنبيه تعرض الملف الجديد
   دون أخطاء.

## 4. إطلاق الإنتاج

1. كرر خطوات staging على عقد Torii الإنتاجية.
2. أعلن نافذة التفعيل (التاريخ/الوقت، فترة السماح، خطة التراجع) لقنوات المشغلين وSDK.
3. ادمج PR الإصدار الذي يتضمن:
   - Fixtures وظرفًا محدثين
   - تغييرات الوثائق (مراجع الميثاق، تقرير الحتمية)
   - تحديث roadmap/status
4. ضع وسم الإصدار وأرشف القطع الموقعة لأغراض provenance.

## 5. تدقيق ما بعد الإطلاق

1. التقط المقاييس النهائية (عدادات discovery، معدل نجاح fetch، هيستوغرامات
   الأخطاء) بعد 24 ساعة من الإطلاق.
2. حدّث `status.md` بملخص قصير ورابط لتقرير الحتمية.
3. سجّل أي مهام متابعة (مثل إرشادات إضافية لكتابة الملفات) في `roadmap.md`.
