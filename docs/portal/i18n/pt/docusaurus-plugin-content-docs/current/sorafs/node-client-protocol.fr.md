---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocolo noœud ↔ cliente SoraFS

Este guia resume a definição canônica do protocolo em
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Use a especificação upstream para os layouts Norito no nível octeto e
os registros de alterações; a cópia do portal protege os pontos operacionais antes de
resto dos runbooks SoraFS.

## Fornecedor e validação de anúncios

Os fornecedores SoraFS difundem cargas úteis `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) assinado pelo operador governamental. Les
anúncios fixam os métodos descobertos e os garde-fous que o orquestrador
aplicação de múltiplas fontes para execução.

- **Duração de validade** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Les
  os fornecedores devem rafraîchir toutes les 12 heures.
- **TLV de capacidades** — a lista TLV anuncia as funcionalidades de transporte
  (Torii, QUIC+Noise, relais SoraNet, fornecedor de extensões). Os códigos desconhecidos
  pode ser ignorado quando `allow_unknown_capabilities = true`, em seguida
  as recomendações GRAXA.
- **Índices QoS** — nível `availability` (Quente/Quente/Frio), latência máxima de
  recuperação, limite de concorrência e orçamento de fluxo opcional. O QoS faz isso
  s'aligner sur la télémétrie observée et est auditée lors de l'admission.
- **Endpoints e tópicos de encontro** — URLs de serviços concretos com
  metadonnées TLS/ALPN, além dos tópicos descobertos pelos clientes
  doivent s'abonner durante a construção dos conjuntos de guarda.
- **Politique de diversité de chemin** — `min_guard_weight`, caps de fan-out
  AS/pool e `provider_failure_threshold` tornam possíveis buscas
  determina multi-ponto.
- **Identificadores de perfil** — os fornecedores devem expor o identificador
  canônico (ex. `sorafs.sf1@1.0.0`); des `profile_aliases` identificador de opcionais
  os antigos clientes migrando.

As regras de validação rejeitam a aposta zero, listas de recursos,
endpoints ou tópicos, durações mal ordenadas, ou objetivos QoS manquants. Les
envelopes de admissão comparam o corpo de anúncio e a proposta
(`compare_core_fields`) antes do difusor das mises do dia.

### Extensões de busca por plages

Os fornecedores com faixa de capacidade incluem os seguintes níveis de metado:

| Campeão | Objetivo |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Declare `max_chunk_span`, `min_granularity` e as bandeiras de alinhamento/previo. |
| `StreamBudgetV1` | Envelope opcional de simultaneidade/taxa de transferência (opcional `max_in_flight`, `max_bytes_per_sec`, `burst`). Requer uma faixa de capacidade. |
| `TransportHintV1` | Préferências de transporte ordenadas (ex. `torii_http_range`, `quic_stream`, `soranet_relay`). As prioridades de `0–15` e os dobrões foram rejeitados. |

Ferramentas de suporte:- Les pipelines d'advert fournisseur doivent valider capacité range, stream
  dicas de orçamento e transporte antes de definir cargas úteis para os
  auditorias.
- `cargo xtask sorafs-admission-fixtures` reagrupamento de anúncios multifonte
  canônicos com luminárias de downgrade em
  `fixtures/sorafs_manifest/provider_admission/`.
- A faixa de anúncios que contém `stream_budget` ou `transport_hints` são
  rejeitados pelos carregadores CLI/SDK antes do planejamento, alinhando o chicote
  multi-fonte nas verificações de admissão Torii.

## Faixa de endpoints do gateway

Os gateways aceitam solicitações HTTP determinadas que refletem os
metadonnées des adverts.

###`GET /v1/sorafs/storage/car/{manifest_id}`

| Exigência | Detalhes |
|----------|---------|
| **Cabeçalhos** | `Range` (alinhamento exclusivo nos deslocamentos de pedaços), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcionais e `X-SoraFS-Stream-Token` base64 obrigatório. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range` descreve a janela de serviço, metado `X-Sora-Chunk-Range` e cabeçalhos chunker/token reenviados. |
| **Modos de verificação** | `416` para placas mal alinhadas, `401` para tokens manquants/invalides, `429` quando os orçamentos de stream/octeto são ultrapassados. |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Busque um único pedaço com os mesmos cabeçalhos, além do resumo determinado por
pedaço. Útil para novas tentativas ou transferências forenses quando
slices CAR são inutiles.

## Fluxo de trabalho do orquestrador multifonte

Quando a busca SF-6 multi-fonte está ativa (CLI Rust via `sorafs_fetch`,
SDKs via `sorafs_orchestrator`):

1. ** Coletar as entradas ** — decodifica o plano de pedaços do manifesto, recupera
   os últimos anúncios e a opção de passar um instantâneo de televisão
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construir um placar** — valor `Orchestrator::build_scoreboard`
   elegibilidade e registro das razões de rejeição; `sorafs_fetch --scoreboard-out`
   persista o JSON.
3. **Planificador de pedaços** — `fetch_with_scoreboard` (ou `--plan`) impõe arquivos
   faixa de restrições, fluxo de orçamentos, limites de repetição/peer (`--retry-budget`,
   `--max-peers`) e foi definido um escopo de token de fluxo no manifesto para cada um
   requête.
4. **Verifique os recursos** — as saídas incluem `chunk_receipts` e
   `provider_reports`; os currículos CLI persistente `provider_reports`,
   `chunk_receipts` e `ineligible_providers` para pacotes de pré-venda.

Erros atuais remontados em operadores/SDKs:

| Erro | Descrição |
|--------|------------|
| `no providers were supplied` | Aucune entrada elegível após a filtração. |
| `no compatible providers available for chunk {index}` | Erro de plage ou de orçamento para um pedaço específico. |
| `retry budget exhausted after {attempts}` | Aumente `--retry-budget` ou verifique os pares na verificação. |
| `no healthy providers remaining` | Todos os fornecedores foram desativados após as verificações repetidas. |
| `streaming observer failed` | Le escritor CAR a jusante avorté. |
| `orchestrator invariant violated` | Capturez manifesto, placar, instantâneo de telemetria e JSON CLI para triagem. |

## Télémétrie et preuves- Métricas emitidas pelo orquestrador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (taggées par manifest/région/fournisseur). Defina `telemetry_region` em
  config ou via flags CLI para particionar os painéis por flotte.
- Os currículos de busca CLI/SDK incluem o scoreboard JSON persistente, os recursos
  de chunks et les rapports fournisseurs qui doivent voyager dans les bundles
  implementação das portas SF-6/SF-7.
- Les manipuladores gateway exposto `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que os painéis SRE correspondam às decisões orquestradas com o
  servidor de comportamento.

## Ajuda CLI e REST

- `iroha app sorafs pin list|show`, `alias list` e `replication list` embalam arquivos
  endpoints REST do pin-registry e impressão do Norito JSON bruto com blocos
  d'attestation pour l'audit.
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` aceitam dados
  manifesta Norito ou JSON, além de provas de opções de alias e sucessores ;
  as provas mal formadas reenviadas `400`, as provas obsoletas expostas `503` com
  `Warning: 110`, e as provas expiradas reenviadas `412`.
- Os endpoints REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) inclui estruturas de atestado para que
  os clientes verificam os dados com os últimos cabeçalhos do bloco antes de começar.

## Referências

- Especificação canônica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Auxiliares CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orquestrador de caixas: `crates/sorafs_orchestrator`
- Pacote de painéis: `dashboards/grafana/sorafs_fetch_observability.json`