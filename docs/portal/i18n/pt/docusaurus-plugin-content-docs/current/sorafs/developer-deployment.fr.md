---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implantação do desenvolvedor
título: Notas de implantação SoraFS
sidebar_label: Notas de implementação
descrição: Lista de verificação para promover o pipeline SoraFS do CI em relação à produção.
---

:::nota Fonte canônica
:::

# Notas de implantação

O fluxo de trabalho de embalagem SoraFS reforça o determinismo, pois passa do CI para a produção necessária principalmente para operações de alto nível. Utilize esta lista de verificação ao implantar a saída nos gateways e fornecedores de rolos de armazenamento.

## Pré-vol

- **Alinhamento de registro** — confirme se os perfis do chunker e os manifestos referem-se à mesma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admissão** — envie os anúncios de fornecedores assinados e as provas de alias necessárias para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook du pin Registry** — guarda `docs/source/sorafs/runbooks/pin_registry_ops.md` à porta para cenários de repetição (rotação de alias, verificações de replicação).

## Configuração do ambiente

- Os gateways devem ativar o ponto final de streaming de prova (`POST /v1/sorafs/proof/stream`) para que a CLI possa gerar currículos de telefonia.
- Configure a política `sorafs_alias_cache` usando os valores padrão de `iroha_config` ou a CLI auxiliar (`sorafs_cli manifest submit --alias-*`).
- Forneça tokens de fluxo (ou identificadores Torii) por meio de um gerenciador de segredos protegidos.
- Ative os exportadores de telefonia (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e envie-os para sua pilha Prometheus/OTel.

## Estratégia de implementação

1. **Manifesta azul/verde**
   - Use `manifest submit --summary-out` para arquivar as respostas de cada implementação.
   - Vigilância `torii_sorafs_gateway_refusals_total` para detectar incompatibilidades de capacidade.
2. **Validação das provas**
   - Traitez les échecs de `sorafs_cli proof stream` como bloqueios de implantação; As fotos de latência indiquentes apresentam um recurso de estrangulamento ou níveis mal configurados.
   - `proof verify` faça parte do teste de fumaça pós-pin para garantir que o CAR seja aberto pelos fornecedores corresponda todos os dias ao resumo do manifesto.
3. **Painéis de Telemetria**
   - Importe `docs/examples/sorafs_proof_streaming_dashboard.json` para Grafana.
   - Adicione painéis para a segurança do registro do pino (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e as estatísticas do intervalo de blocos.
4. **Ativação multifonte**
   - Siga as etapas de implementação progressiva em `docs/source/sorafs/runbooks/multi_source_rollout.md` antes da ativação do orquestrador e arquive os artefatos do placar/telemetria para as auditorias.

## Gerenciamento de incidentes- Siga os caminhos da escalada em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para painéis de gateway e execução de tokens de fluxo.
  - `dispute_revocation_runbook.md` para litígios de replicação.
  - `sorafs_node_ops.md` para manutenção do nível dos nódulos.
  - `multi_source_rollout.md` para substituições do orquestrador, lista negra de pares e implementações em etapas.
- Registre as verificações de provas e as anomalias de latência no GovernanceLog por meio da API do rastreador PoR existente para que o governo possa avaliar o desempenho dos fornecedores.

## Prochaines étapes

- Integre a automação do orquestrador (`sorafs_car::multi_fetch`) quando a busca de múltiplas fontes do orquestrador (SF-6b) estiver disponível.
- Siga as mises do dia PDP/PoTR sob SF-13/SF-14 ; o CLI e os documentos foram desenvolvidos para expor os prazos e a seleção de níveis, uma vez que essas provas foram estabilizadas.

Combinando essas notas de implantação com o início rápido e as receitas CI, as equipes podem passar por experimentos locais nos pipelines SoraFS em produção com um processo repetível e observável.