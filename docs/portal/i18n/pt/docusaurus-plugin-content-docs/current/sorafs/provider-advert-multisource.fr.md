---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Anúncios de fornecedores multi-fonte e planejamento

Esta página condensa a especificação canônica em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilize este documento para os esquemas Norito literalmente e os changelogs; a cópia do portal
conserve os dados operacionais, as notas SDK e as referências de telemetria nas proximidades do resto
dos runbooks SoraFS.

## Adicionado ao esquema Norito

### Capacidade de praia (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – plus grande plage contigue (octetos) par requete, `>= 1`.
- `min_granularity` – resolução de busca, `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets` – permite compensações não contíguas em uma única solicitação.
- `requires_alignment` – se for verdade, os deslocamentos devem ser alinhados em `min_granularity`.
- `supports_merkle_proof` – indica o prêmio en charge des temoins PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplica uma codificação canônica
para que as cargas de fofoca sejam determinísticas.

### `StreamBudgetV1`
- Campeões: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Regras de validação (`StreamBudgetV1::validate`):
  -`max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, quando estiver presente, doit etre `> 0` e `<= max_bytes_per_sec`.

### `TransportHintV1`
- Campeões: `protocol: TransportProtocol`, `priority: u8` (fenetre 0-15 aplicado par
  `TransportHintV1::validate`).
- Conexão de protocolos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Les entrees de protocole dupliquees par fournisseur sont rejetees.

### Adiciona um `ProviderAdvertBodyV1`
- Opcional `stream_budget`: `Option<StreamBudgetV1>`.
- Opcional `transport_hints`: `Option<Vec<TransportHintV1>>`.
- Les deux champs transitent desormais via `ProviderAdmissionProposalV1`, les envelopes
  de governança, os equipamentos CLI e a telemetria JSON.

## Validação e ligação com a governança

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
rejeitando as metadonas mal formadas:

- Les capacites de plage devem ser decodificadas e respeitam os limites de plage/granularite.
- Os orçamentos de fluxo / dicas de transporte exigem um TLV `CapabilityType::ChunkRangeFetch`
  correspondente e uma lista de dicas não vide.
- Os protocolos de transporte duplicados e as prioridades inválidas geram erros
  de validação antes da difusão de anúncios.
- Os envelopes de admissão comparam propostas/anúncios para as metadonnées de plage via
  `compare_core_fields` para que as cargas úteis de fofoca não concordantes sejam rejeitadas totalmente.

A cobertura de regressão se encontrou em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Utilitários e luminárias- As cargas úteis de anúncios de fornecedores devem incluir os metadonnes `range_capability`,
  `stream_budget` e `transport_hints`. Validade por meio das respostas `/v2/sorafs/providers` e das
  luminárias de admissão; os currículos JSON devem incluir a capacidade de análise, o orçamento de fluxo
  e tabelas de dicas para ingestão telemétrica.
- `cargo xtask sorafs-admission-fixtures` expõe os orçamentos de fluxo e dicas de transporte em
  Ses artefatos JSON para que os painéis sigam a adoção da funcionalidade.
- Les fixtures sous `fixtures/sorafs_manifest/provider_admission/` incluem desormais:
  - des adverts canônicos de múltiplas fontes,
  - `multi_fetch_plan.json` para que as suítes SDK possam realizar um plano de busca
    determinismo multiponto.

## Integração com o orquestrador e Torii

- Torii `/v2/sorafs/providers` envia os metadonnes de capacidade de praia analisados ​​com
  `stream_budget` e `transport_hints`. As advertências de downgrade diminuem quando
  os fornecedores oferecem a nova metadona e os pontos finais da praia do gateway
  aplique memes contraintes para clientes diretos.
- L'orchestrateur multi-source (`sorafs_car::multi_fetch`) aplica desormais les limites de
  plage, l'alignement des capacites et les stream budgets durante a afetação do trabalho.
  Os testes unitários cobrem os cenários de pedaços grandes, de busca dispersa e de
  estrangulamento.
- `sorafs_car::multi_fetch` difunde sinais de downgrade (etecs d'alignement,
  requetes estrangulados) para que os operadores possam traçar para certos fornecedores
  ont ete ignora pendente la planification.

## Referência de telemetria

A ferramenta de busca de plage de Torii alimente o painel Grafana
**SoraFS Observabilidade de busca** (`dashboards/grafana/sorafs_fetch_observability.json`) e
as regras de alerta aos associados (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Tipo | Etiquetas | Descrição |
|----------|------|-----------|------------|
| `torii_sorafs_provider_range_capability_total` | Medidor | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Os fornecedores anunciam as funcionalidades de capacidade de praia. |
| `torii_sorafs_range_fetch_throttle_events_total` | Contador | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentativas de buscar noivas na praia por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Medidor | — | Streams ativos correspondem ao orçamento de participação simultânea. |

Exemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilize o controle de aceleração para confirmar o aplicativo de cotas antes de ativá-lo
os valores padrão do orquestrador multi-fonte, e alerta quando a concorrência se
aproximar-se do orçamento máximo de fluxos para sua frota.