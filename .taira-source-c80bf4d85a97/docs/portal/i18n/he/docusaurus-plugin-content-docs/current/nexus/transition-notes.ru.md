---
lang: he
direction: rtl
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
כותרת: Заметки о переходе Nexus
תיאור: Зеркало `docs/source/nexus_transition_notes.md`, охватывающее доказательства перехода Phase B, график аудита и митигации.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Заметки о переходе Nexus

Этот лог отслеживает оставшуюся работу **Phase B - Nexus Transition Foundations** до завершения checklist многоlane запуска. Он дополняет אבן דרך записи в `roadmap.md` и держит доказательства, на которые ссылаются B1-B4, в однемом, метр. лиды SDK могли делиться единым источником истины.

## Область и קצב

- Покрывает מעקות בטיחות מנותבים ומעקבות טלמטריה (B1/B2), דלתא תצורה, ניהול אופנתי (B3), ומעקבים אחר חזרת שיגור רב-נתיב (B4).
- Заменяет временную תו קצב, которая была здесь раньше; с аудита Q1 2026 подробный отчет хранится в `docs/source/nexus_routed_trace_audit_report_2026q1.md`, а эта страница ведет рабочий график и миреги.
- אופציה למעקב אחר מסלול, או חזרת השקה. Когда артефакты перемещаются, отражайте новый путь здесь, чтобы מסמכים במורד הזרם (סטטוס, לוחות מחוונים, פורטלי SDK) могли ссилать ссилать.

## Снимок доказательств (2026 Q1-Q2)

| פוטוק | Доказательства | בעל/ים | Статус | Примечания |
|------------|--------|--------|--------|------|
| **B1 - ביקורת מעקב מנותב** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @ממשל | Завершено (Q1 2026) | Зафиксированы три окна аудита; TLS лаг `TRACE-CONFIG-DELTA` закрыт в Q2 שידור חוזר. |
| **B2 - תיקון טלמטריה ומעקות בטיחות** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Завершено | חבילת התראה, חבילת הבדל בוט או אצווה OTLP (`nexus.scheduler.headroom` יומן + מרווח ראש Grafana) נקודות; ויתור открытых нет. |
| **B3 - אישורי תצורת דלתא** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Завершено | Голосование GOV-2026-03-19 зафиксировано; חבילת подписанный питает חבילת טלמטריה ниже. |
| **B4 - חזרת שיגור רב-נתיב** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Завершено (Q2 2026) | Q2 canary שידור חוזר закрыл митигацию TLS лага; מניפסט validator + `.sha256` фиксирует слот-диапазон 912-936, seed עומס עבודה `NEXUS-REH-2026Q2` ו-hash профиля TLS בהפעלה חוזרת. |

## Квартальный график routed-trace аудита| מזהה מעקב | Окно (UTC) | Результат | Примечания |
|--------|--------------|--------|------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Пройдено | כניסה לתור P95 оставался значительно ниже цели <=750 ms. Действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Пройдено | Hashs של OTLP חוזר приложены к `status.md`; parity SDK diff bot подтвердил нулевой סחף. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Решено | TLS лаг закрыт в Q2 שידור חוזר; חבילת טלמטריה ל-`NEXUS-REH-2026Q2` фиксирует hash профиля TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (см. `artifacts/nexus/tls_profile_rollout_2026q2/`) ו-ноль отстающих. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Пройдено | seed עומס עבודה `NEXUS-REH-2026Q2`; חבילת טלמטריה + מניפסט/תקציר в `artifacts/nexus/rehearsals/2026q1/` (טווח חריצים 912-936) с agenda в `artifacts/nexus/rehearsals/2026q2/`. |

Будущие кварталы должны добавлять новые строки и переносить завершенные записи в приложение, копгди текущий квартал. Ссылайтесь на этот раздел из routed-trace отчетов или דקות ממשל через якорь `#quarterly-routed-trace-audit-schedule`.

## Митигации и backlog

| פריט | Описание | בעלים | Цель | Статус / Примечания |
|------|-------------|--------|--------|----------------|
| `NEXUS-421` | צפה בפרוטוקול TLS, שרת ב-`TRACE-CONFIG-DELTA`, צפה בהצגה חוזרת של ראיות ועבור בסין. | @release-eng, @sre-core | Q2 2026 routed-trace окно | Закрыто - hash TLS профиля `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` зафиксирован в `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; הרץ מחדש подтвердил отсутствие отстающих. |
| `TRACE-MULTILANE-CANARY` הכנה | Запланировать חזרת Q2, приложить fixtures к telemetry pack и убедиться, что SDK רתמות переиспользуют валидированный helper. | @telemetry-ops, תוכנית SDK | Планерка 2026-04-30 | Завершено - agenda хранится в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` עם חריץ/עומס עבודה של מטא נתונים; שימוש חוזר ברתמה отмечен в tracker. |
| סיבוב ערכת טלמטריה | Запускать `scripts/telemetry/validate_nexus_telemetry_pack.py` הצגת חזרות/הוצאה ותקצירים של עיכובים ב-tracker config delta. | @telemetry-ops | מועמד לשחרור На каждый | Завершено - `telemetry_manifest.json` + `.sha256` выпущены в `artifacts/nexus/rehearsals/2026q1/` (טווח חריצים `912-936`, זרע `NEXUS-REH-2026Q2`); digests скопированы в tracker и индекс доказательств. |

## חבילת דלתא תצורת Интеграция- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается каноническим הבדל סיכום. Когда приходят новые `defaults/nexus/*.toml` או изменения genesis, сначала обновляйте tracker, затем отражайте ключевыте ключевые.
- חבילת הגדרות תצורה питают חזרות טלמטריה. חבילה, валидированный `scripts/telemetry/validate_nexus_telemetry_pack.py`, должен публиковаться вместе с доказательствами config delta, чтобы операториз моглиз артефакты, использованные в B4.
- חבילות Iroha 2 אופציות ללא נתיבים: הגדרות с `nexus.enabled = false` теперь отклоняют עוקף נתיב/מרחב נתונים/ניתוב, если не вк1лю00X профиль (`--sora`), поэтому удаляйте секции `nexus.*` из шаблонов חד-נתיב.
- Держите лог голосования governance (GOV-2026-03-19) связанным и с tracker, и с этой заметкой, чтобы будущосов голие копировать формат без повторного поиска ритуала одобрения.

## מעקבים לאחר חזרת השקה

- `docs/source/runbooks/nexus_multilane_rehearsal.md` фиксирует canary план, רוסטר участников и шаги החזרה לאחור; обновляйте runbook при измениях топологии נתיבי או ליצואן телеметрии.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` פתח את הכתבות, התקדמו ב-9 באפריל, וערכו הערות הכנה/אג'נדה של Q2. Добавляйте будущие репетиции в тот же tracker вместо одноразовых tracker, чтобы доказательства были мононитон.
- הוספת קטעי אספן של OTLP ויצוא Grafana (см. `docs/source/telemetry.md`) при изменении יצואנית הנחיית אצווה; התקן את גודל האצווה של Q1 עד 256 דוגמאות, קבל התראות על מרווח ראש.
- Доказательства CI/tests multi-lane теперь живут в `integration_tests/tests/nexus/multilane_pipeline.rs` и запускаются через זרימת עבודה `Nexus Multilane Pipeline` (Prometheus), запускаются через устаревшую ссылку `pytests/nexus/test_multilane_pipeline.py`; держите hash `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) синхронизированным с tracker при обновлении חבילות חזרות.

## מחזור חיים של נתיב זמן ריצה

- מחזור החיים של מסלולי זמן ריצה של מסלולי ריצה מאפשרים חיבורי מרחב נתונים ואפשרות התאמה של אחסון Kura/שכבות, ללא תקלות. Helpers очищают кэшированные ממסרי נתיבים לנתיבים בדימוס, чтобы סינתזה של מיזוג חשבונות ללא הוכחות.
- Применяйте планы через Nexus עוזרי תצורה/מחזור חיים (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) לנתיבי добавления/выводареста бес; ניתוב, צילומי TEU ורשמי מניפסט перезагружаются автоматически после успешного плана.
- Руководство оператора: при сбое плана проверьте отсутствие מרחבי נתונים או שורשי אחסון, которые не удается создать (ספריות שורש קרות/מסלולי Kura). Исправьте базовые пути и повторите; успешные планы повторно эмитят טלמטריה הבדל נתיב/מרחב נתונים, чтобы לוחות מחוונים отражали новую топологию.

## NPoS телеметрия ולחץ אחורי דוק

חזרת השקה של Ретро Phase B . רתמה Интеграционный в `integration_tests/tests/sumeragi_npos_performance.rs` прогоняет эти сценарии и выводит סיכומי JSON (`sumeragi_baseline_summary::<scenario>::...`) при поянныов. Запуск локально:```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Установите `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` או `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`, чтобы исследовать более стрессовые топологии; значения по умолчанию отражают профиль 1 s/`k=3`, использованный в B4.

| Сценарий / מבחן | Покрытие | Ключевая телеметрия |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | בלוק 12 פרקים עם זמן חסימת החזרות, ניתן למצוא מעטפות חביון של EMA, גליונות או מדי שליחת מיותרים מציגים אוסף ראיות מרובות. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Переполняет очередь транзакций, чтобы гарантировать детерминированный запуск דחיות קבלה וקיבולת эксPORT счетчиков. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | סמל ריצוד של קוצב לב וצפייה בתקופות זמן קצובות, לא ניתן להבחין ב-+/-125 פרמיל. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | צור עומסי RBC דרך חנות לימיטוב רכה/קשה, צריך חנות לאחסון קבצים, מאחסן וסטטיסטיקה של סשן/בתים. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | פורצי נתונים, מדידות יחס שליחה יתירות ומונים אספנים על-פי מטרה מספקים, מקצה לקצה מקצה לקצה. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Детерминированно дропает chunks, чтобы backlog monitors поднимали תקלות вместо тихого дренажа מטענים. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Приложите JSON линии, которые выводит רתמה, вместе с Prometheus scrape, захваченным во время запрауской, вся просит доказательства, что אזעקות לחץ גב соответствуют חזרות топологии.

## Чеклист обновления

1. Добавляйте новые routed-trace окна и переносите старые по мере смены кварталов.
2. Обновляйте таблицу митигаций после каждого מעקב Alertmanager, даже если действие - закрыть тикет.
3. קובצי הגדרות דלתות меняются, עוקבים עוקבים, תקצירים ומספרים מעכלים חבילת טלמטריה בבקשת משיכה אודנית.
4. צור קשר עם חזרות/טלמטריה חדשות, מפת הדרכים של מפת הדרכים הוצעה לדרך חדשה. ad-hoc заметки.

## Индекс доказательств| Актив | Локация | Примечания |
|-------|----------------|-------|
| דוח ביקורת מנותב (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник доказательств שלב B1; зеркалируется в портал через `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Config Delta Tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Содержит TRACE-CONFIG-DELTA סיכומי הבדל, инициалы סוקרים и лог голосования GOV-2026-03-19. |
| תוכנית תיקון טלמטריה | `docs/source/nexus_telemetry_remediation_plan.md` | חבילת התראה, גודל אצווה OTLP ומעקות בטיחות בתקציב יצוא, связанные с B2. |
| גשש חזרות רב-נתיבי | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | תיאור הכתבה לתאריך 9 באפריל, מניפסט/תקציר מאמת, הערות/סדר יום של רבעון 2 והוכחות להחזרה. |
| מניפסט/תקציר של חבילת טלמטריה (האחרונה ביותר) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | טווח חריצים Фиксирует 912-936, seed `NEXUS-REH-2026Q2` ו-hashs артефактов חבילות ממשל. |
| מניפסט פרופיל TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash утвержденного TLS профиля, захваченный в Q2 שידור חוזר; ссылайтесь в ניתוב-עקבות נספחים. |
| סדר יום TRACE-Multilanine-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Плановые заметки לחזרה של Q2 (חלון, טווח משבצות, סיד עומס עבודה, בעלי פעולה). |
| השקת ספר חזרות | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Операционный чеклист בימוי -> ביצוע -> חזרה לאחור; обновлять при изменении топологии נתיבים או ליצואנים הנחיה. |
| אימות ערכת טלמטריה | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI, упомянутый в B4 ретро; архивируйте digests вместе с tracker при любом изменении חבילת. |
| רגרסיה רב מסלולית | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Проверяет `nexus.enabled = true` עבור תצורות מרובות נתיבים, сохраняет גיבובים של קטלוג Sora и провижнит נתיבי Kura/מיזוג-יומן נתיבים מקומיים (`blocks/lane_{id:03}_{slug}`) че10дер50 Iп10д0X публикацией חפץ מעכל. |