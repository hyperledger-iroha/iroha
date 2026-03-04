---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de múltiples fuentes
título: Runbook de implementación multifuente y de puesta en lista negra
sidebar_label: Implementación de Runbook de múltiples fuentes
descripción: Checklist opérationnelle pour des déploiements multi-source par étapes et la mise en liste noire d'urgence des fournisseurs.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/runbooks/multi_source_rollout.md`. Guarde las dos copias sincronizadas justo cuando la documentación heredada está retirada.
:::

## Objetivo

Esta guía de runbook de SRE y los ingenieros de análisis a través de dos críticas de flujos de trabajo:

1. Implemente el orquestador multifuente por vagos controles.
2. Poner en lista negra o priorizar los proveedores fallidos sin desestabilizar las sesiones existentes.

Supongo que la pila de orquestación libre bajo SF-6 está ya desplegada (`sorafs_orchestrator`, API de plage de chunks du gateway, exportadores de télémétrie).

> **Voir aussi:** El [Runbook d'exploitation de l'orchestrateur](./orchestrator-ops.md) aplica los procedimientos de ejecución (captura del marcador, bases de implementación de etapas, reversión). Utilice les deux références ensemble lors des changements en producción.

## 1. Validación previa al despliegue1. **Confirmar las entradas de gobierno.**
   - Todos los proveedores candidatos deben publicar sobres `ProviderAdvertV1` con cargas útiles de capacidad de playa y presupuestos de flujo. Valide a través de `/v1/sorafs/providers` y compare los campeones de capacidad de los asistentes.
   - Las instantáneas de télémétrie fournissant les taux de latence/échec doivent dater de moins de 15 minutos antes de cada ejecución canary.
2. **Preparar la configuración.**
   - Continúe la configuración JSON del orquestador en la arborescencia `iroha_config` en sofás:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Guarde el JSON por día con los límites específicos del lanzamiento (`max_providers`, presupuestos de reintento). Utilice el mismo archivo en puesta en escena/producción para guardar las diferencias mínimas.
3. **Ejercer los dispositivos canónicos.**
   - Señale las variables de entorno manifest/token y lance el fetch determinado:

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
     ```Las variables de entorno deben contener el resumen de la carga útil del manifiesto (hexadecimal) y los tokens de flujo codificados en base64 para cada proveedor participante en Canary.
   - Comparez `artifacts/canary.scoreboard.json` con la versión anterior. Todo nuevo proveedor no es elegible o un descuento de peso >10% necesita una revisión.
4. **Verificador que la televisión está cableada.**
   - Abra la exportación Grafana en `docs/examples/sorafs_fetch_dashboard.json`. Assurez-vous que les métriques `sorafs_orchestrator_*` apparaissent in staging avant de poursuivre.

## 2. Mise en liste noire d'urgence des fournisseurs

Suivez este procedimiento cuando un proveedor sirve trozos corrompus, subit des demoras de atención persistentes o escucha aux controles de conformidad.1. **Capturer les preuves.**
   - Exportar el último currículum de fetch (salida `--json-out`). Registre los índices de fragmentos en échec, los alias de los proveedores y las discordancias de resumen.
   - Guarde los extractos de registros pertinentes de los cibles `telemetry::sorafs.fetch.*`.
2. **Aplicar una anulación inmediata.**
   - Marque el proveedor como penalizado en la instantánea de televisión distribuida al orquestador (defina `penalty=true` o fuerce `token_health` a `0`). El marcador prochain excluye automáticamente al proveedor.
   - Para las pruebas de humo ad hoc, pase `--deny-provider gw-alpha` a `sorafs_cli fetch` para ejercitar el camino de falla sin asistir a la propagación de la televisión.
   - Redéployez le bundle télémétrie/configuration mis à jour dans l'environnement touché (puesta en escena → canario → producción). Documentez le changement dans le journal d'incident.
3. **Validar la anulación.**
   - Relancez le fetch de la fixie canonique. Confirme que el marcador marca el proveedor como inelegible con la razón `policy_denied`.
   - Inspeccione `sorafs_orchestrator_provider_failures_total` para verificar que el ordenador ha dejado de aumentar el aumento para que el proveedor lo rechace.
4. **Escalader les interdictions longue durée.**- Si el proveedor permanece bloqueado >24 h, abra un ticket de gobierno para hacer pivote o suspender el anuncio. Jusqu'au vote, conservez la liste de rechazos et rafraîchissez les snapshots de télémétrie pour éviter qu'il ne réintègre le scoreboard.
5. **Protocolo de reversión.**
   - Para restablecer el proveedor, retirar la lista de rechazos, volver a implementarla y capturar una nueva instantánea del marcador. Adjunte el cambio au post mortem d'incident.

## 3. Plan de implementación de etapas| Fase | Portée | Señales requeridas | Criterios Pasa/No pasa |
|-------|--------|----------------|-------------------|
| **Laboratorio** | Clúster de integración creado | Obtener CLI manuel contre las cargas útiles de accesorios | Todos los fragmentos reussissent, les compteurs d'échec fournisseur restent à 0, taux de retry < 5%. |
| **Puesta en escena** | Puesta en escena del plano de control completa | Tablero Grafana conectado; reglas de alerta en modo solo advertencia | `sorafs_orchestrator_active_fetches` vuelve a cero después de cada ejecución de la prueba; aucune alerte `warn/critical`. |
| **Canarias** | ≤10% del tráfico de producción | Buscapersonas más televisión vigilada en tiempo real | Ratio de reintento < 10 %, controles limitados de los pares de golpes connus, histograma de latencia conforme a la estadificación inicial ±20 %. |
| **Disponibilidad general** | 100% del lanzamiento | Reglas del buscapersonas activos | Error cero `NoHealthyProviders` colgante 24 h, ratio de reintento estable, paneles SLA del salpicadero au vert. |

Fase de vertido chaque:

1. Introduzca el JSON del orquestador con los `max_providers` y los presupuestos de reintento anteriores.
2. Lanza `sorafs_cli fetch` o el conjunto de pruebas de integración SDK con el dispositivo canónico y un manifiesto representativo del medio ambiente.
3. Capturez les artefactos de marcador + resumen y adjunte les au registre de release.
4. Revise los paneles de control con el ingeniero de astreinte antes de promocionar en la fase siguiente.## 4. Observabilidad y ganchos del incidente

- **Métricos:** Asegúrese de que Alertmanager surveille `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Un pic soudain significa generalmente que un proveedor se degrada bajo carga.
- **Registros:** Acheminez les cibles `telemetry::sorafs.fetch.*` vers l'agrégateur de logs partagé. Cree las búsquedas registradas para `event=complete status=failed` para acceder al triaje.
- **Marcadores:** Persistez chaque artefacto de marcador en almacenamiento a largo plazo. El JSON sirve además de pista de auditoría para las revisiones de conformidad y las reversiones por etapas.
- **Paneles:** Clonez le tableau Grafana canonique (`docs/examples/sorafs_fetch_dashboard.json`) dans le dossier production avec les règles d'alerte de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicación y documentación

- Periodice cada cambio de denegación/impulso en el registro de cambios de explotación con horodatage, operador, razón e incidente asociado.
- Informe los equipos SDK cuando los pesos de los proveedores o los presupuestos de reintento cambien para alinear los asistentes junto al cliente.
- Una vez finalizada la GA, actualice el `status.md` con el currículum del lanzamiento y archive esta referencia del runbook en las notas de versión.