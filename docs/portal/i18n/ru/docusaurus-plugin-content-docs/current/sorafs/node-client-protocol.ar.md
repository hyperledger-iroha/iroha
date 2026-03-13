---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بروتوكول عقدة ↔ عميل SoraFS

В 2017 году он сказал:
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Установите восходящий поток Norito, чтобы получить доступ к исходному коду.
Вы можете использовать runbooks для создания резервных копий Runbook. SoraFS.

## إعلانات المزوّد والتحقق

يقوم مزوّدو SoraFS ببث حمولات `ProviderAdvertV1` (راجع
`crates/sorafs_manifest::provider_advert`).
Он сказал, что его отец и сын в настоящее время находятся в центре внимания. المصادر
أثناء التشغيل.

- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`. Нэнси Сул
  Началось 12 октября.
- **TLV القدرات** — Зарегистрируйте TLV в режиме ожидания (Torii, QUIC+Noise, ترحيلات
  SoraNet, امتدادات مزوّد). Он сказал:
  `allow_unknown_capabilities = true` используется для смазки.
- **Управление QoS** — طبقة `availability` (Горячий/Теплый/Холодный)
  حد التزامن, وميزانية стрим اختيارية. Обеспечивает качество обслуживания QoS в режиме онлайн.
  Он был убит в Уэльсе.
- **Конечные точки для рандеву** — Зарегистрируйтесь, чтобы получить доступ к TLS/ALPN.
  Он выступит в роли президента США, а также в Нью-Йорке в Сан-Франциско. حراسة.
- **Отключить разветвление** — `min_guard_weight` для разветвления AS/pool و
  `provider_failure_threshold` обеспечивает многоточечное соединение.
- **معرّفات الملف الشخصي** — يجب على المزوّدين نشر المقبض المعتمد (مثل)
  `sorafs.sf1@1.0.0`); تساعد `profile_aliases` Дополнительная информация
  ترحيل.

ترفض قواعد التحقق кола الصفري، и قوائم القدرات/العناوين/المواضيع الفارغة,
Кроме того, вы можете настроить качество обслуживания QoS. Тэхен и его сын Джон Джонс
Код (`compare_core_fields`) был установлен на сайте.

### امتدادات الجلب بالنطاقات

Вы можете получить информацию о том, как это сделать:

| حقل | غرض |
|-------|-------|
| `CapabilityType::ChunkRangeFetch` | Установите `max_chunk_span` и `min_granularity` для проверки/отключения. |
| `StreamBudgetV1` | Установите флажок/заголовок (`max_in_flight`, `max_bytes_per_sec`, و`burst`). يتطلب قدرة نطاق. |
| `TransportHintV1` | Установите флажок (например, `torii_http_range`, `quic_stream`, `soranet_relay`). Установите `0–15` и установите его. |

Ответ:

- В эфире стрима "Вечеринка" в рамках трансляции وتلميحات
  Он сказал, что это будет так.
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصادر مع
  светильники للخفض تحت `fixtures/sorafs_manifest/provider_admission/`.
- تُرفض الإعلانات الداعمة للنطاق التي تُسقط `stream_budget` или `transport_hints`
  Создайте интерфейс CLI/SDK для работы с несколькими исходными кодами.
  Код: Torii.

## نقاط نهاية نطاقات البوابة

Вы можете подключиться к HTTP-серверу с помощью веб-сайта.

### `GET /v2/sorafs/storage/car/{manifest_id}`| المتطلب | تفاصيل |
|---------|----------|
| **Заголовки** | `Range` (отправлено в центральный офис), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` используется и `X-SoraFS-Stream-Token` base64. |
| **Ответы** | `206` и `Content-Type: application/vnd.ipld.car`, а также `Content-Range` в случае необходимости в `X-Sora-Chunk-Range`. Используется для создания чанка/токена. |
| **Режимы отказа** | `416` Защитный экран `401` `429` вызывает поток/байт. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Он прокомментировал ситуацию с Дайджестом новостей. مفيد لإعادة
Он был отправлен в автосалон по автомобилю CAR.

## سير عمل المُنسِّق متعدد المصادر

Создан для SF-6 в версии (CLI Rust عبر `sorafs_fetch`, и SDKs عبر
`sorafs_orchestrator`):

1. **Дизайн ** — в фильме «Старый мир», Уилсон, штат Калифорния.
   Установите флажок (`--telemetry-json` или `TelemetrySnapshot`).
2. **табло табло** — يقوم `Orchestrator::build_scoreboard` بتقييم الأهلية
   وتسجيل أسباب الرفض؛ Загрузите `sorafs_fetch --scoreboard-out` в формате JSON.
3. **Отключить соединение** — `fetch_with_scoreboard` (или `--plan`) в случае необходимости.
   Поток потока, созданный для просмотра/воспроизведения (`--retry-budget`, `--max-peers`)
   Токен потока будет доступен для скачивания.
4. **Установить من الإيصالات** — تشمل المخرجات `chunk_receipts` و`provider_reports`;
   Откройте CLI для `provider_reports` и `chunk_receipts` и `ineligible_providers`.
   لحزم الأدلة.

Доступ к файлам/SDK:

| خطأ | الوصف |
|-------|-------|
| `no providers were supplied` | Это было сделано для того, чтобы сделать это. |
| `no compatible providers available for chunk {index}` | Он был убит в 1980-х годах. |
| `retry budget exhausted after {attempts}` | Это `--retry-budget`, а также дополнительная информация. |
| `no healthy providers remaining` | Он был убит Биллом Пэнсоном в 2007 году. |
| `streaming observer failed` | Это автомобиль CAR в Нью-Йорке. |
| `orchestrator invariant violated` | Откройте таблицу и табло, чтобы просмотреть файлы JSON и использовать CLI. |

## Справочные материалы

- Сообщение от источника:  
  И18НИ00000077Х, И18НИ00000078Х,
  И18НИ00000079Х, И18НИ00000080Х
  (в зависимости от манифеста/региона/поставщика). اضبط `telemetry_region` в الإعداد أو عبر
  Откройте интерфейс командной строки (CLI) и откройте его.
- Отображение результатов в CLI/SDK и табло JSON.
  Он был отправлен в США в рамках проекта SF-6/SF-7.
- تكشف معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  В 1990-х годах он был выбран SRE в Вашингтоне.

## Добавление CLI и REST

- `iroha app sorafs pin list|show` и `alias list` и `replication list` для REST
  Закрепите контакты Norito JSON для проверки подлинности.
- `iroha app sorafs storage pin` и `torii /v2/sorafs/pin/register` манифестирует
  Создайте Norito в формате JSON для доказательства псевдонима и преемника; تؤدي доказательства
  Найдите `400`, проверьте доказательства `503` и `Warning: 110`, нажмите на него.
  доказательства المنتهية تمامًا `412`.
- Нет REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  تتضمن هياكل аттестация حتى يتمكن العملاء من التحقق من البيانات مقابل أحدث
  Он сказал, что это не так.## المراجع

- Введение:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Добавлено Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Интерфейс CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Имя пользователя: `crates/sorafs_orchestrator`.
- Дополнительная информация: `dashboards/grafana/sorafs_fetch_observability.json`.