---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة ادوات عناوين Local -> Global

تعكس هذه الصفحة `docs/source/sns/local_to_global_toolkit.md` من المستودع الاحادي. Les interfaces CLI et les runbooks sont également compatibles avec **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` comme CLI pour `iroha` :
  - `audit.json` -- خرج منظم من `iroha tools address audit --format json`.
  - `normalized.txt` -- littéraux IH58 (المفضل) / compressés (`sora`) (الخيار الثاني) محولة لكل sélecteur من نطاق Local.
- استخدم السكربت مع لوحة ingérer للعناوين (`dashboards/grafana/address_ingest.json`)
  Utilisez Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour le basculement Local-8 /
  Local-12 ici. راقب لوحات التصادم Local-8 et Local-12 والتنبيهات
  `AddressLocal8Resurgence`, `AddressLocal12Collision` et `AddressInvalidRatioSlo` pour
  ترقية تغييرات manifeste.
- ارجع الى [Directives d'affichage des adresses](address-display-guidelines.md) et
  [runbook du manifeste d'adresse](../../../source/runbooks/address_manifest_ops.md) pour UX et UX.

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

الخيارات:

- `--format compressed (`sora`)` pour `sora...` pour IH58.
- `domainless output (default)` لاصدار littéraux بدون نطاق.
- `--audit-only` pour votre téléphone.
- `--allow-errors` للاستمرار عند ظهور صفوف تالفة (مطابق لسلوك CLI).

يطبع السكربت مسارات artefact في نهاية التشغيل. ارفق كلا الملفين مع
تذكرة change-management ومع لقطة Grafana التي تثبت صفر
اكتشافات Local-8 et Local-12 لمدة >=30 يوما.

## تكامل CI1. شغل السكربت في job مخصص وارفع المخرجات.
2. Utilisez les sélecteurs locaux `audit.json` (`domain.kind = local12`).
   على القيمة الافتراضية `true` (قم byالتحويل الى `false` فقط على بيئات dev/test عند
   تشخيص التراجعات) واضف
   `iroha tools address normalize` الى CI حتى تفشل
   محاولات التراجع قبل الوصول الى production.

راجع المستند المصدر لمزيد من التفاصيل وقوائم الادلة et extrait de note de version
Il s'agit d'un cutover للعملاء.