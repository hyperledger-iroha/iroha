---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قائمة التحقق من تسجيل المقطع
العنوان: قائمة الاختيار، أداة الطرح، القطعة SoraFS
Sidebar_label: اختر مقطع الطرح
الوصف: طرح خطة جيدة لتحديث قطع الغيار.
---

:::note Канонический источник
أخرج `docs/source/sorafs/chunker_registry_rollout_checklist.md`. بعد النسخ المتزامنة، حول وثائق أبو الهول، لا يتم عرضها من الاستثناءات.
:::

# اختيار قائمة الطرح SoraFS

هذه القائمة المختارة تصلح الأشياء التي تحتاجها لإنتاج الملف الشخصي الجديد Chunker
أو قبول مزود الحزمة من جديد للبيع بعد التصديق
ميثاق الحوكمة.

> **المحتوى:** يرجى ملاحظة أي إصدار يتم إرساله إليك
> `sorafs_manifest::chunker_registry`، مظاريف قبول الموفر أو
> канонические حزم التركيبات (`fixtures/sorafs_chunker/*`).

## 1. التحقق المسبق

1. إعادة تجديد التركيبات والتحقق من التصميم:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. لاحظ أن التجزئات تحدد التحديد
   `docs/source/sorafs/reports/sf1_determinism.md` (أو ذات الصلة
   الملف الشخصي) متضمن مع القطع الأثرية المتجددة.
3. يرجى ملاحظة أن `sorafs_manifest::chunker_registry` يتم تجميعه مع
   `ensure_charter_compliance()` عند الانتهاء:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. احصل على ملف المقترحات:
   -`docs/source/sorafs/proposals/<profile>.json`
   - سجل الدقائق المطلوبة في `docs/source/sorafs/council_minutes_*.md`
   - توقف عن الحتمية

## 2. التوقيع على الحوكمة1. قم بإلقاء نظرة على مجموعة عمل الأدوات ولخص الاقتراحات في
   لوحة البنية التحتية لبرلمان سورا.
2. قم بتأكيد التفاصيل في
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. نشر المظروف، مناقشة المناقشة، مع التجهيزات:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. التحقق من إمكانية وصول المظروف عن طريق مساعد الحصول على الحوكمة:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. الطرح المرحلي

إرشادات تفصيلية جيدة sм. в [دليل اللعب لبيان التدريج](./staging-manifest-playbook).

1. تحديد Torii مع الاكتشاف الشامل `torii.sorafs` والتنفيذ الشامل
   القبول (`enforce_admission = true`).
2. قم بتعبئة مظاريف قبول المزود المعتمد في دليل التسجيل المرحلي،
   указанный в `torii.sorafs.discovery.admission.envelopes_dir`.
3. التحقق من أن المزود يعلن عن نفسه من خلال واجهة برمجة التطبيقات Discovery:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. ترقية بيان/خطة نقاط النهاية مع رؤوس الإدارة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. اتبع لوحات معلومات القياس عن بعد (`torii_sorafs_*`) وقواعد التنبيه
   قم بنشر ملف شخصي جديد بدون أوشيبوك.

## 4. طرح الإنتاج

1. قم بالبحث عن التدريج للمنتج Torii.
2. قم بالموافقة على التنشيط (البيانات/الوقت، فترة السماح، خطة التراجع) في القناة
   المشغلين وSDK.
3. علاقات عامة قوية مع:
   - التركيبات الأساسية والمغلف
   - وثائق التوثيق (اتفاقية الميثاق، بعد التحديد)
   - خريطة الطريق/الحالة
4. قم بنشر هذا الإصدار وأرشفة القطع الأثرية المنشورة لمصدرها.## 5. تدقيق ما بعد الإطلاق

1. إرسال المقاييس النهائية (عدد الاكتشافات، معدل نجاح الجلب، الخطأ
   الرسوم البيانية) خلال 24 ساعة بعد الطرح.
2. احصل على `status.md` سيرة ذاتية جيدة وأسلوب حاسم آخر.
3. قم بمتابعة الخطوات (على سبيل المثال، إرشادات إضافية حول التأليف)
   الملف الشخصي) في `roadmap.md`.