---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل دعوات المعاينة العامة

## اهداف البرنامج

Он был убит Коллинзом и его коллегой по работе с Сейном. Сан-Франциско. يحافظ على
Загрузите документ DOCS-SORA и установите его в соответствии с требованиями законодательства. ارشادات امنية, ومسارا
واضحا للتغذية الراجعة.

- **Обращение:** было создано в 2017 году, когда специалисты по сопровождению начали работу над проектом. الاستخدام المقبول للمعاينة.
- **Уведомление:** Выполнение задания <= 25 дней, 14 дней в году. Время работы 24 часа.

## قائمة فحص بوابة الاطلاق

В ответ на вопрос:

1. Установите флажок для CI (`docs-portal-preview`,
   контрольная сумма манифеста, дескриптор, пакет SoraFS).
2. `npm run --prefix docs/portal serve` (контрольная сумма) для тега .
3. Попросите его сделать это в режиме онлайн.
4. Защитить себя от боли в очках
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Отправьте отзыв о проблеме, которую вы можете получить (в случае, если вы хотите узнать больше о проблемах, связанных с проблемой). ومعلومات البيئة).
6. Откройте раздел «Docs/DevRel + Governance».

## حزمة الدعوة

Ответ на вопрос:

1. **Обновление файла** — создание манифеста/плана с SoraFS в артефакте на GitHub
   Дескриптор контрольной суммы манифеста. اذكر امر التحقق صراحة حتى يتمكن المراجعون
   В фильме "Тренер" в Нью-Йорке.
2. **Задать контрольную сумму** — проверить контрольную сумму:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Убийство ** — Убийство в Уэльсе, штат Калифорния, в Лос-Анджелесе. مشاركتها،
   Он был убит в 2007 году.
4. **обратная связь** — проблема может быть решена в любой момент.
5. **Воспроизведение видео** — в режиме синхронизации/отключения, синхронизации,
   Это лучший выбор.

بريد النموذجي في
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
يغطي هذه المتطلبات. حدّث العناصر النائبة (создать URL-адреса, и добавить ссылки)
قبل الارسال.

## اظهار مضيف المعاينة

Он сказал, что хочет, чтобы это произошло с ним. راجع
[Для завершения сборки](./preview-host-exposure.md) выполните сборку/публикацию/проверку в соответствующем разделе.
Поговорите с нами.

1. **Обновление:** Отпустите тег выпуска, который будет открыт.

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

   Контактный разъем `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   و`portal.dns-cutover.json` или `artifacts/sorafs/`. ارفق هذه الملفات بموجة الدعوة
   Он был убит в фильме «Старый мир» в Нью-Йорке.

2. **псевдоним المعاينة:** اعد تشغيل الامر بدون `--skip-submit`
   (название `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, псевдоним الصادر عن الحوكمة).
   Получите манифест `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` и `portal.pin.report.json` для проверки.

3. **Открытие:** Отметьте псевдоним и контрольную сумму Хозяина и тег قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Для `npm run serve` (`scripts/serve-verified-preview.mjs`) используется резервный вариант.
   Нажмите на картинку для просмотра в режиме предварительного просмотра.

## جدول الاتصالات| اليوم | Новости | Владелец |
| --- | --- | --- |
| Д-3 | Автомобиль с пробным пробегом, пробный пробег | Документы/Разработчики |
| Д-2 | Справочная информация + Информационный бюллетень | Документы/DevRel + Управление |
| Д-1 | ارسال الدعوات باستخدام القالب, تحديث tracker بقائمة المستلمين | Документы/Разработчики |
| Д | Начало начала работы / рабочее время, مراقبة لوحات القياس | Документы/DevRel + Дежурство |
| Д+7 | дайджест обратной связи в разделе сортировки сообщений | Документы/Разработчики |
| Д+14 | اغلاق الموجة, الغاء الوصول المؤقت, نشر ملخص في `status.md` | Документы/Разработчики |

## تتبع الوصول والقياس عن بعد

1. Предварительный просмотр журнала отзывов и отзывов
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك كل موجة
   В тексте сообщения:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Для `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, و`access-revoked`. يعيش السجل افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json`; ارفقه بتذكرة موجة الدعوة
   Это было сделано. Краткое содержание сводки
   Ответ на вопрос:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Краткое описание JSON в формате обратной связи и обратной связи.
   الوقت لاخر حدث. المساعد يعتمد على
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Рабочий процесс, описанный в CI, описан в CI. استخدم قالب дайджест
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Подведем итоги.
2. Установите `DOCS_RELEASE_TAG` для получения дополнительной информации о том, как это сделать.
   الارتفاعات مع когорта الدعوات.
3. Установите `npm run probe:portal -- --expect-release=<tag>` в исходное состояние.
   Для этого необходимо просмотреть метаданные.
4. Создайте приложение для выполнения программы Runbook в Управлении делами.

## обратная связь

1. Оставляйте отзывы о возникающих проблемах и проблемах. ضع وسم `docs-preview/<wave>` حتى يتمكن
   Дорожная карта создания проекта в 2017 году.
2. Сводная информация в журнале предварительного просмотра.
   `status.md` (поддерживается подключение к Интернету) `roadmap.md`
   Загрузите файл DOCS-SORA.
3. Уход за бортом
   [`reviewer-onboarding`](./reviewer-onboarding.md).
4. Установите флажок для проверки контрольной суммы и проверьте контрольную сумму.
   Он был выбран в честь президента США.

Он был выбран Биллом Бэнксом в 2008 году в Нью-Йорке. Автор документации/DevRel
Он был убит в 1980-х годах в Джорджии.