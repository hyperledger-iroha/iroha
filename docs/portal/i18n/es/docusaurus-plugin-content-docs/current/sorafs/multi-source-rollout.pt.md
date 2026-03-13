---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de múltiples fuentes
título: Runbook de implementación de múltiples orígenes y negación de proveedores
sidebar_label: Runbook de implementación de múltiples orígenes
descripción: Lista de verificación operativa para implementaciones de múltiples orígenes en fases y negación de emergencia de proveedores.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/runbooks/multi_source_rollout.md`. Mantenha ambas como copias sincronizadas.
:::

## Objetivo

Este runbook orienta los SRE y los ingenieros de planta en dos flujos críticos:

1. Fazer o rollout do orquestador multiorigen en ondas controladas.
2. Negar ou despriorizar provedores com mau comportamento sem desestabilizar sessões existente.

Pressupõe que a pilha de orquestração entregue sob SF-6 ya está implantada (`sorafs_orchestrator`, API de intervalo de fragmentos de gateway, exportadores de telemetría).

> **Veja también:** El [Runbook de operações do orquestador](./orchestrator-ops.md) cubre los procedimientos de ejecución (captura de marcador, alternancia de implementación en fases, reversión). Use ambas como referencias en conjunto durante mudanças ao vivo.

## 1. Validación previa al voto1. **Confirmar insumos de gobernanza.**
   - Todos los proveedores candidatos deben publicar sobres `ProviderAdvertV1` com payloads de capacidade de intervalo e orçamentos de stream. Valide vía `/v2/sorafs/providers` y compare con los campos de capacidad esperados.
   - Instantáneas de telemetría que fornecem taxas de latência/falha devem ter menos de 15 minutos antes de cada ejecución canária.
2. **Preparar una configuración.**
   - Persiste la configuración JSON del orquestador en el árbol en camadas `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Actualizar JSON con límites específicos de implementación (`max_providers`, presupuestos de reintento). Utilice el mesmo archivo em staging/produção para manter como diferencias mínimas.
3. **Ejercitar aparatos canônicas.**
   - Preencha as variáveis de ambiente de manifesto/token y ejecuta o fetch determinístico:

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

     Como variaveis de ambiente devem conter o digest do payload do manifesto (hex) y tokens de stream codificados en base64 para cada proveedor participante de canary.
   - Comparar `artifacts/canary.scoreboard.json` con liberación anterior. Cualquier nuevo proveedor inelegível o cambio de peso >10% exige revisión.
4. **Verifique que una telemetría esté conectada.**
   - Abra la exportación de Grafana a `docs/examples/sorafs_fetch_dashboard.json`. Garanta que as métricas `sorafs_orchestrator_*` apareçam em staging antes de avanzar.

## 2. Negação emergencial de provedoresSiga este procedimiento cuando un proveedor sirve trozos corrompidos, estourar tiempos de espera de forma persistente o falhar em verificações de cumplimiento.1. **Capturar evidencias.**
   - Exporte o resumo de fetch mais Recente (saída de `--json-out`). Registre índices de trozos con falha, alias de proveedores y divergencias de digestión.
   - Salve trechos de log relevantes dos target `telemetry::sorafs.fetch.*`.
2. **Aplicar anulación inmediata.**
   - Marque o proveedor como penalizado sin instantánea de telemetría distribuida al orquestador (defina `penalty=true` o limite `token_health` a `0`). La próxima construcción del marcador eliminará el proveedor automáticamente.
   - Para pruebas de humo ad-hoc, pase `--deny-provider gw-alpha` a `sorafs_cli fetch` para ejercitar el camino de falha sin esperar la propagación de la telemetría.
   - Reimplante o bundle atualizado de telemetria/configuração no ambiente afetado (puesta en escena → canario → producción). Documente a mudança no log de incidente.
3. **Validar o anular.**
   - Vuelva a ejecutar o buscar el dispositivo canónico. Confirme que o marcador marca o proveedor como inelegível com o motivo `policy_denied`.
   - Inspeccione `sorafs_orchestrator_provider_failures_total` para garantizar que el contador pare de aumentar para el proveedor negado.
4. **Escalar banimentos prolongados.**
   - Si el proveedor permanece bloqueado por >24 h, abra un ticket de gobierno para rotar o suspender su anuncio. Después de pasar la votación, mantener la lista de negaciones y actualizar las instantáneas de telemetría para que el proveedor no vuelva al marcador.
5. **Protocolo de reversión.**- Para reintegrar el proveedor, eliminar la lista de negaciones, reimplantar y capturar una nueva instantánea del marcador. Anexe a mudança ao post mortem do incidente.

## 3. Plano de implementación en fases

| Fase | Escopo | Sinais obrigatórios | Criterios Pasa/No pasa |
|------|--------|---------------------|--------------------|
| **Laboratorio** | Clúster de integración dedicado | Obtener manual por CLI contra cargas útiles de accesorios | Todos los fragmentos concluyen, contadores de fallas de proveedor ficam en 0, taxa de reintentos < 5%. |
| **Puesta en escena** | Puesta en escena del plano de control completo | Panel de control Grafana conectado; regras de alerta em modo somente advertencia | `sorafs_orchestrator_active_fetches` volta a cero después de cada ejecución de prueba; nenhum alerta `warn/critical`. |
| **Canarias** | ≤10% del tráfico de producción | Buscapersonas silenciado más telemetría monitorizada en tiempo real | Razón de reintentos < 10%, faltas de proveedores aislados y pares ruidosos conhecidos, histograma de latencia coincide con una línea base de estadificación ±20%. |
| **Disponibilidad general** | 100% de implementación | Regras do buscapersonas activas | Cero errores `NoHealthyProviders` por 24 h, la razón de los reintentos está en el SLA del tablero en verde. |

Para cada fase:1. Actualizar el JSON del orquestador con el sistema `max_providers` y los presupuestos de reintento previstos.
2. Ejecute `sorafs_cli fetch` o una suite de pruebas de integración del SDK contra el dispositivo canónico y un manifiesto representativo del ambiente.
3. Capture los artefactos del marcador + resumen y anexos al registro de lanzamiento.
4. Revise los paneles de telemetría con el motor de planta antes de promoverlos para la próxima fase.

## 4. Observabilidade e ganchos de incidente

- **Métricas:** Garantía de que Alertmanager monitorea `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Um pico repentino generalmente significa que um provedor está degradando sollozo carga.
- **Registros:** Roteie os target `telemetry::sorafs.fetch.*` para el agregador de registros compartilados. Grite buscas salvas para `event=complete status=failed` para acelerar o triaje.
- **Marcadores:** Persista cada artefato de marcador em armazenamento de longo prazo. El JSON también sirve como prueba de prueba para revisiones de cumplimiento y reversiones por fase.
- **Paneles:** Clonar el panel Grafana canónico (`docs/examples/sorafs_fetch_dashboard.json`) na pasta de producción con registros de alerta de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Comunicación y documentación- Registre cada cambio de denegación/impulso en el registro de cambios de operaciones con marca de tiempo, operador, motivo e incidente asociado.
- Notifique los equipos de SDK cuando los pesos de los proveedores o los presupuestos de reintento cambien para cumplir con las expectativas del lado del cliente.
- Después de finalizar GA, actualice `status.md` con el resumen del lanzamiento y archive esta referencia de runbook en notas de lanzamiento.