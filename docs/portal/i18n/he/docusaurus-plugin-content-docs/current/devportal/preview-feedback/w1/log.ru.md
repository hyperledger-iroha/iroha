---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-log
כותרת: Лог отзывов и телеметрии W1
sidebar_label: Лог W1
תיאור: Сводный רוסטר, телеметрические מחסומים и заметки סוקרים для первой preview-волны партнеров.
---

Этот лог хранит סגל приглашений, телеметрические מחסומים ו отзывы סוקרים для
**תצוגה מקדימה של партнеров W1**, сопровождающей задачи приема в
[`preview-feedback/w1/plan.md`](./plan.md) и запись трекера волны в
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Обновляйте его, когда отправлено приглашение,
записан телеметрический תמונת מצב или טריאגה выполнен по пункту отзывов, чтобы מבקרי ממשל могли
воспроизвести доказательства без поиска внешних тикетов.

## Рoster когорты

| מזהה שותף | Тикет запроса | NDA получено | Приглашение отправлено (UTC) | Ack/первый логин (UTC) | Статус | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Завершено 2025-04-26 | soraps-op-01; сфокусирован на доказательствах זוגיות למסמכי מתזמר. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 12-04-2025 15:03 | 12-04-2025 15:15 | ✅ Завершено 2025-04-26 | soraps-op-02; проверил צולב קישורים Norito/טלמטריה. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Завершено 2025-04-26 | soraps-op-03; провел תרגילי ריבוי מקורות כשל. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 12-04-2025 15:09 | 2025-04-12 15:21 | ✅ Завершено 2025-04-26 | torii-int-01; ספר בישול ревью Torii `/v1/pipeline` + נסה את זה. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 12-04-2025 15:12 | 2025-04-12 15:23 | ✅ Завершено 2025-04-26 | torii-int-02; участвовал в обновлении скриншота נסה את זה (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 12-04-2025 15:15 | 2025-04-12 15:26 | ✅ Завершено 2025-04-26 | sdk-partner-01; משוב по ספר בישול JS/Swift + בדיקות שפיות ל-ISO Bridge. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Завершено 2025-04-26 | sdk-partner-02; תאימות קוד 2025-04-11, תמונה על חיבור/טלמטריה. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Завершено 2025-04-26 | gateway-ops-01; аудит ops гайда gateway + анонимизированный поток נסה את זה proxy. |

Заполните **Приглашение отправлено** ו-**Ack** сразу после отправки письма.
Привяжите время к UTC расписанию, заданному в плане W1.

## מחסומי Телеметрические| Время (UTC) | לוחות מחוונים / בדיקות | Владелец | Результат | Артефакт |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ Все зеленое | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06-04-2025 18:20 | Транскрипт `npm run manage:tryit-proxy -- --stage preview-w1` | אופס | ✅ Подготовлено | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Дашборды выше + `probe:portal` | Docs/DevRel + Ops | ✅ תמונת מצב של הזמנה מוקדמת, без регрессий | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Дашборды выше + diff по латентности נסה זאת proxy | Docs/DevRel lead | ✅ בדיקת נקודת אמצע прошел (0 алертов; латентность נסה זאת p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Дашборды выше + בדיקה יציאה | Docs/DevRel + קשר ממשל | ✅ יציאה מתמונת מצב, нет активных алертов | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Ежедневные выборки שעות המשרד (2025-04-13 -> 2025-04-25) упакованы как NDJSON + PNG экспорты под
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` с именами файлов
`docs-preview-integrity-<date>.json` и соответствующими скриншотами.

## Лог отзывов и בעיות

Используйте эту таблицу для суммирования результатов מבקרים. Ссылайтесь на каждый элемент ב-GitHub/discuss
תקליט ופורמט צורה, מזדמן
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| הפניה | חומרה | בעלים | סטטוס | הערות |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | נמוך | Docs-core-02 | ✅ נפתר 2025-04-18 | Уточнены формулировка навигации נסה את זה + якорь סרגל צד (`docs/source/sorafs/tryit.md` обновлен новым תווית). |
| `docs-preview/w1 #2` | נמוך | Docs-core-03 | ✅ נפתר 2025-04-19 | Обновлены скриншот נסה את זה и подпись; חפץ `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | מידע | Docs/DevRel lead | סגור | Остальные комментарии были только שאלות ותשובות; зафиксированы в форме каждого партнера под `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## בדיקת ידע и סקרים

1. חידון Запишите результаты (цель >=90%) для каждого מבקר; прикрепите эксPORT CSV рядом с артефактами приглашений.
2. בדיקת סקר אופטימלי, משוב מפורט בפורמט, ובדיקת מידע
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Запланируйте תיקון звонки для тех, кто ниже порога, и отметьте их в этом файле.

Все восемь מבקרים набрали >=94% בבדיקת ידע (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). תיקון звонки не потребовались;
סקר יצוא עבור каждого партнера находятся под
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Инвентаризация артефактов

- מתאר/סיכום בדיקה מקדימה של חבילה: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- בדיקה סיכום + בדיקת קישור: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Лог изменений נסה זאת פרוקסי: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- ייצוא טלמטריה: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- חבילת טלמטריה יומית לשעות המשרד: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- משוב + ייצוא סקרים: размещать папки לכל סוקר под
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV לבדיקת ידע וסיכום: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Держите инвентарь синхронизированным с issue трекера. При копировании артефактов в тикет ממשל
прикладывайте хэши, чтобы аудиторы могли проверить файлы без shell-доступа.