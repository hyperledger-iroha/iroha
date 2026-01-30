---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Protocolo de no <-> cliente da SoraFS

Este guia resume a definicao canonica do protocolo em
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use a especificacao upstream para layouts Norito em nivel de byte e changelogs;
a copia do portal mantem os destaques operacionais perto do restante dos runbooks
SoraFS.

## Adverts de provedor e validacao

Provedores SoraFS disseminam payloads `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) assinados pelo operador governado. Os
adverts fixam os metadados de descoberta e os guardrails que o orquestrador
multi-source aplica em runtime.

- **Vigencia** - `issued_at < expires_at <= issued_at + 86,400 s`. Provedores
  devem renovar a cada 12 horas.
- **TLVs de capacidade** - a lista TLV anuncia recursos de transporte (Torii,
  QUIC+Noise, relays SoraNet, extensoes de fornecedor). Codigos desconhecidos
  podem ser ignorados quando `allow_unknown_capabilities = true`, seguindo a
  orientacao GREASE.
- **Hints de QoS** - tier de `availability` (Hot/Warm/Cold), latencia maxima de
  recuperacao, limite de concorrencia e budget de stream opcional. QoS deve
  alinhar com a telemetria observada e e auditada na admissao.
- **Endpoints e rendezvous topics** - URLs de servico concretas com metadados
  TLS/ALPN mais os topics de descoberta aos quais os clientes devem se inscrever
  ao construir guard sets.
- **Politica de diversidade de caminho** - `min_guard_weight`, caps de fan-out de
  AS/pool e `provider_failure_threshold` tornam possiveis fetches deterministas
  multi-peer.
- **Identificadores de perfil** - provedores devem expor o handle canonico (ex.
  `sorafs.sf1@1.0.0`); `profile_aliases` opcionais ajudam clientes antigos a migrar.

Regras de validacao rejeitam stake zero, listas vazias de capabilities/endpoints/topics,
vigencias fora de ordem ou targets de QoS ausentes. Admission envelopes comparam
os corpos do advert e da proposta (`compare_core_fields`) antes de disseminar
atualizacoes.

### Extensoes de range fetch

Provedores com range incluem os seguintes metadados:

| Campo | Proposito |
|-------|-----------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` e flags de alinhamento/prova. |
| `StreamBudgetV1` | Envelope opcional de concorrencia/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Requer capacidade de range. |
| `TransportHintV1` | Preferencias de transporte ordenadas (ex. `torii_http_range`, `quic_stream`, `soranet_relay`). Prioridades sao `0-15` e duplicados sao rejeitados. |

Suporte de tooling:

- Pipelines de provider advert devem validar capacidade de range, stream budget e
  transport hints antes de emitir payloads deterministas para auditorias.
- `cargo xtask sorafs-admission-fixtures` agrupa adverts multi-source canonicos
  junto com downgrade fixtures em `fixtures/sorafs_manifest/provider_admission/`.
- Adverts com range que omitem `stream_budget` ou `transport_hints` sao rejeitados
  pelos loaders CLI/SDK antes do agendamento, mantendo o harness multi-source
  alinhado com as expectativas de admissao do Torii.

## Endpoints de range do gateway

Gateways aceitam requisicoes HTTP deterministas que espelham os metadados do
advert.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Requisito | Detalhes |
|-----------|----------|
| **Headers** | `Range` (janela unica alinhada aos offsets de chunk), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional e `X-SoraFS-Stream-Token` base64 obrigatorio. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range` descrevendo a janela servida, metadados `X-Sora-Chunk-Range` e headers de chunker/token ecoados. |
| **Falhas** | `416` para ranges desalinhados, `401` para tokens ausentes/invalidos, `429` quando budgets de stream/bytes sao excedidos. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch de chunk unico com os mesmos headers mais o digest determinista do chunk.
Util para retries ou downloads forenses quando slices de CAR sao desnecessarios.

## Workflow do orquestrador multi-source

Quando o fetch multi-source SF-6 esta habilitado (CLI Rust via `sorafs_fetch`,
SDKs via `sorafs_orchestrator`):

1. **Coletar entradas** - decodificar o plano de chunks do manifest, puxar os
   adverts mais recentes e, opcionalmente, passar um telemetry snapshot
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construir o scoreboard** - `Orchestrator::build_scoreboard` avalia a
   elegibilidade e registra razoes de rejeicao; `sorafs_fetch --scoreboard-out`
   persiste o JSON.
3. **Agendar chunks** - `fetch_with_scoreboard` (ou `--plan`) reforca restricoes
   de range, budgets de stream, caps de retry/peer (`--retry-budget`, `--max-peers`)
   e emite um stream token com escopo de manifest para cada requisicao.
4. **Verificar recibos** - as saidas incluem `chunk_receipts` e `provider_reports`;
   sumarios do CLI persistem `provider_reports`, `chunk_receipts` e
   `ineligible_providers` para evidence bundles.

Erros comuns apresentados a operadores/SDKs:

| Erro | Descricao |
|------|-----------|
| `no providers were supplied` | Nenhuma entrada elegivel apos o filtro. |
| `no compatible providers available for chunk {index}` | Mismatch de range ou budget para um chunk especifico. |
| `retry budget exhausted after {attempts}` | Aumente `--retry-budget` ou remova peers com falha. |
| `no healthy providers remaining` | Todos os provedores foram desabilitados apos falhas repetidas. |
| `streaming observer failed` | O writer CAR downstream abortou. |
| `orchestrator invariant violated` | Capture manifest, scoreboard, telemetry snapshot e CLI JSON para triage. |

## Telemetria e evidencias

- Metricas emitidas pelo orquestrador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (tagueadas por manifest/region/provider). Defina `telemetry_region` na config
  ou via flags de CLI para particionar dashboards por frota.
- Sumarios de fetch no CLI/SDK incluem scoreboard JSON persistido, chunk receipts
  e provider reports que devem ir nos rollout bundles para gates SF-6/SF-7.
- Gateway handlers expoem `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que dashboards SRE correlacionem decisoes do orquestrador com o comportamento
  do servidor.

## Helpers de CLI e REST

- `iroha app sorafs pin list|show`, `alias list` e `replication list` envolvem os
  endpoints REST do pin-registry e imprimem Norito JSON bruto com blocos de
  attestation para evidencias de auditoria.
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` aceitam manifests
  Norito ou JSON com alias proofs e successors opcionais; proofs malformados
  geram `400`, proofs stale retornam `503` com `Warning: 110`, e proofs expirados
  retornam `412`.
- Endpoints REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  incluem estruturas de attestation para que clientes verifiquem dados contra os
  ultimos headers de bloco antes de agir.

## Referencias

- Spec canonica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Helpers de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Crate do orquestrador: `crates/sorafs_orchestrator`
- Pack de dashboards: `dashboards/grafana/sorafs_fetch_observability.json`
