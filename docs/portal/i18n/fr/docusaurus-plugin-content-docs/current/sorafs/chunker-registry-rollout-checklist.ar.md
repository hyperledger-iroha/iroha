---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : liste de contrôle du déploiement du registre chunker
titre : قائمة تحقق لإطلاق سجل chunker لسوراFS
sidebar_label : il s'agit d'un chunker
description : Il s'agit d'un chunker.
---

:::note المصدر المعتمد
Voir `docs/source/sorafs/chunker_registry_rollout_checklist.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# قائمة تحقق لإطلاق سجل SoraFS

Ajouter un chunker à un morceau de papier
من مرحلة المراجعة إلى الإنتاج بعد التصديق على ميثاق الحوكمة.

> **النطاق:** ينطبق على كل الإصدارات التي تعدل
> `sorafs_manifest::chunker_registry` pour les luminaires et les luminaires
> المعتمدة (`fixtures/sorafs_chunker/*`).

## 1. تحقق ما قبل الإطلاق

1. Les matchs à venir et les matchs à venir :
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تأكد من أن بصمات الحتمية في
   `docs/source/sorafs/reports/sf1_determinism.md` (أو تقرير الملف المعني)
   تتطابق مع الآثار المُعاد توليدها.
3. تأكد من أن `sorafs_manifest::chunker_registry` يبنى مع
   `ensure_charter_compliance()` pour :
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. حدّث ملف اقتراح الحزمة:
   -`docs/source/sorafs/proposals/<profile>.json`
   - إدخال محاضر المجلس في `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحتمية

## 2. اعتماد الحوكمة

1. قدّم تقرير Tooling Working Group وdigest الاقتراح إلى Sora Parliament Infrastructure Panel.
2. سجّل تفاصيل الموافقة في
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Calendrier des matchs de la Ligue des Champions:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. تحقق من إمكانية الوصول إلى الظرف عبر مساعد جلب الحوكمة:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Mise en scène إطلاقارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح مفصل.

1. Utiliser Torii pour la découverte de `torii.sorafs` et l'application de la loi
   (`enforce_admission = true`).
2. ادفع أظرف قبول المزوّدين المعتمدة إلى دليل سجل mise en scène المشار إليه في
   `torii.sorafs.discovery.admission.envelopes_dir`.
3. تحقق من انتشار إعلانات المزوّد عبر واجهة découverte :
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. اختبر نقاط manifeste/plan مع رؤوس الحوكمة :
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تأكد من أن لوحات التليمترية (`torii_sorafs_*`) et تعرض الملف الجديد
   دون أخطاء.

## 4. إطلاق الإنتاج

1. La mise en scène est effectuée via Torii.
2. أعلن نافذة التفعيل (التاريخ/الوقت، فترة السماح، خطة التراجع) et SDK.
3. ادمج PR الإصدار الذي يتضمن:
   - Calendrier وظرفًا محدثين
   - تغييرات الوثائق (مراجع الميثاق، تقرير الحتمية)
   - تحديث feuille de route/statut
4. La provenance est la suivante.

## 5. تدقيق ما بعد الإطلاق

1. التقط المقاييس النهائية (عدادات Discovery, معدل نجاح fetch, هيستوغرامات
   الأخطاء) du 24 au 24 avril.
2. حدّث `status.md` بملخص قصير ورابط لتقرير الحتمية.
3. سجّل أي مهام متابعة (مثل إرشادات إضافية لكتابة الملفات) dans `roadmap.md`.