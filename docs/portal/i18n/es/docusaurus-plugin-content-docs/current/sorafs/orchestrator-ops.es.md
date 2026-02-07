---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de orquestador
título: Runbook de operaciones del orquestador de SoraFS
sidebar_label: Runbook del orquestador
descripción: Guía operativa paso a paso para desplegar, supervisar y revertir el orquestador multi-origen.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantén ambas copias sincronizadas hasta que el conjunto de documentación Sphinx heredado se haya migrado por completo.
:::

Este runbook guía a los SRE en la preparación, el despliegue y la operación del orquestador de fetch multi-origen. Complementa la guía de desarrollo con procedimientos ajustados a despliegues de producción, incluida la habilitación por etapas y el bloqueo de pares.

> **Ver también:** El [Runbook de despliegue multi-origen](./multi-source-rollout.md) se centra en oleadas de despliegue a nivel de flota y en la denegación de proveedores en emergencias. Consúltalo para la coordinación de gobernanza / staging mientras usas este documento para las operaciones diarias del orquestador.

## 1. Lista de verificación previa

1. **Recopilar entradas de proveedores**
   - Los últimos anuncios de proveedores (`ProviderAdvertV1`) y la instantánea de telemetría de la flota objetivo.
   - El plan de carga útil (`plan.json`) derivado del manifiesto bajo prueba.
2. **Generar un marcador determinista**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- Valida que `artifacts/scoreboard.json` liste a cada proveedor de producción como `eligible`.
   - Archiva el JSON de resumen junto al marcador; los auditores se apoyan en los contadores de reintentos de trozos al certificar la solicitud de cambio.
3. **Dry-run con aparatos** — Ejecuta el mismo comando contra los aparatos públicos en `docs/examples/sorafs_ci_sample/` para garantizar que el binario del orquestador coincida con la versión esperada antes de tocar cargas útiles de producción.

## 2. Procedimiento de implementación por etapas1. **Etapa canaria (≤2 proveedores)**
   - Reconstruye el marcador y ejecuta con `--max-peers=2` para limitar el orquestador a un subconjunto pequeño.
   - Supervisar:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Continúa cuando las tasas de reintentos se mantienen por debajo del 1% para un fetch completo del manifiesto y ningún proveedor acumula fallos.
2. **Etapa de rampa (50% de proveedores)**
   - Incrementa `--max-peers` y vuelve a ejecutar con una instantánea de telemetría reciente.
   - Persiste cada ejecución con `--provider-metrics-out` y `--chunk-receipts-out`. Conserva los artefactos durante ≥7 días.
3. **Despliegue completo**
   - Elimina `--max-peers` (o configúralo al recuento completo de elegibles).
   - Habilita el modo orquestador en los despliegues cliente: distribuye el marcador persistente y el JSON de configuración mediante tu sistema de gestión de configuración.
   - Actualiza los paneles para mostrar `sorafs_orchestrator_fetch_duration_ms` p95/p99 y los histogramas de reintentos por región.

## 3. Bloqueo y refuerzo de pares

Utilice las anulaciones de la política de puntuación del CLI para clasificar proveedores no saludables sin esperar actualizaciones de gobernanza.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```- `--deny-provider` elimina el alias indicado de la consideración en la sesión actual.
- `--boost-provider=<alias>=<weight>` aumenta el peso del planificador del proveedor. Los valores se suman al peso normalizado del marcador y solo se aplican a la ejecución local.
- Registre las anulaciones en el ticket de incidente y adjunte las salidas JSON para que el equipo responsable pueda reconciliar el estado una vez que se corrija el problema subyacente.

Para cambios permanentes, modifique la telemetría de origen (marca al infractor como penalizado) o actualice el anuncio con presupuestos de flujo actualizados antes de limpiar las anulaciones del CLI.

## 4. Diagnóstico de fallos

Cuando un fetch falla:

1. Captura los siguientes artefactos antes de volver a ejecutar:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Inspecciona `session.summary.json` para ver la cadena de error legible:
   - `no providers were supplied` → verifica las rutas de los proveedores y los anuncios.
   - `retry budget exhausted ...` → incrementa `--retry-budget` o eliminar pares inestables.
   - `no compatible providers available ...` → audita la metainformación de capacidad de rango del proveedor infractor.
3. Correlaciona el nombre del proveedor con `sorafs_orchestrator_provider_failures_total` y crea un ticket de seguimiento si la métrica se dispara.
4. Reproduzca la obtención sin conexión con `--scoreboard-json` y la telemetría capturada para reproducir el fallo de forma determinista.## 5. Reversión

Para revertir un despliegue del orquestador:

1. Distribuye una configuración que establece `--max-peers=1` (deshabilita de hecho la planificación multi-origen) o vuelve a los clientes a la ruta de fetch heredada de un solo origen.
2. Elimina cualquier anulación `--boost-provider` para que el marcador vuelva a un peso neutral.
3. Continúa recolectando las métricas del orquestador durante al menos un día para confirmar que no queden fetches en vuelo.

Mantener una captura disciplinada de artefactos y despliegues por etapas garantiza que el orquestador multi-origen pueda operarse de forma segura en flotas heterogéneas de proveedores mientras se mantienen los requisitos de observabilidad y auditoría.