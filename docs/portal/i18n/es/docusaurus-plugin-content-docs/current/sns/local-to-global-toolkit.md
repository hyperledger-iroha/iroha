---
lang: es
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Kit de direcciones Local -> Global

Esta pagina refleja `docs/source/sns/local_to_global_toolkit.md` del mono-repo. Empaqueta los helpers de CLI y runbooks requeridos por el item de roadmap **ADDR-5c**.

## Resumen

- `scripts/address_local_toolkit.sh` envuelve la CLI `iroha` para producir:
  - `audit.json` -- salida estructurada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literales I105 (preferido) / I105 (segunda mejor opcion) convertidos para cada selector de dominio Local.
- Combina el script con el dashboard de ingesta de direcciones (`dashboards/grafana/address_ingest.json`)
  y las reglas de Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para probar que el cutover Local-8 /
  Local-12 es seguro. Observa los paneles de colision Local-8 y Local-12 y las alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, y `AddressInvalidRatioSlo` antes de
  promover cambios de manifest.
- Referencia las [Address Display Guidelines](address-display-guidelines.md) y el
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) para contexto de UX y respuesta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Opciones:

- `--format I105` para salida `sora...` en lugar de I105.
- `domainless output (default)` para emitir literales sin dominio.
- `--audit-only` para omitir el paso de conversion.
- `--allow-errors` para seguir escaneando cuando aparezcan filas malformadas (coincide con el comportamiento de la CLI).

El script escribe las rutas de artefactos al final de la ejecucion. Adjunta ambos archivos a
tu ticket de gestion de cambios junto con el screenshot de Grafana que pruebe cero
detecciones Local-8 y cero colisiones Local-12 por >=30 dias.

## Integracion CI

1. Ejecuta el script en un job dedicado y sube sus salidas.
2. Bloquea merges cuando `audit.json` reporte selectores Local (`domain.kind = local12`).
   en su valor por defecto `true` (solo override a `false` en clusters dev/test al
   diagnosticar regresiones) y agrega
   `iroha tools address normalize` a CI para que intentos de
   regresion fallen antes de llegar a produccion.

Consulta el documento fuente para mas detalles, checklists de evidencia y el snippet de
release notes que puedes reutilizar al anunciar el cutover a clientes.
