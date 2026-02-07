---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قائمة التحقق من تسجيل المقطع
العنوان: قائمة التحقق من بدء تشغيل سجل Chunker SoraFS
Sidebar_label: مُقطع طرح قائمة التحقق
الوصف: خطة الطرح غير قابلة للتنفيذ من أجل عمليات التسجيل الحالية.
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/sorafs/chunker_registry_rollout_checklist.md`. قم بمزامنة النسختين حتى تكتمل مجموعة أبو الهول الموروثة.
:::

# قائمة التحقق من بدء التسجيل SoraFS

هذه القائمة المرجعية تحتوي على تفاصيل الخطوات اللازمة لتعزيز ملف تعريف جديد
قطعة أو حزمة من مقدمي العروض بعد الإنتاج
التصديق على ميثاق الحوكمة.

> **الصورة:** قم بتطبيق جميع الإصدارات التي تم تعديلها
> `sorafs_manifest::chunker_registry`، مظاريف القبول أو
> حزم التركيبات الكنسيه (`fixtures/sorafs_chunker/*`).

## 1. التحقق الأولي

1. قم بإعادة ضبط التركيبات والتحقق من التحديد:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تأكد من تحديد التجزئات داخل
   `docs/source/sorafs/reports/sf1_determinism.md` (أو تقرير الملف الشخصي
   ذات الصلة) مراسل aux القطع الأثرية régénérés.
3. تأكد من تجميع `sorafs_manifest::chunker_registry` مع
   `ensure_charter_compliance()` متألق :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. اعرض ملف الاقتراح يوميًا:
   -`docs/source/sorafs/proposals/<profile>.json`
   - أدخل محضر الجلسة `docs/source/sorafs/council_minutes_*.md`
   - تقرير التحديد

## 2. حوكمة التحقق1. اعرض تقرير Tooling Working Group وملخص الاقتراح
   أو سورا لوحة البنية التحتية البرلمانية.
2. قم بتسجيل تفاصيل الموافقة عليها
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. نشر المظروف الموقع من قبل البرلمان على طول التجهيزات :
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. تحقق من إمكانية الوصول إلى المظروف عبر مساعد جلب الإدارة :
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. مرحلة الطرح

الرجوع إلى [التدريج بيان دليل قواعد اللعبة](./staging-manifest-playbook) من أجل واحد
الإجراء التفصيلي.

1. نشر Torii مع Discovery `torii.sorafs` نشط وتطبيق
   القبول نشط (`enforce_admission = true`).
2. ادفع مظاريف القبول المعتمدة من خلال السجل
   سجل التدريج المرجعي لـ `torii.sorafs.discovery.admission.envelopes_dir`.
3. التحقق من نشر إعلانات الموفر عبر اكتشاف واجهة برمجة التطبيقات:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. ممارسة نقاط النهاية/التخطيط مع رؤوس الإدارة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من لوحات المعلومات عن بعد (`torii_sorafs_*`) والضوابط
   تنبيه أبلغ عن ملفك الشخصي الجديد بدون أخطاء.

## 4. إنتاج الطرح1. قم بتكرار خطوات الإعداد على عناوين الإنتاج Torii.
2. الإعلان عن نافذة التنشيط (التاريخ/الساعة، فترة السماح، خطة التراجع)
   مشغلي القنوات وSDK.
3. قم بدمج العلاقات العامة لتحرير المحتوى:
   - مواعيد المباريات والمغلفات يوميا
   - تغييرات في الوثائق (المراجع المستندة إلى المخطط، تقرير التحديد)
   - تحديث خريطة الطريق/الحالة
4. قم بتحرير القطع الأثرية وأرشفتها من المصدر.

## 5. التدقيق بعد الإطلاق

1. التقط المقاييس النهائية (حساب الاكتشاف، وجلب النجاح،
   الرسم البياني للأخطاء) بعد 24 ساعة من بدء التشغيل.
2. قم بالاطلاع على `status.md` بسيرة ذاتية مختصرة وامتياز على تقرير التحديد.
3. أرسل لمسات المتابعة (على سبيل المثال، إرشادات تأليف ملفات التعريف) في
   `roadmap.md`.