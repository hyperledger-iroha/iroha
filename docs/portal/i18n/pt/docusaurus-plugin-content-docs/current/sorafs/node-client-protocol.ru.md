---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#Protocolo SoraFS usado ↔ cliente

Este é o procedimento de configuração canônica do protocolo de implementação
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use a especificação upstream para o layout Norito do banco de dados e o changelog'ов;
A cópia do portal contém contas de operação executadas com o runbook SoraFS.

## Teste de verificação e validação

O testador SoraFS transfere a carga útil `ProviderAdvertV1` (см.
`crates/sorafs_manifest::provider_advert`), verifique a operação.
Объявления фиксируют метаданные обнаружения и guardrails, которые
O multi-orquestrador inicia a sua jornada.

- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Provadores
  должны обновлять каждые 12 horas.
- **Capacidade TLV** — TLV-список рекламирует транспортные возможности (Torii,
  QUIC+Noise, relés SoraNet, extensões de fornecedores). Códigos novos podem ser
  proposta por `allow_unknown_capabilities = true`, conforme recomendação
  GRAXA.
- **Dicas de QoS** — nível `availability` (Quente/Quente/Frio), максимальная задержка
  извлечения, лимит конкурентности e опциональный orçamento de fluxo. Qualidade de QoS
  совпадать с наблюдаемой телеметрией и проверяется при admissão.
- **Endpoints e tópicos de encontro** — URL de serviço conectado com TLS/ALPN
  метаданными плюс tópicos de descoberta, на которые клиенты подписываются при
  conjuntos de guarda построении.
- **Политика разнообразия путей** — `min_guard_weight`, limites de fan-out para
  AS/пулов e `provider_failure_threshold` determinam a determinação
  busca multi-peer.
- **Perfil de identificação** — provedor de perfil público canônico
  alça (por exemplo, `sorafs.sf1@1.0.0`); Opção `profile_aliases`
  миграции старых clientes.

Правила валидации отвергают нулевой stake, пустые списки capacidades/endpoints/tópicos,
перепутанные сроки, либо отсутствующие alvos de QoS. Envelopes de admissão
сравнивают тела объявления и предложения (`compare_core_fields`) antes
распространением обновлений.

### Расширения busca de intervalo

Range-совместимые провайдеры включают следующую метадату:

| Pólo | Atualizado |
|------|------------|
| `CapabilityType::ChunkRangeFetch` | Verifique `max_chunk_span`, `min_granularity` e coloque as bandeiras/depósitos. |
| `StreamBudgetV1` | Conversão de envelope opcional/taxa de transferência (`max_in_flight`, `max_bytes_per_sec`, опциональный `burst`). Capacidade de alcance Требует. |
| `TransportHintV1` | Упорядоченные предпочтения transporte (por exemplo, `torii_http_range`, `quic_stream`, `soranet_relay`). Приоритеты `0–15`, дубли отклоняются. |

Ferramentas adicionais:

- Anúncio do fornecedor Пайплайны должны валидировать capacidade de alcance, orçamento de fluxo
  e dicas de transporte para determinar a carga útil dos auditores.
- `cargo xtask sorafs-admission-fixtures` pacote canônico multifonte
  anúncios em vez de rebaixar luminárias em
  `fixtures/sorafs_manifest/provider_admission/`.
- Anúncios de gama entre `stream_budget` ou `transport_hints` отклоняются загрузчиками
  CLI/SDK para planejamento, gerenciamento de chicote de múltiplas fontes em suporte
  admissão ожиданиями Torii.

## Gateway de endpoints de intervaloOs gateways determinam a definição de protocolos HTTP, fornecendo metadados.

###`GET /v1/sorafs/storage/car/{manifest_id}`

| Treino | Detalhes |
|------------|--------|
| **Cabeçalhos** | `Range` (intervalo, usado para deslocamentos de pedaços), `dag-scope: block`, `X-SoraFS-Chunker`, opcional `X-SoraFS-Nonce` e base64 `X-SoraFS-Stream-Token`. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range`, descrição do arquivo, metadaнными `X-Sora-Chunk-Range` e outros cabeçalhos pedaço/token. |
| **Modos de falha** | `416` para novos dias úteis, `401` para novos tokens/novos tokens, `429` при превышении orçamento de fluxo/byte. |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Buscar um pedaço diferente com os cabeçalhos e determinar o pedaço de resumo.
Полезно для ретраев или downloads forenses, когда CAR slices не нужны.

## Fluxo de trabalho multifuncional

Como usar a busca de várias fontes SF-6 (Rust CLI é `sorafs_fetch`,
SDKs são `sorafs_orchestrator`):

1. **Собрать входные данные** — декодировать plano chunk'ов manifesto, получить
   anúncios publicitários e, opcionalmente, transmitir instantâneo de telemetria
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Placar de avaliação** — `Orchestrator::build_scoreboard` оценивает
   пригодность и записывает причины отказа; `sorafs_fetch --scoreboard-out`
   gera JSON.
3. **Plaнировать chunk'и** — `fetch_with_scoreboard` (ou `--plan`) primeiro
   faixa de alcance, orçamento de fluxo, limites de nova tentativa/peer (`--retry-budget`,
   `--max-peers`) e use o token de fluxo no manifesto do escopo para o caso.
4. **Verificar recibos** — результаты включают `chunk_receipts` e
   `provider_reports`; Resumo CLI definido `provider_reports`, `chunk_receipts`
   e `ineligible_providers` para pacotes de evidências.

Распространенные ошибки, возвращаемые операторам/SDKs:

| Óscar | Descrição |
|--------|----------|
| `no providers were supplied` | Não é possível inserir filtros adicionais. |
| `no compatible providers available for chunk {index}` | Não há necessidade de um dia ou de um pedaço de concreto. |
| `retry budget exhausted after {attempts}` | Use `--retry-budget` ou isole peers. |
| `no healthy providers remaining` | Cada um dos provedores pode ser usado para obter mais informações. |
| `streaming observer failed` | Escritor CAR downstream завершился с ошибкой. |
| `orchestrator invariant violated` | Crie manifesto, placar, instantâneo de telemetria e CLI JSON para triagem. |

## Telemetria e doação

- Метрики, эмитируемые оркестратором:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (no arquivo manifesto/região/provedor). Verifique `telemetry_region` na configuração ou
  Ao usar a bandeira CLI, esses dados serão distribuídos na flutuação.
- Resumos de busca CLI/SDK fornecem scoreboard JSON, recibos de pedaços e
  relatórios do provedor, que são adicionados aos pacotes de implementação para o gate'ов SF-6/SF-7.
- Manipuladores de gateway публикуют `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`,
  Os painéis SRE conectam a configuração do orquestrador com o servidor principal.

## CLI e REST ajuda- `iroha app sorafs pin list|show`, `alias list` e `replication list` atualizados
  endpoints REST pin-registry e печатают сырой Norito JSON com atestado de bloco
  para o auditor.
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` são usados Norito
  ou manifestos JSON плюс опциональные provas de alias e sucessores; provas malformadas
  возвращают `400`, provas obsoletas de `503` com `Warning: 110`, uma provas expiradas
  instale `412`.
- Pontos de extremidade REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) включают структуры atestado, чтобы клиенты могли
  проверить данные относительно последних block headers antes de действием.

## Ссылки

- Especificação canônica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ajuda CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Organizador de caixa: `crates/sorafs_orchestrator`
- Pacote de dados: `dashboards/grafana/sorafs_fetch_observability.json`