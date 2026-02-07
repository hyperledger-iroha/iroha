---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implantação do desenvolvedor
título: Заметки по развертыванию SoraFS
sidebar_label: Nome da configuração
descrição: Verifique a solução de produção SoraFS do CI no produto.
---

:::nota História Canônica
:::

# Заметки по развертыванию

A configuração SoraFS usa a determinação, para que o CI seja processado em um processo contínuo guarda-corpos operacionais. Use esta verificação através de ferramentas de implementação em gateways reais e provedores de armazenamento.

## Prova de prova

- **Выравнивание реестра** — убедитесь, что профили chunker e manifestos ссылаются на одинаковый кортеж `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Admissão de política** — forneça anúncios de fornecedores e provas de alias, necessários para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Registro de pinos do runbook** — держите `docs/source/sorafs/runbooks/pin_registry_ops.md` pode ser usado para o cenário de inicialização (alias de ротация, сбои репликации).

## Configuração de configuração

- Os gateways oferecem streaming à prova de endpoint (`POST /v1/sorafs/proof/stream`), a CLI pode usar a rede telefônica.
- Altere a política `sorafs_alias_cache` com a ajuda de um auxiliar de `iroha_config` ou CLI (`sorafs_cli manifest submit --alias-*`).
- Transfira tokens de fluxo (ou учетные данные Torii) para um gerenciador de segredos exclusivo.
- Включите телеметрические exportadores (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e отправляйте их в ваш стек Prometheus/OTel.

## Lançamento da estratégia

1. **Manifestos azuis/verdes**
   - Use `manifest submit --summary-out` para implementação de software de distribuição.
   - Confie em `torii_sorafs_gateway_refusals_total`, isso significa que você não precisa dele.
2. **Provas de prova**
   - Считайте сбои в `sorafs_cli proof stream` блокерами развертывания; всплески латентности часто означают throttling провайдера или неверно настроенные níveis.
   - `proof verify` должен входить в smoke test после pin, чтобы удостовериться, что CAR у провайдеров все еще совпадает с digest manifestação.
3. **Painéis de controle**
   - Importe `docs/examples/sorafs_proof_streaming_dashboard.json` para Grafana.
   - Adicione painéis para registrar o pino de registro (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e intervalo de blocos de estatísticas.
4. **Recurso multifonte**
   - Следуйте этапным шагам rollout из `docs/source/sorafs/runbooks/multi_source_rollout.md` при включении orquestrador e архивируйте артефакты scoreboard/telеметрии для аудитов.

## Обработка инцидентов

- Coloque a escala em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para gateway de interrupção e token de fluxo de execução.
  - `dispute_revocation_runbook.md`, que contém replicações.
  - `sorafs_node_ops.md` para instalação em seu local de trabalho.
  - `multi_source_rollout.md` para substituir a organização, colocar pares na lista negra e implementar implementações adicionais.
- Registre provas e anomalias latentes no GovernanceLog para obter suporte à API do rastreador de PoR, obtenha recursos de governança производительность провайдеров.

## Следующие шаги- Integre o orquestrador automático (`sorafs_car::multi_fetch`), orquestrador de busca multi-fonte compatível (SF-6b).
- Отслеживайте обновления PDP/PoTR em рамках SF-13/SF-14; CLI e docs podem ser implementados, você pode definir os níveis e suas camadas, para que as provas sejam estabilizadas.

Verifique se você está usando o início rápido e os repositórios CI, os comandos serão ativados no local экспериментов к oleodutos SoraFS de nível de produção com sucesso e processo de montagem.