<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: developer-deployment
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/deployment.md`. Mantenha ambas as copias sincronizadas.
:::

# Notas de deployment

O workflow de empacotamento da SoraFS fortalece o determinismo, entao a passagem de CI para producao requer principalmente guardrails operacionais. Use esta checklist ao levar as ferramentas para gateways e provedores de armazenamento reais.

## Pre-flight

- **Alinhamento do registro** - confirme que os perfis de chunker e manifests referenciam a mesma tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politica de admission** - revise os provider adverts assinados e alias proofs necessarios para `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook do pin registry** - mantenha `docs/source/sorafs/runbooks/pin_registry_ops.md` por perto para cenarios de recuperacao (rotacao de alias, falhas de replicacao).

## Configuracao do ambiente

- Gateways devem habilitar o endpoint de proof streaming (`POST /v1/sorafs/proof/stream`) para que o CLI emita resumos de telemetria.
- Configure a politica `sorafs_alias_cache` usando os padroes em `iroha_config` ou o helper do CLI (`sorafs_cli manifest submit --alias-*`).
- Forneca stream tokens (ou credenciais Torii) via um secret manager seguro.
- Habilite exporters de telemetria (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) e envie para seu stack Prometheus/OTel.

## Estrategia de rollout

1. **Manifests blue/green**
   - Use `manifest submit --summary-out` para arquivar respostas de cada rollout.
   - Observe `torii_sorafs_gateway_refusals_total` para captar mismatches de capacidade cedo.
2. **Validacao de proofs**
   - Trate falhas em `sorafs_cli proof stream` como bloqueadores de deployment; picos de latencia costumam indicar throttling do provedor ou tiers mal configurados.
   - `proof verify` deve fazer parte do smoke test pos-pin para garantir que o CAR hospedado pelos provedores ainda corresponde ao digest do manifest.
3. **Dashboards de telemetria**
   - Importe `docs/examples/sorafs_proof_streaming_dashboard.json` no Grafana.
   - Adicione paineis para saude do pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e estatisticas de chunk range.
4. **Habilitacao multi-source**
   - Siga os passos de rollout em etapas em `docs/source/sorafs/runbooks/multi_source_rollout.md` ao ativar o orquestrador e arquive artefatos de scoreboard/telemetria para auditorias.

## Tratamento de incidentes

- Siga os caminhos de escalonamento em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` para quedas de gateway e esgotamento de stream-token.
  - `dispute_revocation_runbook.md` quando ocorrerem disputas de replicacao.
  - `sorafs_node_ops.md` para manutencao no nivel de nodo.
  - `multi_source_rollout.md` para overrides do orquestrador, blacklisting de peers e rollouts em etapas.
- Registre falhas de proofs e anomalias de latencia no GovernanceLog via as APIs de PoR tracker existentes para que a governanca avalie o desempenho dos provedores.

## Proximos passos

- Integre a automacao do orquestrador (`sorafs_car::multi_fetch`) quando o orquestrador de multi-source fetch (SF-6b) chegar.
- Acompanhe upgrades de PDP/PoTR sob SF-13/SF-14; o CLI e a documentacao vao evoluir para expor prazos e selecao de tiers quando essas proofs estabilizarem.

Ao combinar estas notas de deployment com o quickstart e as receitas de CI, as equipes podem passar de experimentos locais para pipelines SoraFS em producao com um processo repetivel e observavel.
