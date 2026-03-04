---
lang: he
direction: rtl
source: docs/portal/docs/nexus/operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operations
כותרת: Runbook по операциям Nexus
תיאור: Практичный полевой обзор рабочего процесса оператора Nexus, отражающий `docs/source/nexus_operations.md`.
---

Используйте эту страницу как быстрый справочник к `docs/source/nexus_operations.md`. Она концентрирует операционный чек-лист, точки управления изменениями и требования к телеметрии, котлетрыми אופציות Nexus.

## Чек-лист жизненного цикла

| Этап | Действия | Доказательства |
|-------|--------|--------|
| Предстарт | הצג את האפשרויות/הצגות, הצג את `profile = "iroha3"` ושלח את הקונפיגורציות. | Вывод `scripts/select_release_profile.py`, журнал checksum, подписанный bundle манифестов. |
| Выравнивание каталога | התקן את הקטלוג `[nexus]`, הסכם המכירות והפרוייקטים של ה-DA על הסכם הניהול, החודש תאריך I180200X. | Вывод `irohad --sora --config ... --trace-config`, сохраненный с тикетом onboarding. |
| עשן וחתך | הצג את `irohad --sora --config ... --trace-config`, הפעל את עשן CLI (`FindNetworkStatus`), הצע טלפונים לחנות וקבלת כניסה. | Лог עשן-test + подтверждение Alertmanager. |
| Штатный режим | התקן את לוחות המחוונים/ההתראות, התקן את המקשים על הניהול של המשחקים ותצורות ההפעלה/רונבוקס של המנהלים. | Протоколы квартального обзора, скриншоты дашбордов, ID тикетов ротации. |

Подробный onboarding (замена ключей, шаблоны маршрутизации, шаги профиля релиза) остается в Nexus.

## Управление изменениями

1. **Обновления релиза** - следите за объявлениями в `status.md`/`roadmap.md`; прикладывайте чек-лист onboarding к каждому PR релиза.
2. **Изменения манифестов lane** - הצג חבילות של Space Directory ו-Aрхивируйте их ב `docs/source/project_tracker/nexus_config_deltas/`.
3. **Дельты конфигурации** - каждое изменение `config/config.toml` требует тикет с ссылкой на lane/data-space. Сохраняйте редактированную копию эффективной конфигурации при הצטרף/שדרג узлов.
4. ** Тренировки rollback** - ежеквартально репетируйте עצור/שחזור/עשן; фиксируйте результаты в `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **תאימות סגולה** - נתיבים פרטיים/CBDC должны получить одобрение תאימות перед изменением политики DA или כפתורים редамктировия ( `docs/source/cbdc_lane_playbook.md`).

## טלמטרים ו SLOs

- דגמים: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, כמו גם SDK-специфичные виды (ראשי, `android_operator_console.json`).
- אלטרנטים: `dashboards/alerts/nexus_audit_rules.yml` и правила Torii/Norito הובלה (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Метрики для мониторинга:
  - `nexus_lane_height{lane_id}` - алерт при отсутствии прогресса три слота.
  - `nexus_da_backlog_chunks{lane_id}` - алерт выше порогов לנתיב (по умолчанию 64 ציבורי / 8 פרטי).
  - `nexus_settlement_latency_seconds{lane_id}` - алерт когда P99 превышает 900 ms (ציבורי) או 1200 ms (פרטי).
  - `torii_request_failures_total{scheme="norito_rpc"}` - אלטרט 5-minутная доля ошибок >2%.
  - `telemetry_redaction_override_total` - Sev 2 немедленно; убедитесь, что עוקף את התאימות של имеют тикеты.
- Выполняйте чек-лист телеметрии из [Nexus תוכנית תיקון טלמטריה](./nexus-telemetry-remediation) минимум ежеквартартально ит заполненную FORMу к заметкам операционного обзора.## Матрица инцидентов

| חומרה | Определение | Ответ |
|--------|------------|--------|
| סב 1 | Нарушение изоляции מרחב נתונים, התיישבות אוסטרלית > 15 דקות או ניהול ממשלתי. | Пейджинг Nexus Primary + Release Engineering + Compliance, заморозить קבלה, собрать артефакты, выпустить коммуникации <=60 миничии <=60 мин дней. |
| סב' 2 | צפו ב-SLA backlog lane, слепая зона телеметрии > 30 דקות, провал השקה מניפסט. | Пейджинг Nexus Primary + SRE, смягчение <=4 часа, צור מעקבים в течение 2 рабочих дней. |
| סוו 3 | Не блокирующий дрейф (מסמכים, התראות). | Зафиксировать в tracker и запланировать исправление в спринте. |

Тикеты инцидентов должны фиксировать затронутые מזהי נתיב/חלל נתונים, מניפים, טלפונים, תקליטורים/דירוגים, מעקב задачи/ответственных.

## Архив доказательств

- הצג חבילות/מניפסטים/טלפונים ניידים ב-`artifacts/nexus/<lane>/<date>/`.
- Сохраняйте редактированные configs + вывод `--trace-config` ל-каждого релиза.
- תמיכה בפרוטוקולים + סיוע בשירותים כלכליים או ארגונים.
- הצג צילומי מצב Prometheus על ידי Nexus בטכניקה של 12 חודשים.
- Фиксируйте правки runbook в `docs/source/project_tracker/nexus_config_deltas/README.md`, чтобы аудиторы знали, когда менялись обязанности.

## Связанные материалы

- מאפיינים: [סקירה כללית של Nexus](./nexus-overview)
- מפרט: [Nexus מפרט](./nexus-spec)
- נתיב Геометрия: [דגם נתיב Nexus](./nexus-lane-model)
- Переход и shims ניתוב: [Nexus הערות מעבר](./nexus-transition-notes)
- אופציית כניסה למטוס: [הכנסת מפעיל Sora Nexus](./nexus-operator-onboarding)
- Ремедиация телеметрии: [Nexus תוכנית תיקון טלמטריה](./nexus-telemetry-remediation)