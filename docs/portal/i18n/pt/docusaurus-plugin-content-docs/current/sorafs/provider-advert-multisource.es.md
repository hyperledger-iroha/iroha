---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Anúncios de provedores de múltiplas origens e planejamento

Esta página destila a especificação canônica em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Use este documento para os esquemas Norito literalmente e os changelogs; a cópia do portal
mantenha o guia de operadoras, as notas do SDK e as referências de telemetria em torno do resto
dos runbooks de SoraFS.

## Adicionados ao esquema Norito

### Capacidade de alcance (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – maior linha contínua (bytes) por solicitação, `>= 1`.
- `min_granularity` – resolução de busca, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` – permite compensações não contíguas em uma solicitação.
- `requires_alignment` – quando é verdadeiro, os deslocamentos devem ser alinhados com `min_granularity`.
- `supports_merkle_proof` – indica suporte de testículos PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplica codificação canônica
para que as cargas de fofoca sigam sendo deterministas.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Regras de validação (`StreamBudgetV1::validate`):
  -`max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, quando está presente, deve ser `> 0` e `<= max_bytes_per_sec`.

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8` (ventana 0-15 aplicada por
  `TransportHintV1::validate`).
- Protocolos conhecidos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Se rechazan entradas duplicadas de protocolo pelo fornecedor.

### Adicionado a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Ambos os campos agora fluem por `ProviderAdmissionProposalV1`, os envelopes de governo,
  os fixtures de CLI e o JSON telemétrico.

## Validação e vinculação com governo

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
rechazan metadados malformados:

- As capacidades de rango devem ser decodificadas e cumpridas os limites de amplitude/granularidade.
- Os orçamentos de fluxo / dicas de transporte requerem um TLV `CapabilityType::ChunkRangeFetch`
  coincidente e uma lista de dicas não está vazia.
- Protocolos de transporte duplicados e prioridades inválidas geram erros de validação
  antes de os anúncios serem fofocados.
- Os envelopes de admissão comparam propostas/anúncios para metadados de rango via
  `compare_core_fields` para que as cargas úteis de fofoca dessalineadas sejam recuperadas temporariamente.

A cobertura de regressão vive em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Ferramentas e acessórios- As cargas úteis de anúncios de provedor devem incluir metadados `range_capability`,
  `stream_budget` e `transport_hints`. Validado por meio de respostas de `/v2/sorafs/providers` e
  luminárias de admissão; os currículos JSON devem incluir a capacidade analisada,
  o orçamento de fluxo e as matrizes de dicas para ingestão de telemetria.
- `cargo xtask sorafs-admission-fixtures` expõe orçamentos de fluxo e dicas de transporte dentro de
  seus artefatos JSON para que os painéis sigam a adoção do recurso.
- Os equipamentos abaixo `fixtures/sorafs_manifest/provider_admission/` agora incluem:
  - adverts multi-origen canónicos,
  - uma variante sem classificação para testes de downgrade, e
  - `multi_fetch_plan.json` para que as suítes do SDK reproduzam um plano de busca
    determinista multiponto.

## Integração com orquestrador e Torii

- Torii `/v2/sorafs/providers` fornece metadados de capacidade de rango analisados junto com
  `stream_budget` e `transport_hints`. Se houver avisos de downgrade quando eles
  os provedores omitem os novos metadados e os endpoints de rango do gateway aplicam-se
  múltiplas restrições para clientes diretos.
- O orquestrador multi-origem (`sorafs_car::multi_fetch`) agora cumpriu os limites de
  rango, alinhamento de capacidades e fluxo de orçamentos para atribuição de trabalho. As tentativas unitárias
  criar cenários de pedaços muito grandes, busca de dispersão e estrangulamento.
- `sorafs_car::multi_fetch` emite sinais de downgrade (falhas de alinhamento,
  solicitudes estranguladas) para que os operadores rastreem por que se omitem
  fornecedores específicos durante o planejamento.

## Referência de telemetria

A instrumentação de busca de rango de Torii alimenta o painel de Grafana
**SoraFS Observabilidade de busca** (`dashboards/grafana/sorafs_fetch_observability.json`) y
as regras de alerta associadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Tipo | Etiquetas | Descrição |
|--------|------|-----------|------------|
| `torii_sorafs_provider_range_capability_total` | Medidor | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Proveedores que anunciam características de capacidade de rango. |
| `torii_sorafs_range_fetch_throttle_events_total` | Contador | `reason` (`quota`, `concurrency`, `byte_rate`) | Intenções de busca de rango com estrangulamento agrupados por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Medidor | — | Transmite ativos protegidos que consomem o pressuposto de participação de concorrência. |

Exemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Use o contador de aceleração para confirmar a aplicação de cotas antes de ativar
os valores por defeito do orquestrador multi-origem e alerta quando a concorrência se
acerque os máximos do pressuposto de streams de sua flota.