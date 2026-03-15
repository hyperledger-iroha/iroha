---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قائمة التحقق من تسجيل المقطع
العنوان: قائمة تحقق سجل chunker لسورافس
Sidebar_label: قائمة تحقق طموحها
الوصف: خطة مفصلة لتحديث سجل Chunker.
---

:::ملحوظة المصدر مؤهل
احترام `docs/source/sorafs/chunker_registry_rollout_checklist.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

#قائمة تحقق سجل SoraFS

العديد من هذه الخطوات الأساسية إلى ملف Chunker الجديد أو الحزمة المقبولة
من مرحلة المراجعة إلى الإنتاج بعد التصديق على ميثاق الـ و.

> **النطاق:** ينطبق على جميع الاختلافات التي يتم تعديلها
> `sorafs_manifest::chunker_registry` أو أظرف يقبل المحاسبين أو حزم الـ Installations
> معتمد (`fixtures/sorafs_chunker/*`).

## 1. تحقق مما سبق

1. إنشاء الـ التركيبات والتحقق من الحتمية:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. التأكد من أن البصمات الحتمية في
   `docs/source/sorafs/reports/sf1_determinism.md` (أو إقرار الملف الشامل)
   تتطابق مع الآثار المُعاد توليدها.
3. تأكد من أن `sorafs_manifest::chunker_registry` يبنى مع
   `ensure_charter_compliance()` عبر:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. تحديث ملف الصندوق:
   -`docs/source/sorafs/proposals/<profile>.json`
   - الحجم محاضر المجلس في `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحامية

## 2. الاعتماد على

1. تقرير مجموعة عمل الأدوات وملخص الاقتراحات المقدمة إلى لجنة البنية التحتية لبرلمان سورا.
2. سجل تفاصيل الموافقة في
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. نشر الظرف الموقّع من البرلمان بجوار الـ التركيبات:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. التحقق من إمكانية الوصول إلى الظرف عبر مساعد الـ تور:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. التدريج الحرارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح المفصل.

1. انشر Torii مع تفعيل الاكتشاف الخاص بـ `torii.sorafs` وتشغيل التنفيذ للقبول
   (`enforce_admission = true`).
2. ادفع أظرف يقبل المحاسبين المعتمد إلى دليل سجل التدريج المشار إليه في
   `torii.sorafs.discovery.admission.envelopes_dir`.
3. تحقق من انتشار إعلانات المنظم عبر واجهة الاكتشاف:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. اختبر نقاط/خطة البيان/الشبكة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن اللوحات التليميترية (`torii_sorafs_*`) وقواعد التنبيه لإصدار الملف الجديد
   دون أخطاء.

##4.تخصيص الإنتاج

1. كرر خطوات التدريج على العقد Torii.
2.شركة نافذة التفعيل (التاريخ/الوقت، فترة لذلك، خطة السماء) لقنوات تشغيل وSDK.
3. تم دمج نسخة العلاقات العامة التي تشمل:
   - تركيبات ورفًا محدثين
   - تفاصيل الوثائق (مراجع الميثاق، تقرير الحتمية)
   - تحديث خريطة الطريق/الحالة
4. ضع "أبحث عن حل لكشف قطع الموقع" من حيث المصدر.

## 5.دقيق ما بعد الاكتشاف

1.بروتوكول المقاييس النهائية (عدادات الاكتشاف، نجاح نجاح الجلب، هيستوغرامات
   سبب) بعد 24 ساعة من الفشل.
2. التحديث `status.md` بملخص مختصر ورابط لتقرير الحتمية.
3. سجل أي مهمة متابعة (مثل الإرشادات الإضافية والملفات) في `roadmap.md`.