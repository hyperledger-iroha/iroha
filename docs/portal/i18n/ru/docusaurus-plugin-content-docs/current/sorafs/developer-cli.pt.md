---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-cli
заголовок: Получение CLI от SoraFS
Sidebar_label: Получение интерфейса командной строки
описание: Помогите найти дополнительную информацию о консолидации `sorafs_cli`.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/developer/cli.md`. Мантенья представился как копиас синхронизадас.
:::

Поверхностная консолидация `sorafs_cli` (ящик для кожи `sorafs_car` с привычным рекурсивным курсом `cli`) показывает каждый этап, необходимый для подготовки артефатов из SoraFS. Используйте эту кулинарную книгу непосредственно для общих рабочих процессов; Объединение конвейера манифеста и модулей Runbook ОС для выполнения контекстных операций.

## Полезные нагрузки Empacotar

Используйте `car pack` для создания определенных файлов CAR и планов фрагментов. Если команда автоматического выбора или блокировщика SF-1, то вам нужно будет справиться с этим.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Ручка блока управления: `sorafs.sf1@1.0.0`.
- Вводы директорий выполняются в порядке лексикографии для того, чтобы контрольные суммы оставались на платформах.
- В резюме JSON, включая дайджесты, содержатся полезные данные, метаданные для фрагментов и CID, которые позволяют повторно согласовать реестр и организатора.

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

- Используйте `--pin-*`, отображаемый непосредственно на территории `PinPolicy` в `sorafs_manifest::ManifestBuilder`.
- Forneca `--chunk-plan`, когда пересчет CLI или дайджест SHA3 делают фрагмент перед отправкой; В противном случае повторное использование или переваривание невозможных результатов.
- Указан JSON или полезная нагрузка Norito для простых различий во время внесения изменений.

## Ассинар проявляет себя как длинная длинная дюрасао

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Встроенные жетоны Aceita, различные варианты окружения или шрифты, основанные в архивах.
- Добавление метаданных процесса (`token_source`, `token_hash_hex`, дайджест фрагмента) остается неизменным или грубо JWT, а также `--include-token=true`.
— Функция в CI: объедините com OIDC с действиями GitHub, определенными `--identity-token-provider=github-actions`.

## Enviar манифестирует пункт Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Фаз декодирования Norito для подтверждения псевдонимов и проверки соответствия и дайджеста, чтобы манифестировать перед отправкой через POST или Torii.
- Пересчет или дайджест SHA3 составят часть плана, чтобы предотвратить атаки расхождения.
- Резюме ответа захватывают статус HTTP, заголовки и полезные данные регистрируются для задней аудитории.

## Проверка содержания CAR и доказательств

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Восстановить массив PoR и сравнить дайджесты с полезной нагрузкой или резюме с манифестом.
- Captura contagens e identificadores exigidos или передать доказательства репликации для управления.

## Передача телеметрии доказательств

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Выделите NDJSON для надежной передачи (нежелательно или воспроизводите с помощью `--emit-events=false`).
- Совокупность результатов и результатов, гистограмм задержек и результатов в формате JSON для того, чтобы панели мониторинга могли строить результаты в журналах.
- Скажите, как будет получен нулевой код, когда будет получен отчет о шлюзе или когда будет подтверждена локальная проверка PoR (через `--por-root-hex`) с подтверждением. Отрегулируйте границы `--max-failures` и `--max-verification-failures` для выполнения задач.
- Поддержка PoR hoje; PDP и PoTR можно повторно использовать или использовать конверт, когда используется SF-13/SF-14.
- `--governance-evidence-dir` предоставляет результаты рендеринга, метаданные (временная метка, версия CLI, URL-адрес шлюза, дайджест манифеста) и копия манифеста, не указывающая путь к архиву документов, подтверждающих поток доказательств, которые можно повторить и выполнить.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` - дополнительная документация по флагам.
- `docs/source/sorafs_proof_streaming.md` - задача телеметрии доказательств и шаблон приборной панели Grafana.
- `docs/source/sorafs/manifest_pipeline.md` - глубокое разделение на фрагменты, составление манифеста и управление CAR.