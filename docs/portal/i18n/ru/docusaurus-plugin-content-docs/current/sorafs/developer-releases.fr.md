---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Процесс выпуска
Краткое описание: Выполните ворота выпуска CLI/SDK, примените политику частичного управления версиями и опубликуйте примечания к каноническим выпускам.
---

# Процесс выпуска

Двоичные файлы SoraFS (`sorafs_cli`, `sorafs_fetch`, помощники) и ящики SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) в ансамбле из книг. Ле трубопровод
де-релиз гарде ле CLI и ле библиотеки выровнены, убедитесь, что кувертюра lint/test
и захватывайте артефакты для потребителей, находящихся ниже по течению. Выполнение контрольного списка
ci-dessous pour chaque тег кандидат.

## 0. Подтверждение проверки безопасности обзора

Перед выполнением техники освобождения ворот захватите последние артефакты
ревю безопасности:

- Отправьте записку о безопасности SF-6 плюс последние ([reports/sf6-security-review](./reports/sf6-security-review.md))
  и зарегистрируйте свой хэш SHA256 в билете выпуска.
- Ждите залогового билета на исправление ситуации (например, `governance/tickets/SF6-SR-2026.md`) и примечаний.
  les approbateurs de Security Engineering et du Tooling Рабочая группа.
- Проверьте наличие контрольного списка исправлений записки; элементы, не блокирующие выпуск.
- Подготовка к загрузке журналов проводки (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  с пакетом манифеста.
- Подтвердите команду подписи, которую вы выполняете, включая `--identity-token-provider` и т. д.
  UN `--identity-token-audience=<aud>` явно предназначен для захвата области Fulcio в пределах прав на выпуск.

Включите эти артефакты в уведомления об управлении и публикации.

## 1. Выполнение ворот выпуска/тестов

Помощник `ci/check_sorafs_cli_release.sh` для выполнения форматирования, Clippy и тестов
интерфейс командной строки и SDK с набором целевых локальных или рабочих пространств (`.target`)
для устранения конфликтов разрешений при выполнении операций в контейнерах CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Скрипт действует на следующие утверждения:

- `cargo fmt --all -- --check` (рабочая область)
- `cargo clippy --locked --all-targets` для `sorafs_car` (с функцией `cli`),
  `sorafs_manifest` и `sorafs_chunker`
- `cargo test --locked --all-targets` для ящиков этих мемов

Если этот этап отзвучит, исправьте регрессию перед меткой. Сборки выпуска
doivent être continus avec main ; не вишнёвые па-де-корректировки в ветках
выпуск. Ворота проверяются также, как флаги подписи без ключа (`--identity-token-issuer`,
`--identity-token-audience`) sont fournis quand requis; Шрифт Les Arguments Manquants
échouer l'execution.

## 2. Применение политики версий

Все ящики CLI/SDK SoraFS с использованием SemVer:- `MAJOR`: введение в премьерный выпуск 1.0. Avant 1.0, мой удар `0.y`
  **Индикация изменений** на поверхности CLI или в схемах Norito.
- `MINOR` : Новые функции (новые команды/флаги, новые чемпионы Norito
  здесь есть политическая опция, дополнительные возможности телеметрии).
- `PATCH`: исправление ошибок, выпуск уникальной документации и многое другое.
  зависимости, которые не модифицируют наблюдаемое поведение.

Gardez toujours `sorafs_car`, `sorafs_manifest` и `sorafs_chunker` в мемной версии
для того, чтобы последующие пользователи SDK могли в зависимости от конкретной цепочки версий
выравнивание. Дополнительные неровности версии:

1. Меттез на полях `version =` в чаке `Cargo.toml`.
2. Обновите `Cargo.lock` через `cargo update -p <crate>@<new-version>` (рабочая область
   наложить явные версии).
3. Отпустите ворота для удаления артефактов.

## 3. Подготовка примечаний к выпуску

Каждый выпуск публикуется в журнале изменений с уценкой и в преддверии изменений.
влияние на CLI, SDK и управление. Используйте шаблоны в
`docs/examples/sorafs_release_notes.md` (копия в вашем репертуаре артефактов)
выпустите и восстановите разделы с конкретными деталями).

Минимальное содержание:

- **Основные характеристики**: функциональные возможности для пользователей CLI и SDK.
- **Совместимость**: изменения кассантов, политические обновления, минимальные требования.
  шлюз/ноуд.
- **Этапы обновления**: команды TL;DR для доставки груза и др.
  переустановить детерминированные светильники.
- **Проверка**: хэши сортировки или конверты и точные исправления.
  `ci/check_sorafs_cli_release.sh` выполнено.

Ждите примечаний к выпуску с тегом (например, корпус выпуска GitHub) и
stockez-les à Côté des Artefacts Générés de Façon Determinist.

## 4. Выполните освобождение крючков

Выполните `scripts/release_sorafs_cli.sh` для создания пакета подписей и других файлов.
резюме проверки livrés с выпуском Chaque. Обертка создает CLI
nécessaire, обращение `sorafs_cli manifest sign` и немедленно радуйтесь
`manifest verify-signature` для ремонта элементов до бирки. Пример:

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

Советы:- Отслеживание входных данных выпуска (полезная нагрузка, планы, сводки, хеш-токен посещаемости)
  в вашем репозитории или конфигурации развертывания для хранения воспроизводимого сценария.
  Комплект CI sous `fixtures/sorafs_manifest/ci_sample/` с каноническим макетом.
- База автоматизации CI sur `.github/workflows/sorafs-cli-release.yml` ; она выполняет
  ворота выпуска, вызов сценария ci-dessus и архивирование пакетов/подписей как обычно
  артефакты рабочего процесса. Reproduisez le même ordre de Commandes (ворота → подпись →
  проверка) в других системах CI для выравнивания журналов аудита с хэшами.
- Гардез `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` и др.
  `manifest.verify.summary.json` ансамбль: ils forment le paquet référence dans la
  уведомление правительства.
- Lorsque la Release встретился с каноническими светильниками, скопировав манифест rafraîchi,
  план фрагмента и сводки в `fixtures/sorafs_manifest/ci_sample/` (и
  в течение дня `docs/examples/sorafs_ci_sample/manifest.template.json`) перед тегом. Лес
  последующие операторы зависят от комиссий по оборудованию для воспроизведения комплекта.
- Запишите журнал выполнения проверки ограниченных каналов.
  `sorafs_cli proof stream` и используйте пакет выпуска для демонстрации того, что ле
  garde-fous de доказательство потоковой передачи оставшихся действий.
- Notez l'`--identity-token-audience` точно используется в подписи в заметках.
  де релиз; управление возмещает аудиторию с помощью политики Фульчо до одобрения.

Используйте `scripts/sorafs_gateway_self_cert.sh` при выпуске, включая австралийское развертывание.
шлюз. Pointez-le sur le même Bundle de Manife for Prouver que l'attestation
соответствует кандидату на артефакт:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Теггер и издатель

После прохождения проверок и концов крючков:1. Выполните `sorafs_cli --version` и `sorafs_fetch --version` для подтверждения бинарников.
   репортажная новая версия.
2. Подготовьте конфигурацию выпуска в версии `sorafs_release.toml` (предпочтительная версия).
   или другой файл конфигурации, соответствующий вашему репозиторию развертывания. Эвитес де Депендре
   специальные переменные среды; пройти через CLI с `--config` (или
   эквивалентно) в зависимости от того, какие входные данные являются явными и воспроизводимыми.
3. Создайте подписанный тег (предпочитаемый) или тег аннотации:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Загрузите артефакты (пакеты CAR, манифесты, резюме доказательств, примечания к выпуску,
   результаты аттестации) vers le реестра проекта, selon la checklist de gouvernance
   в [руководстве по развертыванию] (./developer-deployment.md). Если выпусти продукт
   новые светильники, poussez-les vers le repo de lampage partage или l'object store
   Afin que l'automatization d'audit puisse Compare le Bundle publié au Source Control.
5. Уведомите канал управления с указанием залогов и подписанных меток, примечаний к выпуску,
   хеши пакетов/подписей манифеста, архивы резюме `manifest.sign/verify`
   и все конверты для аттестации. Включите URL-адрес задания CI (или архив журналов), который
   выполните `ci/check_sorafs_cli_release.sh` и `scripts/release_sorafs_cli.sh`. Меттес à
   Jour le Ticket de Governance для того, чтобы аудиторы могли больше доверять одобрениям
   дополнительные артефакты; lorsque `.github/workflows/sorafs-cli-release.yml` посланник уведомлений,
   зарегистрируйте хэши или запишите специальные резюме.

## 6. Пост-релиз Suivi

- Убедитесь, что документация соответствует новой версии (краткие руководства, шаблоны CI)
  Это день или подтверждение того, что изменение не является обязательным.
- Créez des intrées de roadmap si un travail de suivi est nécessaire (например, флаги миграции,
- Архив журналов вылазок на ворота для проверяющих: stockez-les à côté des
  знаки артефактов.

Новый конвейер поддерживает CLI, ящики SDK и элементы управления.
выравнивается по циклу выпуска.