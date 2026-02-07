---
lang: he
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Плейбук приглашений בתצוגה מקדימה ציבורית

## Цели программы

Этот плейбук объясняет, как анонсировать и проводить תצוגה מקדימה ציבורית после того, как
זרימת עבודה онбординга ревьюеров запущен. Он сохраняет честность дорожной карты DOCS-SORA,
гарантируя, что каждое приглашение уходит с проверяемыми артефактами, инструкциями
по безопасности и ясным каналом обратной связи.

- **Аудитория:** курированный список членов сообщества, партнеров и мейнтейнеров, которые
  подписали политику שימוש מקובל לתצוגה מקדימה.
- **אורגנציה:** размер волны по умолчанию <= 25 ревьюеров, окно доступа 14 дней, реакция на
  инциденты в течение 24 שעות.

## שער Чеклист перед запуском

Выполните эти задачи перед отправкой приглашений:

1. Последние preview-артефакты загружены в CI (`docs-portal-preview`,
   מניפסט checksum, מתאר, חבילת SoraFS).
2. `npm run --prefix docs/portal serve` (שער checksum) протестирован на том же תג.
3. Тикеты онбординга ревьюеров одобрены и связаны с волной приглашений.
4. Документы по אבטחה, צפיות и אירוע проверены
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. תבנית משוב או תבנית בעיה (דרגת חומרה, шаги воспроизведения,
   скриншоты и информация об окружении).
6. Текст анонса проверен Docs/DevRel + Governance.

## Пакет приглашения

Каждое приглашение должно включать:

1. **Проверенные артефакты** — Дайте ссылки на SoraFS מניפסט/תוכנית או GitHub artefact,
   а также מניפסט checksum и מתאר. Явно укажите команду верификации, чтобы
   ревьюеры могли запустить ее перед запуском сайта.
2. **Инструкции serve** — Включите תצוגה מקדימה команду с checksum gate:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Напоминания по безопасности** — Укажите, что токены истекают автоматически, ссылки нельзя
   делиться, а инциденты нужно сообщать немедленно.
4. **Канал обратной связи** — בדוק את הפרטים בתבנית/פורמה של בעיה וקבלת מידע נוסף.
5. ** שירותים מתקדמים** — תקנו את השירותים, שעות המשרד או סנכרון וריענון.

Пример письма в
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
покрывает эти требования. ביטול מצייני מיקום (כתובות, כתובות URL, קשרים)
перед отправкой.

## מארח תצוגה מקדימה של Публикация

Продвигайте מארח תצוגה מקדימה только после завершения онбординга и утверждения שנה כרטיס.
См. [руководство по מארח תצוגה מקדימה של חשיפה](./preview-host-exposure.md) עבור шагов מקצה לקצה
לבנות/לפרסם/לאמת, используемых в этом разделе.

1. **בנה ותקשורת:** תג שחרור Проставьте и создайте детерминированные артефакты.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   סקריפט פין записывает `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   ו `portal.dns-cutover.json` על `artifacts/sorafs/`. Приложите эти файлы к волне
   приглашений, чтобы каждый ревьюер мог проверить те же биты.2. **כינוי תצוגה מקדימה של Публикация:** Повторите команду без `--skip-submit`
   (укажите `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, וכינויי ממשל).
   Скрипт привяжет מניפסט к `docs-preview.sora` и выдаст
   `portal.manifest.submit.summary.json` плюс `portal.pin.report.json` לצרור ראיות.

3. **Проба деплоя:** Убедитесь, что alias разрешается и checksum соответствует תג
   перед отправкой приглашений.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Держите `npm run serve` (`scripts/serve-verified-preview.mjs`) под рукой как fallback,
   чтобы ревьюеры могли поднять локальную копию, если תצוגה מקדימה edge даст сбой.

## Таймлайн коммуникаций

| День | Действие | בעלים |
| --- | --- | --- |
| D-3 | Финализировать текст приглашения, обновить артефакты, верификации בשטח יבש | Docs/DevRel |
| D-2 | אישור ממשל + שינוי כרטיס | Docs/DevRel + ממשל |
| D-1 | Отправить приглашения по шаблону, обновить tracker со списком получателей | Docs/DevRel |
| ד | שיחת בעיטה / שעות עבודה, мониторинг телеметрии | Docs/DevRel + כוננות |
| D+7 | תקציר משוב Промежуточный, בעיות טריאג' בלוקטיות | Docs/DevRel |
| D+14 | Закрыть волну, отозвать временный доступ, опубликовать סיכום в `status.md` | Docs/DevRel |

## Трекинг доступа и телеметрия

1. Запишите каждого получателя, חותמת זמן приглашения и дату отзыва в logger feedback preview
   (см. [`preview-feedback-log`](./preview-feedback-log)), чтобы каждая волна разделяла
   один и тот же שביל ראיות:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   שיתוף פעולה נוסף: `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, ו-`access-revoked`. Лог по умолчанию находится в
   `artifacts/docs_portal_preview/feedback_log.json`; приложите его к тикету волны
   вместе с формами согласия. Используйте סיכום עוזר, чтобы подготовить аудируемый
   רול-אפ перед финальной заметкой:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   סיכום JSON перечисляет приглашения по волнам, открытых получателей, счетчики משוב
   и חותמת זמן последнего события. עוזר основан на
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   поэтому тот же זרימת עבודה можно запускать локально или в CI. Используйте шаблон digest в
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   при публикации סיכום волны.
2. לוחות מחוונים Тегируйте телеметрийные `DOCS_RELEASE_TAG`, использованным для волны, чтобы пики
   можно было сопоставлять с cohort приглашений.
3. התקן את `npm run probe:portal -- --expect-release=<tag>` כדי לפרוס, чтобы подтвердить,
   что preview среда объявляет корректную שחרור מטא נתונים.
4. Любые инциденты фиксируйте в шаблоне runbook и связывайте с cohort.

## משוב и закрытие1. קבל משוב בלוח הנושאים. Маркируйте элементы `docs-preview/<wave>`,
   чтобы בעלי מפת הדרכים могли быстро их найти.
2. תקציר Используйте вывод לוגר תצוגה מקדימה для отчета по волне, затем опишите когорту в
   `status.md` (участники, ключевые находки, планируемые фиксы) и обновите `roadmap.md`,
   если אבן דרך DOCS-SORA изменился.
3. Следуйте шагам offboarding из
   [`reviewer-onboarding`](./reviewer-onboarding.md): отзовите доступ, архивируйте заявки и
   поблагодарите участников.
4. Подготовьте следующую волну, обновив артефакты, перезапустив checksum gates и
   обновив шаблон приглашения с новыми датами.

Последовательное применение этого плейбука делает программу תצוגה מקדימה ניתנת לביקורת и дает
Docs/DevRel повторяемый способ масштабировать приглашения по мере приближения портала к GA.