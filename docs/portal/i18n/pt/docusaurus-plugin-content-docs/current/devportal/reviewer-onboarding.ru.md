---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Онбординг ревьюеров preview

## Обзор

DOCS-SORA отслеживает поэтапный запуск портала разработчиков. Сборки с gate по checksum
(`npm run serve`) и усиленные потоки Try it открывают следующий этап: онбординг проверенных
ревьюеров до широкого открытия public preview. Этот гид описывает, как собирать заявки,
проверять соответствие, выдавать доступ и безопасно завершать участие. См.
[preview invite flow](./preview-invite-flow.md) для планирования когорт, каденции приглашений
и экспорта телеметрии; шаги ниже фокусируются на действиях после выбора ревьюера.

- **В рамках:** ревьюеры, которым нужен доступ к preview docs (`docs-preview.sora`,
  сборки GitHub Pages или бандлы SoraFS) до GA.
- **Вне рамок:** операторы Torii или SoraFS (покрываются собственными onboarding-китами) и
  продакшн-развертывания портала (см.
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Роли и требования

| Роль | Типичные цели | Требуемые артефакты | Примечания |
| --- | --- | --- | --- |
| Core maintainer | Проверить новые гайды, запустить smoke tests. | GitHub handle, контакт в Matrix, подписанная CLA. | Обычно уже в команде GitHub `docs-preview`; все равно подайте заявку, чтобы доступ был аудируем. |
| Partner reviewer | Проверить SDK-сниппеты или контент по управлению до публичного релиза. | Корпоративный email, legal POC, подписанные preview terms. | Должен подтвердить требования по телеметрии и обработке данных. |
| Community volunteer | Дать feedback по удобству использования гайдов. | GitHub handle, предпочитаемый контакт, часовой пояс, согласие с CoC. | Держите когорты небольшими; приоритет ревьюерам, подписавшим contributor agreement. |

Все типы ревьюеров должны:

1. Подтвердить политику допустимого использования preview-артефактов.
2. Прочитать приложения по security/observability
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Согласиться запускать `docs/portal/scripts/preview_verify.sh` перед тем, как
   обслуживать любой локальный snapshot.

## Процесс intake

1. Попросите заявителя заполнить
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   форму (или вставить ее в issue). Зафиксируйте как минимум: личность, способ связи,
   GitHub handle, планируемые даты ревью и подтверждение чтения security-доков.
2. Зафиксируйте заявку в трекере `docs-preview` (GitHub issue или тикет управления)
   и назначьте approver.
3. Проверьте требования:
   - CLA / соглашение контрибьютора в наличии (или ссылка на партнерский контракт).
   - Подтверждение допустимого использования хранится в заявке.
   - Риск-оценка завершена (например, партнерские ревьюеры одобрены Legal).
4. Approver подписывает заявку и связывает tracking issue с любой записью
   change-management (пример: `DOCS-SORA-Preview-####`).

## Provisioning и инструменты

1. **Передать артефакты** — Предоставьте последний preview descriptor + archive из
   CI workflow или pin SoraFS (артефакт `docs-portal-preview`). Напомните ревьюерам запустить:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Serve с enforcement checksum** — Укажите команду с gate по checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Это переиспользует `scripts/serve-verified-preview.mjs`, чтобы неподтвержденный build
   не запускался случайно.

3. **Дать доступ к GitHub (опционально)** — Если нужны неопубликованные ветки, добавьте
   ревьюеров в команду GitHub `docs-preview` на период ревью и зафиксируйте изменение членства
   в заявке.

4. **Коммуникация каналов поддержки** — Поделитесь on-call контактом (Matrix/Slack) и
   процедурой инцидентов из [`incident-runbooks`](./incident-runbooks.md).

5. **Телеметрия + feedback** — Напомните, что собирается анонимизированная аналитика
   (см. [`observability`](./observability.md)). Дайте форму feedback или issue-шаблон,
   указанный в приглашении, и зафиксируйте событие через helper
   [`preview-feedback-log`](./preview-feedback-log), чтобы сводка волны оставалась актуальной.

## Чеклист ревьюера

Перед доступом к preview ревьюеры должны выполнить:

1. Проверить скачанные артефакты (`preview_verify.sh`).
2. Запустить портал через `npm run serve` (или `serve:verified`), чтобы guard checksum был активен.
3. Прочитать упомянутые выше security и observability заметки.
4. Проверить OAuth/Try it консоль через device-code login (если применимо) и не переиспользовать
   production токены.
5. Фиксировать находки в согласованном трекере (issue, общий документ или форма) и тегировать
   их релизным тегом preview.

## Ответственности мейнтейнеров и offboarding

| Фаза | Действия |
| --- | --- |
| Kickoff | Убедиться, что intake чеклист приложен к заявке, поделиться артефактами + инструкциями, добавить запись `invite-sent` через [`preview-feedback-log`](./preview-feedback-log), и запланировать промежуточный sync, если ревью длится более недели. |
| Monitoring | Отслеживать preview телеметрию (необычный трафик Try it, сбои probe) и следовать инцидент-ранбуку при подозрениях. Логировать события `feedback-submitted`/`issue-opened` по мере поступления находок, чтобы метрики волны были точными. |
| Offboarding | Отозвать временный доступ GitHub или SoraFS, записать `access-revoked`, архивировать заявку (включить summary feedback + открытые действия), и обновить реестр ревьюеров. Попросить ревьюера удалить локальные сборки и приложить digest из [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Используйте тот же процесс при ротации ревьюеров между волнами. Сохранение следов в репозитории
(issue + шаблоны) помогает DOCS-SORA оставаться аудируемым и позволяет governance подтвердить,
что доступ к preview следовал документированным контролям.

## Шаблоны приглашений и трекинг

- Начинайте каждое обращение с файла
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Он фиксирует минимальный юридический язык, инструкции по checksum preview и ожидание,
  что ревьюеры признают политику допустимого использования.
- При редактировании шаблона замените плейсхолдеры `<preview_tag>`, `<request_ticket>` и каналы связи.
  Сохраните копию финального сообщения в intake тикете, чтобы ревьюеры, approver'ы и аудиторы
  могли сослаться на точный текст.
- После отправки приглашения обновите tracking spreadsheet или issue с timestamp `invite_sent_at`
  и ожидаемой датой завершения, чтобы отчет
  [preview invite flow](./preview-invite-flow.md) автоматически подхватил когорту.
