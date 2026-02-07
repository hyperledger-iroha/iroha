---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Процесс релиза
תקציר: Запустите релизный гейт CLI/SDK.
---

# Процесс релиза

Бинарники SoraFS (`sorafs_cli`, `sorafs_fetch`, עוזרים) וארגזי SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) выпускаются вместе. Релизный
pipeline держит CLI и библиотеки согласованными, обеспечивает покрытие מוך/בדיקה
и фиксирует артефакты для downstream потребителей. Выполните רשימת בדיקה ниже для
каждого תג מועמד.

## 0. צור חתימה בחינם

Перед запуском технического שער שחרור соберите свежие артефакты סקירת אבטחה:

- Скачайте самый свежий меморандум SF-6 по безопасности ([דוחות/sf6-security-review](./reports/sf6-security-review.md))
  и зафиксируйте его SHA256 hash בכרטיס שחרור.
- Приложите ссылку на ticket remediation (например, `governance/tickets/SF6-SR-2026.md`) отметьте
  approve-ответственных из הנדסת אבטחה ו- Tooling Working Group.
- Проверьте, что checklist לתיקון в мемо закрыт; незакрытые пункты блокируют релиз.
- Подготовьте загрузку логов רתמה זוגית (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  вместе с מניפסט צרור.
- Убедитесь, что команда подписи, которую вы планируете выполнить, включает ו-`--identity-token-provider`, וכן
  явный `--identity-token-audience=<aud>`, чтобы scope Fulcio был зафиксирован в релизных ראיות.

בוחנים את האמנות על ידי הממשל והן הפוליטיות.

## 1. Выполнить שער שחרור/בדיקה

Хелпер `ci/check_sorafs_cli_release.sh` запускает форматирование, Clippy ו тесты
по CLI и SDK ארגזים с סביבת עבודה-מטרה מקומית директорией (`.target`) чтобы избежать
конфликтов прав при запуске внутри CI контейнеров.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Скрипт выполняет следующие проверки:

- `cargo fmt --all -- --check` (סביבת עבודה)
- `cargo clippy --locked --all-targets` ל-`sorafs_car` (תכונה `cli`),
  `sorafs_manifest` ו-`sorafs_chunker`
- `cargo test --locked --all-targets` עבור этих же ארגזים

Если какой-либо шаг падает, исправьте регрессию до תיוג. Релизные сборки должны
идти непрерывно от main; не делайте cherry-pick фиксов в release-ветки. שער также
проверяет наличие חתימה ללא מפתח флагов (`--identity-token-issuer`,
`--identity-token-audience`) там, где требуется; отсутствующие аргументы валят запуск.

## 2. Применить политику версионирования

שאר ארגזי SoraFS CLI/SDK используют SemVer:

- `MAJOR`: Вводится с первым релизом 1.0. До 1.0 повышение מינור `0.y`
  **означает breaking изменения** в CLI משטח или схемах Norito.
  за опциональной политикой, добавления телеметрии).
- `PATCH`: Исправления багов, תיעוד בלבד релизы обновления зависимостей,
  не меняющие наблюдаемое поведение.

Держите `sorafs_car`, `sorafs_manifest` ו-`sorafs_chunker` על одной версии, чтобы
SDK במורד הזרם. При повышении версий:

1. Обновите поля `version =` в каждом `Cargo.toml`.
2. Перегенерируйте `Cargo.lock` через `cargo update -p <crate>@<new-version>` (מרחב עבודה
   требует явных версий).
3. שער שחרור חדש, чтобы не осталось устаревших артефактов.## 3. Подготовить הערות שחרור

Каждый релиз должен публиковать markdown changelog с акцентом на измениях CLI, SDK
и ממשל. Используйте шаблон `docs/examples/sorafs_release_notes.md` (скопируйте
его в директорию релизных артефактов и заполните секции конкретикой).

Минимальный набор:

- **הדגשים**: заголовки фич для потребителей CLI и SDK.
- **שלבי שדרוג**: TL;DR команды для обновления cargo зависимостей и перезапуска
  גופי детерминированных.
- **אימות**: хэши или מעטפות вывода команд и точная ревизия
  `ci/check_sorafs_cli_release.sh`, которая была выполнена.

Приложите заполненные הערות שחרור к тегу (לדוגמה, в тело מהדורת GitHub) и
храните рядом с детерминированно сгенерированными артефактами.

## 4. Выполнить ווי שחרור

Запустите `scripts/release_sorafs_cli.sh`, чтобы сгенерировать חבילת חתימה и
סיכום אימות, которые отгружаются с каждым релизом. עטיפה при необходимости
צור קשר עם CLI, דגם `sorafs_cli manifest sign` ו-CLI.
`manifest verify-signature`, чтобы сбои проявились до תיוג. דוגמה:

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

- Отслеживайте релизные תשומות (עומס, תוכניות, סיכומים, hash אסימון צפוי)
  в репозитории или פריסה конфиге, чтобы скрипт оставался воспроизводимым. CI
  חבילה под `fixtures/sorafs_manifest/ci_sample/` показывает канонический פריסה.
- Стройте CI אוטומציה על `.github/workflows/sorafs-cli-release.yml`; он выполняет
  שער שחרור, הורד את חבילות/חתימות עבור חבילות/חתימות בעלות זרימת עבודה.
  Повторяйте тот же порядок команд (שער שחרור → סימן → לאמת) в других CI системах,
  чтобы יומני ביקורת совпадали с сгенерированными hashes.
- Храните `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json` ו
  `manifest.verify.summary.json` вместе - это пакет, на который ссылается הודעת ממשל.
- Если релиз обновляет מתקנים קנוניים, скопируйте обновленный מניפסט, תוכנית נתחים и
  סיכומים в `fixtures/sorafs_manifest/ci_sample/` (ו обновите
  `docs/examples/sorafs_ci_sample/manifest.template.json`) עד תיוג. במורד הזרם операторы
  מצא את אביזרי ההפצה עבור חבילת שחרור.
- Зафиксируйте лог выполнения проверки bounded-channel ל-`sorafs_cli proof stream` ו
  приложите его к релизному пакету, чтобы показать, что safeguards proof streaming actions.
- Запишите точный `--identity-token-audience`, использованный при подписи, в הערות שחרור;
  ממשל сверяет קהל с политикой Fulcio перед одобрением публикации.

Используйте `scripts/sorafs_gateway_self_cert.sh`, если релиз включает שער השקה.
Укажите тот же חבילת מניפסט, чтобы доказать, что attestation совпадает с candidate артефактом:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Тегирование и публикация

После прохождения צ'קים и завершения ווים:1. Запустите `sorafs_cli --version` ו-`sorafs_fetch --version`, чтобы убедиться, что
   бинарники показывают новую версию.
2. Подготовьте release конфиг в `sorafs_release.toml` под контролем версий (предпочтительно)
   или в другом конфиг-файле, отслеживаемом вашим репозиторием. Избегайте
   אד-הוק переменных окружения; передавайте пути в CLI через `--config` (או רגיל)
   чтобы שחרור כניסות были явными и воспроизводимыми.
3. Создайте подписанный тег (предпочтительно) או аннотированный тег:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Загрузите артефакты (חבילות רכב, מניפסטים, סיכומי הוכחה, הערות שחרור,
   יציאות אישורים) в רישום הפרויקטים, следуя רשימת תיוג ממשל из
   [מדריך פריסה](./developer-deployment.md). Если релиз создал новые מתקנים,
   отправьте их в общий fixture repo или חנות אובייקטים, чтобы audit automation могла
   сравнить опубликованный חבילה с контролем версий.
5. Уведомите governance канал ссылками на подписанный тег, הערות שחרור, hashes
   חבילה/מניפסט подписей, архивированные `manifest.sign/verify` סיכומים и любые
   מעטפות אישור. Приложите URL CI job (или архив логов), который выполнил
   `ci/check_sorafs_cli_release.sh` ו-`scripts/release_sorafs_cli.sh`. הסבר על ממשל
   כרטיס, чтобы аудиторы могли связать אישורים с артефактами; когда
   `.github/workflows/sorafs-cli-release.yml` публикует уведомления, связывайте
   зафиксированные hashes вместо סיכומים אד-הוק.

## 6. Пост-релизные действия

- Убедитесь, что документация, указывающая на новую версию (התחלות מהירות, תבניות CI),
  обновлена, либо подтвердите отсутствие изменений.
- Заведите מפת הדרכים записи, если нужна последующая работа (לדוגמה, הגירה флаги,
- Архивируйте логи вывода שער שחרור ל- аудиторов - храните их рядом с подписанными
  артефактами.

צינור צינור מתקן CLI, ארגזי SDK ומטרות ממשל.
в каждом релизном цикле.