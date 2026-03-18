---
lang: ru
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

# Пример fixtures для SoraFS CI

Этот каталог упаковывает детерминированные artefacts, сгенерированные из sample payload в `fixtures/sorafs_manifest/ci_sample/`. Пакет демонстрирует end-to-end пайплайн упаковки и подписи SoraFS, который запускают CI workflows.

## Инвентарь артефактов

| Файл | Описание |
|------|-------------|
| `payload.txt` | Исходный payload, используемый fixture скриптами (текстовый образец). |
| `payload.car` | CAR архив, созданный `sorafs_cli car pack`. |
| `car_summary.json` | Сводка `car pack`, фиксирующая chunk digests и метаданные. |
| `chunk_plan.json` | Fetch-plan JSON, описывающий диапазоны chunks и ожидания провайдеров. |
| `manifest.to` | Norito manifest, созданный `sorafs_cli manifest build`. |
| `manifest.json` | Читаемое отображение manifest для debug. |
| `proof.json` | PoR summary из `sorafs_cli proof verify`. |
| `manifest.bundle.json` | Keyless signature bundle, созданный `sorafs_cli manifest sign`. |
| `manifest.sig` | Отдельная Ed25519 подпись для manifest. |
| `manifest.sign.summary.json` | CLI summary при подписании (hashes, metadata bundle). |
| `manifest.verify.summary.json` | CLI summary из `manifest verify-signature`. |

Все digests, упомянутые в release notes и документации, получены из этих файлов. `ci/check_sorafs_cli_release.sh` пересоздает те же artefacts и сравнивает их с закоммиченными версиями.

## Перегенерация fixtures

Запустите команды ниже из корня репозитория, чтобы пересоздать fixtures. Они повторяют шаги workflow `sorafs-cli-fixture`:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Если какой-либо шаг дает другие hashes, разберитесь перед обновлением fixtures. CI workflows полагаются на детерминированный output для выявления регрессий.

## Будущее покрытие

По мере того как дополнительные chunker профили и форматы proof выходят из roadmap, их канонические fixtures будут добавлены в этот каталог (например,
`sorafs.sf2@1.0.0` (см. `fixtures/sorafs_manifest/ci_sample_sf2/`) или PDP streaming proofs). Каждый новый профиль будет иметь ту же структуру — payload, CAR,
plan, manifest, proofs и artefacts подписи — чтобы downstream автоматизация могла сравнивать релизы без кастомных скриптов.
