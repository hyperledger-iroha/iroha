---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قائمة التحقق من تسجيل المقطع
العنوان: قائمة التحقق من بدء تشغيل سجل القطع في SoraFS
Sidebar_label: قائمة التحقق من بدء تشغيل القطعة
الوصف: خطة بدء التشغيل لتحديث سجل القطع.
---

:::ملاحظة فونتي كانونيكا
ريفليت `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas as copias sincronzadas.
:::

# قائمة التحقق من بدء التسجيل في SoraFS

تلتقط قائمة المراجعة هذه الخطوات اللازمة للترويج لملف جديد للقطعة
أو حزمة القبول التي تثبت المراجعة لإنتاجها بعد صدور الميثاق
دي الحاكم للتصديق عليها.

> **اللغة:** قم بتطبيق جميع الإصدارات التي تم تعديلها
> `sorafs_manifest::chunker_registry`، مظاريف قبول المزود، أو الحزم
> دي تركيبات Canonicos (`fixtures/sorafs_chunker/*`).

## 1. رحلة فاليداكاو قبل الرحلة

1. تجديد التركيبات والتحقق من التحديد:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تأكد من وجود تجزئات التحديد الخاصة بها
   `docs/source/sorafs/reports/sf1_determinism.md` (أو رابط الملف الشخصي
   ذات الصلة) Batem com os artefatos regenerados.
3. تأكد من تجميع `sorafs_manifest::chunker_registry` com
   تنفيذ `ensure_charter_compliance()`:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. قم بتحديث ملف الاقتراح:
   -`docs/source/sorafs/proposals/<profile>.json`
   - أدخل هذه النصيحة في `docs/source/sorafs/council_minutes_*.md`
   - علاقة الحتمية

## 2. التوقيع على الحكم1. اعرض علاقة مجموعة عمل الأدوات ولخص الاقتراح
   لوحة البنية التحتية لبرلمان سورا.
2. سجل تفاصيل الموافقة
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. قم بنشر المغلف الذي تم تجميعه من البرلمان جنبًا إلى جنب مع التركيبات:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. التحقق من إمكانية الوصول إلى المغلف عبر مساعد جلب الإدارة:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. الطرح المرحلي

راجع [دليل التشغيل للبيان المرحلي](./staging-manifest-playbook) لها
passo a passo detalhado.

1. الزرع Torii com Discover `torii.sorafs` تأهيل وإنفاذ القبول
   وصلة (`enforce_admission = true`).
2. قم بإرسال مظاريف قبول المزود الخاصة بمدير التسجيل
   مرجع التدريج `torii.sorafs.discovery.admission.envelopes_dir`.
3. التحقق من قيام المزود بالإعلان عن الإعلانات عبر واجهة برمجة التطبيقات للاكتشاف:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. قم بتمرين نقاط النهاية للبيان/الخطة مع رؤوس الحوكمة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن لوحات معلومات القياس عن بعد (`torii_sorafs_*`) ولوائح التنبيه
   تقرير أو معلومات جديدة عن الأخطاء.

## 4. طرح المنتج1. قم بتكرار خطوات التدريج في العقد Torii من الإنتاج.
2. الإعلان عن تاريخ البدء (البيانات/الوقت، فترة السماح، خطة التراجع) الآن
   قنوات المشغلين وSDK.
3. دمج العلاقات العامة للإصدار التنافسي:
   - تركيبات و مغلف تم تحديثه
   - Mudancas na documentacao (المراجع المتعلقة بالميثاق، وعلاقة الحتمية)
   - تحديث خريطة الطريق/الحالة
4. قم بإصدار وتسجيل القطع الأثرية التي تم قتلها من أجل مصدرها.

## 5. عملية بدء تشغيل غرفة المراجعة

1. التقاط المقاييس النهائية (أعداد الاكتشافات، وأصناف نجاح الجلب، والرسوم البيانية
   خطأ) بعد 24 ساعة أو الطرح.
2. قم بتحديث `status.md` كملخص قصير ورابط لنسبة التحديد.
3. تسجيل متطلبات المرافقة (على سبيل المثال، التوجيه الإضافي للتأليف)
   من بيرفيس) في `roadmap.md`.