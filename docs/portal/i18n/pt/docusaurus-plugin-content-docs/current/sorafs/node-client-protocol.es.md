---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocolo de nó ↔ cliente de SoraFS

Este guia resume a definição canônica do protocolo em
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use a especificação upstream para os layouts Norito no nível de bytes e
registros de alterações; a cópia do portal mantém os pontos operacionais perto do resto
dos runbooks de SoraFS.

## Anúncios de provedor e validação

Os provedores de SoraFS difundem cargas úteis `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) firmado pelo operador governamental.
Os anúncios fixam os metadados de descoberta e os salvam
o orquestrador multifuente impõe o tempo de execução.

- **Vigência** — `issued_at < expires_at ≤ issued_at + 86,400 s`. Os provadores
  deve atualizar a cada 12 horas.
- **TLVs de capacidades** — a lista TLV anuncia funções de transporte (Torii,
  QUIC+Noise, relés SoraNet, extensões de provedor). Os códigos desconhecidos
  você pode omitir quando `allow_unknown_capabilities = true`, seguindo o guia
  GRAXA.
- **Pistas de QoS** — nível de `availability` (Quente/Quente/Frio), latência máxima de
  recuperação, limite de concorrência e pressuposto de transmissão opcional. A QoS
  deve ser alinhado com a telemetria observada e auditado na admissão.
- **Endpoints e tópicos de encontro** — URLs de serviço específicos com
  metadados TLS/ALPN além dos tópicos de descoberta para os clientes
  deben se inscrever para construir conjuntos de guardas.
- **Política de diversidade de rotas** — `min_guard_weight`, topos de fan-out de
  AS/pool e `provider_failure_threshold` possibilitam buscas deterministas
  multiponto.
- **Identificadores de perfil** — os provedores devem expor o identificador
  canônico (p. ej., `sorafs.sf1@1.0.0`); `profile_aliases` opcionais ajudam a
  migrar clientes antigos.

As regras de validação rechazan stake zero, listas de vagas de capacidades,
endpoints ou tópicos, vigilâncias desordenadas ou objetivos de QoS faltantes. Los
sobres de admissão comparan los cuerpos del anuncio y la propuesta
(`compare_core_fields`) antes de divulgar atualizações.

### Extensões de busca por rangos

Os provedores com rango incluem os seguintes metadados:

| Campo | Propósito |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` e sinalizadores de alinhamento/teste. |
| `StreamBudgetV1` | Envelope opcional de concorrência/taxa de transferência (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Requer uma capacidade de rango. |
| `TransportHintV1` | Preferências de transporte ordenadas (p. ej., `torii_http_range`, `quic_stream`, `soranet_relay`). As prioridades de `0–15` e são rechazan duplicadas. |

Suporte de ferramentas:- Os pipelines do provedor de anúncio devem validar a capacidade de rango, stream
  dicas de orçamento e transporte antes de emitir cargas deterministas para auditorias.
- `cargo xtask sorafs-admission-fixtures` pacote de anúncios multifuente
  canônicos junto com fixtures de downgrade en
  `fixtures/sorafs_manifest/provider_admission/`.
- Os anúncios com rango que omitem `stream_budget` ou `transport_hints` filho
  rechazados pelos carregadores CLI/SDK antes de programar, mantendo o arnês
  multifuente alinhado com as expectativas de admissão de Torii.

## Endpoints do intervalo do gateway

Os gateways aceitam solicitações de deterministas HTTP que reflitam os metadados de
os anúncios.

###`GET /v2/sorafs/storage/car/{manifest_id}`

| Requisito | Detalhes |
|-----------|----------|
| **Cabeçalhos** | `Range` (ventana única alinhada com deslocamentos de pedaços), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional e `X-SoraFS-Stream-Token` base64 obrigatório. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range` que descreve a janela servida, metadados `X-Sora-Chunk-Range` e cabeçalhos de chunker/token ecoados. |
| **Modos de queda** | `416` para intervalos desalineados, `401` para tokens ausentes ou inválidos, `429` quando se excedem os pressupostos de stream/bytes. |

###`GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Buscar um único pedaço com os mesmos cabeçalhos além do resumo determinista do
pedaço. Útil para reintenções ou descargas forenses quando não se precisa de fatias
CARRO.

## Fluxo de trabalho do orquestrador multifuente

Quando a busca multifuente SF-6 (CLI Rust via `sorafs_fetch` é habilitada,
SDKs via `sorafs_orchestrator`):

1. **Recopilar entradas** — decodifica o plano de pedaços do manifesto, traer
   os anúncios mais recentes e, opcionalmente, passar um instantâneo de telemetria
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construir um placar** — `Orchestrator::build_scoreboard` avaliar
   elegibilidade e registro de motivos de rechazo; `sorafs_fetch --scoreboard-out`
   persista o JSON.
3. **Pedaços de programa** — `fetch_with_scoreboard` (ou `--plan`) impone
   restrições de rango, pressupostos de stream, topos de reintentos/peers
   (`--retry-budget`, `--max-peers`) e emite um token de fluxo com alcance do
   manifestar-se por cada solicitação.
4. **Verificar recibos** — as saídas incluem `chunk_receipts` e
   `provider_reports`; os resumos da CLI persistem `provider_reports`,
   `chunk_receipts` e `ineligible_providers` para pacotes de evidências.

Erros comuns que são transmitidos a operadores/SDKs:

| Erro | Descrição |
|-------|------------|
| `no providers were supplied` | Não há entradas elegíveis além do filtrado. |
| `no compatible providers available for chunk {index}` | Desajuste de rango ou pressuposto para um pedaço específico. |
| `retry budget exhausted after {attempts}` | Incrementa `--retry-budget` para expulsar peers falidos. |
| `no healthy providers remaining` | Todos os provedores ficaram desfigurados após repetidas falhas. |
| `streaming observer failed` | O escritor CAR a jusante abortou. |
| `orchestrator invariant violated` | Captura de manifesto, placar, instantâneo de telemetria e JSON da CLI para triagem. |

## Telemetria e evidência- Métricas emitidas pelo orquestrador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (etiquetadas por manifesto/região/provedor). Configurar `telemetry_region` pt
  config ou por meio de sinalizadores CLI para que os painéis sejam separados por flota.
- Os currículos de busca no CLI/SDK incluem scoreboard JSON persistido, recibos
  de pedaços e relatórios de fornecedores que devem viajar em pacotes de lançamento
  para as portas SF-6/SF-7.
- Os manipuladores do gateway expõem `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que os painéis SRE correlacionem as decisões do orquestrador com o
  comportamento do servidor.

## Ajudas de CLI e REST

- `iroha app sorafs pin list|show`, `alias list` e `replication list` os enviam
  endpoints REST do pin-registry e imprimir Norito JSON bruto com blocos de
  atestado para evidência de auditoria.
- Manifestos de aceptan `iroha app sorafs storage pin` e `torii /v2/sorafs/pin/register`
  Norito ou JSON mais provas de alias opcionais e sucessores; provas malformadas
  elevan `400`, provas obsoletos exponen `503` com `Warning: 110`, e provas
  expirados devuelven `412`.
- Os endpoints REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) inclui estruturas de atestado para que
  os clientes verificam os dados dos cabeçalhos do último bloco antes de atuar.

## Referências

- Especificação canônica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ajuda CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caixa do orquestrador: `crates/sorafs_orchestrator`
- Pacote de painéis: `dashboards/grafana/sorafs_fetch_observability.json`