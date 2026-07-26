---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פריסת מפתח
כותרת: Заметки по развертыванию SoraFS
sidebar_label: Заметки по развертыванию
תיאור: Чеклист для продвижения пайплайна SoraFS из CI в продакшен.
---

:::note Канонический источник
:::

# Заметки по развертыванию

Пайплайн упаковки SoraFS усиливает детерминизм, поэтому переход из CI в продакшен в основном מעקות בטיחות. התקן את ההטמעה של ההשקה ב-gateways וספקי אחסון.

## Предварительная проверка

- **Выравнивание реестра** — убедитесь, что профили chunker и manifests ссылаются на одинаковый кортеж `namespace.name@semver` (010800007X).
- **כניסה לפוליטיקה** — הצג פרסומות של ספקים והוכחות כינוי, необходимые для `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **רישום PIN של Runbook** — התקן את `docs/source/sorafs/runbooks/pin_registry_ops.md` למשתמש עבור сценариев восстановления (כינוי ротация, сбои репликации).

## Конфигурация окружения

- Gateways должны включить הזרמת נקודות קצה (`POST /v1/sorafs/proof/stream`), чтобы CLI мог выпускать телеметрические сводки.
- תקשורת `sorafs_alias_cache` עם מוצר זמין עבור `iroha_config` או CLI עוזר (Prometheus).
- הפק אסימוני זרמים (אולי אחרים Torii) הם מנהל סודי ללא תשלום.
- יצואנים Включите телеметрические (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) ו- отправляйте их в ваш стек Prometheus.

## השקת Стратегия

1. **מניפסטים כחול/ירוק**
   - Используйте `manifest submit --summary-out` להפעלה של архивирования ответов каждого.
   - Следите за `torii_sorafs_gateway_refusals_total`, чтобы рано ловить несовместимость возможностей.
2. **הוכחות Проверка**
   - Считайте сбои в `sorafs_cli proof stream` блокерами развертывания; всплески латентности часто означают מצערת провайдера или неверно настроенные שכבות.
   - `proof verify` должен входить в בדיקת עשן после pin, чтобы удостовериться, что CAR у провайдеров все еще санспадетаетает.
3. **לוחות מחוונים טלמטרים**
   - מכשיר `docs/examples/sorafs_proof_streaming_dashboard.json` ב-Grafana.
   - הצג לוחות עבור רישום סיכות (`docs/source/sorafs/runbooks/pin_registry_ops.md`) וטווח נתחים סטטיסטיים.
4. **Включение ריבוי מקורות**
   - הפעל את ההפצה הקטנה של `docs/source/sorafs/runbooks/multi_source_rollout.md` עם מתזמר ולוח התוצאות של ארטאפקטים/טלטלים.

## Обработка инцидентов

- Следуйте путям эскалации в `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` עבור שער הפסקה ו-stream-token.
  - `dispute_revocation_runbook.md`, когда возникают споры репликации.
  - `sorafs_node_ops.md` для обслуживания на уровне нод.
  - `multi_source_rollout.md` לעקוף оркестратора, הוספת עמיתים לרשימה השחורה והשקת מקורות מידע.
- הוכחות הוכחות ואנומליות ב-GovernanceLog через существующие PoR Tracker API, чтобы governance моглаоцитьн провайдеров.

## Следующие шаги- Интегрируйте автоматизацию מתזמר (`sorafs_car::multi_fetch`), когда появится מתזמר אחזור רב מקורות (SF-6b).
- Отслеживайте обновления PDP/PoTR в рамках SF-13/SF-14; CLI и docs будут развиваться, чтобы отображать дедлайны и выбор tiers, когда эти הוכחות стабилизируются.

Объединив эти заметки по развертыванию с quickstart и CI рецептами, команды смогут перейти от локальнковики экальнкових צינורות SoraFS с повторяемым и наблюдаемым процессом.