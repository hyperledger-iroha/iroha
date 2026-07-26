---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de orquestador
título: Runbook de operaciones del orquestador SoraFS
sidebar_label: Runbook del orquestador
descripción: Guía operativa paso a paso para implantar, monitorizar y revertir el orquestador multiorigen.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantenha ambas como copias sincronizadas.
:::

Este runbook orienta los SRE en la preparación, la implementación y la operación del orquestador de búsqueda de múltiples orígenes. Ele complementa la guía de desarrollo con procedimientos ajustados para implementaciones en producción, incluyendo habilitación en fases y bloqueo de pares.

> **Veja também:** O [Runbook de rollout multi-origem](./multi-source-rollout.md) foca em ondas de rollout em toda a frota e na negação emergencial de provedores. Consulte-o para coordinación de gobierno / puesta en escena mientras usa este documento para las operaciones diarias del orquestador.

## 1. Lista de verificación previa

1. **Coletar insumos de proveedores**
   - Últimos anuncios de proveedores (`ProviderAdvertV1`) e o snapshot de telemetria da frota alvo.
   - Plano de carga útil (`plan.json`) derivado del manifiesto en prueba.
2. **Gerar um marcador determinístico**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- Valide se `artifacts/scoreboard.json` lista cada proveedor de producción como `eligible`.
   - Archivo JSON de resumen junto al marcador; Los auditores dependen de dos contadores de reintento de fragmentos para certificar la solicitud de cambio.
3. **Ejecución en seco con dispositivos** — Ejecute el mismo comando contra los dispositivos públicos en `docs/examples/sorafs_ci_sample/` para garantizar que el binario del orquestador corresponda a la versión esperada antes de tocar las cargas útiles de producción.

## 2. Procedimiento de implementación en fases1. **Fase canário (≤2 provedores)**
   - Grabe el marcador y ejecútelo con `--max-peers=2` para restringir el orquestador a un subconjunto pequeño.
   - Monitorear:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Prossiga quando as taxas de reintry permanecerem abaixo de 1% para um fetch completo do manifesto e nenhum provedor acumular falhas.
2. **Fase de rampa (50% dos proveedores)**
   - Aumente `--max-peers` y ejecute novamente con una instantánea de telemetría reciente.
   - Persista cada ejecución con `--provider-metrics-out` e `--chunk-receipts-out`. Retenha os artefatos por ≥7 días.
3. **Implementación completa**
   - Elimina `--max-peers` (o define un contagio total de elegíveis).
   - Activo o modo orquestador en implementaciones de clientes: distribuye el marcador persistente y el JSON de configuración a través de su sistema de gestión de configuración.
   - Actualizar los paneles de control para mostrar `sorafs_orchestrator_fetch_duration_ms` p95/p99 e histogramas de reintento por región.

## 3. Bloqueio y refuerzo de pares

Utilice las anulaciones de política de pontuação do CLI para clasificar a los proveedores sin esperar actualizaciones de gobierno.

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
```- `--deny-provider` elimine el alias indicado para la consideración de la sesión actual.
- `--boost-provider=<alias>=<weight>` aumenta el peso del proveedor sin agendador. Los valores son iguales al peso normalizado del marcador y se aplican apenas a la ejecución local.
- Registre os overrides no ticket de incidente e anexe as saídas JSON para que a equipe responsável possa reconciliar o estado quando o problema subyacente para resolver.

Para cambios permanentes, ajuste la telemetría de origen (marca o infrarrojo como penalizado) o actualice el anuncio con señales de flujo actualizadas antes de limpiar las anulaciones de CLI.

## 4. Triagema de falhas

Cuando um buscar falha:

1. Capture los siguientes artefactos antes de ejecutar novamente:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Inspeccione `session.summary.json` para una cadena de error legal:
   - `no providers were supplied` → verifique os caminhos dos provedores e os anúncios.
   - `retry budget exhausted ...` → aumenta `--retry-budget` o elimina pares instalados.
   - `no compatible providers available ...` → audite os metadados de capacidade de faixa do provedor infrator.
3. Correlacione o nome do provedor com `sorafs_orchestrator_provider_failures_total` e abra um ticket de acompanhamento se a métrica disparar.
4. Reproduza o fetch offline con `--scoreboard-json` y una telemetría capturada para reproducir en forma determinística.

## 5. Revertir

Para revertir el despliegue del orquestador:1. Distribua una configuración que define `--max-peers=1` (desactiva eficazmente la agenda de múltiples orígenes) o regresa a los clientes al camino de búsqueda de fuente única.
2. Remove quaisquer anula `--boost-provider` para que el marcador vuelva a una ponderación neutra.
3. Continúe coletando as métricas do orquestador por pelo menos un día para confirmar que não há fetches residuais em andamento.

Manter a captura disciplinada de artefatos e rollouts en fases garantizando que el orquestador multi-origen possa ser operado com segurança em frotas heterogêneas de provedores, manteniendo los requisitos de observabilidade e auditoria intactos.