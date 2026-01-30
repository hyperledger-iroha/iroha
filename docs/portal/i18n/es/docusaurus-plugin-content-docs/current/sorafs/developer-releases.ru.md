---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Процесс релиза
summary: Запустите релизный гейт CLI/SDK, примените общую политику версионирования и опубликуйте канонические notes релиза.
---

# Процесс релиза

Бинарники SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) и SDK crates
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) выпускаются вместе. Релизный
pipeline держит CLI и библиотеки согласованными, обеспечивает покрытие lint/test
и фиксирует артефакты для downstream потребителей. Выполните checklist ниже для
каждого candidate tag.

## 0. Подтвердить sign-off по безопасности

Перед запуском технического release gate соберите свежие артефакты security review:

- Скачайте самый свежий меморандум SF-6 по безопасности ([reports/sf6-security-review](./reports/sf6-security-review.md))
  и зафиксируйте его SHA256 hash в release ticket.
- Приложите ссылку на remediation ticket (например, `governance/tickets/SF6-SR-2026.md`) и отметьте
  approve-ответственных из Security Engineering и Tooling Working Group.
- Проверьте, что remediation checklist в мемо закрыт; незакрытые пункты блокируют релиз.
- Подготовьте загрузку логов parity harness (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  вместе с bundle manifest.
- Убедитесь, что команда подписи, которую вы планируете выполнить, включает и `--identity-token-provider`, и
  явный `--identity-token-audience=<aud>`, чтобы scope Fulcio был зафиксирован в релизных evidence.

Включите эти артефакты при уведомлении governance и публикации релиза.

## 1. Выполнить release/test gate

Хелпер `ci/check_sorafs_cli_release.sh` запускает форматирование, Clippy и тесты
по CLI и SDK crates с workspace-local target директорией (`.target`) чтобы избежать
конфликтов прав при запуске внутри CI контейнеров.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Скрипт выполняет следующие проверки:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` для `sorafs_car` (с feature `cli`),
  `sorafs_manifest` и `sorafs_chunker`
- `cargo test --locked --all-targets` для этих же crates

Если какой-либо шаг падает, исправьте регрессию до tagging. Релизные сборки должны
идти непрерывно от main; не делайте cherry-pick фиксов в release-ветки. Gate также
проверяет наличие keyless signing флагов (`--identity-token-issuer`,
`--identity-token-audience`) там, где требуется; отсутствующие аргументы валят запуск.

## 2. Применить политику версионирования

Все SoraFS CLI/SDK crates используют SemVer:

- `MAJOR`: Вводится с первым релизом 1.0. До 1.0 повышение minor `0.y`
  **означает breaking изменения** в CLI surface или схемах Norito.
  за опциональной политикой, добавления телеметрии).
- `PATCH`: Исправления багов, documentation-only релизы и обновления зависимостей,
  не меняющие наблюдаемое поведение.

Держите `sorafs_car`, `sorafs_manifest` и `sorafs_chunker` на одной версии, чтобы
downstream SDK потребители могли опираться на единый version string. При повышении версий:

1. Обновите поля `version =` в каждом `Cargo.toml`.
2. Перегенерируйте `Cargo.lock` через `cargo update -p <crate>@<new-version>` (workspace
   требует явных версий).
3. Снова запустите release gate, чтобы не осталось устаревших артефактов.

## 3. Подготовить release notes

Каждый релиз должен публиковать markdown changelog с акцентом на изменениях CLI, SDK
и governance. Используйте шаблон `docs/examples/sorafs_release_notes.md` (скопируйте
его в директорию релизных артефактов и заполните секции конкретикой).

Минимальный набор:

- **Highlights**: заголовки фич для потребителей CLI и SDK.
- **Upgrade steps**: TL;DR команды для обновления cargo зависимостей и перезапуска
  детерминированных fixtures.
- **Verification**: хэши или envelopes вывода команд и точная ревизия
  `ci/check_sorafs_cli_release.sh`, которая была выполнена.

Приложите заполненные release notes к тегу (например, в тело GitHub release) и
храните рядом с детерминированно сгенерированными артефактами.

## 4. Выполнить release hooks

Запустите `scripts/release_sorafs_cli.sh`, чтобы сгенерировать signature bundle и
verification summary, которые отгружаются с каждым релизом. Wrapper при необходимости
собирает CLI, вызывает `sorafs_cli manifest sign` и сразу воспроизводит
`manifest verify-signature`, чтобы сбои проявились до tagging. Пример:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Подсказки:

- Отслеживайте релизные inputs (payload, plans, summaries, expected token hash)
  в репозитории или deployment конфиге, чтобы скрипт оставался воспроизводимым. CI
  bundle под `fixtures/sorafs_manifest/ci_sample/` показывает канонический layout.
- Стройте CI automation на `.github/workflows/sorafs-cli-release.yml`; он выполняет
  release gate, вызывает скрипт выше и архивирует bundles/signatures как workflow артефакты.
  Повторяйте тот же порядок команд (release gate → sign → verify) в других CI системах,
  чтобы audit logs совпадали с сгенерированными hashes.
- Храните `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` и
  `manifest.verify.summary.json` вместе - это пакет, на который ссылается governance notification.
- Если релиз обновляет canonical fixtures, скопируйте обновленный manifest, chunk plan и
  summaries в `fixtures/sorafs_manifest/ci_sample/` (и обновите
  `docs/examples/sorafs_ci_sample/manifest.template.json`) до tagging. Downstream операторы
  зависят от закоммиченных fixtures для воспроизводимости release bundle.
- Зафиксируйте лог выполнения проверки bounded-channel для `sorafs_cli proof stream` и
  приложите его к релизному пакету, чтобы показать, что safeguards proof streaming активны.
- Запишите точный `--identity-token-audience`, использованный при подписи, в release notes;
  governance сверяет audience с политикой Fulcio перед одобрением публикации.

Используйте `scripts/sorafs_gateway_self_cert.sh`, если релиз включает rollout gateway.
Укажите тот же manifest bundle, чтобы доказать, что attestation совпадает с candidate артефактом:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Тегирование и публикация

После прохождения checks и завершения hooks:

1. Запустите `sorafs_cli --version` и `sorafs_fetch --version`, чтобы убедиться, что
   бинарники показывают новую версию.
2. Подготовьте release конфиг в `sorafs_release.toml` под контролем версий (предпочтительно)
   или в другом конфиг-файле, отслеживаемом вашим deployment репозиторием. Избегайте
   ad-hoc переменных окружения; передавайте пути в CLI через `--config` (или аналог)
   чтобы release inputs были явными и воспроизводимыми.
3. Создайте подписанный тег (предпочтительно) или аннотированный тег:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Загрузите артефакты (CAR bundles, manifests, proof summaries, release notes,
   attestation outputs) в project registry, следуя governance checklist из
   [deployment guide](./developer-deployment.md). Если релиз создал новые fixtures,
   отправьте их в общий fixture repo или object store, чтобы audit automation могла
   сравнить опубликованный bundle с контролем версий.
5. Уведомите governance канал ссылками на подписанный тег, release notes, hashes
   bundle/подписей manifest, архивированные `manifest.sign/verify` summaries и любые
   attestation envelopes. Приложите URL CI job (или архив логов), который выполнил
   `ci/check_sorafs_cli_release.sh` и `scripts/release_sorafs_cli.sh`. Обновите governance
   ticket, чтобы аудиторы могли связать approvals с артефактами; когда
   `.github/workflows/sorafs-cli-release.yml` публикует уведомления, связывайте
   зафиксированные hashes вместо ad-hoc summaries.

## 6. Пост-релизные действия

- Убедитесь, что документация, указывающая на новую версию (quickstarts, CI templates),
  обновлена, либо подтвердите отсутствие изменений.
- Заведите roadmap записи, если нужна последующая работа (например, migration флаги,
- Архивируйте логи вывода release gate для аудиторов - храните их рядом с подписанными
  артефактами.

Следование этому pipeline держит CLI, SDK crates и governance материалы синхронизированными
в каждом релизном цикле.
