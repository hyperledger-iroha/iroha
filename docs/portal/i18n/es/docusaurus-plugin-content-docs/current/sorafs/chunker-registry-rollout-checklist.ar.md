---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificación de implementación de registro fragmentado
título: قائمة تحقق لإطلاق سجل fragmentador لسوراFS
sidebar_label: قائمة تحقق لإطلاق fragmentador
descripción: خطة إطلاق خطوة بخطوة لتحديثات سجل fragmentador.
---

:::nota المصدر المعتمد
Nombre `docs/source/sorafs/chunker_registry_rollout_checklist.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# قائمة تحقق لإطلاق سجل SoraFS

تجمع هذه القائمة الخطوات المطلوبة لترقية ملف chunker جديد أو حزمة قبول مزوّد
من مرحلة المراجعة إلى الإنتاج بعد التصديق على ميثاق الحوكمة.

> **النطاق:** ينطبق على كل الإصدارات التي تعدل
> `sorafs_manifest::chunker_registry` أو أظرف قبول المزوّدين أو حزم الـ accesorios
> المعتمدة (`fixtures/sorafs_chunker/*`).

## 1. تحقق ما قبل الإطلاق

1. أعد توليد الـ accesorios y تحقق من الحتمية:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تأكد من أن بصمات الحتمية في
   `docs/source/sorafs/reports/sf1_determinism.md` (أو تقرير الملف المعني)
   تتطابق مع الآثار المُعاد توليدها.
3. تأكد من أن `sorafs_manifest::chunker_registry` يبنى مع
   `ensure_charter_compliance()` Ver:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. حدّث ملف اقتراح الحزمة:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Filtro de aire de `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحتمية

## 2. اعتماد الحوكمة

1. قدّم تقرير Grupo de Trabajo de Herramientas y resumen del Panel de Infraestructura del Parlamento de Sora.
2. سجّل تفاصيل الموافقة في
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Calendario de partidos de انشر الظرف الموقّع من البرلمان بجوار الـ:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. تحقق من إمكانية الوصول إلى الظرف عبر مساعد جلب الحوكمة:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Puesta en escenaارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح مفصل.

1. انشر Torii مع تفعيل descubrimiento الخاص بـ `torii.sorafs` وتشغيل aplicación للقبول
   (`enforce_admission = true`).
2. ادفع أظرف قبول المزوّدين المعتمدة إلى دليل سجل puesta en escena المشار إليه في
   `torii.sorafs.discovery.admission.envelopes_dir`.
3. تحقق من انتشار إعلانات المزوّد عبر واجهة descubrimiento:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. اختبر نقاط manifiesto/plan مع رؤوس الحوكمة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن لوحات التليمترية (`torii_sorafs_*`) وقواعد التنبيه تعرض الملف الجديد
   دون أخطاء.

## 4. إطلاق الإنتاج

1. كرر خطوات puesta en escena على عقد Torii الإنتاجية.
2. أعلن نافذة التفعيل (التاريخ/الوقت، فترة السماح، خطة التراجع) لقنوات المشغلين y SDK.
3. ادمج PR الإصدار الذي يتضمن:
   - Calendario وظرفًا محدثين
   - تغييرات الوثائق (مراجع الميثاق، تقرير الحتمية)
   - Hoja de ruta/estado de تحديث
4. ضع وسم الإصدار وأرشف القطع الموقعة لأغراض procedencia.

## 5. تدقيق ما بعد الإطلاق

1. التقط المقاييس النهائية (عدادات descubrimiento, معدل نجاح buscar, هيستوغرامات
   الأخطاء) بعد 24 ساعة من الإطلاق.
2. Pulse `status.md` para conectar y desconectar.
3. Haga clic en el botón `roadmap.md`.