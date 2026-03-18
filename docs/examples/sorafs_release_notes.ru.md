---
lang: ru
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff09443144d6d078ee365f21232656e9683d9aa8331b1741c486e5082299b80c
source_last_modified: "2025-11-02T17:40:03.493141+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraFS CLI и SDK - Заметки о релизе (v0.1.0)

## Основные изменения
- `sorafs_cli` теперь охватывает весь пайплайн упаковки (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`), поэтому CI-раннеры вызывают
  один бинарник вместо отдельных хелперов. Новый безключевой поток подписи по умолчанию
  использует `SIGSTORE_ID_TOKEN`, понимает OIDC-провайдеров GitHub Actions и пишет
  детерминированный JSON-резюме вместе с пакетом подписи.
- Мульти-источниковый fetch *scoreboard* поставляется в составе `sorafs_car`: нормализует
  телеметрию провайдеров, применяет штрафы по возможностям, сохраняет JSON/Norito отчеты и
  подпитывает симулятор оркестратора (`sorafs_fetch`) через общий registry handle.
  Фикстуры в `fixtures/sorafs_manifest/ci_sample/` демонстрируют детерминированные входы
  и выходы, с которыми CI/CD должен сравнивать.
- Автоматизация релиза зафиксирована в `ci/check_sorafs_cli_release.sh` и
  `scripts/release_sorafs_cli.sh`. Каждый релиз теперь архивирует manifest bundle,
  подпись, сводки `manifest.sign/verify` и snapshot scoreboard, чтобы reviewers governance
  могли отслеживать артефакты без повторного запуска пайплайна.

## Совместимость
- Ломающих изменений: **Нет.** Все добавления CLI - это аддитивные флаги/подкоманды; существующие
  вызовы продолжают работать без изменений.
- Минимальные версии gateway/node: требуется Torii `2.0.0-rc.2.0` (или новее), чтобы были доступны
  API chunk-range, квоты stream-token и заголовки возможностей, экспортируемые
  `crates/iroha_torii`. Ноды хранения должны работать на SoraFS host stack из коммита
  `c6cc192ac3d83dadb0c80d04ea975ab1fd484113` (включает новые входы scoreboard и телеметрическое подключение).
- Upstream зависимости: сторонних обновлений сверх базового набора workspace нет; релиз повторно
  использует зафиксированные версии `blake3`, `reqwest` и `sigstore` из `Cargo.lock`.

## Шаги обновления
1. Обновите согласованные crates в вашем workspace:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Повторно запустите релизный gate локально (или в CI), чтобы подтвердить покрытие fmt/clippy/tests:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. Сгенерируйте заново подписанные артефакты и сводки с курированной конфигурацией:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Скопируйте обновленные bundles/proofs в `fixtures/sorafs_manifest/ci_sample/`, если релиз
   обновляет канонические фикстуры.

## Проверка
- Коммит релизного gate: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD` сразу после успешного прохождения gate).
- Вывод `ci/check_sorafs_cli_release.sh`: архивирован в
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (приложен к release bundle).
- Дайджест manifest bundle: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Дайджест proof summary: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Дайджест манифеста (для downstream attestation cross-checks):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (из `manifest.sign.summary.json`).

## Заметки для операторов
- Gateway Torii теперь применяет capability header `X-Sora-Chunk-Range`. Обновите allowlists,
  чтобы допускать клиентов с новыми scopes stream token; старые токены без range claim будут
  throttled.
- `scripts/sorafs_gateway_self_cert.sh` интегрирует проверку манифеста. При запуске harness self-cert
  передайте свежесгенерированный manifest bundle, чтобы wrapper быстро упал при drift подписи.
- Телеметрические дашборды должны ingest новый экспорт scoreboard (`scoreboard.json`), чтобы
  согласовать пригодность провайдеров, назначение весов и причины отказа.
- Архивируйте четыре канонических summary при каждом rollout:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. Governance tickets ссылаются на эти точные файлы при утверждении.

## Благодарности
- Storage Team - end-to-end консолидация CLI, renderer chunk-plan и telemetria scoreboard.
- Tooling WG - release pipeline (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) и deterministic bundle для fixtures.
- Gateway Operations - capability gating, обзор политики stream-token и обновленные
  self-cert playbooks.
