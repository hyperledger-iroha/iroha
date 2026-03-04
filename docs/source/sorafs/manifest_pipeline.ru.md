---
lang: ru
direction: ltr
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2026-01-30
---

# Чанкинг SoraFS → Pipeline манифестов

Эта заметка фиксирует минимальные шаги, необходимые для преобразования байтового payload
в манифест Norito, пригодный для пиннинга в реестре SoraFS.

1. **Детерминированно чанкуйте payload**
   - Используйте `sorafs_car::CarBuildPlan::single_file` (внутри использует chunker SF-1),
     чтобы получить смещения чанков, длины и BLAKE3-дайджесты.
   - План раскрывает дайджест payload и метаданные чанков, которые downstream-инструменты
     могут переиспользовать для сборки CAR и планирования Proof-of-Replication.
   - В качестве альтернативы прототип `sorafs_car::ChunkStore` принимает байты и
     записывает детерминированные метаданные чанков для последующей сборки CAR.
     Store теперь выводит PoR-дерево выборки 64 KiB / 4 KiB (с доменным тегом,
     выровненное по чанкам), чтобы планировщики могли запрашивать Merkle-доказательства
     без повторного чтения payload.
    Используйте `--por-proof=<chunk>:<segment>:<leaf>`, чтобы вывести JSON-свидетель для
    выбранного листа, и `--por-json-out`, чтобы записать снимок корневого дайджеста для
    последующей проверки. Сочетайте `--por-proof` с `--por-proof-out=path`, чтобы
    сохранить свидетельство, и используйте `--por-proof-verify=path`, чтобы подтвердить,
    что существующее доказательство соответствует вычисленному `por_root_hex` для текущего
    payload. Для нескольких листьев `--por-sample=<count>` (с опциональными
    `--por-sample-seed` и `--por-sample-out`) генерирует детерминированные выборки и
    выставляет `por_samples_truncated=true`, когда запрос превышает доступные листья.
   - Сохраняйте смещения/длины/дайджесты чанков, если планируете строить bundle-доказательства
     (CAR-манифесты, расписания PoR).
   - Смотрите [`sorafs/chunker_registry.md`](chunker_registry.md) для канонических записей
     реестра и рекомендаций по переговорам.

2. **Обернуть манифест**
   - Передайте метаданные чанкинга, корневой CID, коммитменты CAR, политику pin, alias-claims
     и подписи governance в `sorafs_manifest::ManifestBuilder`.
   - Вызовите `ManifestV1::encode`, чтобы получить Norito-байты, и
     `ManifestV1::digest`, чтобы получить канонический дайджест, записываемый в Pin
     Registry.

3. **Публикация**
   - Отправьте дайджест манифеста через governance (подпись совета, alias-доказательства)
     и закрепите байты манифеста в SoraFS с использованием детерминированного pipeline.
   - Убедитесь, что файл CAR (и опциональный индекс CAR), на который ссылается манифест,
     хранится в том же наборе SoraFS pins.

### Быстрый старт CLI

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

Команда печатает дайджесты чанков и детали манифеста; когда переданы `--manifest-out`
и/или `--car-out`, она записывает Norito-payload и CARv2-архив, соответствующий
спецификации (pragma + header + блоки CARv1 + индекс Multihash), на диск. Если передать
путь к директории, инструмент проходит её рекурсивно (лексикографический порядок),
чанкует каждый файл и формирует дерево dag-cbor, корневой CID которого указан и в
манифесте, и в CAR. JSON-отчёт включает вычисленный дайджест CAR payload, полный дайджест
архива, размер, raw CID и корень (с выделением dag-cbor codec), а также alias/metadata
записи манифеста. Используйте `--root-cid`/`--dag-codec`, чтобы *проверить* корень или
codec в CI, `--car-digest` — чтобы принудительно задать hash payload, `--car-cid` — чтобы
зафиксировать заранее вычисленный raw CAR идентификатор (CIDv1, codec `raw`, multihash
BLAKE3), и `--json-out` — чтобы сохранить напечатанный JSON рядом с артефактами
манифеста/CAR для downstream-автоматизации.

Когда задан `--manifest-signatures-out` (и как минимум один флаг `--council-signature*`),
инструмент также пишет конверт `manifest_signatures.json`, содержащий BLAKE3-дайджест
манифеста, агрегированный SHA3-256 дайджест плана чанков (смещения, длины и BLAKE3-дайджесты
чанков) и предоставленные подписи совета. Конверт теперь фиксирует профиль chunker в
каноническом виде `namespace.name@semver`; старые конверты `namespace-name` продолжают
проходить проверку ради совместимости. Downstream-автоматизация может публиковать конверт
в логах governance или распространять его вместе с артефактами манифеста и CAR. Когда вы
получаете конверт от внешнего подписанта, добавьте `--manifest-signatures-in=<path>`, чтобы
CLI подтвердила дайджесты и проверила каждую подпись Ed25519 относительно только что
вычисленного дайджеста манифеста.

Если зарегистрировано несколько профилей chunker, вы можете выбрать один явно через
`--chunker-profile-id=<id>`. Флаг сопоставляется с числовыми идентификаторами в
[`chunker_registry`](chunker_registry.md) и гарантирует, что и проход чанкинга, и выданный
манифест ссылаются на одну и ту же `(namespace, name, semver)`-кортеж. Предпочитайте
каноническую форму handle в автоматизации (`--chunker-profile=sorafs.sf1@1.0.0`), чтобы не
фиксировать числовые ID. Запустите `sorafs_manifest_chunk_store --list-profiles`, чтобы
увидеть актуальные записи реестра (вывод повторяет список, предоставленный
`sorafs_manifest_chunk_store`), или используйте `--promote-profile=<handle>` для экспорта
канонического handle и alias-метаданных при подготовке обновления реестра.

Аудиторы могут запросить полное дерево Proof-of-Retrievability через `--por-json-out=path`,
которое сериализует дайджесты chunk/segment/leaf для проверки выборки. Индивидуальные
свидетельства можно экспортировать через `--por-proof=<chunk>:<segment>:<leaf>` (и
валидировать через `--por-proof-verify=path`), а `--por-sample=<count>` генерирует
детерминированные, без дублей, выборки для точечных проверок.

Любой флаг, записывающий JSON (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`, и т. д.),
также принимает `-` в качестве пути, позволяя стримить payload напрямую в stdout без создания
временных файлов.

Используйте `--chunk-fetch-plan-out=path`, чтобы сохранить упорядоченную спецификацию fetch
чанков (индекс чанка, смещение payload, длина, BLAKE3-дайджест), сопровождающую план манифеста.
Клиенты с несколькими источниками могут подавать полученный JSON напрямую в SoraFS fetch
оркестратор без повторного чтения исходного payload. JSON-отчёт, печатаемый CLI, также
включает этот массив в `chunk_fetch_specs`. И секция `chunking`, и объект `manifest`
публикуют `profile_aliases` рядом с каноническим handle `profile`, чтобы SDK могли мигрировать
от устаревшей формы `namespace-name` без потери совместимости.

При повторном запуске stub (например, в CI или release-пайплайне) можно передать
`--plan=chunk_fetch_specs.json` или `--plan=-`, чтобы импортировать ранее сгенерированную
спецификацию. CLI проверяет, что индекс, смещение, длина и BLAKE3-дайджест каждого чанка
всё ещё совпадают со свежесформированным CAR-планом перед продолжением ingestion, что
защищает от устаревших или подменённых планов.

### Локальный smoke-test оркестрации

Crate `sorafs_car` теперь поставляет `sorafs-fetch` — CLI для разработчиков, который
потребляет массив `chunk_fetch_specs` и симулирует мульти-провайдерную выборку из
локальных файлов. Укажите JSON, сгенерированный `--chunk-fetch-plan-out`, задайте один или
несколько путей к payload провайдеров (опционально с `#N` для увеличения параллелизма), и
CLI проверит чанки, соберёт payload и выведет JSON-отчёт с итогами успехов/ошибок по
провайдерам и квитанциями по каждому чанку:

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

Используйте этот поток, чтобы проверить поведение оркестратора или сравнить payload
провайдеров до подключения реальных сетевых транспортов к узлу SoraFS.

Когда нужно обращаться к живому Torii gateway вместо локальных файлов, замените флаги
`--provider=/path` на новые HTTP-ориентированные параметры:

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

CLI проверяет stream token, обеспечивает соответствие chunker/profile и записывает
метаданные gateway вместе с обычными квитанциями провайдеров, чтобы операторы могли
архивировать отчёт как доказательство rollout (см. deployment handbook для полного
blue/green потока).

Если вы передаёте `--provider-advert=name=/path/to/advert.to`, CLI теперь декодирует
Norito-конверт, проверяет подпись Ed25519 и требует, чтобы провайдер объявлял способность
`chunk_range_fetch`. Это держит мульти-источниковую симуляцию fetch в рамках политики
допуска governance и предотвращает случайное использование устаревших провайдеров, которые
не умеют обслуживать запросы chunk range.

Суффикс `#N` увеличивает лимит параллелизма провайдера, а `@W` задаёт вес планирования
(по умолчанию 1 при отсутствии). Когда переданы adverts или дескрипторы gateway, CLI теперь
оценивает scoreboard оркестратора перед запуском fetch: допустимые провайдеры получают веса
с учётом телеметрии, а JSON-снимок сохраняется в `--scoreboard-out=<path>` при наличии.
Провайдеры, не прошедшие проверки возможностей или дедлайнов governance, автоматически
отбрасываются с предупреждением, чтобы запуски оставались в рамках политики допуска. См.
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` для примерной
пары вход/выход.

Передавайте `--expect-payload-digest=<hex>` и/или `--expect-payload-len=<bytes>`, чтобы
утверждать соответствие собранного payload ожиданиям манифеста перед записью выходов —
удобно для CI smoke-tests, когда нужно убедиться, что оркестратор не тихо удалил или не
переупорядочил чанки.

Если у вас уже есть JSON-отчёт, созданный `sorafs-manifest-stub`, передайте его напрямую
через `--manifest-report=docs.report.json`. Fetch CLI переиспользует встроенные поля
`chunk_fetch_specs`, `payload_digest_hex` и `payload_len`, поэтому не требуется отдельно
управлять файлами плана или валидации.

Fetch-отчёт также выводит агрегированную телеметрию для мониторинга:
`chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`,
`chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
`provider_failure_rate` и `provider_disabled_total` описывают общее состояние сессии
fetch и подходят для дашбордов Grafana/Loki или CI-ассертов. Используйте
`--provider-metrics-out`, чтобы записывать только массив `provider_reports`, если
downstream-инструментам нужны лишь статистики на уровне провайдера.

### Следующие шаги

- Фиксируйте CAR-метаданные рядом с дайджестами манифеста в логах governance, чтобы
  наблюдатели могли проверять содержимое CAR без повторной загрузки payload.
- Интегрируйте поток публикации манифеста и CAR в CI, чтобы каждый build docs/artefacts
  автоматически создавал манифест, получал подписи и пиннил результирующие payloads.
