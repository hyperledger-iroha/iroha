---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مسار دعوات المعاينة

## غرض

Для этого необходимо установить **DOCS-SORA**. В бета-версии был создан новый проект, созданный в 2017 году. Он выступил в роли Кейна в фильме "Миссисипи", а также в роли президента США. В 2007 году он был убит в 1997 году. Сообщение:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) للتعامل مع كل مراجع.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) контрольная сумма.
- [`devportal/observability`](./observability.md)

## خطة الموجات

| Новости | جمهور | معايير الدخول | معايير الخروج | ملاحظات |
| --- | --- | --- | --- | --- |
| **W0 — Поддержка разработчиков** | Сопровождающие в Docs/SDK были созданы в 2017 году. | На GitHub `docs-portal-preview` загрузите контрольную сумму для `npm run serve`, запустите Alertmanager и запустите 7 дней назад. | В первом раунде P0 отставание было начато в 2007 году. | تستخدم للتحقق в المسار؛ Он был убит в 2007 году. |
| **П1 – Партнеры** | Код SoraFS, код Torii, а также соглашение о неразглашении. | В программе W0, в программе «Try-it» в постановке. | Выпущено издание (выпуск в Новом выпуске), опубликовано в журнале =2, необходимо выполнить откат. | تحديد الدعوات المتزامنة (<=25). |

В качестве примера можно привести `status.md`, который был использован в качестве программного обеспечения. Пожалуйста, обратите внимание.

## قائمة تحقق предполетная подготовка

اتم هذه الاجراءات **قبل** جدولة الدعوات لموجة:

1. ** اثار CI متاحة**
   - اخر `docs-portal-preview` + дескриптор تم رفعه بواسطة `.github/workflows/docs-portal-preview.yml`.
   - Вывод SoraFS для `docs/portal/docs/devportal/deploy-guide.md` (дескриптор التحويل موجود).
2. **Контрольная сумма**
   - `docs/portal/scripts/serve-verified-preview.mjs` или `npm run serve`.
   - Используется версия `scripts/preview_verify.sh` для macOS + Linux.
3. **خط اساس للقياس عن بعد**
   - `dashboards/grafana/docs_portal.json`. Попробуйте попробовать. Попробуйте `docs.preview.integrity`.
   - Для `docs/portal/docs/devportal/observability.md` используется Grafana.
4. **Вечеринка**
   - Выпуск لمتعقب الدعوات جاهزة (выпуск واحدة لكل موجة).
   - قالب سجل المراجعين منسوخ (انظر [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Вопрос о проблеме, связанной с выпуском SRE المطلوبة مرفقة بالـ.

Предполетная подготовка к полету в Нью-Йорке в Нью-Йорке.

## خطوات المسار1. **Вечеринка**
   - Это произошло в 2017 году и в 2017 году.
   - Он сказал, что это произошло с Люком Миссисипи.
2. **Вечернее сообщение**
   - Он ответил на выпуск журнала «The دعوات».
   - Сделано в Лос-Анджелесе (CLA/عقد, استخدام مقبول, موجز امني).
3. **Вечеринка**
   - اكمال الحقول في [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, جهات الاتصال).
   - Дескриптор + хеш
   - Создан файл (в рамках Matrix/Slack) для выпуска.
4. **Вечеринка**
   - Для установки `invite_sent_at` и `expected_exit_at` (`pending`, `active`, `complete`, `revoked`).
   - ربط طلب دخول المراجع لضمان التدقيق.
5. **Вечеринка в центре**
   - Установите `docs.preview.session_active` и установите `TryItProxyErrors`.
   - В 1990-х годах он был убит в 1980-х годах, когда его пригласили на работу. الدعوة.
6. **Последний день рождения**
   - Установите флажок `expected_exit_at`.
   - выпуск выпуск الموجة بملخص قصير (نتائج، حوادث, خطوات تالية) قبل الانتقال لمجموعة التالية.

## الادلة والتقارير

| الاثر | مكان التخزين | Информационные технологии |
| --- | --- | --- |
| Выпуск متعقب الدعوات | Добавлен GitHub `docs-portal-preview` | Он сказал, что это не так. |
| تصدير قائمة المراجعين | Дополнительная информация для `docs/portal/docs/devportal/reviewer-onboarding.md` | اسبوعي. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (Обзорная информация о вещах) | لكل موجة + بعد الحوادث. |
| Новости | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (название обложки) | 5 января 2018 года. |
| ملاحظة الحوكمة | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Загрузите файл DOCS-SORA. |

Код `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
Он был отправлен в Лас-Вегас-Лейк-Сити. Создание JSON-файла для решения проблемы Он был убит в 2007 году.

ارفق قائمة الادلة بـ `status.md` عند انتهاء كل موجة حتى يتم تحديث مدخل خارطة الطريق بسرعة.

## معايير التراجع والتوقف

Он сказал:

- حادثة وكيل Попробуйте выполнить откат (`npm run manage:tryit-proxy`).
- В игре: >3 صفحات تنبيه لنقاط نهاية خاصة بالمعاينة 7 ايام.
- В фильме: Он сказал, что Бреннан Сингх был свидетелем того, как он устроился на работу.
- Ошибка: несоответствие контрольной суммы в случае ошибки `scripts/preview_verify.sh`.

Он был создан в 1980-х годах в Нью-Йорке. Это произошло в 48 лет назад.