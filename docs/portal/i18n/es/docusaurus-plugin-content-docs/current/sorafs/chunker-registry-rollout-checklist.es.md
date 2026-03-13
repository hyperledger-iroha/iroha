---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificación de implementación de registro fragmentado
título: Lista de verificación de implementación del registro de fragmentador de SoraFS
sidebar_label: Lista de verificación de implementación de fragmentador
descripción: Plan de implementación paso a paso para actualizaciones del registro de fragmentador.
---

:::nota Fuente canónica
Refleja `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantén ambas copias sincronizadas hasta que se retire el conjunto de documentación Sphinx heredado.
:::

# Lista de verificación de implementación del registro de SoraFS

Esta lista de verificación captura los pasos necesarios para promover un nuevo perfil de chunker
o un paquete de admisión de proveedores desde revisión a producción después de que la
carta de gobernanza haya sido ratificada.

> **Alcance:** Aplica a todas las versiones que modifican
> `sorafs_manifest::chunker_registry`, los sobres de admisión de proveedores o los
> paquetes de accesorios canónicos (`fixtures/sorafs_chunker/*`).

## 1. Validación previa

1. Regenera calendarios y verifica el determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirma que los hashes de determinismo en
   `docs/source/sorafs/reports/sf1_determinism.md` (o el reporte de perfil relevante)
   coinciden con los artefactos regenerados.
3. Asegura que `sorafs_manifest::chunker_registry` compila con
   `ensure_charter_compliance()` ejecutando:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Actualiza el expediente de la propuesta:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de actas del consejo en `docs/source/sorafs/council_minutes_*.md`
   - Informe de determinismo

## 2. Aprobación de gobernanza1. Presenta el informe del Tooling Working Group y el resumen de la propuesta al
   Panel de Infraestructura del Parlamento de Sora.
2. Registra los detalles de aprobación en
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publica el sobre firmado por el Parlamento junto a los partidos:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifica que el sobre sea accesible vía el helper de fetch de gobernanza:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Implementación y puesta en escena

Consulta el [playbook de manifest en staging](./staging-manifest-playbook) para un
recorrido detallado de estos pasos.

1. Despliega Torii con descubrimiento `torii.sorafs` habilitado y la aplicación de
   admisión activada (`enforce_admission = true`).
2. Sube los sobres de admisión de proveedores aprobados al directorio de registro
   de puesta en escena referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique que los anuncios del proveedor se propaguen vía la API de descubrimiento:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. Ejecuta los endpoints de manifest/plan con headers de gobernanza:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirma que los paneles de telemetría (`torii_sorafs_*`) y las reglas de
   alerta reporten el nuevo perfil sin errores.

## 4. Lanzamiento en producción1. Repite los pasos de staging contra los nodos Torii de producción.
2. Anuncia la ventana de activación (fecha/hora, periodo de gracia, plan de rollback)
   a los canales de operadores y SDK.
3. Fusiona el PR de liberación que contiene:
   - Accesorios y sobre actualizaciones
   - Cambios de documentación (referencias a la carta, reporte de determinismo)
   - Actualizar la hoja de ruta/estado
4. Etiqueta la liberación y archivo de los artefactos firmados para la procedencia.

## 5. Auditoría post-implementación

1. Captura métricas finales (conteos de descubrimiento, tasa de éxito de fetch,
   histogramas de error) 24 h después del rollout.
2. Actualiza `status.md` con un resumen corto y un enlace al reporte de determinismo.
3. Registra cualquier tarea de seguimiento (p. ej., más guía de autoría de perfiles)
   es `roadmap.md`.