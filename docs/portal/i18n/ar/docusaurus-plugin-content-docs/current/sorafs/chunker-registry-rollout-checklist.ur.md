---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قائمة التحقق من تسجيل المقطع
العنوان: طرح سجل SoraFS چیك لٹ
Sidebar_label: طرح Chunker چیک لٹ
الوصف: تحديثات التسجيل الخاصة بـ Chunker هي ميزة إضافية مقدمة للطرح في الخطة۔
---

:::ملاحظة مستند ماخذ
:::

# SoraFS طرح التسجيل چیك لٹ

هناك نوع من الملف الشخصي الجديد أو حزمة قبول المزود ومراجعة الإنتاج
قم بالترويج لمراحل الإنشاءات الجديدة والتقاط العناصر الأساسية وميثاق الحوكمة
التصديق على ہو چکا ہو۔

> **النطاق:** جميع الإصدارات متاحة للجميع
> `sorafs_manifest::chunker_registry`، مظاريف قبول الموفر، أو الكنسي
> يتم تغيير حزم التركيبات (`fixtures/sorafs_chunker/*`).

## 1. التحقق من صحة ما قبل الرحلة

1. التركيبات المتكررة تولد الثقة والتحقق من الحتمية:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2.`docs/source/sorafs/reports/sf1_determinism.md` (تقرير الملف الشخصي المتعلق بالموضوع)
   تجزئات الحتمية القطع الأثرية المجددة سے تتطابق مع کریں۔
3. هذا هو `sorafs_manifest::chunker_registry`،
   `ensure_charter_compliance()` الذي تم تجميعه، كلاي:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. ملف المقترحات:
   -`docs/source/sorafs/proposals/<profile>.json`
   - إدخال محضر المجلس `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحتمية

## 2. التوقيع على الحوكمة1. تقرير مجموعة عمل الأدوات وملخص الاقتراح
   تم مناقشة لجنة البنية التحتية لبرلمان سورا.
2. تفاصيل الموافقة کو
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` سجل مكالماتي.
3. سيتم نشر المظروف الموقع من البرلمان والذي يتضمن التجهيزات:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. مساعد جلب الحكم الذي يمكن الوصول إليه من خلال مظروف يمكن الوصول إليه أثناء البحث:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. الطرح المرحلي

خطوات تفصيلية تفصيلية لـ [دليل اللعب لبيان التدريج](./staging-manifest-playbook) موصى به.

1. تم تمكين اكتشاف Torii و `torii.sorafs` وفرض القبول على
   (`enforce_admission = true`) يتم النشر بشكل مستمر.
2. يتم الضغط على مظاريف قبول المزود المعتمد في دليل التسجيل المرحلي
   جسے `torii.sorafs.discovery.admission.envelopes_dir` راجع كرتا ہے.
3. اكتشاف API موفر إعلانات التحقق من الانتشار:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. رؤوس الحوكمة هي تمرين نقاط نهاية البيان/الخطة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. لوحات معلومات القياس عن بعد (`torii_sorafs_*`) وقواعد التنبيه هي ملف تعريف جديد
   أخطاء التقارير الصحفية تؤكد صحة البيانات.

## 4. طرح الإنتاج1. خطوات التدريج لإنتاج العقد Torii تكرار التكرار.
2. نافذة التنشيط (التاريخ/الوقت، فترة السماح، خطة التراجع) ومشغل SDK
   القنوات پر تعلن کریں۔
3. إطلاق سراح دمج العلاقات العامة الذي يشمل:
   - تحديث التركيبات والمغلف
   - تغييرات الوثائق (مراجع الميثاق، تقرير الحتمية)
   - تحديث خريطة الطريق/الحالة
4. قم بتحرير العلامة والتحف الموقعة من مصدرها لحفظها في الأرشيف.

## 5. تدقيق ما بعد الإطلاق

1. الطرح خلال 24 ساعة بعد المقاييس النهائية (عدد الاكتشافات، ومعدل نجاح الجلب، والخطأ
   الرسوم البيانية) التقاط کریں۔
2.`status.md` ملخص موجز وتقرير الحتمية رابط التحديث المستمر.
3. مهام المتابعة (مثل إرشادات تأليف الملف الشخصي الإضافية) والتي تتضمن `roadmap.md`.