---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implantação do desenvolvedor
título: Notas de aplicação de SoraFS
sidebar_label: Notas de saída
description: Lista de verificação para promover o pipeline de SoraFS de CI para produção.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/deployment.md`. Mantenha ambas as versões sincronizadas até que os documentos herdados sejam retirados.
:::

# Notas de despliegue

O fluxo de empaquetado de SoraFS rejeita a determinação, porque o que passa de CI a produção requer principalmente salvamentos operacionais. Use esta lista quando despliegues la herramienta en gateways e fornecedores de armazenamento real.

## Preparação prévia

- **Alineação do registro** — confirma que os perfis de chunker e os manifestos se referem à mesma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admissão** — revise os anúncios do provedor firmados e as provas de alias necessárias para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de registro de pinos** — mantém `docs/source/sorafs/runbooks/pin_registry_ops.md` à mão para cenários de recuperação (rotação de alias, falhas de replicação).

## Configuração do ambiente

- Os gateways devem habilitar o endpoint de streaming de provas (`POST /v2/sorafs/proof/stream`) para que a CLI emita currículos de telemetria.
- Configure a política `sorafs_alias_cache` usando os valores predeterminados de `iroha_config` ou o auxiliar da CLI (`sorafs_cli manifest submit --alias-*`).
- Proporciona stream tokens (ou credenciais de Torii) por meio de um gerenciador de segredos seguros.
- Habilite os exportadores de telemetria (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e envie-os para sua pilha Prometheus/OTel.

## Estratégia de despliegue

1. **Manifesta azul/verde**
   - Use `manifest submit --summary-out` para arquivar as respostas de cada resposta.
   - Vigila `torii_sorafs_gateway_refusals_total` para detectar desajustes de capacidades temprano.
2. **Validação de provas**
   - Tratar as falhas em `sorafs_cli proof stream` como bloqueios do despliegue; Os picos de latência geralmente indicam estrangulamento do provedor ou níveis mal configurados.
   - `proof verify` deve formar parte do teste de fumaça posterior ao pino para garantir que o CAR alojado pelos fornecedores coincida com o resumo do manifesto.
3. **Painéis de telemetria**
   - Importa `docs/examples/sorafs_proof_streaming_dashboard.json` e Grafana.
   - Adicionados painéis adicionais para a saúde do registro de pinos (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e estatísticas de faixa de pedaços.
4. **Habilitação multifonte**
   - Siga os passos de despliegue por etapas em `docs/source/sorafs/runbooks/multi_source_rollout.md` para ativar o orquestrador e arquivar os artefatos de placar/telemetria para auditórios.

## Gestão de incidentes- Siga as rotas escaladas em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para queda de gateway e ativação de stream-token.
  - `dispute_revocation_runbook.md` quando ocorreram disputas de replicação.
  - `sorafs_node_ops.md` para manutenção no nível do nó.
  - `multi_source_rollout.md` para substituições do orquestrador, listas negras de peers e despliegues por etapas.
- Registrar falhas de provas e anomalias de latência no GovernanceLog por meio das APIs do rastreador PoR existentes para que o governo possa avaliar o rendimento do provedor.

## Próximos passos

- Integra a automatização do orquestrador (`sorafs_car::multi_fetch`) ao ligar o orquestrador de busca de múltiplas fontes (SF-6b).
- Siga as atualizações do PDP/PoTR abaixo do SF-13/SF-14; a CLI e os documentos evoluíram para expor áreas e selecionar níveis quando essas provas são estabelecidas.

Ao combinar essas notas de aplicação com o início rápido e as receitas de CI, as equipes podem passar por experimentos locais em pipelines SoraFS em produção com um processo repetitivo e observável.