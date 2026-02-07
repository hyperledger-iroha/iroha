---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: предварительный просмотр-обратная связь-w1-план
титул: Чемпионат мира по футболу W1
Sidebar_label: خطة W1
описание: Миссис Мэнни, которую он назвал "Лихостернами".
---

| بند | تفاصيل |
| --- | --- |
| Новости | W1 - Защитный экран Torii |
| Новости | Начало года 2025 года 3 |
| وسم الاثر (مخطط) | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |

## الاهداف

1. Он сказал, что хочет, чтобы его забрали.
2. Попробуй это сделать, чтобы получить больше удовольствия от игры.
3. Установите контрольную сумму и проверьте датчики.
4. Когда вы сделаете это, вы сможете сделать это в ближайшее время.

## تفصيل المهام

| المعرف | المهمة | المالك | الاستحقاق | حالة | ملاحظات |
| --- | --- | --- | --- | --- | --- |
| П1-П1 | على موافقة قانونية على ملحق شروط المعاينة | Руководитель отдела документации и разработки -> Юридические вопросы | 05.04.2025 | ✅ Видео | Добавлено в программу `DOCS-SORA-Preview-W1-Legal` от 05.04.2025. В формате PDF. |
| П1-П2 | Дэнни постановка фильма «Попробуй» (10 апреля 2025 г.) Документы/DevRel + Ops | 06.04.2025 | ✅ Видео | Дата выпуска `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` от 06.04.2025; Для запуска CLI и `.env.tryit-proxy.bak`. |
| П1-П3 | Создать код (`preview-2025-04-12`), добавить `scripts/preview_verify.sh` + `npm run probe:portal`, добавить дескриптор/контрольные суммы | Портал TL | 08.04.2025 | ✅ Видео | تم حفظ الاثر وسجلات التحقق تحت `artifacts/docs_preview/W1/preview-2025-04-12/`; Было проведено исследование по делу. |
| W1-P4 | Впускной коллектор (`DOCS-SORA-Preview-REQ-P01...P08`), дополнительная информация и соглашение о неразглашении | Связь с управлением | 07.04.2025 | ✅ Видео | Объявлено, что он сказал: "Старый мир" (написано на 11 апреля 2025 г.); روابط موجودة في المتتبع. |
| П1-П5 | Вызов `docs/examples/docs_preview_invite_template.md`, `<preview_tag>` и `<request_ticket>` | Руководитель отдела документации и разработки | 08.04.2025 | ✅ Видео | Сообщение будет опубликовано 12 апреля 2025 г., 15:00 UTC, в воскресенье. |

## قائمة التحقق قبل الاطلاق

> Задайте: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` для проверки 1-5 операций (сборка, проверка контрольной суммы, проверка соединения, проверка ссылок, وتحديث وكيل Попробуйте). Создайте файл JSON для создания файлов для просмотра.

1. `npm run build` (с `DOCS_RELEASE_TAG=preview-2025-04-12`) для подключения `build/checksums.sha256` и `build/release.json`.
2. И18НИ00000033Х.
3. И18НИ00000034Х.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` и `build/link-report.json` — дескриптор.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (по умолчанию `--tryit-target`); Установите флажок `.env.tryit-proxy` и установите флажок `.bak`.
6. Запустите W1 в режиме ожидания (дескриптор контрольной суммы, пробный зонд, тест-драйв). Попробуйте, пожалуйста. Grafana).

## قائمة ادلة الاثبات

- [x] Создан файл (PDF в формате PDF) с кодом `DOCS-SORA-Preview-W1`.
- [x] Grafana и `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] дескриптор содержит контрольную сумму `preview-2025-04-12`, созданную `artifacts/docs_preview/W1/`.
- [x] Состав команды появился в составе حقول `invite_sent_at` (Рейтер Сэйл W1 в сезоне 2016).
- [x] اثار التغذية الراجعة منعكسة في [`preview-feedback/w1/log.md`](./log.md) в случае необходимости (تم) تحديثه 2025-04-26 Список участников/телеметрия/проблемы).

حدّث هذه الخطة كلما تقدمت المهام؛ Он был убит в 2007 году.## سير عمل التغذية الراجعة

1. Убийство, исчезновение
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   املأ البيانات الوصفية, واحفظ النسخة المكتملة تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Свободное время, когда он находится в центре внимания, в конце концов.
   [`preview-feedback/w1/log.md`](./log.md) بالكامل
   Сделал это.
3. Он выступил в роли режиссёра и министра финансов в Нью-Йорке. السجل
   Он был открыт.