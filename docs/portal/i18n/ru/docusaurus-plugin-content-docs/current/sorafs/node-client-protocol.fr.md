---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Протокол nœud ↔ client SoraFS

Краткое изложение канонического определения протокола
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Используйте предварительную спецификацию для макетов Norito в октете и выше.
журналы изменений; La Copie du Portail Garde les Points Opérationnels Pres Du
остальные Runbooks SoraFS.

## Объявления Fournisseur et Validation

Les Fournisseurs SoraFS Распространение полезной нагрузки `ProviderAdvertV1` (voir
`crates/sorafs_manifest::provider_advert`) подписаны правительственным оператором. Лес
реклама fixent les métadonnées de decouverte et les garde-fous que l'orchestrateur
аппликация из нескольких источников для выполнения.

- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Лес
  Fournisseurs doivent rafraîchir toutes les 12 heures.
- **TLV de capacités** — список TLV, объявляющий о функционировании транспорта
  (Torii, QUIC+Noise, реле SoraNet, поставщик расширений). Непонятные коды
  peuvent être ignorés lorsque `allow_unknown_capabilities = true`, в другом месте
  Рекомендации СМАЗКА.
- **Индексы QoS** — уровень `availability` (Горячий/Теплый/Холодный), максимальная задержка
  рекуперация, лимит параллелизма и бюджет потока. La QoS doit
  s'aligner sur la télémétrie observée et est Auditée los de l'admission.
- **Конечные точки и темы встреч** — URL-адреса конкретных служб с доступом.
  метадонные TLS/ALPN, а также открытые темы для клиентов
  doivent s'abonner lors de la Construction des Guard Sets.
- **Политика разнообразия химизма** — `min_guard_weight`, разветвление
  AS/pool et `provider_failure_threshold` можно арендовать файлы
  детерминированный многоранговый.
- **Идентификаторы профиля** — les Fournisseurs doivent Exposure le handle
  канонический (например, `sorafs.sf1@1.0.0`) ; дополнительные опции `profile_aliases`
  старые клиенты-мигранты.

Правила проверки нулевой ставки, списки возможностей,
конечные точки или темы, длительные сроки или неудовлетворительные цели QoS. Лес
конверты для приема по сравнению с корпусом рекламы и предложения
(`compare_core_fields`) перед диффузором для дневных помещений.

### Расширения выборки страниц

Четырехместные повара имеют диапазон возможностей, включая сверхспособные метадонники:

| Чемпион | Объектиф |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Объявите `max_chunk_span`, `min_granularity` и флаги выравнивания/предварения. |
| `StreamBudgetV1` | Опция конверта согласования/пропускной способности (опция `max_in_flight`, `max_bytes_per_sec`, опция `burst`). Требуемый диапазон емкости. |
| `TransportHintV1` | Предпочтения при транспортировке (например, `torii_http_range`, `quic_stream`, `soranet_relay`). Приоритеты от `0–15` и дублоны отклонены. |

Инструменты поддержки:- Диапазон пропускной способности рекламных трубопроводов, поток
  Советы по бюджету и транспорту, авангард детерминированной полезной нагрузки для лесов
  аудиты.
- `cargo xtask sorafs-admission-fixtures` перегруппировать рекламу с несколькими источниками
  Canoniques с приборами понижения рейтинга в
  `fixtures/sorafs_manifest/provider_admission/`.
- Диапазон рекламы, который соответствует `stream_budget` или `transport_hints`.
  изменения в загрузчиках CLI/SDK, предварительное планирование, согласование проводки
  множественный источник sur les attentes d'admission Torii.

## Диапазон конечных точек шлюза

Шлюзы, принимающие запросы HTTP, определенные, которые отражают файлы
метадонники рекламы.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Необходимость | Детали |
|----------|---------|
| **Заголовки** | `Range` (уникальное выравнивание по смещениям фрагментов), `dag-scope: block`, `X-SoraFS-Chunker`, опция `X-SoraFS-Nonce`, а также обязательная опция `X-SoraFS-Stream-Token` base64. |
| **Ответы** | `206` с `Content-Type: application/vnd.ipld.car`, `Content-Range` обеспечивают доступ к сервису, метадонам `X-Sora-Chunk-Range` и блокированию заголовков/отправленных токенов. |
| **Режимы обучения** | `416` для неправильных пляжей, `401` для жетонов просроченных/инвалидов, `429` для потока бюджетов/октета, который исчез. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Получите отдельный фрагмент с заголовками мемов, а также определенный дайджест.
кусок. Утилита для повторных попыток или судебных разбирательств, когда
ломтики CAR Sont бесполезны.

## Рабочий процесс оркестратора с несколькими источниками

Когда активна выборка SF-6 из нескольких источников (CLI Rust через `sorafs_fetch`,
SDK через `sorafs_orchestrator`):

1. **Collecter les entrées** — декодер плана фрагментов манифеста, сборщик
   последние рекламные объявления и опции, позволяющие передать снимок телеметрии
   (`--telemetry-json` или `TelemetrySnapshot`).
2. **Построить табло** — значение `Orchestrator::build_scoreboard`.
   l'éligabilité et enregistre les raisons de rejet; `sorafs_fetch --scoreboard-out`
   сохраните JSON.
3. **Планификатор блоков** — `fetch_with_scoreboard` (или `--plan`) накладывает файлы
   диапазон ограничений, поток бюджетов, количество повторных попыток/одноранговых узлов (`--retry-budget`,
   `--max-peers`) и получает область токена потока в манифесте для каждого фрагмента.
   просьба.
4. **Vérifier les reçus** — вылеты включают `chunk_receipts` и др.
   `provider_reports`; постоянные CLI резюме `provider_reports`,
   `chunk_receipts` и `ineligible_providers` для предварительных пакетов.

Ошибки при ремонте операторов/SDK:

| Ошибка | Описание |
|--------|-------------|
| `no providers were supplied` | Aucune доступен после фильтрации. |
| `no compatible providers available for chunk {index}` | Ошибка пляжа или бюджета для конкретного куска. |
| `retry budget exhausted after {attempts}` | Дополните `--retry-budget` или проверьте сверстников в школе. |
| `no healthy providers remaining` | Все повара отключаются после повторных тренировок. |
| `streaming observer failed` | Писатель CAR отклонился от курса. |
| `orchestrator invariant violated` | Захват манифеста, табло, моментального снимка телеметрии и JSON CLI для сортировки. |

## Телеметрия и преуве- Метрики оркестра:  
  И18НИ00000077Х, И18НИ00000078Х,
  И18НИ00000079Х, И18НИ00000080Х
  (теги по манифесту/региону/фурниссеру). Définissez `telemetry_region` ru
  config или с помощью флагов CLI для разделения панелей мониторинга по назначению.
- Резюме получения CLI/SDK, включая постоянное табло JSON, результаты.
  куски и взаимосвязи, которые делают путешественника в связках
  развертывание шлюзов SF-6/SF-7.
- Шлюз обработчиков Les, открытый `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  для того, чтобы панели мониторинга SRE коррелировали с оркестраторами решений
  сервер обслуживания.

## Помощники CLI и REST

- `iroha app sorafs pin list|show`, `alias list` и `replication list` украшающие файлы
  конечные точки REST для регистрации контактов и ввода Norito JSON брут с блоками
  аттестация для аудита.
- `iroha app sorafs storage pin` и `torii /v1/sorafs/pin/register` принимаются
  манифестирует Norito или JSON, а также доказательства псевдонимов и преемников;
  доказательства неправильных форм отозваны `400`, доказательства устарели, открыты `503` с
  `Warning: 110`, и срок действия доказательств истекает после `412`.
- Конечные точки REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`), включая структуры аттестации для этих файлов
  клиенты проверяют своих клиентов с последними заголовками блоков перед началом работы.

## Ссылки

- Каноническая спецификация:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Типы Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Помощник CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Организатор ящиков: `crates/sorafs_orchestrator`
- Пакет информационных панелей: `dashboards/grafana/sorafs_fetch_observability.json`