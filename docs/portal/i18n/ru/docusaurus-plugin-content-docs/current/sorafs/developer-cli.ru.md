---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-cli
Название: Книга рецептов CLI SoraFS
Sidebar_label: Книга рецептов CLI
Описание: Практический разбор по задачам для консолидированной поверхности `sorafs_cli`.
---

:::note Канонический источник
:::

Консолидированная поверхность `sorafs_cli` (предоставляется ящик `sorafs_car` с включенной функцией `cli`) раскрывает каждый шаг, необходимые для подготовки документов SoraFS. Используйте эту кулинарную книгу, чтобы сразу перейти к типовым рабочим процессам; Объедините его с манифестом конвейера и книгами выполнения оркестратора для операционного контекста.

## Упаковка полезных данных

Используйте `car pack`, чтобы получить определенные CAR-архивы и чанки планов. Команда автоматически выбирает чанкёр СФ-1, если не указан дескриптор.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Стандартный чанкер с ручкой: `sorafs.sf1@1.0.0`.
- Входные каталоги обходят в лексикографическом порядке, чтобы контрольные суммы оставались стабильными между платформами.
- JSON-резюме включает дайджесты полезной нагрузки, метаданные по чанкам и корневой CID, распознаваемый реестром и оркестратором.

##Сборка манифестирует

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Опции `--pin-*` напрямую сопоставляются с полями `PinPolicy` в `sorafs_manifest::ManifestBuilder`.
- Укажите `--chunk-plan`, если хотите, чтобы CLI пересчитал дайджест SHA3 для чанка перед отправкой; В противном случае он использует дайджест из сводки.
- JSON-вывод отметки полезных данных Norito для удобных различий при просмотре.

## Подписание манифестов без долгоживущих ключей

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Принимает встроенные токены, переменные окружения или файловые источники.
- Добавляет метаданные происхождения (`token_source`, `token_hash_hex`, дайджест-чанк) без сохранения необработанного JWT, если не задано `--include-token=true`.
- Удобно для CI: взаимодействие с GitHub Actions OIDC, установленное `--identity-token-provider=github-actions`.

## Отправка манифестов в Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority soraカタカナ... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Выполняется Norito-декодирование псевдонимов и, наконец, соответствие дайджеста манифеста POST в Torii.
- Перечитать фрагмент дайджеста SHA3 согласно плану, чтобы предотвратить действие на несоответствие.
- Резюме ответа фиксируют статус HTTP, заголовки и реестр полезных данных для проведения аудита.

## Проверка ценности CAR и доказательства

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Перестраивает дерево PoR и сравнивает дайджесты полезной нагрузки с резюме манифеста.
- Фиксирует количество и идентификаторы, необходимые при отправке доказательств репликации в управлении.

## Потоковая телеметрия доказательства

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Генерирует элементы NDJSON для каждого переданного доказательства (отключите воспроизведение через `--emit-events=false`).
- Агрегирует количество успехов/ошибок, гистограммы латентности и выборочных сбои в сводном формате JSON, чтобы информационные панели могли строить графики без чтения журналов.
- Завершает работу с ненулевым кодом, когда шлюз сообщает об ошибках или локальная проверка PoR (через `--por-root-hex`) отклоняет доказательства. Настройте пороги через `--max-failures` и `--max-verification-failures` для повторений.
- Сегодняшние мероприятия PoR; PDP и PoTR переиспользуют тот же конверт после выхода SF-13/SF-14.
- `--governance-evidence-dir` записывает рендерированное резюме, метаданные (метка времени, версия CLI, URL-шлюз, дайджест-манифест) и резервирует манифест в указанную директорию, чтобы пакеты управления могли архивировать поток доказательств без повторного запуска.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — приводящая документация по флагам.
- `docs/source/sorafs_proof_streaming.md` — схема телеметрии доказательства и шаблон приборной панели Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — подробный разбор чанкинга, сборки манифеста и обработки CAR.