---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de direcciones Local -> Global

Esta página refleja `docs/source/sns/local_to_global_toolkit.md` del mono-repo. Empaqueta los ayudantes de CLI y runbooks requeridos por el elemento de hoja de ruta **ADDR-5c**.

## Resumen

- `scripts/address_local_toolkit.sh` envuelve la CLI `iroha` para producir:
  - `audit.json` -- salida estructurada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literales I105 (preferido) / comprimido (`sora`) (segunda mejor opción) convertidos para cada selector de dominio Local.
- Combina el script con el tablero de ingesta de direcciones (`dashboards/grafana/address_ingest.json`)
  y las reglas de Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para probar que el cutover Local-8 /
  Local-12 es seguro. Observa los paneles de colisión Local-8 y Local-12 y las alertas.
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, y `AddressInvalidRatioSlo` antes de
  promover cambios de manifiesto.
- Referencia las [Pautas de visualización de direcciones](address-display-guidelines.md) y el
  [Runbook de manifiesto de dirección](../../../source/runbooks/address_manifest_ops.md) para contexto de UX y respuesta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Opciones:

- `--format I105` para salida `sora...` en lugar de I105.
- `domainless output (default)` para emisión literal sin dominio.
- `--audit-only` para omitir el paso de conversión.
- `--allow-errors` para seguir escaneando cuando aparezcan filas malformadas (coinciden con el comportamiento de la CLI).El guión escribe las rutas de artefactos al final de la ejecución. Adjunta ambos archivos a
tu ticket de gestión de cambios junto con la captura de pantalla de Grafana que prueba cero
detecciones Local-8 y cero colisiones Local-12 por >=30 dias.

## Integración CI

1. Ejecuta el guión en un trabajo dedicado y sube sus salidas.
2. Bloquea se fusiona cuando `audit.json` reporte selectores Local (`domain.kind = local12`).
   en su valor por defecto `true` (solo override a `false` en clusters dev/test al
   diagnosticar regresiones) y agrega
   `iroha tools address normalize` un CI para que intentos de
   regresión caída antes de llegar a producción.

Consulta el documento fuente para más detalles, checklists de evidencia y el snippet de
notas de la versión que puedes reutilizar al anunciar el cutover a los clientes.