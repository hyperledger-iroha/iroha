---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Публичный предварительный просмотр сборника приглашений

## Цели программы

Ce playbook поясняет комментарий annoncer et faire Tourner le Preview Public une fois que le
Рабочий процесс адаптации рецензентов активен. Следите за дорожной картой DOCS-SORA honnete ru
s'assurant que chaque, часть приглашения с проверяемыми артефактами, грузоотправителями
et un chemin clair для обратной связи.

– **Аудитория:** список членов сообщества, партнеров и тех, кто занимается сопровождением
  Подпишитесь на политику приемлемого использования для предварительного просмотра.
- **Плафоны:** хвост расплывчатый по умолчанию <= 25 рецензентов, доступ в течение 14 дней,
  реагирование на дополнительные инциденты в течение 24 часов.

## Контрольный список ворот

Terminez ces taches avant d'envoyer une приглашение:

1. Последние артефакты предварительного просмотра зарядов в CI (`docs-portal-preview`,
   манифест контрольной суммы, дескриптор, пакет SoraFS).
2. `npm run --prefix docs/portal serve` (контрольная сумма шлюза) проверяется на теге meme.
3. Входные билеты рецензентов одобряют и лгут в духе расплывчатого приглашения.
4. Документы безопасны, наблюдательны и действительны
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Формула обратной связи или шаблон подготовки проблемы (включая поля для серьезных вопросов,
   этапы воспроизведения, снимки экрана и информация об окружающей среде).
6. Скопируйте сообщение об изменении документа Docs/DevRel + Governance.

## Пригласительный пакет

Приглашение на чак включает в себя:

1. **Проверка артефактов** — Fournir les Liens vers le Manifest/plan SoraFS ou les artefacts
   GitHub, а также манифест контрольной суммы и дескриптор. Указание ссылки на команду
   проверка для того, чтобы рецензенты могли действовать перед просмотром сайта.
2. **Инструкции по подаче** – включите контрольную сумму команды предварительного просмотра:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels securite** — Индикация автоматического истечения срока действия токенов, que les залога
   ne doivent pas etre partages, и que les Incidents doivent etre signales немедленно.
4. **Канал обратной связи** – шаблон/формула и пояснение временных ответов.
5. **Даты программы** — даты дебюта/финала, рабочее время или синхронизация и т. д.
   Фенетре освежения.

Пример электронной почты в этом документе
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
couvre ces exigences. Mettre a jour les заполнители (даты, URL-адреса, контакты)
авант посланника.

## Предварительный просмотр Exposer l'hote

Не рекламируйте предварительный просмотр, когда будет завершена посадка и одобрен билет на изменение.
Voir le [предварительный просмотр экспозиции отеля] (./preview-host-exposure.md) для сквозных этапов
сборка/публикация/проверка пользователей в этом разделе.

1. **Сборка и упаковка:** Отметьте тег выпуска и создайте определенные артефакты.

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

   Le script de pin ecrit `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` sous `artifacts/sorafs/`. Joindre ces fichiers а-ля расплывчатый
   Я приглашаю вас, чтобы рецензент мог проверить биты мемов.2. **Предварительный просмотр псевдонима:** Relancer la Commande sans `--skip-submit`
   (fournir `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]` и т. д.
   «по принципу управления»). Сценарий лежит в манифесте `docs-preview.sora` и др.
   `portal.manifest.submit.summary.json` плюс `portal.pin.report.json` для предварительного пакета.

3. **Проверка развертывания:** Подтверждение того, что псевдоним получен и что контрольная сумма соответствует тегу.
   заранее отправив приглашения.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) под основной запасной вариант
   что рецензенты могут скопировать локаль и фланш предварительного просмотра Edge.

## Хронология общения

| Жур | Действие | Владелец |
| --- | --- | --- |
| Д-3 | Завершите копирование приглашения, удалите артефакты, пробную проверку | Документы/Разработчики |
| Д-2 | Завершение управления + билет на обмен | Документы/DevRel + Управление |
| Д-1 | Отправляйте приглашения через шаблон, используйте трекер со списком мест назначения | Документы/Разработчики |
| Д | Начальный звонок/рабочие часы, мониторинг панелей телеметрии | Документы/DevRel + Дежурство |
| Д+7 | Дайджест отзывов и расплывчатые вопросы, сортировка проблем | Документы/Разработчики |
| Д+14 | Закройте неопределённость, отмените временный доступ, опубликуйте резюме в `status.md` | Документы/Разработчики |

## Доступ к доступу и телеметрии

1. Регистратор назначения, временная метка приглашения и дата отзыва с указанием
   предварительный просмотр журнала обратной связи (voir
   [`preview-feedback-log`](./preview-feedback-log)) afin que chaque смутно разделяет мем
   След де прев:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   За мероприятия платят `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` и `access-revoked`. Le log se trouve a
   `artifacts/docs_portal_preview/feedback_log.json` по умолчанию; Жуаньез-ле-о-билет-де
   расплывчатое приглашение с формулировками согласия. Используйте краткое описание помощника
   для создания сводного проверяемого документа перед закрытием:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   В сводном формате JSON перечислены расплывчатые приглашения,
   счетчики отзывов и временная метка последнего вечера. Помощник на отдыхе
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Donc le Meme Workflow может быть локализован или en CI. Использование шаблона дайджеста
   в [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Лорс-де-ла-публикация дю-резюме де-ла-неопределенности.
2. Пометьте информационные панели телеметрии с помощью `DOCS_RELEASE_TAG`, чтобы использовать их для неопределенного будущего.
   les pics puissent etre correles aux cohortes d'invitation.
3. Исполнитель `npm run probe:portal -- --expect-release=<tag>` после развертывания для подтверждения очереди.
   Предварительный просмотр l'environnement объявляет о выпуске хороших метаданных.
4. Грузоотправитель рекламирует инцидент в шаблоне Runbook и в группе.

## Обратная связь и закрытие1. Собирайте отзывы в документах или на доске по вопросам. Пометить элементы с помощью тегов
   `docs-preview/<wave>`, чтобы владельцы дорожной карты могли облегчить восстановление.
2. Используйте сводную информацию о поиске в журнале предварительного просмотра, чтобы восстановить расплывчатое взаимопонимание, которое вы можете использовать.
   резюме la cohorte dans `status.md` (участники, основные константы, предыдущие исправления) и др.
   Mettre a Jour `roadmap.md` и jalon DOCS-SORA изменение.
3. Suivre les etapes d'offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md): отменить доступ, архивировать запросы и т. д.
   remercier les участников.
4. Подготовьте расплывчатое сообщение и редактирование артефактов, а также возврат к контрольным суммам.
   и воспользуйтесь шаблоном приглашения с новыми датами.

Appliquer ce playbook de facon соответствует предварительному просмотру программы, который можно проверить и сделать
Docs/DevRel un moyen repetable de faire grandir les приглашения, которые будут подходить к общедоступному порталу.