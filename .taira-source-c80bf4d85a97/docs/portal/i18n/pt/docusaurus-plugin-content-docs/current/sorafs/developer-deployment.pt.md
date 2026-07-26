---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implantação do desenvolvedor
título: Notas de implantação de SoraFS
sidebar_label: Notas de implantação
descrição: Checklist para promover o pipeline da SoraFS de CI para produção.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/developer/deployment.md`. Mantenha ambas as cópias sincronizadas.
:::

# Notas de implantação

O fluxo de trabalho de empacotamento da SoraFS fortalece o determinismo, então a passagem de CI para produção requer principalmente guardrails operacionais. Use esta lista de verificação para levar as ferramentas para gateways e provedores de armazenamento real.

## Pré-voo

- **Alinhamento do registro** - confirma que os perfis de chunker e manifestos referenciam a mesma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admissão** - revisar os anúncios do fornecedor assinados e alias provas necessárias para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook do pin record** - mantenha `docs/source/sorafs/runbooks/pin_registry_ops.md` por perto para cenários de recuperação (rotação de alias, falhas de replicação).

## Configuração do ambiente

- Os gateways devem habilitar o endpoint de streaming de prova (`POST /v1/sorafs/proof/stream`) para que a CLI emita resumos de telemetria.
- Configure a política `sorafs_alias_cache` usando os padrões em `iroha_config` ou o helper do CLI (`sorafs_cli manifest submit --alias-*`).
- Forneça stream tokens (ou credenciais Torii) através de um gerenciador de segredos seguro.
- Habilite exportadores de telemetria (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e envie para sua pilha Prometheus/OTel.

## Estratégia de implementação

1. **Manifesta azul/verde**
   - Use `manifest submit --summary-out` para arquivar respostas de cada rollout.
   - Observe `torii_sorafs_gateway_refusals_total` para captar incompatibilidades de capacidade cedo.
2. **Validação de provas**
   - Trate falhas em `sorafs_cli proof stream` como bloqueios de implantação; Picos de latência costumam indicar estrangulamento do provedor ou níveis mal configurados.
   - `proof verify` deve fazer parte do teste de fumaça pos-pin para garantir que o CAR hospedado pelos provedores ainda corresponda ao resumo do manifesto.
3. **Painéis de telemetria**
   - Importe `docs/examples/sorafs_proof_streaming_dashboard.json` no Grafana.
   - Adicione painéis para saúde do registro de pinos (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e estatísticas de intervalo de blocos.
4. **Habilitação multifonte**
   - Siga os passos de rollout em etapas em `docs/source/sorafs/runbooks/multi_source_rollout.md` para ativar o orquestrador e arquivar artistas de scoreboard/telemetria para auditórios.

## Tratamento de incidentes

- Siga os caminhos de escalonamento em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para quedas de gateway e esgotamento de stream-token.
  - `dispute_revocation_runbook.md` quando ocorrerem disputas de replicação.
  - `sorafs_node_ops.md` para manutenção no nível de nó.
  - `multi_source_rollout.md` para overrides do orquestrador, blacklisting de peers e rollouts em etapas.
- Registrar falhas de provas e anomalias de latência no GovernanceLog através das APIs de PoR tracker existentes para que a governança avalie o desempenho dos provedores.

## Próximos passos- Integre a automação do orquestrador (`sorafs_car::multi_fetch`) quando o orquestrador de busca multi-fonte (SF-6b) chegar.
- Acompanhar atualizações de PDP/PoTR sob SF-13/SF-14; o CLI e a documentação vão evoluir para exportar prazos e selecionar níveis quando essas provas se estabilizarem.

Ao combinar essas notas de implantação com o início rápido e as receitas de CI, as equipes podem passar de experimentos locais para pipelines SoraFS em produção com um processo repetitivo e observavel.