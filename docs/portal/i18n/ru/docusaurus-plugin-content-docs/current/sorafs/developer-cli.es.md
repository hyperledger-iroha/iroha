---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-cli
заголовок: Получение CLI от SoraFS
Sidebar_label: Получение CLI
описание: Запись ориентирована на консолидацию поверхности `sorafs_cli`.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/developer/cli.md`. Mantén ambas copyas sincronizadas.
:::

Консолидированная поверхность `sorafs_cli` (пропорциональная корпусу `sorafs_car` с привычной функцией `cli`) показана каждый раз, когда необходимо подготовить артефакты SoraFS. США este Recetario Para Saltar Directamente Flujos Comunes; объединение с конвейером манифеста и Runbooks Orquestador для оперативного контекста.

## Полезные нагрузки Empaquetar

США `car pack` для создания архивов CAR, определенных и фрагментов плоскостей. Команда автоматического выбора чанера SF-1 будет соответствовать ручке.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Предопределенный дескриптор чанкера: `sorafs.sf1@1.0.0`.
- Вводы директорий повторяются в лексикографическом порядке, чтобы контрольные суммы были установлены на платформах.
- Возобновленный JSON включает в себя дайджесты полезной нагрузки, метаданные для фрагментов и CID для получения информации о реестре и орвесторе.

## Конструировать манифесты

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Опции `--pin-*` назначаются непосредственно в кампус `PinPolicy` и `sorafs_manifest::ManifestBuilder`.
- Usa `--chunk-plan`, когда требуется, чтобы CLI пересчитал дайджест SHA3 фрагмента перед отправкой; в противном случае повторно используйте дайджест, вставленный в резюме.
- Сброс JSON отражает полезную нагрузку Norito для простых различий во время последних изменений.

## Firmar проявляет грех claves de larga duración

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Встроенные токены Acepta, переменные или файлы, хранящиеся в архивах.
- Добавление метаданных обработки (`token_source`, `token_hash_hex`, дайджест фрагмента) без сохранения JWT в грубом залпе, как `--include-token=true`.
- Хорошая функция CI: комбинация с OIDC в GitHub Actions, настроенная `--identity-token-provider=github-actions`.

## Enviar манифестирует Torii

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

- Реализовать декодификацию Norito для доказательства псевдонима и проверки совпадения с дайджестом манифеста перед POST. Сохраните Torii.
- Пересчитать фрагмент SHA3 дайджеста из плана, чтобы предотвратить атаки по дестабилизации.
- Захваченные ответы на запросы HTTP, заголовки и полезные данные реестра для задних аудиторий.

## Проверка содержания CAR и доказательств

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Реконструировать сообщение PoR и сравнить дайджесты полезной нагрузки с резюме манифеста.
- Captura conteos e identificadores requeridos al enviar доказательства де репликации губернатора.

## Передатчик телеметрии доказательств

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Создайте элементы NDJSON для проверки передачи (отключите повтор с `--emit-events=false`).
- Объединение данных об исходах/попаданиях, гистограмм задержек и введенных данных в возобновленном JSON для того, чтобы панели мониторинга могли отображать графические результаты из журналов.
- Продажа с отдельным кодом сертификата, когда отчет о шлюзе падает или локальная проверка PoR (через `--por-root-hex`) требует повторных доказательств. Отрегулируйте тени с `--max-failures` и `--max-verification-failures` для выброса жидкости.
- Сопорта ПоР хой; PDP и PoTR повторно используют устройство, когда его соединяют с SF-13/SF-14.
- `--governance-evidence-dir` записывает резюме рендеринга, метаданные (метка времени, версия CLI, URL-адрес шлюза, дайджест манифеста) и копию манифеста в указанном каталоге, чтобы пакеты управления архивировались для доказательства потока доказательств без повторного выброса.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — исчерпывающая документация по флагам.
- `docs/source/sorafs_proof_streaming.md` — программа телеметрии доказательств и установка приборной панели Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — углубленное разбиение на фрагменты, составление манифеста и управление CAR.