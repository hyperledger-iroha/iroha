---
lang: es
direction: ltr
source: docs/source/sorafs/chunker_registry_rollout_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e4d52f449ae561d75e4723d9573ad6e099fda92ccae1f0fe683baacbbcae494c
source_last_modified: "2025-11-04T10:34:05.774019+00:00"
translation_last_reviewed: "2026-01-30"
---

# Checklist de rollout del registro SoraFS

Este checklist captura los pasos requeridos para promover un nuevo perfil de
chunker o bundle de admision de providers desde review a produccion despues de
que el charter de governance haya sido ratificado.

> **Alcance:** Aplica a todos los releases que modifican
> `sorafs_manifest::chunker_registry`, sobres de admision de providers o los
> bundles canonicos de fixtures (`fixtures/sorafs_chunker/*`).

## 1. Validacion pre-flight

1. Regenerar fixtures y verificar determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirmar que los hashes de determinismo en
   `docs/source/sorafs/reports/sf1_determinism.md` (o el reporte del perfil
   relevante) coinciden con los artefactos regenerados.
3. Asegurar que `sorafs_manifest::chunker_registry` compila con
   `ensure_charter_compliance()` ejecutando:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Actualizar el dossier de propuesta:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de council minutes bajo `docs/source/sorafs/council_minutes_*.md`
   - Reporte de determinismo

## 2. Sign-off de governance

1. Presentar el reporte del Tooling Working Group y el digest de la propuesta al
   Sora Parliament Infrastructure Panel.
2. Registrar detalles de aprobacion en
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publicar el sobre firmado por el Parliament junto a los fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verificar que el sobre sea accesible via el helper de governance fetch:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Rollout en staging

Referirse al [staging manifest playbook](runbooks/staging_manifest_playbook.md) para
una guia detallada de estos pasos.

1. Desplegar Torii con `torii.sorafs` discovery habilitado y enforcement de
   admision encendido (`enforce_admission = true`).
2. Empujar los sobres de admision aprobados al directorio de registry staging
   referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verificar que los provider adverts se propaguen via la API discovery:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Ejercitar endpoints de manifest/plan con headers de governance:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirmar que dashboards de telemetria (`torii_sorafs_*`) y reglas de alerta
   reportan el nuevo perfil sin errores.

## 4. Rollout en produccion

1. Repetir los pasos de staging contra nodos Torii de produccion.
2. Anunciar la ventana de activacion (fecha/hora, periodo de gracia, plan de rollback)
   a canales de operadores y SDK.
3. Mergear el PR de release que contiene:
   - Fixtures y sobre actualizado
   - Cambios de documentacion (referencias de charter, reporte de determinismo)
   - Refresh de roadmap/status
4. Taggear el release y archivar los artefactos firmados para provenance.

## 5. Auditoria post-rollout

1. Capturar metricas finales (conteos discovery, tasa de exito de fetch,
   histogramas de error) 24h despues del rollout.
2. Actualizar `status.md` con un resumen corto y link al reporte de determinismo.
3. Registrar tareas de seguimiento (p. ej., guidance adicional de authoring de perfiles)
   en `roadmap.md`.
