---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry-charter
title: ميثاق سجل chunker لـ SoraFS
sidebar_label : que vous utilisez chunker
description: ميثاق الحوكمة لتقديم ملفات chunker واعتمادها.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/chunker_registry_charter.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# ميثاق حوكمة سجل chunker pour SoraFS

> **مُصادَق عليه:** 2025-10-29 من قبل Panel sur les infrastructures du Parlement de Sora (انظر
> `docs/source/sorafs/council_minutes_2025-10-29.md`). أي تعديلات تتطلب تصويت حوكمة رسمي؛
> ويجب على فرق التنفيذ اعتبار هذه الوثيقة معيارية حتى يعتمد ميثاق بديل.

Vous avez besoin d'un chunker pour SoraFS.
وهو يكمل [دليل تأليف ملفات chunker](./chunker-profile-authoring.md) عبر وصف كيفية اقتراح الملفات الجديدة ومراجعتها
وتصديقها ثم إيقافها لاحقاً.

## النطاق

ينطبق الميثاق على كل إدخال في `sorafs_manifest::chunker_registry` وعلى
Il y a des outils disponibles (CLI manifeste et CLI d'annonce de fournisseur et SDK). ويُطبّق ثوابت alias et handle التي
Télécharger `chunker_registry::ensure_charter_compliance()` :

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب.
- يجب أن يظهر المقبض المعتمد `namespace.name@semver` كأول إدخال في `profile_aliases`.
  تليه البدائل القديمة.
- سلاسل البدائل يتم قصها وتكون فريدة ولا تتعارض مع المقابض المعتمدة في إدخالات أخرى.

## الأدوار- **Auteur(s)** – يُعدّون المقترح، ويعيدون توليد luminaires, ويجمعون أدلة الحتمية.
- **Groupe de travail sur l'outillage (TWG)** – يتحقق من المقترح باستخدام قوائم التحقق المنشورة ويضمن ثبات قواعد السجل.
- **Conseil de gouvernance (GC)** – يراجع تقرير TWG, ويوقع ظرف المقترح، ويوافق على جداول النشر/الإيقاف.
- **Équipe de stockage** – يدير تنفيذ السجل وينشر تحديثات التوثيق.

## سير العمل عبر دورة الحياة

1. **تقديم المقترح**
   - Vous pouvez utiliser JSON pour `ChunkerProfileProposalV1`.
     `docs/source/sorafs/proposals/`.
   - أدرج مخرجات CLI من :
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - قدّم PR يحتوي على luminaires والمقترح وتقرير الحتمية وتحديثات السجل.

2. **outillage de qualité (TWG)**
   - Il s'agit d'un système d'éclairage (fixtures, fuzz, manifeste/PoR).
   - شغّل `cargo test -p sorafs_car --chunker-registry` وتأكد من نجاح
     `ensure_charter_compliance()` مع الإدخال الجديد.
   - تحقق من أن سلوك CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) يعكس البدائل والمقابض المحدثة.
   - قدّم تقريراً قصيراً يلخص النتائج وحالة النجاح/الفشل.

3. **موافقة المجلس (GC)**
   - راجع تقرير TWG et بيانات المقترح.
   - وقّع digest المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`) وأضف التواقيع إلى
     ظرف المجلس المرفق مع prochains matchs.
   - سجّل نتيجة التصويت في محاضر الحوكمة.

4. **النشر**
   - ادمج PR مع تحديث:
     - `sorafs_manifest::chunker_registry_data`.
     - التوثيق (`chunker_registry.md` وأدلة التأليف/المطابقة).
     - les luminaires et les luminaires.
   - Utilisez le SDK pour créer des liens et des liens.5. **الإيقاف / الإزالة التدريجية**
   - يجب أن تتضمن المقترحات التي تستبدل ملفاً قائماً نافذة نشر مزدوجة (فترات سماح) وخطة ترقية.

6. **تغييرات طارئة**
   - تتطلب الإزالة أو الإصلاحات العاجلة تصويتاً بالأغلبية من المجلس.
   - يجب على TWG توثيق خطوات الحد من المخاطر وتحديث سجل الحوادث.

## outillage de توقعات

- `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` pour :
  - `--list-profiles` pour la lecture.
  - `--promote-profile=<handle>` لتوليد كتلة البيانات المعتمدة المستخدمة عند ترقية ملف.
  - `--json-out=-` est connecté à la sortie standard et est connecté à une connexion Internet.
- يتم استدعاء `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`, `provider_advert_stub`). يجب أن تفشل اختبارات CI إذا كانت
  الإدخالات الجديدة تنتهك الميثاق.

## حفظ السجلات

- خزّن كل تقارير الحتمية في `docs/source/sorafs/reports/`.
- محاضر المجلس التي تشير إلى قرارات chunker محفوظة ضمن
  `docs/source/sorafs/migration_ledger.md`.
- Utilisez `roadmap.md` et `status.md` pour vous connecter à votre ordinateur.

## المراجع

- دليل التأليف : [دليل تأليف ملفات chunker](./chunker-profile-authoring.md)
- Numéro de téléphone : `docs/source/sorafs/chunker_conformance.md`
- Nom de l'utilisateur : [Cuncker de fichiers inclus](./chunker-registry.md)