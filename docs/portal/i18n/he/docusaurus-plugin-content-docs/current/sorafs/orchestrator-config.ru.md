---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-config
title: Конфигурация оркестратора SoraFS
sidebar_label: Конфигурация оркестратора
description: Настройка мульти-источникового fetch-оркестратора, интерпретация сбоев и отладка телеметрии.
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/developer/orchestrator.md`. Держите обе копии синхронизированными, пока устаревшая документация не будет выведена из обращения.
:::

# Руководство по мульти-источниковому fetch-оркестратору

Мульти-источниковый fetch-оркестратор SoraFS управляет детерминированными
параллельными загрузками из набора провайдеров, опубликованного в adverts под
контролем governance. В этом руководстве описано, как настраивать оркестратор,
какие сигналы сбоев ожидать при rollout, и какие потоки телеметрии показывают
индикаторы здоровья.

## 1. Обзор конфигурации

Оркестратор объединяет три источника конфигурации:

| Источник | Назначение | Примечания |
|----------|------------|------------|
| `OrchestratorConfig.scoreboard` | Нормализует веса провайдеров, проверяет свежесть телеметрии и сохраняет JSON scoreboard для аудитов. | Основан на `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Применяет runtime-ограничения (бюджеты ретраев, лимиты параллелизма, переключатели верификации). | Маппится на `FetchOptions` в `crates/sorafs_car::multi_fetch`. |
| Параметры CLI / SDK | Ограничивают число пиров, добавляют регионы телеметрии и выводят политики deny/boost. | `sorafs_cli fetch` раскрывает эти флаги напрямую; SDK передают их через `OrchestratorConfig`. |

JSON-хелперы в `crates/sorafs_orchestrator::bindings` сериализуют всю
конфигурацию в Norito JSON, делая её переносимой между SDK и автоматизацией.

### 1.1 Пример JSON-конфигурации

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Сохраняйте файл через стандартное наслаивание `iroha_config` (`defaults/`, user,
actual), чтобы детерминированные деплои наследовали одинаковые лимиты на всех
нодах. Для профиля direct-only, соответствующего rollout SNNet-5a, смотрите
`docs/examples/sorafs_direct_mode_policy.json` и сопутствующие рекомендации в
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Overrides соответствия

SNNet-9 встраивает governance-driven compliance в оркестратор. Новый объект
`compliance` в конфигурации Norito JSON фиксирует carve-outs, которые переводят
pipeline fetch в direct-only:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` объявляет коды ISO‑3166 alpha‑2, где работает эта
  инстанция оркестратора. Коды нормализуются в верхний регистр при парсинге.
- `jurisdiction_opt_outs` зеркалирует реестр governance. Когда любая юрисдикция
  оператора присутствует в списке, оркестратор применяет
  `transport_policy=direct-only` и излучает причину fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` перечисляет digest манифеста (blinded CID, в верхнем
  hex). Совпадающие payloads также форсят direct-only планирование и публикуют
  fallback `compliance_blinded_cid_opt_out` в телеметрии.
- `audit_contacts` записывает URI, которые governance ожидает увидеть в GAR
  playbooks операторов.
- `attestations` фиксирует подписанные compliance-пакеты, на которых держится
  политика. Каждая запись задает опциональную `jurisdiction` (ISO‑3166 alpha‑2),
  `document_uri`, канонический `digest_hex` (64 символа), timestamp выдачи
  `issued_at_ms` и опциональный `expires_at_ms`. Эти артефакты попадают в
  аудит-чеклист оркестратора, чтобы governance tooling связывал overrides с
  подписанными документами.

Передавайте блок compliance через стандартное наслаивание конфигурации, чтобы
операторы получали детерминированные overrides. Оркестратор применяет
compliance _после_ write-mode hints: даже если SDK запрашивает `upload-pq-only`,
opt-out по юрисдикции или манифесту всё равно переводит транспорт в direct-only
и быстро завершает ошибкой, если не осталось совместимых провайдеров.

Канонические каталоги opt-out находятся в
`governance/compliance/soranet_opt_outs.json`; Совет по governance публикует
обновления через tagged releases. Полный пример конфигурации (включая
attestations) доступен в `docs/examples/sorafs_compliance_policy.json`, а
операционный процесс описан в
[playbook соответствия GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Рычаги CLI и SDK

| Флаг / Поле | Эффект |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничивает, сколько провайдеров пройдут фильтр scoreboard. Установите `None`, чтобы использовать всех eligible провайдеров. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Ограничивает число ретраев на chunk. Превышение лимита вызывает `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Вкалывает snapshots латентности/сбоев в построитель scoreboard. Устаревшая телеметрия за пределами `telemetry_grace_secs` делает провайдеров неeligible. |
| `--scoreboard-out` | Сохраняет вычисленный scoreboard (eligible + ineligible провайдеры) для пост-анализа. |
| `--scoreboard-now` | Переопределяет timestamp scoreboard (Unix seconds), чтобы capture fixtures оставались детерминированными. |
| `--deny-provider` / hook политики score | Детерминированно исключает провайдеров из планирования без удаления adverts. Полезно для быстрого blacklisting. |
| `--boost-provider=name:delta` | Корректирует weighted round-robin кредиты провайдера, не меняя веса governance. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Маркирует метрики и структурированные логи, чтобы дашборды могли группировать по географии или волне rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | По умолчанию `soranet-first`, так как мульти-источниковый оркестратор — базовый. Используйте `direct-only` при downgrades или по директиве compliance, а `soranet-strict` оставьте для PQ-only пилотов; compliance overrides остаются жестким потолком. |

SoraNet-first теперь дефолт, а rollbacks должны ссылаться на соответствующий
SNNet blocker. После выпуска SNNet-4/5/5a/5b/6a/7/8/12/13 governance ужесточит
требуемую позу (в сторону `soranet-strict`); до этого только override по
инцидентам должны приоритизировать `direct-only`, и их нужно фиксировать в log
rollout.

Все флаги выше принимают синтаксис `--` как в `sorafs_cli fetch`, так и в
разработческом бинаре `sorafs_fetch`. SDK предоставляют те же опции через typed
builders.

### 1.4 Управление guard cache

CLI теперь подключает selector guards SoraNet, чтобы операторы могли
детерминированно закреплять entry relays до полноценного rollout SNNet-5.
Рабочий процесс контролируют три новых флага:

| Флаг | Назначение |
|------|-----------|
| `--guard-directory <PATH>` | Указывает JSON-файл с последним relay consensus (подмножество ниже). Передача directory обновляет guard cache перед fetch. |
| `--guard-cache <PATH>` | Сохраняет Norito-encoded `GuardSet`. Следующие прогоны используют cache, даже если новый directory не задан. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Опциональные overrides для числа entry guards (по умолчанию 3) и окна удержания (по умолчанию 30 дней). |
| `--guard-cache-key <HEX>` | Опциональный 32-байтовый ключ для тега guard cache с Blake3 MAC, чтобы файл можно было проверить перед повторным использованием. |

Payloads guard directory используют компактную схему:

Флаг `--guard-directory` теперь ожидает Norito-encoded payload
`GuardDirectorySnapshotV2`. Бинарный snapshot содержит:

- `version` — версия схемы (сейчас `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  метаданные consensus, которые должны совпадать с каждым встроенным сертификатом.
- `validation_phase` — gate политики сертификатов (`1` = разрешить одну Ed25519
  подпись, `2` = предпочесть двойные подписи, `3` = требовать двойные подписи).
- `issuers` — эмитенты governance с `fingerprint`, `ed25519_public` и
  `mldsa65_public`. Fingerprint вычисляется как
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — список SRCv2 bundles (выход `RelayCertificateBundleV2::to_cbor()`).
  Каждый bundle содержит descriptor relay, capability flags, политику ML-KEM и
  двойные подписи Ed25519/ML-DSA-65.

CLI проверяет каждый bundle против объявленных ключей issuer перед объединением
snapshots.

Вызывайте CLI с `--guard-directory`, чтобы объединить актуальный consensus с
существующим cache. Selector сохраняет закрепленные guards, которые еще в окне
удержания и допустимы в directory; новые relays заменяют просроченные записи.
После успешного fetch обновленный cache записывается по пути `--guard-cache`,
обеспечивая детерминированность следующих сессий. SDK воспроизводят поведение,
вызывая `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
и передавая полученный `GuardSet` в `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` позволяет selector приоритизировать PQ-guards во время
rollout SNNet-5. Stage toggles (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) теперь автоматически понижают классические relays: когда
доступен PQ guard, selector сбрасывает лишние classical pins, чтобы последующие
сессии предпочитали гибридные handshakes. CLI/SDK summaries показывают итоговый
микс через `anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` и связанные поля кандидатов/дефицита/дельт supply,
делая brownouts и classical fallbacks явными.

Guard directories теперь могут содержать полный SRCv2 bundle через
`certificate_base64`. Оркестратор декодирует каждый bundle, повторно проверяет
подписи Ed25519/ML-DSA и сохраняет разобранный сертификат вместе с guard cache.
Когда сертификат присутствует, он становится каноническим источником PQ keys,
настроек handshakes и весов; просроченные сертификаты отбрасываются, и selector
жизненным циклом circuit и доступны через `telemetry::sorafs.guard` и
`telemetry::sorafs.circuit`, фиксируя окно валидности, handshake suites и
наличие двойных подписей для каждого guard.

Используйте CLI helpers, чтобы держать snapshots синхронизированными с
публикаторами:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` скачивает и валидирует SRCv2 snapshot перед записью на диск, а `verify`
повторяет pipeline валидации для артефактов из других команд, выдавая JSON
summary, который зеркалит output guard selector CLI/SDK.

### 1.5 Менеджер жизненного цикла circuit

Когда доступны и relay directory, и guard cache, оркестратор активирует circuit
lifecycle manager для предварительного построения и обновления SoraNet circuits
перед каждым fetch. Конфигурация находится в `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) через два новых поля:

- `relay_directory`: хранит SNNet-3 directory snapshot, чтобы middle/exit hops
  выбирались детерминированно.
- `circuit_manager`: опциональная конфигурация (включена по умолчанию),
  контролирующая TTL цепей.

Norito JSON теперь принимает блок `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK передают данные directory через
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), а CLI подключает их автоматически, когда
передан `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Менеджер обновляет circuits, когда меняются метаданные guard (endpoint, PQ key
или pinned timestamp) или истекает TTL. Хелпер `refresh_circuits`, вызываемый
перед каждым fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`), эмитит логи
`CircuitEvent`, позволяя операторам отслеживать решения жизненного цикла. Soak
тест `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) демонстрирует стабильную
латентность на трех rotations guards; смотрите отчет в
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Локальный QUIC-прокси

Оркестратор может опционально запускать локальный QUIC-прокси, чтобы браузерные
расширения и SDK адаптеры не управляли сертификатами или guard cache keys. Прокси
слушает loopback-адрес, завершает QUIC соединения и возвращает Norito manifest,
описывающий сертификат и опциональный guard cache key. Transport события,
эмитируемые прокси, учитываются в `sorafs_orchestrator_transport_events_total`.

Включите прокси через новый блок `local_proxy` в JSON оркестратора:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` задает адрес прослушивания (используйте порт `0` для эфемерного
  порта).
- `telemetry_label` распространяется в метриках, чтобы дашборды различали прокси
  и fetch-сессии.
- `guard_cache_key_hex` (опционально) позволяет прокси отдавать тот же keyed
  guard cache, что используют CLI/SDK, чтобы браузерные расширения оставались
  синхронизированными.
- `emit_browser_manifest` включает выдачу manifest, который расширения могут
  сохранять и проверять.
- `proxy_mode` выбирает, будет ли прокси мостить трафик локально (`bridge`) или
  только отдавать метаданные, чтобы SDK открывали SoraNet circuits сами
  (`metadata-only`). По умолчанию `bridge`; используйте `metadata-only`, если
  рабочая станция должна выдавать manifest без ретрансляции потоков.
- `prewarm_circuits`, `max_streams_per_circuit` и `circuit_ttl_hint_secs`
  передают браузеру дополнительные hints, чтобы он мог бюджетировать параллельные
  потоки и понимать агрессивность reuse circuits.
- `car_bridge` (опционально) указывает на локальный cache CAR-архивов. Поле
  `extension` задает суффикс, добавляемый когда target не содержит `*.car`; задайте
  `allow_zst = true` для прямой выдачи `*.car.zst`.
- `kaigi_bridge` (опционально) экспонирует Kaigi routes из spool в прокси. Поле
  `room_policy` объявляет режим `public` или `authenticated`, чтобы браузерные
  клиенты заранее выбирали корректные GAR labels.
- `sorafs_cli fetch` предоставляет overrides `--local-proxy-mode=bridge|metadata-only`
  и `--local-proxy-norito-spool=PATH`, позволяя менять runtime-режим или указывать
  альтернативные spools без изменения JSON-политики.
- `downgrade_remediation` (опционально) настраивает автоматический downgrade hook.
  Когда включено, оркестратор следит за telemetry relays для всплесков downgrade и,
  после превышения `threshold` в окне `window_secs`, принудительно переводит прокси
  в `target_mode` (по умолчанию `metadata-only`). Когда downgrades прекращаются,
  прокси возвращается к `resume_mode` после `cooldown_secs`. Используйте массив
  `modes`, чтобы ограничить триггер конкретными ролями relays (по умолчанию entry relays).

Когда прокси работает в bridge-режиме, он обслуживает два приложения:

- **`norito`** — stream target клиента разрешается относительно
  `norito_bridge.spool_dir`. Targets санитизируются (без traversal, без
  абсолютных путей), и если файл без расширения, применяется настроенный суффикс
  до отправки payload в браузер.
- **`car`** — stream targets разрешаются внутри `car_bridge.cache_dir`, наследуют
  дефолтное расширение и отклоняют сжатые payloads, если `allow_zst` не включён.
  Успешный bridge отвечает `STREAM_ACK_OK` перед передачей байтов архива, чтобы
  клиенты могли pipeline verification.

В обоих случаях прокси предоставляет HMAC cache-tag (если guard cache key был во
время handshake) и записывает `norito_*` / `car_*` reason codes, чтобы дашборды
различали успехи, отсутствие файлов и ошибки санитизации.

`Orchestrator::local_proxy().await` раскрывает handle для чтения PEM
сертификата, получения browser manifest или корректного завершения при выходе
приложения.

Когда прокси включен, он теперь отдает записи **manifest v2**. Помимо
существующих сертификата и guard cache key, v2 добавляет:

- `alpn` (`"sorafs-proxy/1"`) и массив `capabilities`, чтобы клиенты знали
  протокол потока.
- `session_id` на handshake и `cache_tagging` salt block для derivation
  session guard affinity и HMAC tags.
- Hints по circuit и guard selection (`circuit`, `guard_selection`,
  `route_hints`) для более богатого UI до открытия потоков.
- `telemetry_v2` с knobs сэмплинга и приватности для локальной инструментации.
- Каждый `STREAM_ACK_OK` включает `cache_tag_hex`. Клиенты отражают это значение
  в заголовке `x-sorafs-cache-tag` при HTTP/TCP запросах, чтобы cached guard
  selections оставались зашифрованными на диске.

Эти поля обратно совместимы — старые клиенты могут игнорировать новые ключи и
продолжать использовать v1 subset.

## 2. Семантика отказов

Оркестратор применяет строгие проверки возможностей и бюджетов до передачи
первого байта. Отказы делятся на три категории:

1. **Отказы по eligible (pre-flight).** Провайдеры без range capability,
   просроченные adverts или устаревшая телеметрия фиксируются в scoreboard
   артефакте и не попадают в планирование. CLI summaries заполняют массив
   `ineligible_providers` причинами, чтобы операторы могли увидеть drift
   governance без парсинга логов.
2. **Runtime exhaustion.** Каждый провайдер отслеживает последовательные ошибки.
   Когда достигается `provider_failure_threshold`, провайдер помечается как
   `disabled` до конца сессии. Если все провайдеры стали `disabled`, оркестратор
   возвращает `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Детерминированные прерывания.** Жесткие лимиты поднимаются как структурные
   ошибки:
   - `MultiSourceError::NoCompatibleProviders` — манифест требует span/alignment,
     который оставшиеся провайдеры не могут соблюсти.
   - `MultiSourceError::ExhaustedRetries` — исчерпан budget ретраев на chunk.
   - `MultiSourceError::ObserverFailed` — downstream observers (streaming hooks)
     отклонили проверенный chunk.

Каждая ошибка содержит индекс проблемного chunk и, когда доступно, финальную
причину отказа провайдера. Считайте эти ошибки release blockers — повторные
попытки с тем же input воспроизведут сбой, пока advert, телеметрия или здоровье
провайдера не изменятся.

### 2.1 Сохранение scoreboard

При настройке `persist_path` оркестратор записывает финальный scoreboard после
каждого прогона. JSON документ содержит:

- `eligibility` (`eligible` или `ineligible::<reason>`).
- `weight` (нормализованный вес, назначенный для этого прогона).
- метаданные `provider` (идентификатор, endpoints, бюджет параллелизма).

Архивируйте snapshots scoreboard вместе с release артефактами, чтобы решения по
blacklist и rollout оставались проверяемыми.

## 3. Телеметрия и отладка

### 3.1 Метрики Prometheus

Оркестратор эмитит следующие метрики через `iroha_telemetry`:

| Метрика | Labels | Описание |
|---------|--------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge активных fetch-операций. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма полной латентности fetch. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Счетчик финальных отказов (исчерпаны ретраи, нет провайдеров, ошибка observer). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Счетчик попыток ретраев по провайдерам. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Счетчик провайдерских отказов на уровне сессии, приводящих к отключению. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Число решений политики анонимности (выполнено vs brownout) по стадиям rollout и причинам fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Гистограмма доли PQ relays среди выбранного набора SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Гистограмма доли PQ relays в snapshot scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Гистограмма дефицита политики (разница между целью и фактической долей PQ). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Гистограмма доли классических relays в каждой сессии. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Гистограмма числа выбранных классических relays в сессии. |

Интегрируйте метрики в staging dashboards до включения production knobs.
Рекомендуемая раскладка повторяет план наблюдаемости SF-6:

1. **Active fetches** — алерт, если gauge растет без соответствующих completions.
2. **Retry ratio** — предупреждает при превышении исторических baselines по `retry`.
3. **Provider failures** — триггерит pager alerts, когда любой провайдер превышает
   `session_failure > 0` в пределах 15 минут.

### 3.2 Structured log targets

Оркестратор публикует структурированные события в детерминированные targets:

- `telemetry::sorafs.fetch.lifecycle` — маркеры `start` и `complete` с числом
  chunks, ретраев и общей длительностью.
- `telemetry::sorafs.fetch.retry` — события ретраев (`provider`, `reason`,
  `attempts`) для ручного triage.
- `telemetry::sorafs.fetch.provider_failure` — провайдеры, отключенные из-за
  повторяющихся ошибок.
- `telemetry::sorafs.fetch.error` — финальные отказы с `reason` и опциональными
  метаданными провайдера.

Направляйте эти потоки в существующий Norito log pipeline, чтобы у incident
response был единый источник истины. Lifecycle события показывают PQ/classical
mix через `anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` и связанные счетчики, что упрощает настройку
дашбордов без парсинга метрик. Во время GA rollouts держите уровень логов
`info` для lifecycle/retry событий и используйте `warn` для terminal errors.

### 3.3 JSON summaries

`sorafs_cli fetch` и Rust SDK возвращают структурированный summary, содержащий:

- `provider_reports` с числами успехов/сбоев и статусом отключения провайдера.
- `chunk_receipts`, показывающий какой провайдер обслужил каждый chunk.
- массивы `retry_stats` и `ineligible_providers`.

Архивируйте summary при отладке проблемных провайдеров — receipts напрямую
соотносятся с лог-метаданными выше.

## 4. Операционный чеклист

1. **Задеплойте конфигурацию в CI.** Запустите `sorafs_fetch` с целевой
   конфигурацией, передайте `--scoreboard-out` для фиксации eligibility-view и
   сравните с предыдущим release. Любой неожиданный ineligible провайдер
   блокирует промоут.
2. **Проверьте телеметрию.** Убедитесь, что деплой экспортирует метрики
   `sorafs.fetch.*` и структурированные логи перед включением multi-source fetch
   для пользователей. Отсутствие метрик обычно означает, что фасад оркестратора
   не был вызван.
3. **Документируйте overrides.** При emergency `--deny-provider` или
   `--boost-provider` зафиксируйте JSON (или CLI вызов) в changelog. Rollbacks
   должны отменить override и снять новый scoreboard snapshot.
4. **Повторите smoke tests.** После изменения budgets ретраев или caps
   провайдеров заново выполните fetch канонического fixture
   (`fixtures/sorafs_manifest/ci_sample/`) и убедитесь, что receipts по chunks
   остаются детерминированными.

Следование шагам выше делает поведение оркестратора воспроизводимым в staged
rollouts и предоставляет телеметрию для incident response.

### 4.1 Overrides политики

Операторы могут закрепить активный transport/anonymity этап без изменения базовой
конфигурации, задав `policy_override.transport_policy` и
`policy_override.anonymity_policy` в JSON `orchestrator` (или передав
`--transport-policy-override=` / `--anonymity-policy-override=` в
`sorafs_cli fetch`). Если override присутствует, оркестратор пропускает обычный
brownout fallback: если требуемый PQ tier недостижим, fetch завершается с
`no providers` вместо тихого downgrade. Возврат к поведению по умолчанию —
простое очищение override полей.
