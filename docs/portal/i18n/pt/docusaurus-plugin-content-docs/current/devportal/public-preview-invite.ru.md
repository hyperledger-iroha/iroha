---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Плейбук приглашений на public preview

## Цели программы

Этот плейбук объясняет, как анонсировать и проводить public preview после того, как
workflow онбординга ревьюеров запущен. Он сохраняет честность дорожной карты DOCS-SORA,
гарантируя, что каждое приглашение уходит с проверяемыми артефактами, инструкциями
по безопасности и ясным каналом обратной связи.

- **Аудитория:** курированный список членов сообщества, партнеров и мейнтейнеров, которые
  подписали политику acceptable-use для preview.
- **Ограничения:** размер волны по умолчанию <= 25 ревьюеров, окно доступа 14 дней, реакция на
  инциденты в течение 24h.

## Чеклист gate перед запуском

Выполните эти задачи перед отправкой приглашений:

1. Последние preview-артефакты загружены в CI (`docs-portal-preview`,
   checksum manifest, descriptor, SoraFS bundle).
2. `npm run --prefix docs/portal serve` (checksum gate) протестирован на том же tag.
3. Тикеты онбординга ревьюеров одобрены и связаны с волной приглашений.
4. Документы по security, observability и incident проверены
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Подготовлен feedback формуляр или issue template (поля severity, шаги воспроизведения,
   скриншоты и информация об окружении).
6. Текст анонса проверен Docs/DevRel + Governance.

## Пакет приглашения

Каждое приглашение должно включать:

1. **Проверенные артефакты** — Дайте ссылки на SoraFS manifest/plan или GitHub artefact,
   а также checksum manifest и descriptor. Явно укажите команду верификации, чтобы
   ревьюеры могли запустить ее перед запуском сайта.
2. **Инструкции serve** — Включите preview команду с checksum gate:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Напоминания по безопасности** — Укажите, что токены истекают автоматически, ссылки нельзя
   делиться, а инциденты нужно сообщать немедленно.
4. **Канал обратной связи** — Дайте ссылку на issue template/форму и обозначьте ожидания по времени ответа.
5. **Даты программы** — Укажите даты начала/окончания, office hours или sync встречи и следующее окно refresh.

Пример письма в
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
покрывает эти требования. Обновите placeholders (даты, URLs, контакты)
перед отправкой.

## Публикация preview host

Продвигайте preview host только после завершения онбординга и утверждения change ticket.
См. [руководство по exposure preview host](./preview-host-exposure.md) для end-to-end шагов
build/publish/verify, используемых в этом разделе.

1. **Build и упаковка:** Проставьте release tag и создайте детерминированные артефакты.

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

   Скрипт pin записывает `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   и `portal.dns-cutover.json` в `artifacts/sorafs/`. Приложите эти файлы к волне
   приглашений, чтобы каждый ревьюер мог проверить те же биты.

2. **Публикация preview alias:** Повторите команду без `--skip-submit`
   (укажите `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, и доказательство alias от governance).
   Скрипт привяжет manifest к `docs-preview.sora` и выдаст
   `portal.manifest.submit.summary.json` плюс `portal.pin.report.json` для evidence bundle.

3. **Проба деплоя:** Убедитесь, что alias разрешается и checksum соответствует tag
   перед отправкой приглашений.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Держите `npm run serve` (`scripts/serve-verified-preview.mjs`) под рукой как fallback,
   чтобы ревьюеры могли поднять локальную копию, если preview edge даст сбой.

## Таймлайн коммуникаций

| День | Действие | Owner |
| --- | --- | --- |
| D-3 | Финализировать текст приглашения, обновить артефакты, dry-run верификации | Docs/DevRel |
| D-2 | Governance sign-off + change ticket | Docs/DevRel + Governance |
| D-1 | Отправить приглашения по шаблону, обновить tracker со списком получателей | Docs/DevRel |
| D | Kickoff call / office hours, мониторинг телеметрии | Docs/DevRel + On-call |
| D+7 | Промежуточный feedback digest, triage блокирующих issues | Docs/DevRel |
| D+14 | Закрыть волну, отозвать временный доступ, опубликовать summary в `status.md` | Docs/DevRel |

## Трекинг доступа и телеметрия

1. Запишите каждого получателя, timestamp приглашения и дату отзыва в preview feedback logger
   (см. [`preview-feedback-log`](./preview-feedback-log)), чтобы каждая волна разделяла
   один и тот же evidence trail:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Поддерживаемые события: `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, и `access-revoked`. Лог по умолчанию находится в
   `artifacts/docs_portal_preview/feedback_log.json`; приложите его к тикету волны
   вместе с формами согласия. Используйте helper summary, чтобы подготовить аудируемый
   roll-up перед финальной заметкой:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Summary JSON перечисляет приглашения по волнам, открытых получателей, счетчики feedback
   и timestamp последнего события. Helper основан на
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   поэтому тот же workflow можно запускать локально или в CI. Используйте шаблон digest в
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   при публикации recap волны.
2. Тегируйте телеметрийные dashboards `DOCS_RELEASE_TAG`, использованным для волны, чтобы пики
   можно было сопоставлять с cohort приглашений.
3. Запустите `npm run probe:portal -- --expect-release=<tag>` после deploy, чтобы подтвердить,
   что preview среда объявляет корректную release metadata.
4. Любые инциденты фиксируйте в шаблоне runbook и связывайте с cohort.

## Feedback и закрытие

1. Собирайте feedback в общем документе или issue board. Маркируйте элементы `docs-preview/<wave>`,
   чтобы owners roadmap могли быстро их найти.
2. Используйте summary вывод preview logger для отчета по волне, затем опишите когорту в
   `status.md` (участники, ключевые находки, планируемые фиксы) и обновите `roadmap.md`,
   если milestone DOCS-SORA изменился.
3. Следуйте шагам offboarding из
   [`reviewer-onboarding`](./reviewer-onboarding.md): отзовите доступ, архивируйте заявки и
   поблагодарите участников.
4. Подготовьте следующую волну, обновив артефакты, перезапустив checksum gates и
   обновив шаблон приглашения с новыми датами.

Последовательное применение этого плейбука делает программу preview auditable и дает
Docs/DevRel повторяемый способ масштабировать приглашения по мере приближения портала к GA.
