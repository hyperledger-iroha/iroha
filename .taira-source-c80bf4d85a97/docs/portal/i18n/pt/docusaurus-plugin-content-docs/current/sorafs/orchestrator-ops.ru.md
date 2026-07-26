---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-ops
título: Runbook по эксплуатации оркестратора SoraFS
sidebar_label: Runbook оркестратора
description: Use a operação correta para monitorar, monitorar e monitorar vários operadores.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Se você tiver uma cópia sincronizada, a documentação do Sphinx não será migrada.
:::

Este runbook fornece SRE через подготовку, развёртывание и эксплуатацию мульти-источникового fetch-оркестратора. Ao iniciar o processo de distribuição, instale o produto, instale o dispositivo e Verifique os dados na página principal.

> **См. Veja também:** [Runbook para lançamento de vários aplicativos](./multi-source-rollout.md) экстренному отклонению провайдеров. Use-o para coordenação de governança/preparação, e este documento — para o оркестратора эксплуатации.

## 1. Предстартовый чек-list

1. **Confira seus dados de prova**
   - Последние анонсы провайдеров (`ProviderAdvertV1`) e снимок телеметрии para целевого флота.
   - Carga útil planejada (`plan.json`), usada no manual de teste.
2. **Painel de avaliação de determinação**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Verifique se o `artifacts/scoreboard.json` é compatível com o produto de produção como `eligible`.
   - Архивируйте JSON-сводку вместе со scoreboard; аудиторы опираются на счётчики ретраев чанков при сертификации запроса на изменение.
3. **Dry-run с fixtures** — Verifique seu comando em luminárias públicas de `docs/examples/sorafs_ci_sample/`, чтобы убедиться, что бинарник оркестратора соответствует ожидаемой версии, прежде чем трогать прод-payloads.

## 2. Implementação do processo de implementação

1. **Canal (≤2 testes)**
   - Coloque o placar e instale-o no `--max-peers=2`, para que o orquestrador não possa ser instalado.
   - Monitorar:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Продолжайте, когда доля ретраев держится ниже 1% para полного fetch манифеста и ни один провайдер не накапливает ошибок.
2. **Etap разгона (50% de aprovação)**
   - Use `--max-peers` e instale-o com um telefone inteligente.
   - Verifique o cabo com `--provider-metrics-out` e `--chunk-receipts-out`. Храните артефакты ≥7 dias.
3. **Lançamento do programa**
   - Use `--max-peers` (ou use-o como elegível).
   - Включите режим оркестратора в клиентских деплоях: распространяйте сохранённый scoreboard e JSON-конфиг через систему configuração de atualização.
   - Abra os dados, selecione `sorafs_orchestrator_fetch_duration_ms` p95/p99 e registre os registros na região.

## 3. Bloqueio e uso de pirovas

A implementação substitui a pontuação política na CLI, o que torna o problema de governança sem a governança.

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
```- `--deny-provider` исключает указанный alias из рассмотрения в текущей сессии.
- `--boost-provider=<alias>=<weight>` permite que você teste o plano. Значения добавляются к нормализованному seu placar e применяются только к локальному запуску.
- Зафиксируйте substitui no инцидент-тикете e приложите JSON-выходы, чтобы ответственная команда могла согласовать состояние после устранения первопричины.

Para postar um anúncio, você pode usar um telefone celular (por exemplo, enviar um anúncio como penalizado) ou alterar um anúncio Você pode usar a substituição CLI.

## 4. Corte de cabelo

Para buscar o padrão:

1. Antes de começar, verifique os artefatos:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Verifique `session.summary.json` em uma chave de fenda:
   - `no providers were supplied` → verifique se há verificação e verificação.
   - `retry budget exhausted ...` → увеличьте `--retry-budget` ou исключите нестабильных пиров.
   - `no compatible providers available ...` → proverьте метаданные диапазонных возможностей провайдера-нарушителя.
3. Solicite um teste com `sorafs_orchestrator_provider_failures_total` e verifique o valor do bilhete, exceto o valor métrico.
4. Selecione fetch офлайн com `--scoreboard-json` e instale o telefone, чтобы детерминированно воспроизвести sim.

## 5. Reversão

O que fazer com o organizador de lançamento:

1. Configure a configuração em `--max-peers=1` (faturamento de planos múltiplos) ou верните клиентов на устаревший одноисточниковый fetch-путь.
2. Уберите любые substitui `--boost-provider`, чтобы scoreboard вернулся к нейтральному весу.
3. Produza a métrica do orquestrador com o mínimo de espaço possível, o que pode ser feito buscar-операций.

Дисциплинированный сбор артефактов e поэтапные rollouts garantem безопасную эксплуатацию vários orquestradores de testes de fluxo de trabalho para garantir o trabalho e аудиту.