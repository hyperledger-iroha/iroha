---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c865b3130520742fa6583ceb0316f5e4ca1e64945c555c2940f6c76c3475cfd
source_last_modified: "2025-11-14T04:43:21.827781+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/multi_source_rollout.md`. Mantén ambas copias alineadas hasta que se retire el conjunto de documentación heredado.
:::

## Propósito

Este runbook guía a los SRE y a los ingenieros de guardia a través de dos flujos críticos:

1. Desplegar el orquestador multi-origen en oleadas controladas.
2. Denegar o depriorizar proveedores que se comportan mal sin desestabilizar las sesiones existentes.

Asume que la pila de orquestación entregada bajo SF-6 ya está desplegada (`sorafs_orchestrator`, API de rango de chunks del gateway, exportadores de telemetría).

> **Ver también:** El [Runbook de operaciones del orquestador](./orchestrator-ops.md) profundiza en los procedimientos por ejecución (captura de scoreboard, toggles de despliegue por etapas, rollback). Usa ambas referencias en conjunto durante cambios en vivo.

## 1. Validación previa

1. **Confirmar entradas de gobernanza.**
   - Todos los proveedores candidatos deben publicar sobres `ProviderAdvertV1` con payloads de capacidad de rango y presupuestos de stream. Valídalo mediante `/v1/sorafs/providers` y compara con los campos de capacidad esperados.
   - Las instantáneas de telemetría que aportan tasas de latencia/fallo deben tener menos de 15 minutos antes de cada ejecución canaria.
2. **Preparar la configuración.**
   - Persiste la configuración JSON del orquestador en el árbol `iroha_config` por capas:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Actualiza el JSON con límites específicos del rollout (`max_providers`, presupuestos de reintentos). Usa el mismo archivo en staging/producción para que las diferencias sean mínimas.
3. **Ejercitar los fixtures canónicos.**
   - Rellena las variables de entorno del manifiesto/token y ejecuta el fetch determinista:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Las variables de entorno deben contener el digest del payload del manifiesto (hex) y los tokens de stream codificados en base64 para cada proveedor que participe en el canary.
   - Compara `artifacts/canary.scoreboard.json` con el release anterior. Cualquier proveedor nuevo no elegible o un cambio de peso >10% requiere revisión.
4. **Verificar que la telemetría está conectada.**
   - Abre la exportación de Grafana en `docs/examples/sorafs_fetch_dashboard.json`. Asegúrate de que las métricas `sorafs_orchestrator_*` se poblen en staging antes de continuar.

## 2. Denegación de proveedores en emergencias

Sigue este procedimiento cuando un proveedor entregue chunks corruptos, agote tiempos de espera de forma persistente o falle comprobaciones de cumplimiento.

1. **Capturar evidencias.**
   - Exporta el resumen de fetch más reciente (salida de `--json-out`). Registra índices de chunks fallidos, alias de proveedores y desajustes de digest.
   - Guarda extractos de logs relevantes de los targets `telemetry::sorafs.fetch.*`.
2. **Aplicar un override inmediato.**
   - Marca al proveedor como penalizado en la instantánea de telemetría distribuida al orquestador (establece `penalty=true` o limita `token_health` a `0`). La siguiente construcción del scoreboard excluirá automáticamente al proveedor.
   - Para pruebas de humo ad-hoc, pasa `--deny-provider gw-alpha` a `sorafs_cli fetch` para ejercitar la ruta de fallo sin esperar a la propagación de la telemetría.
   - Vuelve a desplegar el paquete actualizado de telemetría/configuración en el entorno afectado (staging → canary → production). Documenta el cambio en el log del incidente.
3. **Validar el override.**
   - Repite el fetch del fixture canónico. Confirma que el scoreboard marca al proveedor como no elegible con el motivo `policy_denied`.
   - Inspecciona `sorafs_orchestrator_provider_failures_total` para asegurarte de que el contador deja de incrementarse para el proveedor denegado.
4. **Escalar bloqueos prolongados.**
   - Si el proveedor seguirá bloqueado durante >24 h, abre un ticket de gobernanza para rotar o suspender su advert. Hasta que pase la votación, mantén la lista de denegación y refresca las instantáneas de telemetría para que el proveedor no reingrese al scoreboard.
5. **Protocolo de rollback.**
   - Para restablecer al proveedor, elimínalo de la lista de denegación, vuelve a desplegar y captura una instantánea fresca del scoreboard. Adjunta el cambio al postmortem del incidente.

## 3. Plan de despliegue por etapas

| Fase | Alcance | Señales requeridas | Criterio de Go/No-Go |
|------|---------|--------------------|----------------------|
| **Lab** | Clúster de integración dedicado | Fetch manual por CLI contra payloads de fixtures | Todos los chunks se completan, los contadores de fallos de proveedor permanecen en 0, tasa de reintentos < 5%. |
| **Staging** | Staging del plano de control completo | Dashboard de Grafana conectado; reglas de alertas en modo solo warning | `sorafs_orchestrator_active_fetches` vuelve a cero después de cada ejecución de prueba; no se disparan alertas `warn/critical`. |
| **Canary** | ≤10% del tráfico de producción | Pager silenciado pero telemetría monitorizada en tiempo real | Ratio de reintentos < 10%, fallos de proveedores aislados a peers ruidosos conocidos, histograma de latencia coincide con la línea base de staging ±20%. |
| **Disponibilidad general** | 100% del rollout | Reglas del pager activas | Cero errores `NoHealthyProviders` durante 24 h, ratio de reintentos estable, paneles SLA del dashboard en verde. |

Para cada fase:

1. Actualiza el JSON del orquestador con los `max_providers` y presupuestos de reintentos previstos.
2. Ejecuta `sorafs_cli fetch` o la suite de pruebas de integración del SDK contra el fixture canónico y un manifiesto representativo del entorno.
3. Captura los artefactos del scoreboard y el summary, y adjúntalos al registro de release.
4. Revisa los dashboards de telemetría con el ingeniero de guardia antes de promocionar a la siguiente fase.

## 4. Observabilidad y ganchos de incidentes

- **Métricas:** Asegúrate de que Alertmanager monitoree `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` y `sorafs_orchestrator_retries_total`. Un pico repentino suele significar que un proveedor se degrada bajo carga.
- **Logs:** Enruta los targets `telemetry::sorafs.fetch.*` al agregador de logs compartido. Crea búsquedas guardadas para `event=complete status=failed` para acelerar el triage.
- **Scoreboards:** Persiste cada artefacto de scoreboard en almacenamiento a largo plazo. El JSON también sirve como rastro de evidencia para revisiones de cumplimiento y rollbacks por etapas.
- **Dashboards:** Clona el tablero Grafana canónico (`docs/examples/sorafs_fetch_dashboard.json`) en la carpeta de producción con las reglas de alerta de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicación y documentación

- Registra cada cambio de denegación/boost en el changelog de operaciones con marca de tiempo, operador, motivo e incidente asociado.
- Notifica a los equipos de SDK cuando los pesos de proveedores o los presupuestos de reintentos cambien para alinear las expectativas del lado del cliente.
- Después de completar GA, actualiza `status.md` con el resumen del rollout y archiva esta referencia del runbook en las notas de la versión.
