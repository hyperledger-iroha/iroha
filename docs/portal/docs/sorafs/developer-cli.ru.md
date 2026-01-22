<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56fc1f52e849b109927d6e39e9675b97bf34f9991ab9fe578cb3c8ee51a671c6
source_last_modified: "2025-11-15T06:46:46.423964+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: developer-cli
title: Книга рецептов CLI SoraFS
sidebar_label: Книга рецептов CLI
description: Практический разбор по задачам для консолидированной поверхности `sorafs_cli`.
---

:::note Канонический источник
:::

Консолидированная поверхность `sorafs_cli` (предоставляется crate `sorafs_car` с включенной feature `cli`) раскрывает каждый шаг, необходимый для подготовки артефактов SoraFS. Используйте этот cookbook, чтобы сразу перейти к типовым workflows; сочетайте его с pipeline манифеста и runbooks оркестратора для операционного контекста.

## Упаковка payloads

Используйте `car pack`, чтобы получить детерминированные CAR-архивы и планы chunk. Команда автоматически выбирает chunker SF-1, если не указан handle.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Стандартный handle chunker: `sorafs.sf1@1.0.0`.
- Входные директории обходятся в лексикографическом порядке, чтобы checksums оставались стабильными между платформами.
- JSON-резюме включает payload digests, метаданные по chunk и корневой CID, распознаваемый реестром и оркестратором.

## Сборка manifests

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
- Укажите `--chunk-plan`, если хотите, чтобы CLI пересчитал SHA3 digest для chunk перед отправкой; иначе он использует digest из summary.
- JSON-вывод отражает Norito payload для удобных diffs при ревью.

## Подписание manifests без долгоживущих ключей

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Принимает inline токены, переменные окружения или файловые источники.
- Добавляет provenance метаданные (`token_source`, `token_hash_hex`, digest chunk) без сохранения raw JWT, если не задано `--include-token=true`.
- Удобно для CI: объедините с GitHub Actions OIDC, установив `--identity-token-provider=github-actions`.

## Отправка manifests в Torii

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

- Выполняет Norito-декодирование alias proofs и проверяет соответствие digest манифеста до POST в Torii.
- Пересчитывает SHA3 digest chunk из плана, чтобы предотвращать атаки на несоответствие.
- Резюме ответа фиксируют HTTP статус, headers и payloads реестра для последующего аудита.

## Проверка содержимого CAR и proofs

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Перестраивает дерево PoR и сравнивает payload digests с резюме манифеста.
- Фиксирует количества и идентификаторы, нужные при отправке proofs репликации в governance.

## Потоковая телеметрия proofs

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Генерирует NDJSON элементы для каждого переданного proof (отключите replay через `--emit-events=false`).
- Агрегирует количества успехов/ошибок, гистограммы латентности и выборочные сбои в summary JSON, чтобы dashboards могли строить графики без чтения логов.
- Завершает работу с ненулевым кодом, когда gateway сообщает об ошибках или локальная проверка PoR (через `--por-root-hex`) отклоняет proofs. Настройте пороги через `--max-failures` и `--max-verification-failures` для репетиций.
- Сегодня поддерживается PoR; PDP и PoTR переиспользуют тот же envelope после выхода SF-13/SF-14.
- `--governance-evidence-dir` записывает рендеренное резюме, метаданные (timestamp, версия CLI, URL gateway, digest манифеста) и копию манифеста в указанную директорию, чтобы governance пакеты могли архивировать доказательства proof-stream без повторного запуска.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — исчерпывающая документация по флагам.
- `docs/source/sorafs_proof_streaming.md` — схема телеметрии proofs и шаблон Grafana dashboard.
- `docs/source/sorafs/manifest_pipeline.md` — подробный разбор chunking, сборки manifest и обработки CAR.
