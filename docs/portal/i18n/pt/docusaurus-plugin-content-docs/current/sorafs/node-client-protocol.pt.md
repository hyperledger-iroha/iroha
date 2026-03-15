---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocolo de não  cliente da SoraFS

Este guia resume a definição canônica do protocolo em
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use uma especificação upstream para layouts Norito em nível de byte e changelogs;
a cópia do portal mantém os destaques operacionais perto do restante dos runbooks
SoraFS.

## Anúncios de provedor e validação

Provedores SoraFS disseminam payloads `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) contratados pelo operador governado. Os
os anúncios fixam os metadados de descoberta e os guardrails que o orquestrador
aplicativo multi-fonte em tempo de execução.

- **Vigência** - `issued_at < expires_at <= issued_at + 86,400 s`. Provedores
  devem renovar a cada 12 horas.
- **TLVs de capacidade** - a lista TLV anuncia recursos de transporte (Torii,
  QUIC+Noise, relés SoraNet, extensos de fornecedor). Códigos desconhecidos
  podem ser ignorados quando `allow_unknown_capabilities = true`, seguindo a
  orientação GREASE.
- **Dicas de QoS** - nível de `availability` (Quente/Quente/Frio), latência máxima de
  recuperação, limite de concorrência e orçamento de fluxo opcional. Desenvolvimento de QoS
  alinhar com a telemetria observada e auditada na admissão.
- **Endpoints e tópicos de encontro** - URLs de serviço concreto com metadados
  TLS/ALPN mais os tópicos de descoberta aos quais os clientes devem se inscrever
  ao construir conjuntos de guardas.
- **Politica de diversidade de caminho** - `min_guard_weight`, caps de fan-out de
  AS/pool e `provider_failure_threshold` tornam possíveis buscas deterministas
  multiponto.
- **Identificadores de perfil** - provedores devem exportar o identificador canônico (ex.
  `sorafs.sf1@1.0.0`); `profile_aliases` ajudaram clientes antigos a migrar.

Regras de validação rejeitam stake zero, listas vazias de capacidades/endpoints/tópicos,
vigilâncias fora de ordem ou alvos de QoS ausentes. Comparação de envelopes de admissão
os corpos do anúncio e da proposta (`compare_core_fields`) antes de divulgar
atualizações.

### Extensões de range fetch

Provedores com gama incluem os seguintes metadados:

| Campo | Proposta |
|-------|-----------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` e bandeiras de alinhamento/prova. |
| `StreamBudgetV1` | Envelope opcional de concorrência/taxa de transferência (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Solicite capacidade de alcance. |
| `TransportHintV1` | Preferências de transporte ordenadas (ex. `torii_http_range`, `quic_stream`, `soranet_relay`). Prioridades são `0-15` e duplicados são rejeitados. |

Suporte de ferramentas:

- Pipelines de provedor de anúncio devem validar capacidade de alcance, orçamento de fluxo e
  dicas de transporte antes de emitir cargas deterministas para auditorias.
- `cargo xtask sorafs-admission-fixtures` grupos de anúncios canônicos de múltiplas fontes
  junto com downgrade fixtures em `fixtures/sorafs_manifest/provider_admission/`.
- Anúncios com faixa que omitem `stream_budget` ou `transport_hints` são rejeitados
  pelos loaders CLI/SDK antes do agendamento, mantendo o chicote multi-fonte
  alinhado com as expectativas de admissão do Torii.

## Endpoints de intervalo do gatewayGateways aceitam requisitos HTTP deterministas que espelham os metadados do
anúncio.

###`GET /v2/sorafs/storage/car/{manifest_id}`

| Requisito | Detalhes |
|-----------|----------|
| **Cabeçalhos** | `Range` (janela única homologada aos offsets de chunk), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional e `X-SoraFS-Stream-Token` base64 obrigatório. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range` descrevendo a janela servida, metadados `X-Sora-Chunk-Range` e cabeçalhos de chunker/token ecoados. |
| **Falhas** | `416` para intervalos desalinhados, `401` para tokens ausentes/invalidos, `429` quando orçamentos de stream/bytes são excedidos. |

###`GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch de chunk único com os mesmos cabeçalhos mais o resumo determinista do chunk.
Util para novas tentativas ou downloads forenses quando fatias de CAR são desnecessárias.

## Workflow do orquestrador multi-source

Quando o fetch multi-source SF-6 está habilitado (CLI Rust via `sorafs_fetch`,
SDKs via `sorafs_orchestrator`):

1. **Coletar entradas** - decodificar o plano de chunks do manifesto, puxar os
   anúncios mais recentes e, opcionalmente, passar um instantâneo de telemetria
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construir o placar** - `Orchestrator::build_scoreboard` avaliar
   elegibilidade e registro motivos de rejeição; `sorafs_fetch --scoreboard-out`
   persistir o JSON.
3. **Partes da agenda** - `fetch_with_scoreboard` (ou `--plan`) restrições de reforço
   intervalo, orçamentos de fluxo, limites de repetição/peer (`--retry-budget`, `--max-peers`)
   e emite um stream token com escopo de manifesto para cada requisição.
4. **Verificar recibos** - conforme dito inclui `chunk_receipts` e `provider_reports`;
   resumos do CLI persistem `provider_reports`, `chunk_receipts` e
   `ineligible_providers` para pacotes de evidências.

Erros comuns apresentados em operadores/SDKs:

| Erro | Descrição |
|------|-----------|
| `no providers were supplied` | Nenhuma entrada elegível após o filtro. |
| `no compatible providers available for chunk {index}` | Incompatibilidade de faixa ou orçamento para um pedaço específico. |
| `retry budget exhausted after {attempts}` | Aumente `--retry-budget` ou remoção de peers com falha. |
| `no healthy providers remaining` | Todos os provedores foram desabilitados após falhas repetidas. |
| `streaming observer failed` | O escritor do CAR rio abaixo abortou. |
| `orchestrator invariant violated` | Capture manifesto, placar, instantâneo de telemetria e CLI JSON para triagem. |

## Telemetria e evidências

- Métricas emitidas pelo orquestrador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (tagueadas por manifesto/região/provedor). Defina `telemetry_region` na configuração
  ou via flags de CLI para particionar dashboards por frota.
- Resumos de busca no CLI/SDK incluem scoreboard JSON persistido, chunk recibos
  O provedor relata que deve nos lançar pacotes para portas SF-6/SF-7.
- Exposição de manipuladores de gateway `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que dashboards SRE correlacionem decisões do orquestrador com o comportamento
  do servidor.

## Ajudantes de CLI e REST- `iroha app sorafs pin list|show`, `alias list` e `replication list` envolvimento dos
  endpoints REST do pin-registry e imprimem Norito JSON bruto com blocos de
  atestado para evidências de auditoria.
- `iroha app sorafs storage pin` e `torii /v2/sorafs/pin/register` aceitam manifestos
  Norito ou JSON com alias provas e sucessores questionários; provas malformadas
  geram `400`, provas obsoletas retornam `503` com `Warning: 110`, e provas expiradas
  retornam `412`.
- Terminais REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  incluem estruturas de atestação para que os clientes verifiquem dados contra os
  últimos cabeçalhos de bloco antes de agir.

## Referências

- Especificação canônica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ajudantes de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caixa do orquestrador: `crates/sorafs_orchestrator`
- Pacote de painéis: `dashboards/grafana/sorafs_fetch_observability.json`