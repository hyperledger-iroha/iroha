---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-cli
title: SoraFS Рецепты CLI
Sidebar_label: кулинарная книга CLI
описание: Консолидированное поверхностное руководство `sorafs_cli`, ориентированное на задачи.
---

:::примечание
:::

Консолидированная поверхность `sorafs_cli` (ящик `sorafs_car` имеет функцию `cli`, которая может быть использована в случае необходимости) SoraFS артефакты تیار کرنے کے لیے درکار ہر قدم выставить کرتا ہے۔ Поваренная книга и рабочие процессы операционный контекст, конвейер манифестов, Runbook оркестратора, пара.

## Полезные данные пакета

Детерминированные архивы CAR и планы фрагментов `car pack` کریں۔ Ручка فراہم نہ ہو تو خودکار طور پر SF-1 chunker منتخب کرتا ہے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

— Дескриптор чанкера по умолчанию: `sorafs.sf1@1.0.0`.
- Входные данные каталога в лексикографическом порядке, ходьба, контрольные суммы, контрольные суммы, стабильный результат.
- Сводка JSON, дайджесты полезной нагрузки, метаданные фрагментов, реестр/оркестратор, а также корневой CID или корневой CID.

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

- `--pin-*` options براہ راست `sorafs_manifest::ManifestBuilder` میں `PinPolicy` поля سے карта ہوتے ہیں۔
- `--chunk-plan` может быть использован для отправки CLI и дайджеста фрагмента SHA3 для вычислений. ورنہ وہ summary میں embed شدہ дайджест повторное использование کرتا ہے۔
- Выходные данные JSON Norito. Полезная нагрузка. Возможность просмотра обзоров и различий.

## Подписывать манифесты без долгоживущих ключей

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Встроенные токены, переменные среды, файловые источники и многое другое.
- Метаданные происхождения (`token_source`, `token_hash_hex`, дайджест фрагментов). `--include-token=true` نہ ہو۔
- Поддержка CI: GitHub Actions OIDC или `--identity-token-provider=github-actions`.

## Отправьте манифесты на Torii

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

- Доказательства псевдонимов и Norito, декодирование и Torii, и POST, и проверка соответствия дайджеста манифеста. کرتا ہے۔
- Планирование дайджеста SHA3 фрагмента и вычисление атак на несоответствие в случае несоответствия.
- Сводки ответов, аудит, статус HTTP, заголовки, полезные данные реестра и многое другое.

## Проверьте содержимое и доказательства CAR

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Дерево PoR позволяет просмотреть дайджесты полезной нагрузки и сводку манифеста, а также сравнить результаты.
- Доказательства репликации для управления и отправки данных для подсчета идентификаторов и захвата идентификаторов.

## Потоковая телеметрия

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- ہر потоковое доказательство کے لیے NDJSON элементы излучают کرتا ہے (`--emit-events=false` سے replay بند کریں)۔
- Подсчет успехов/неуспехов, гистограммы задержек, выборочные неудачи и сводные данные в формате JSON, агрегированные данные и журналы информационных панелей, очистка журналов и т. д.
- Отчет об ошибках шлюза کرے یا локальная проверка PoR (`--por-root-hex` کے ذریعے) доказательства отклонения کرے تو ненулевой выход دیتا ہے۔ репетиция کے لیے `--max-failures` اور `--max-verification-failures` سے регулировка порогов کریں۔
- Поддержка PoR всегда доступна. PDP и PoTR SF-13/SF-14 в конверте с возможностью повторного использования
- `--governance-evidence-dir` отображает сводку, метаданные (метка времени, версия CLI, URL-адрес шлюза, дайджест манифеста), копию манифеста и копию каталога, а также пакеты управления, подтверждающие поток доказательств. Можно запустить архив и создать архив.

## Дополнительные ссылки

- `docs/source/sorafs_cli.md` — تمام flags کی جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md` — схема проверки телеметрии и шаблон информационной панели Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — фрагментирование, композиция манифеста, обработка CAR и другие функции.