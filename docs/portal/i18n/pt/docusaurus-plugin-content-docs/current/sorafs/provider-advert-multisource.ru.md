---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Мульти-istoчниковые объявления testes e planejamento

Esta página contém especificações canônicas em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Use este documento para o conjunto Norito e changelog; versão do portal
Obtenha instruções de operação do sistema, instale o SDK e use a telefonia para instalação
Use runbooks SoraFS.

## Дополнения к схеме Norito

### Diapasão de energia (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – máximo de interface possível (intervalo) na caixa, `>= 1`.
- `min_granularity` – pesquisa de busca, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets` – fornece compensações desnecessárias no mesmo período.
- `requires_alignment` – exceto verdadeiro, compensações são usadas em `min_granularity`.
- `supports_merkle_proof` – указывает поддержку свидетельств PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` обеспечивают каноническое
кодирование, чтобы payloads fofoca оставались детерминированными.

### `StreamBudgetV1`
- Política: `max_in_flight`, `max_bytes_per_sec`, opcional `burst_bytes`.
- Validação de validação (`StreamBudgetV1::validate`):
  -`max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, é claro, mas é `> 0` e `<= max_bytes_per_sec`.

###`TransportHintV1`
- Política: `protocol: TransportProtocol`, `priority: u8` (controle 0-15
  `TransportHintV1::validate`).
- Protocolos de implementação: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Дублирующиеся записи протоколов на провайдера отклоняются.

### Doação para `ProviderAdvertBodyV1`
- Opcional `stream_budget: Option<StreamBudgetV1>`.
- Opcional `transport_hints: Option<Vec<TransportHintV1>>`.
- Оба поля проходят через `ProviderAdmissionProposalV1`, envelopes de governança,
  Luminárias CLI e JSON de telemetria.

## Validação e privacidade da governança

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
отклоняют поврежденные метаданные:

- Диапазонные возможности должны корректно декодироваться и соблюдать лимиты
  диапазона/гранулярности.
- Transmitir orçamentos / dicas de transporte требуют TLV `CapabilityType::ChunkRangeFetch`
  e não há dicas de espionagem.
- Protocolos de transporte duplicados e prioridades desnecessárias de uso
  валидации до fofocas рассылки anúncios.
- Envelopes de admissão сравнивают propostas/anúncios по диапазонным метаданным через
  `compare_core_fields`, não há cargas úteis de fofoca disponíveis.

Registre a configuração de acesso em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

##Instrumentos e luminárias

- Cargas úteis fornecidas com `range_capability`, `stream_budget`
  e `transport_hints`. Проверяйте через ответы `/v2/sorafs/providers` e acessórios de admissão;
  JSON-резюме должны включать разобранную capacidade, orçamento de fluxo e muitas dicas
  para ingestão telemétrica.
- `cargo xtask sorafs-admission-fixtures` выводит orçamentos de fluxo e dicas de transporte
  Nos seus artefatos JSON, os painéis de controle são desativados pela função desejada.
- Luminárias em `fixtures/sorafs_manifest/provider_admission/` теперь включают:
  - anúncios canônicos de múltiplas histórias,
  - `multi_fetch_plan.json`, este SDK pode ser configurado para determinar
    plano de busca multi-peer.

## Integração com organização e Torii- Torii `/v2/sorafs/providers` возвращает разобранные метаданные диапазонных возможностей
  use `stream_budget` e `transport_hints`. Предупреждения downgrade срабатывают, когда
  провайдеры пропускают новые метаданные, um range endpoints шлюза применяют те же ограничения
  para clientes de negócios.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяет лимиты диапазона,
  выравнивание возможностей и fluxo de orçamentos при распределении работы. Unidade-teste покрывают
  случаи слишком больших pedaço, разреженного busca e estrangulamento.
- `sorafs_car::multi_fetch` передает сигналы downgrade (ошибки выравнивания,
  запросы estrangulados), чтобы операторы могли понимать, почему конкретные провайдеры
  Você tem propostas de planejamento.

## Справочник телеметрии

Инструментация range fetch em Torii питает painel Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) e alertas de segurança
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Tipo | Metica | Descrição |
|--------|-----|-------|----------|
| `torii_sorafs_provider_range_capability_total` | Medidor | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Провайдеры, объявляющие funções de diapasão. |
| `torii_sorafs_range_fetch_throttle_events_total` | Contador | `reason` (`quota`, `concurrency`, `byte_rate`) | Попытки range fetch с throttling, сгруппированные по политике. |
| `torii_sorafs_range_fetch_concurrency_current` | Medidor | — | Ao ativar as ações, você pode ativar a conexão. |

Exemplos PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Use a configuração de estrangulamento, o que você pode fazer antes de começar a usar
дефолтов мульти-источникового оркестратора, e поднимайте алерты, когда конкуренция
приближается к максимальным значениям orçamento de fluxo para sua frota.