---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قائمة التحقق من تسجيل المقطع
العنوان: قائمة التحقق من بدء تشغيل السجل الخاص بـ SoraFS
Sidebar_label: قائمة التحقق من بدء تشغيل القطعة
الوصف: خطة الطرح خطوة بخطوة لتحديث سجل القطع.
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/sorafs/chunker_registry_rollout_checklist.md`. حافظ على النسخ المتزامنة حتى يتم سحب مجموعة وثائق Sphinx المتوارثة.
:::

# قائمة التحقق من بدء تشغيل السجل SoraFS

تلتقط قائمة التحقق هذه الخطوات اللازمة لتعزيز ملف جديد للقطعة
أو حزمة قبول الموردين بعد مراجعة الإنتاج بعد ذلك
كارتا دي غوبرنانزا هايا سيدو تمت التصديق عليها.

> **التحويل:** قم بتطبيق جميع الإصدارات التي تم تعديلها
> `sorafs_manifest::chunker_registry`، عناوين قبول الموردين أو الأشخاص
> حزم التركيبات الكنسي (`fixtures/sorafs_chunker/*`).

## 1. التحقق المسبق

1. تجديد التركيبات والتحقق من التحديد:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. قم بتأكيد تجزئات التحديد أون
   `docs/source/sorafs/reports/sf1_determinism.md` (تقرير الملف الشخصي ذي الصلة)
   يتزامن مع القطع الأثرية المجددة.
3. تأكد من تجميع `sorafs_manifest::chunker_registry` مع
   تشغيل `ensure_charter_compliance()`:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. تحديث ملف العرض:
   -`docs/source/sorafs/proposals/<profile>.json`
   - إدخال أعمال المشورة في `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحتمية

## 2. موافقة الإدارة1. تقديم معلومات عن مجموعة عمل الأدوات وملخص الاقتراح جميعًا
   لوحة البنية التحتية لبرلمان سورا.
2. سجل تفاصيل الموافقة في
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. نشر الشركة في البرلمان جنبًا إلى جنب مع المباريات:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. التحقق من إمكانية الوصول إلى البحر عبر مساعد الجلب:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. الطرح والتدريج

راجع [دليل البيانات المرحلية](./staging-manifest-playbook) من أجل un
سجل تفاصيل هذه الخطوات.

1. قم بتجربة Torii مع Discovery `torii.sorafs` المجهزة والتطبيق
   تم تفعيل القبول (`enforce_admission = true`).
2. قم بإدراج عناوين الموردين المعتمدين في دليل التسجيل
   مرجع التدريج `torii.sorafs.discovery.admission.envelopes_dir`.
3. التحقق من أن إعلانات الموفر يتم نشرها عبر واجهة برمجة التطبيقات للاكتشاف:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. تنفيذ نقاط نهاية البيان/الخطة مع رؤوس الإدارة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن لوحات معلومات القياس عن بعد (`torii_sorafs_*`) وضوابطها
   تنبيه تقرير عن الأخطاء الجديدة.

## 4. الطرح في الإنتاج1. كرر خطوات التدريج مقابل عقد الإنتاج Torii.
2. الإعلان عن نافذة التنشيط (تذكير/ساعة، فترة السماح، خطة التراجع)
   قنوات المشغلين وSDK.
3. دمج العلاقات العامة التي تحتوي على:
   - المباريات والتحديثات
   - تحويلات التوثيق (المراجع إلى المذكرة، تقرير التحديد)
   - تحديث خريطة الطريق/الحالة
4. قم بتحرير المنتجات وحفظها من أجل الإجراءات.

## 5. القاعة بعد الطرح

1. التقاط المقاييس النهائية (حسابات الاكتشاف، وطريقة الجلب، والإخراج،
   رسم بياني للخطأ) بعد 24 ساعة من بدء التشغيل.
2. قم بتحديث `status.md` مع ملخص السيرة الذاتية وإرفاق تقرير التحديد.
3. تسجيل أي مساحة تعقب (على سبيل المثال، دليل إضافي لملفات التعريف)
   أون `roadmap.md`.