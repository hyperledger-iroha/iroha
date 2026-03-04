---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Local -> Global ایڈریس ٹول کٹ

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ یہ hoja de ruta آئٹم **ADDR-5c** کے لیے درکار CLI helpers اور runbooks اکٹھے کرتا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` CLI کو wrap کرتا ہے تاکہ یہ پیدا کرے:
  - `audit.json` -- `iroha tools address audit --format json` کا salida estructurada۔
  - `normalized.txt` -- ہر Selector de dominio local کے لیے IH58 (ترجیحی) / literales comprimidos (`sora`, segundo mejor) ۔
- Panel de ingesta de direcciones de اس اسکرپٹ کو (`dashboards/grafana/address_ingest.json`)
  اور Reglas de Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) کے ساتھ استعمال کریں تاکہ
  Corte Local-8 / Local-12 کی حفاظت ثابت ہو۔ Local-8 اور Local-12 paneles de colisión اور
  Alertas `AddressLocal8Resurgence`, `AddressLocal12Collision` y `AddressInvalidRatioSlo`
  کو manifiesto تبدیلیاں promover کرنے سے پہلے دیکھیں۔
- UX اور respuesta a incidentes کے لیے [Pautas de visualización de direcciones](address-display-guidelines.md) اور
  [Runbook del manifiesto de dirección](../../../source/runbooks/address_manifest_ops.md) کو دیکھیں۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Artículos:

- `--format compressed` IH58 کے بجائے `sora...` salida کے لیے۔
- `--no-append-domain` تاکہ literales desnudos نکلیں۔
- Paso de conversión `--audit-only` چھوڑنے کے لیے۔
- `--allow-errors` Elimina filas con formato incorrecto پر بھی escaneo جاری رہے (comportamiento CLI جیسا)۔

اسکرپٹ رن کے آخر میں rutas de artefactos لکھتا ہے۔ دونوں فائلیں
ticket de gestión de cambios کے ساتھ منسلک کریں اور Grafana captura de pantalla بھی شامل کریں جو
>=30 دن تک صفر Detecciones de Local-8 اور صفر Colisiones de Local-12 دکھائے۔

## CI انضمام1. اسکرپٹ کو trabajo dedicado میں چلائیں اور salidas اپ لوڈ کریں۔
2. جب `audit.json` Selectores locales رپورٹ کرے (`domain.kind = local12`) تو fusiones روک دیں۔
   default `true` پر رکھیں (صرف dev/test میں regresiones کی تشخیص کے وقت `false` کریں) اور
   `iroha tools address normalize --fail-on-warning --only-local` Tarjeta CI میں شامل کریں تاکہ
   producción de regresiones تک پہنچنے سے پہلے فیل ہوں۔

مزید تفصیلات، listas de verificación de evidencia, اور fragmento de nota de lanzamiento کے لیے سورس دستاویز دیکھیں
جسے آپ cutover کا اعلان کرتے وقت دوبارہ استعمال کر سکتے ہیں۔