---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ערכת הנחיות מקומית -> גלובלית

Esta page refleja `docs/source/sns/local_to_global_toolkit.md` del mono-repo. Empaqueta los helpers de CLI y runbooks requeridos por el item de roadmap **ADDR-5c**.

## קורות חיים

- `scripts/address_local_toolkit.sh` envuelve la CLI `iroha` עבור מפיק:
  - `audit.json` -- salida estructurada de `iroha tools address audit --format json`.
  - `normalized.txt` -- ליטרלים I105 (מועדף) / דחוס (`sora`) (אופציה גדולה יותר) convertidos para cada selector de dominio Local.
- שילוב של סקריפט עם לוח המחוונים של אינסטציה הנחיות (`dashboards/grafana/address_ingest.json`)
  y las reglas de Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para probar que el cutover Local-8 /
  Local-12 es seguro. Observa los paneles de colision Local-8 y Local-12 y las alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, y `AddressInvalidRatioSlo` antes de
  מקדם cambios de manifest.
- Referencia las [הנחיות להצגת כתובת](address-display-guidelines.md) y el
  [פנקס מניפסט כתובות](../../../source/runbooks/address_manifest_ops.md) להקשר של UX ותשובות לאירועים.

## שימוש

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

אפשרויות:

- `--format I105` para salida `sora...` en lugar de I105.
- `domainless output (default)` para emitir literales sin dominio.
- `--audit-only` להסרת המרה.
- `--allow-errors` para seguir escaneando cuando aparezcan filas malformadas (בהתאמה ל-el comportamiento de la CLI).

אל התסריט כתוב לאס רוטאס דה ארטפקטוס אל סופי דה לה פליטה. Adjunta ambos archivos א
tu ticket de gestion de cambios junto con el screen de Grafana que pruebe cero
detecciones Local-8 y cero colisiones Local-12 por >=30 dias.

## אינטגרציה CI

1. Ejecuta el script en un job dedicado y sube sus salidas.
2. Bloquea ממזג את cuando `audit.json` reporte selectores Local (`domain.kind = local12`).
   en su valor por defecto `true` (החלפת יחידה של `false` en clusters dev/test al
   diagnosticar regresiones) y agrega
   `iroha tools address normalize` a CI para que intentos de
   רגרסיה נפל antes de llegar a produccion.

Consulta el documento fuente para mas detalles, checklists de evidencia y el snippet de
הערות שחרור que puedes reutilizar al ununciar el cutover a clientes.