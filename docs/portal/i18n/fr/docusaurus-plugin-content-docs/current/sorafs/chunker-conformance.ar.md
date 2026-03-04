---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : conformité du chunker
title: دليل مطابقة chunker pour SoraFS
sidebar_label : plus chunker
Description : Des fonctionnalités telles que des chunker pour SF1, des luminaires et des SDK.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/chunker_conformance.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة.
:::

يوثق هذا الدليل المتطلبات التي يجب على كل تطبيق اتباعها للبقاء متوافقاً مع ملف chunker الحتمي في SoraFS (SF1).
Vous pouvez également utiliser les SDK pour les luminaires et les SDK.

## الملف المعتمد

- Nom du produit : `sorafs.sf1@1.0.0` (Nouveau produit `sorafs.sf1@1.0.0`)
- Nom de la personne (hex) : `0000000000dec0ded`
- Format de fichier : 262 144 octets (256 Ko)
- Format de fichier : 65 536 octets (64 Ko)
- Volume de chargement : 524 288 octets (512 Ko)
- Nom du produit : `0x3DA3358B4DC173`
- Vitesse de transmission : `sorafs-v1-gear`
- Nom du produit : `0x0000FFFF`

Nom du produit : `sorafs_chunker::chunk_bytes_with_digests_profile`.
يجب أن ينتج أي تسريع SIMD نفس الحدود والـ digests.

## calendriers de حزمة

`cargo run --locked -p sorafs_chunker --bin export_vectors` يعيد توليد
luminaires et accessoires pour `fixtures/sorafs_chunker/` :- `sf1_profile_v1.{json,rs,ts,go}` — Un morceau de morceau pour Rust, TypeScript et Go.
  يعلن كل ملف المقبض المعتمد كأول إدخال في `profile_aliases`, يتبعه أي بدائل قديمة (مثل
  `sorafs.sf1@1.0.0` ou `sorafs.sf1@1.0.0`). يتم فرض الترتيب بواسطة
  `ensure_charter_compliance` est également disponible.
- `manifest_blake3.json` — manifeste pour les luminaires de BLAKE3.
- `manifest_signatures.json` — توقيعات المجلس (Ed25519) pour digérer le manifeste.
- `sf1_profile_v1_backpressure.json` et corpus de personnes `fuzz/` —
  Il s'agit d'un outil de contre-pression pour chunker.

### سياسة التوقيع

يجب أن تشمل إعادة توليد luminaires توقيعاً صالحاً من المجلس. يرفض المولد
الإخراج غير الموقّع ما لم يتم تمرير `--allow-unsigned` صراحة (مخصص
للتجارب المحلية فقط). Les ajouts uniquement et les ajouts à l'application sont également possibles.

لإضافة توقيع من المجلس:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

يعيد مساعد CI `ci/check_sorafs_fixtures.sh` تشغيل المولد مع
`--locked`. Les rencontres et les matchs sont terminés. استخدم
Il s'agit de flux de travail et de luminaires.

خطوات التحقق اليدوية:

1. Voir `cargo test -p sorafs_chunker`.
2. نفّذ `ci/check_sorafs_fixtures.sh` محلياً.
3. Vérifiez le `git status -- fixtures/sorafs_chunker` نظيف.

## دليل الترقية

J'utilise le chunker de SF1 :

انظر أيضاً: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) لمتطلبات
البيانات الوصفية وقوالب المقترح وقوائم التحقق.1. Utilisez `ChunkProfileUpgradeProposalV1` (appelé RFC SF-1) pour cela.
2. أعد توليد luminaires عبر `export_vectors` وسجل digest الجديد للـ manifest.
3. وقّع الـ manifest بحصة المجلس المطلوبة. يجب إلحاق كل التواقيع بـ `manifest_signatures.json`.
4. Mettre en place des luminaires avec des SDK (Rust/Go/TS) et des kits de développement logiciel.
5. Utilisez Corpo Fuzz pour vos besoins.
6. حدّث هذا الدليل بالمقبض الجديد للملف والبذور وdigest.
7. Feuille de route de la feuille de route.

التغييرات التي تؤثر على حدود الـ chunk أو الـ digests دون اتباع هذه العملية
غير صالحة ولا يجب دمجها.