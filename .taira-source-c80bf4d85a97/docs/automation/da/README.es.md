---
lang: es
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9e5fce128259ae2b2c40782b3c96c38048fce6f3b4522319bd60b59db87a8252
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Automatización del modelo de amenazas de Disponibilidad de Datos (DA-1)

El elemento del roadmap DA-1 y `status.md` piden un bucle de automatización
 determinista que produzca los resúmenes del modelo de amenazas Norito PDP/PoTR
 publicados en `docs/source/da/threat_model.md` y su espejo en Docusaurus. Este
 directorio captura los artefactos referenciados por:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (que ejecuta `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Flujo

1. **Genera el reporte**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   El resumen JSON registra la tasa simulada de fallas de replicación, los
   umbrales del chunker y cualquier violación de política detectada por el
   harness PDP/PoTR en `integration_tests/src/da/pdp_potr.rs`.
2. **Renderiza las tablas Markdown**
   ```bash
   make docs-da-threat-model
   ```
   Esto ejecuta `scripts/docs/render_da_threat_model_tables.py` para reescribir
   `docs/source/da/threat_model.md` y `docs/portal/docs/da/threat-model.md`.
3. **Archiva el artefacto** copiando el reporte JSON (y el log opcional de CLI)
   en `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Cuando
   decisiones de gobernanza dependan de una ejecución específica, incluye el hash
   del commit y la semilla del simulador en un archivo `<timestamp>-metadata.md`.

## Expectativas de evidencia

- Los archivos JSON deben mantenerse <100 KiB para vivir en git. Trazas de mayor
  tamaño deben ir a almacenamiento externo; referencia su hash firmado en la nota
  de metadatos si hace falta.
- Cada archivo archivado debe listar la semilla, la ruta de configuración y la
  versión del simulador para que las ejecuciones se puedan reproducir con exactitud.
- Enlaza al archivo archivado desde `status.md` o la entrada del roadmap cada
  vez que los criterios de aceptación de DA-1 avancen, asegurando que los
  revisores puedan verificar la baseline sin volver a ejecutar el harness.

## Reconciliación de compromisos (omisión de secuenciador)

Usa `cargo xtask da-commitment-reconcile` para comparar recibos de ingestión DA
con los registros de compromisos DA, detectando omisiones o manipulaciones del
secuenciador:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Acepta recibos en formato Norito o JSON y compromisos de `SignedBlockWire`,
  `.norito` o bundles JSON.
- Falla cuando falta algún ticket en el log de bloques o cuando divergen los
  hashes; `--allow-unexpected` ignora tickets solo-en-bloque cuando acotas el
  conjunto de recibos de manera intencional.
- Adjunta el JSON emitido a paquetes de gobernanza/Alertmanager para alertas de
  omisión; por defecto se usa `artifacts/da/commitment_reconciliation.json`.

## Auditoría de privilegios (revisión trimestral de acceso)

Usa `cargo xtask da-privilege-audit` para escanear los directorios de manifiestos
/replay DA (y rutas extra opcionales) en busca de entradas faltantes, que no son
carpetas o con permisos de escritura global:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Lee las rutas de ingestión DA desde la configuración de Torii e inspecciona
  permisos Unix cuando están disponibles.
- Señala rutas faltantes/no directorio/con permisos world-writable y devuelve un
  exit code distinto de cero cuando hay problemas.
- Firma y adjunta el bundle JSON (`artifacts/da/privilege_audit.json` por
  defecto) a paquetes y dashboards de revisión trimestral.
