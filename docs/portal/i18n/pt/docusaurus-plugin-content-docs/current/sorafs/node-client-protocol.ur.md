---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS نوڈ ↔ کلائنٹ پروٹوکول

یہ گائیڈ پروٹوکول کی canônico تعریف کو
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
میں resumir کرتی ہے۔ layouts Norito de nível de byte e changelogs e upstream
especificações cópia do portal runbooks SoraFS کے ساتھ destaques operacionais کو
قریب رکھتی ہے۔

## Anúncios de provedores e validação

Provedores SoraFS Cargas úteis `ProviderAdvertV1` (دیکھیں
`crates/sorafs_manifest::provider_advert`) fofoca کرتے ہیں جو operador governado
نے sinal کیے ہوتے ہیں۔ metadados de descoberta de anúncios اور guardrails کو pin کرتے ہیں
Tempo de execução do orquestrador multi-fonte میں impor کرتا ہے۔

- **Vitalício** — `issued_at < expires_at ≤ issued_at + 86,400 s`. provedores کو
  12 گھنٹے میں atualizar کرنا چاہیے۔
- **Capacidade TLVs** — Os recursos de transporte da lista TLV anunciam کرتی ہے (Torii,
  QUIC+Noise, relés SoraNet, extensões de fornecedores). códigos desconhecidos کو
  `allow_unknown_capabilities = true` پر skip کیا جا سکتا ہے, orientação GREASE کے مطابق۔
- **Dicas de QoS** — Camada `availability` (quente/quente/frio), latência máxima de recuperação,
  limite de simultaneidade e orçamento de fluxo opcional. QoS کو telemetria observada کے
  مطابق ہونا چاہیے اور admissão میں auditoria کیا جاتا ہے۔
- **Endpoints e tópicos de encontro** — Metadados TLS/ALPN são concretos
  URLs de serviço, tópicos de descoberta, clientes, conjuntos de proteção, assinatura e assinatura
  کرنا چاہیے۔
- **Política de diversidade de caminho** — `min_guard_weight`, tampas de distribuição AS/pool.
  `provider_failure_threshold` buscas determinísticas de vários pares
- **Identificadores de perfil** — provedores e identificador canônico expor کرنا ہوتا ہے
  (exemplo `sorafs.sf1@1.0.0`); opcional `profile_aliases` پرانے clientes کی
  migração میں مدد دیتے ہیں۔

Regras de validação com participação zero, listas de capacidades/endpoints/tópicos vazias, ordenadas incorretamente
vidas, یا alvos de QoS ausentes کو rejeitar کرتی ہیں۔ anúncio de envelopes de admissão
اور órgãos de proposta (`compare_core_fields`) کو comparar کرتے ہیں پھر atualizações fofocas
کرتے ہیں۔

### Extensões de busca de intervalo

Provedores com capacidade de alcance têm metadados que podem ser usados:

| Campo | Finalidade |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` e sinalizadores de alinhamento/prova declaram کرتا ہے۔ |
| `StreamBudgetV1` | envelope de simultaneidade/taxa de transferência opcional (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). capacidade de alcance necessária ہے۔ |
| `TransportHintV1` | preferências de transporte solicitadas (exemplo `torii_http_range`, `quic_stream`, `soranet_relay`). prioridades `0–15` ہیں اور duplicatas rejeitadas ہوتے ہیں۔ |

Suporte de ferramentas:

- Pipelines de anúncios do provedor, capacidade de alcance, orçamento de fluxo e dicas de transporte
  validar کرنے چاہییں پھر auditorias کے لیے cargas úteis determinísticas emitem کریں۔
- `cargo xtask sorafs-admission-fixtures` anúncios canônicos de múltiplas fontes کو
  downgrade fixtures کے ساتھ `fixtures/sorafs_manifest/provider_admission/` میں pacote کرتا ہے۔
- Anúncios com capacidade de alcance جو `stream_budget` یا `transport_hints` omitir کریں, CLI/SDK
  carregadores انہیں agendamento سے پہلے rejeitar کرتے ہیں تاکہ chicote de múltiplas fontes
  Expectativas de admissão Torii کے ساتھ alinhadas رہے۔

## Terminais de intervalo de gatewayAs solicitações HTTP determinísticas dos gateways aceitam metadados de anúncio e espelho
کرتی ہیں۔

###`GET /v1/sorafs/storage/car/{manifest_id}`

| Requisito | Detalhes |
|------------|---------|
| **Cabeçalhos** | `Range` (janela única alinhada aos deslocamentos de pedaços), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional, اور لازمی base64 `X-SoraFS-Stream-Token`. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range` جو janela servida کو بیان کرتا ہے, `X-Sora-Chunk-Range` metadados, e cabeçalhos chunker/token کا echo۔ |
| **Modos de falha** | intervalos desalinhados پر `416`, tokens ausentes/inválidos پر `401`, اور orçamentos de fluxo/byte excedem ہونے پر `429`۔ |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Busca de pedaço único انہی cabeçalhos کے ساتھ mais resumo determinístico de pedaços ۔ novas tentativas
یا downloads forenses کے لیے مفید جب CAR slices غیر ضروری ہوں۔

## Fluxo de trabalho do orquestrador de várias fontes

جب Busca multi-fonte SF-6 habilitada ہو (Rust CLI via `sorafs_fetch`, SDKs via
`sorafs_orchestrator`):

1. **Coletar entradas** - decodificação do plano de bloco de manifesto کریں, pull de anúncios mais recentes کریں،
   Um instantâneo de telemetria opcional (`--telemetry-json` ou `TelemetrySnapshot`) está disponível
2. **Crie um placar** — Avaliação de elegibilidade `Orchestrator::build_scoreboard`
   کرتا ہے اور registro de motivos de rejeição کرتا ہے؛ `sorafs_fetch --scoreboard-out`
   JSON persistir کرتا ہے۔
3. **Partes do cronograma** — restrições de intervalo `fetch_with_scoreboard` (یا `--plan`),
   orçamentos de fluxo, limites de novas tentativas/peer (`--retry-budget`, `--max-peers`) impor restrições
   Para uma solicitação de emissão de token de fluxo com escopo de manifesto کرتا ہے۔
4. **Verificar recibos** - saídas `chunk_receipts` e `provider_reports` شامل
   ہوتے ہیں؛ Resumos CLI `provider_reports`, `chunk_receipts`, E
   `ineligible_providers` کو pacotes de evidências کے لیے persist کرتے ہیں۔

Erros de operadores/SDKs کو ملنے والی عام:

| Erro | Descrição |
|-------|------------|
| `no providers were supplied` | filtragem کے بعد کوئی entrada elegível نہیں۔ |
| `no compatible providers available for chunk {index}` | مخصوص pedaço کے لیے intervalo یا incompatibilidade de orçamento۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` بڑھائیں یا pares com falha کو despejar کریں۔ |
| `no healthy providers remaining` | falhas repetidas کے بعد تمام provedores desabilitam ہو گئے۔ |
| `streaming observer failed` | abortar gravador CAR downstream ہو گیا۔ |
| `orchestrator invariant violated` | triagem کے لیے manifesto, placar, instantâneo de telemetria, e captura CLI JSON کریں۔ |

## Telemetria e evidências

- Métricas do orquestrador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifesto/região/provedor کے tags کے ساتھ). dashboards کو frota کے لحاظ سے
  partição کرنے کے لیے config یا CLI flags میں `telemetry_region` سیٹ کریں۔
- Resumos de busca CLI/SDK com painel de avaliação persistente JSON, recibos de pedaços e
  relatórios do provedor شامل ہوتے ہیں جو portas SF-6/SF-7 کے pacotes de implementação میں شامل ہونے چاہییں۔
- Manipuladores de gateway `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  expor کرتے ہیں تاکہ Decisões do orquestrador de painéis SRE e comportamento do servidor
  correlacionar کر سکیں۔

## CLI e ajudantes REST- `iroha app sorafs pin list|show`, `alias list`, e `replication list` registro de pinos
  Endpoints REST wrap کرتے ہیں اور evidência de auditoria کے لیے blocos de atestado کے ساتھ
  impressão JSON raw Norito کرتے ہیں۔
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` Norito ou JSON
  manifestos کے ساتھ provas de alias opcionais اور sucessores aceitam کرتے ہیں؛ malformado
  provas de `400`, provas obsoletas de `503` de `Warning: 110`, e provas expiradas
  پر `412`۔
- Pontos de extremidade REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  estruturas de atestado شامل کرتے ہیں تاکہ clientes cabeçalhos de bloco mais recentes کے
  Verificação de dados کر سکیں۔

## Referências

- Especificações canônicas:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ajudantes CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caixa do orquestrador: `crates/sorafs_orchestrator`
Pacote de painel: `dashboards/grafana/sorafs_fetch_observability.json`