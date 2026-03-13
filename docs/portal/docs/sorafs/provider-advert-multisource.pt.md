---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e9f2cc35c57ca6e054276972f341d4fa44ff4b164a5c0bb707025b80c4e7bf25
source_last_modified: "2025-11-08T17:35:21.580244+00:00"
translation_last_reviewed: 2026-01-30
---

# Adverts de provedores multi-origem e agendamento

Esta pagina resume a especificacao canonica em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Use esse documento para schemas Norito verbatim e changelogs; a copia do portal
mantem a orientacao para operadores, notas de SDK e referencias de telemetria perto do restante
dos runbooks SoraFS.

## Adicoes ao esquema Norito

### Range capability (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - maior span continuo (bytes) por requisicao, `>= 1`.
- `min_granularity` - resolucao de seek, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - permite offsets nao contiguos em uma requisicao.
- `requires_alignment` - quando true, offsets devem alinhar com `min_granularity`.
- `supports_merkle_proof` - indica suporte a testemunhas PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplicam encoding canonico
para que payloads de gossip permanecam deterministas.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Regras de validacao (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, quando presente, deve ser `> 0` e `<= max_bytes_per_sec`.

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8` (janela 0-15 aplicada por
  `TransportHintV1::validate`).
- Protocolos conhecidos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Entradas duplicadas de protocolo por provedor sao rejeitadas.

### Adicoes a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Ambos os campos agora passam por `ProviderAdmissionProposalV1`, envelopes de governanca,
  fixtures de CLI e JSON de telemetria.

## Validacao e vinculacao com governanca

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
rejeitam metadata malformada:

- Range capabilities devem decodificar e cumprir limites de span/granularidade.
- Stream budgets / transport hints exigem um TLV `CapabilityType::ChunkRangeFetch`
  correspondente e lista de hints nao vazia.
- Protocolos de transporte duplicados e prioridades invalidas geram erros de validacao
  antes de adverts serem gossiped.
- Admission envelopes comparam proposal/adverts para metadata de range via
  `compare_core_fields` para que payloads de gossip divergentes sejam rejeitados cedo.

A cobertura de regressao vive em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Tooling e fixtures

- Payloads de adverts de provedor devem incluir metadata `range_capability`,
  `stream_budget` e `transport_hints`. Valide via respostas de `/v2/sorafs/providers`
  e admission fixtures; resumos JSON devem incluir a capability parseada, o stream
  budget e arrays de hints para ingestao de telemetria.
- `cargo xtask sorafs-admission-fixtures` mostra stream budgets e transport hints dentro
  de seus artefatos JSON para que dashboards acompanhem a adocao da feature.
- Fixtures sob `fixtures/sorafs_manifest/provider_admission/` agora incluem:
  - adverts multi-origem canonicos,
  - `multi_fetch_plan.json` para que suites de SDK reproduzam um plano de fetch
    multi-peer deterministico.

## Integracao com orchestrator e Torii

- Torii `/v2/sorafs/providers` retorna metadata de range parseada junto com
  `stream_budget` e `transport_hints`. Avisos de downgrade disparam quando
  provedores omitem a nova metadata, e endpoints de range do gateway aplicam as
  mesmas restricoes para clientes diretos.
- O orchestrator multi-origem (`sorafs_car::multi_fetch`) agora aplica limites de
  range, alinhamento de capabilities e stream budgets ao atribuir trabalho. Unit
  tests cobrem cenarios de chunk muito grande, sparse-seek e throttling.
- `sorafs_car::multi_fetch` transmite sinais de downgrade (falhas de alinhamento,
  requisicoes throttled) para que operadores rastreiem por que provedores especificos
  foram ignorados durante o planejamento.

## Referencia de telemetria

A instrumentacao de range fetch de Torii alimenta o dashboard Grafana
**SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) e
as regras de alerta associadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Metrica | Tipo | Labels | Descricao |
|---------|------|--------|-----------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Provedores que anunciam features de range capability. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentativas de range fetch throttled agrupadas por politica. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | - | Streams ativos protegidos consumindo o budget compartilhado de concorrencia. |

Exemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Use o contador de throttling para confirmar a aplicacao de quotas antes de ativar
aos defaults do orchestrator multi-origem e alerte quando a concorrencia se aproximar
dos maximos do stream budget da sua frota.
