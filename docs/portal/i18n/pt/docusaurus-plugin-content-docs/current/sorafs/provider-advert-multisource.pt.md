---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Anúncios de provedores multi-origem e agendamento

Esta página resume a especificação canônica em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Use esse documento para esquemas Norito literalmente e changelogs; uma cópia do portal
manter orientação para operadores, notas de SDK e referências de telemetria perto do restante
dos runbooks SoraFS.

## Adicoes ao esquema Norito

### Capacidade de alcance (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - maior span contínuo (bytes) por requisição, `>= 1`.
- `min_granularity` - resolução de busca, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - permite compensações não contíguas em uma requisição.
- `requires_alignment` - quando verdadeiro, os offsets devem se alinhar com `min_granularity`.
- `supports_merkle_proof` - indica suporte a testemunhas PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplicação codificação canônica
para que cargas de fofoca permaneçam deterministas.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Regras de validação (`StreamBudgetV1::validate`):
  -`max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, quando presente, deve ser `> 0` e `<= max_bytes_per_sec`.

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8` (janela 0-15 aplicada por
  `TransportHintV1::validate`).
- Protocolos conhecidos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Entradas duplicadas de protocolo por provedor são rejeitadas.

### Adicoes a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Ambos os campos agora passam por `ProviderAdmissionProposalV1`, envelopes de governança,
  fixtures de CLI e JSON de telemetria.

## Validação e vinculação com governança

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
rejeitaram metadados malformados:

- As capacidades de alcance devem ser decodificadas e cumprir limites de span/granularidade.
- Transmitir orçamentos/dicas de transporte desativar um TLV `CapabilityType::ChunkRangeFetch`
  correspondente e lista de dicas não vazia.
- Protocolos de transporte duplicados e prioridades invalidas geram erros de validação
  antes de os anúncios serem fofocados.
- Envelopes de admissão comparam propostas/anúncios para metadados de gama via
  `compare_core_fields` para que cargas de fofocas divergentes sejam rejeitadas cedo.

A cobertura de regressão vive em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Ferramentas e acessórios

- Payloads de anúncios de provedor devem incluir metadados `range_capability`,
  `stream_budget` e `transport_hints`. Valide via respostas de `/v1/sorafs/providers`
  e luminárias de admissão; resumos JSON devem incluir uma capacidade analisada, o stream
  orçamento e matrizes de dicas para ingestão de telemetria.
- `cargo xtask sorafs-admission-fixtures` mostra orçamentos de fluxo e dicas de transporte dentro
  de seus artistas JSON para que os dashboards acompanhem a adoção do feature.
- Luminárias sob `fixtures/sorafs_manifest/provider_admission/` agora incluem:
  - anúncios canônicos multi-origem,
  - `multi_fetch_plan.json` para que suítes de SDK reproduzam um plano de busca
    determinístico multi-ponto.

## Integração com orquestrador e Torii- Torii `/v1/sorafs/providers` retorna metadados do intervalo analisado junto com
  `stream_budget` e `transport_hints`. Avisos de downgrade disparam quando
  provedores omitem a novos metadados, e endpoints de range do gateway aplicam as
  mesmas restrições para clientes diretos.
- O orquestrador multi-origem (`sorafs_car::multi_fetch`) agora aplica limites de
  alcance, alinhamento de capacidades e fluxo de orçamentos ao considerar trabalho. Unidade
  testes cobrem cenários de chunk muito grande, sparse-seek e throttling.
- `sorafs_car::multi_fetch` transmite sinais de downgrade (falhas de alinhamento,
  requisições estranguladas) para que operadores rastreiem por provedores específicos
  foram ignorados durante o planejamento.

## Referência de telemetria

A instrumentação de range fetch de Torii alimenta o painel Grafana
**SoraFS Observabilidade de busca** (`dashboards/grafana/sorafs_fetch_observability.json`) e
as regras de alerta associadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Tipo | Etiquetas | Descrição |
|--------|------|--------|-----------|
| `torii_sorafs_provider_range_capability_total` | Medidor | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Fornecedores que anunciam recursos de capacidade de alcance. |
| `torii_sorafs_range_fetch_throttle_events_total` | Contador | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentativas de range fetch estranguladas agrupadas por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Medidor | - | Streams ativos protegidos consumindo o orçamento compartilhado de concorrência. |

Exemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilize o contador de throttling para confirmar a aplicação de cotas antes de ativar
aos defaults do orquestrador multi-origem e alertar quando a concorrência se aproximar
dos máximos do stream budget da sua frota.