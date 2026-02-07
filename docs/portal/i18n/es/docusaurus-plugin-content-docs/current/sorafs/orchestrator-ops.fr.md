---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de orquestador
título: Runbook d'exploitation de l'orchestrateur SoraFS
sidebar_label: orquestador de Runbook
descripción: Guía operativa para desplegar, vigilar y realizar tareas de seguimiento en el orquestador de múltiples fuentes.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Guarde las dos copias sincronizadas justo con el conjunto de documentación Sphinx hérité soit entièrement migré.
:::

Este runbook guía los SRE en la preparación, el despliegue y la explotación del orquestador de búsqueda multifuente. La guía completa del desarrollador con procedimientos adaptados a los despliegues en producción y la activación por etapas y la puesta en lista negra de pares.

> **Voir aussi:** El [Runbook de implementación multifuente](./multi-source-rollout.md) se concentra en las vagas de implementación en la zona del parque y en los rechazos de urgencia de los proveedores. Référez-vous-y pour la coordination gouvernance / staging tout en utilisant ce document pour les opérations quotidiennes de l’orchestrateur.

## 1. Lista de verificación previa al despliegue

1. **Recoge los platos principales**
   - Dernières annonces fournisseurs (`ProviderAdvertV1`) et instantané de télémétrie pour la flotte cible.
   - Plan de carga útil (`plan.json`) derivado de la prueba manifiesta.
2. **Générer un marcador determinado**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- Verifique que `artifacts/scoreboard.json` repertorio cada proveedor de producción como `eligible`.
   - Archivar el JSON de síntesis con el marcador; Los auditores se activan en los contadores de reintento de fragmentos antes de la certificación de la demanda de cambio.
3. **Ejecución en seco con los dispositivos** — Ejecute el mismo comando en los dispositivos públicos de `docs/examples/sorafs_ci_sample/` para asegurarse de que el binario del orquestador corresponda a la versión presente antes de tocar las cargas útiles de producción.

## 2. Procedimiento de implementación por etapas1. **Étape canari (≤2 proveedores)**
   - Reconstruya el marcador y ejecútelo con `--max-peers=2` para limitar el orquestador a un pequeño subconjunto.
   - Vigilancia:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Poursuivez una fois que les taux de retry restent sous 1 % pour un fetch complet du manifeste et qu’aucun fournisseur n’accumule d’échecs.
2. **Étape de montée en charge (50 % de los proveedores)**
   - Aumente `--max-peers` y relancez con una instantánea de televisión reciente.
   - Persiste cada ejecución con `--provider-metrics-out` y `--chunk-receipts-out`. Conservar los artefactos durante ≥7 días.
3. **Despliegue completo**
   - Supprimez `--max-peers` (ou fixez-le au nombre total de fournisseurs elegibles).
   - Active el modo orquestador en los clientes de implementación: distribuya el marcador persistente y el JSON de configuración a través de su sistema de gestión de configuración.
   - Agregue al día las tablas de borde para mostrar `sorafs_orchestrator_fetch_duration_ms` p95/p99 y los histogramas de reintento por región.

## 3. Mise en liste noire et boosting des pairs

Utilice las anulaciones de política de puntuación del CLI para comprobar los proveedores fallidos sin asistir a las actualizaciones del día de gobierno.

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
```- `--deny-provider` retire el alias indicado de la sesión en curso.
- `--boost-provider=<alias>=<weight>` aumenta los pesos del proveedor en el planificador. Los valores se ajustan a los valores normalizados del marcador y no se aplican en el lugar de ejecución.
- Registre las anulaciones en el ticket de incidente y active las salidas JSON para que el equipo responsable pueda reconciliar el estado una vez que el problema esté subyacente corregido.

Para cambios permanentes, modifique la fuente de télémétrie (marque la opción como penalizada) o agregue cada día el anuncio con los presupuestos de flujo revisados ​​antes de suprimir las anulaciones de CLI.

## 4. Paneles de diagnóstico

Lorsqu'un busca échoue:

1. Capturez les artefactos siguientes antes de relanzar:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Inspeccione `session.summary.json` para que la cadena de error sea responsable:
   - `no providers were supplied` → verifique los caminos de los proveedores y los anuncios.
   - `retry budget exhausted ...` → aumenta `--retry-budget` o elimina pares inestables.
   - `no compatible providers available ...` → auditez les métadonnées de capacité de plage du fournisseur fautif.
3. Corrélez le nom du fournisseur avec `sorafs_orchestrator_provider_failures_total` et créez un ticket de suivi si la métrique monte en flèche.
4. Rejouez le fetch hors ligne avec `--scoreboard-json` et la télémétrie capturée pour reproduire o chec de manière deterministe.

## 5. RevertirPour revenir sur un déploiement de l'orchestrateur:

1. Distribuya una configuración que defina `--max-peers=1` (desactivada efectiva la orden de múltiples fuentes) o revenez au chemin de fetch mono-source historique côté clientes.
2. Suprima toda anulación `--boost-provider` para que el marcador regrese a un peso neutro.
3. Continúe revisando las métricas del orquestador durante menos tiempo para confirmar que no hay restos en el vol.

Mantener una captura disciplinada de los artefactos y de las implementaciones por etapas garantiza que el orquestador de múltiples fuentes pueda operar con toda seguridad en las flotas heterogéneas de proveedores todos respetando las exigencias de observación y auditoría.