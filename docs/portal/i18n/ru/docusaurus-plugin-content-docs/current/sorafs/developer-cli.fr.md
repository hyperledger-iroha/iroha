---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-cli
Название: Recettes CLI SoraFS
Sidebar_label: Recettes CLI
описание: Parcours Orienté Tâches de La Surface Consolidée `sorafs_cli`.
---

:::note Источник канонический
:::

Консолидированная поверхность `sorafs_cli` (четыре ящика `sorafs_car` с активированной функцией `cli`) обнажить необходимый этап для подготовки артефактов SoraFS. Используйте эту кулинарную книгу для всех направлений рабочих процессов; associez-le au Pipeline de Manifest и aux runbooks de l'orchestrateur pour le contexte Operationnel.

## Упаковать полезные данные

Используйте `car pack` для создания архивов CAR déterministes и планов фрагментов. Автоматический выбор команды для измельчителя SF-1 будет осуществляться с помощью четырехручного устройства.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Ручка блока по умолчанию: `sorafs.sf1@1.0.0`.
- Лесные входы в репертуар находятся в парках и в лексикографическом порядке, где контрольные суммы остаются стабильными между пластинчатыми формами.
- Резюме JSON включает в себя дайджесты полезной нагрузки, метадонные фрагменты и сбор данных CID для регистрации и оркестрации.

## Создание манифестов

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Опции `--pin-*` отображаются в направлении полей `PinPolicy` в `sorafs_manifest::ManifestBuilder`.
- Fournissez `--chunk-plan` lorsque vous souhaitez que le CLI пересчитывает дайджест SHA3 de chunk avant soumission ; просто повторно используйте полный дайджест в резюме.
- Вылазка JSON отражает полезную нагрузку Norito для простых различий в обзорах.

## Подписание манифестов без длительного срока действия

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Принимайте встроенные токены, переменные среды или источники, основанные на документах.
- Добавлены метадоны происхождения (`token_source`, `token_hash_hex`, дайджест фрагмента) без сохранения JWT brut sauf si `--include-token=true`.
- Функция bien en CI: объединение с действиями GitHub OIDC и определенным `--identity-token-provider=github-actions`.

## Сообщения о манифестах в Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Выполните декодирование Norito псевдонимов и проверите их, соответствующие дайджесту манифеста перед POST версии Torii.
- Пересчитайте дайджест SHA3 фрагмента плана, чтобы избежать несоответствий.
- Резюме ответов фиксируют статус HTTP, заголовки и полезные данные регистрации для дальнейшего аудита.

## Проверка содержания CAR и доказательств

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Восстановите массив PoR и сравните дайджесты полезной нагрузки с резюме манифеста.
- Соберите данные и идентификаторы, необходимые для получения доказательств репликации в целях управления.

## Диффузор телеметрии доказательств

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- Включите элементы NDJSON для потокового воспроизведения (отключите воспроизведение с `--emit-events=false`).
- Соберите успешные результаты/проверку, гистограммы задержки и повторяющиеся проверки в резюме JSON, чтобы панели мониторинга могли отслеживать результаты без проверки журналов.
- Выйдите с ненулевым кодом или сигналом шлюза проверки или локалью проверки PoR (через `--por-root-hex`) и отмените подтверждение. Отрегулируйте значения с `--max-failures` и `--max-verification-failures` для повторения прогонов.
- Поддержка PoR aujourd'hui ; PDP и PoTR повторно используют папку для SF-13/SF-14 на месте.
- `--governance-evidence-dir` écrit le резюме, les métadonnées (временная метка, версия CLI, URL-адрес шлюза, дайджест манифеста) и копия манифеста в репертуаре, которая будет использоваться для мощного архиватора пакетов управления без повторной проверки казнь.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — исчерпывающая документация по флагам.
- `docs/source/sorafs_proof_streaming.md` — схема телеметрии доказательств и шаблон информационной панели Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — расширенное приближение к фрагментированию, составу манифеста и действию CAR.