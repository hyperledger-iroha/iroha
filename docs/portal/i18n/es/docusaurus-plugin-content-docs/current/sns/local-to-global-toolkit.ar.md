---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة ادوات عناوين Local -> Global

Utilice el dispositivo `docs/source/sns/local_to_global_toolkit.md` para conectarlo. Utilice CLI y runbooks para ejecutar **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` desde la CLI de `iroha`:
  - `audit.json` -- Está conectado a `iroha tools address audit --format json`.
  - `normalized.txt` -- literales IH58 (المفضل) / comprimido (`sora`) (الخيار الثاني) محولة لكل selector من نطاق Local.
- استخدم السكربت مع لوحة ingest للعناوين (`dashboards/grafana/address_ingest.json`)
  وقواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) لاثبات ان cutover Local-8 /
  Local-12 de marzo. راقب لوحات التصادم Local-8 y Local-12 والتنبيهات
  `AddressLocal8Resurgence`, `AddressLocal12Collision` y `AddressInvalidRatioSlo`
  Manifiesto de ترقية تغييرات.
- ارجع الى [Pautas de visualización de direcciones](address-display-guidelines.md) y
  [Runbook del manifiesto de dirección](../../../source/runbooks/address_manifest_ops.md) لسياق UX واستجابة الحوادث.

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

الخيارات:

- `--format compressed (`sora`)` para `sora...` para IH58.
- `--no-append-domain` لاصدار literales بدون نطاق.
- `--audit-only` لتخطي خطوة التحويل.
- `--allow-errors` Para obtener más información, consulte la CLI.

يطبع السكربت مسارات artefacto في نهاية التشغيل. ارفق كلا الملفين مع
تذكرة gestión de cambios y لقطة Grafana التي تثبت صفر
اكتشافات Local-8 y تصادمات Local-12 لمدة >=30 يوما.

## تكامل CI1. شغل السكربت في trabajo مخصص وارفع المخرجات.
2. احظر عمليات الدمج عندما يبلغ `audit.json` عن Selectores locales (`domain.kind = local12`).
   على القيمة الافتراضية `true` (قم بالتحويل الى `false` فقط على بيئات dev/test عند
   تشخيص التراجعات) y
   `iroha tools address normalize --fail-on-warning --only-local` El CI está conectado
   محاولات التراجع قبل الوصول الى producción.

Información detallada sobre el contenido y el fragmento de nota de la versión
الذي يمكنك اعادة استخدامه عند اعلان cutover للعملاء.