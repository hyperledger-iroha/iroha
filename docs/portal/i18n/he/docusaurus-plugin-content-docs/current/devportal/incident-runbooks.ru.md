---
lang: he
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбуки инцидентов и тренировки חזרה לאחור

## Назначение

Пункт дорожной карты **DOCS-9** требует прикладных playbook'ов и плана репетиций, чтобы
операторы портала могли восстанавливаться после сбоев доставки без догадок. Эта заметка
охватывает три высокосигнальных инцидента - неудачные деплои, деградацию репликации и
сбои аналитики - и документирует квартальные тренировки, доказывающие, что rollback alias
и синтетическая валидация продолжают работать מקצה לקצה.

### Связанные материалы

- [`devportal/deploy-guide`](./deploy-guide) - זרימת עבודה упаковки, подписи וכינוי קידום.
- [`devportal/observability`](./observability) - תגי שחרור, ניתוחים ובדיקות, פעולות לא.
- `docs/source/sorafs_node_client_protocol.md`
  ו [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — телеметрия реестра и пороги эскалации.
- `docs/portal/scripts/sorafs-pin-release.sh` ועזרים `npm run probe:*`
  упомянуты в чеклистах.

### Общая телеметрия и инструменты

| Сигнал / Инструмент | Назначение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (נפגש/הוחמצה/בהמתנה) | Выявляет остановки репликации и нарушения SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Измеряет глубину backlog и задержку завершения לטריאג'. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Показывает сбои со стороны gateway, часто следующие за плохим. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | בדיקות סינתטיות, בדיקות שערים וחידושים. |
| `npm run check:links` | שער битых ссылок; используется после каждой הפחתה. |
| `sorafs_cli manifest submit ... --alias-*` (через `scripts/sorafs-pin-release.sh`) | קידום/כינוי החזרה של Механизм. |
| לוח `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Агрегирует סרבנות/כינוי/TLS/רפליקציה телеметрию. Алерты PagerDuty ссылаются на эти панели как на доказательства. |

## ראנבוק - Неудачный деплой или плохой артефакт

### Условия срабатывания

- תוכנית תצוגה מקדימה/הפקה של Пробы (`npm run probe:portal -- --expect-release=...`).
- התראות Grafana עבור `torii_sorafs_gateway_refusals_total` או
  השקה של `torii_sorafs_manifest_submit_total{status="error"}`.
- QA вручную замечает сломанные маршруты или сбои proxy נסה זאת сразу после
  כינוי קידום.

### Немедленное сдерживание

1. **Заморозить деплой:** отметить CI pipeline `DEPLOY_FREEZE=1` (זרימת עבודה של GitHub קלט)
   или приостановить ג'נקינס עבודה, чтобы новые артефакты не выходили.
2. **Зафиксировать артефакты:** скачать `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, ובדיקות בדיקה שנכשלו בבנייה,
   чтобы rollback ссылался на точные digests.
3. **Уведомить стейкхолдеров:** SRE אחסון, Docs/DevRel מוביל וקצין ניהול ממשל
   (особенно если затронут `docs.sora`).

### החזרה לאחור של Процедура

1. Определить המניפסט האחרון הידוע-טוב (LKG). זרימת עבודה של ייצור хранит их в
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Перепривязать alias к этому manifest с помощью shipping helper:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. החזרת תקציר לכרטיס אירוע вместе с digest LKG и неудачного מניפסט.

### Валидация

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` ו-`sorafs_cli proof verify ...`
   (מדריך לפריסה), чтобы убедиться, что מניפסט מקודם מחדש совпадает с архивным CAR.
4. `npm run probe:tryit-proxy` чтобы убедиться, что Try-It staging proxy вернулся.

### После инцидента1. Включить צינור деплоя только после понимания שורש הסיבה.
2. Дополнить раздел "לקחים שנלמדו" ב-[`devportal/deploy-guide`](./deploy-guide)
   новыми выводами, если есть.
3. פגמים Завести для провалившихся тестов (בדיקה, בודק קישורים, וכן т.д.).

## ראנבוק - Деградация репликации

### Условия срабатывания

- אלטרט: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` в течение 10 דקות.
- `torii_sorafs_replication_backlog_total > 10` в течение 10 דקות (см.
  `pin-registry-ops.md`).
- ממשל сообщает о медленной доступности כינוי после שחרור.

### טריאז'

1. Проверить לוחות מחוונים [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops), чтобы
   понять, локализован ли צבר אחסון במחלקת אחסון или в flet провайдеров.
2. Перепроверить логи Torii על `sorafs_registry::submit_manifest`, чтобы определить,
   не падают ли הגשות.
3. Выборочно проверить здоровье реплик через `sorafs_cli manifest status --manifest ...`
   (показывает исходы репликации по провайдерам).

### הקלה

1. Перевыпустить manifest с более высоким числом реплик (`--pin-min-replicas 7`) через
   `scripts/sorafs-pin-release.sh`, מתזמן чтобы распределил нагрузку на больший набор
   провайдеров. Зафиксировать новый digest в יומן אירועים.
2. Если backlog привязан к одному провайдеру, временно отключить его через מתזמן השכפול
   (описано в `pin-registry-ops.md`) и отправить новый מניפסט, принуждающий остальных
   провайдеров обновить כינוי.
3. Когда свежесть alias важнее parity репликации, התחבר מחדש כינוי на warm manifest уже в staging
   (`docs-preview`), затем опубликовать המשך מניפסט после очистки backlog SRE.

### שחזור וסגירה

1. Мониторить `torii_sorafs_replication_sla_total{outcome="missed"}` и убедиться, что
   счетчик стабилизировался.
2. Сохранить вывод `sorafs_cli manifest status` как הוכחות, что каждая реплика снова в норме.
3. צור או בטל צבר שכפול שלאחר המוות с дальнейшими шагами
   (масштабирование провайдеров, chunker כוונון, и т.д.).

## ראנבוק - Отключение аналитики или телеметрии

### Условия срабатывания

- `npm run probe:portal` מוצר, אין לוחות מחוונים перестают принимать события
  `AnalyticsTracker` עוד זמן של 15 דקות.
- סקירת פרטיות фиксирует неожиданный рост הורידו אירועים.
- `npm run probe:tryit-proxy` רכיב על מכשיר `/probe/analytics`.

### תגובה

1. צור כניסות בזמן בנייה: `DOCS_ANALYTICS_ENDPOINT` и
   `DOCS_ANALYTICS_SAMPLE_RATE` עם חפץ שחרור (`build/release.json`).
2. Перезапустить `npm run probe:portal` с `DOCS_ANALYTICS_ENDPOINT`, направленным
   על אספן בימוי, чтобы подтвердить, что tracker продолжает слать מטענים.
3. אוספים אספנות חדשים, התקן את `DOCS_ANALYTICS_ENDPOINT=""` ובניה מחדש,
   чтобы גשש קצר חשמלי; зафиксировать окно הפסקה בציר הזמן של תקרית.
4. Проверить, что `scripts/check-links.mjs` продолжает טביעת אצבע `checksums.sha256`
   (аналитические сбои *не* должны блокировать проверку מפת האתר).
5. После восстановления collector запустить `npm run test:widgets`, чтобы прогнать
   העוזר לניתוח בדיקות יחידה перед לפרסם מחדש.

### После инцидента1. Обновить [`devportal/observability`](./observability) с новыми ограничениями
   אספן или требованиями דגימה.
2. Выпустить הודעת ממשל, если данные analytics были потеряны или отредактированы
   вне политики.

## Квартальные учения по устойчивости

מקדחה Запускайте оба в **первый вторник каждого квартала** (ינואר/אפריל/יולי/אוקטובר)
или сразу после любого крупного изменения инфраструктуры. Храните артефакты в
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Учение | Шаги | Доказательства |
| ----- | ----- | -------- |
| Репетиция כינוי rollback | 1. פנה להחזרה של "פריסה נכשלה" עם מניפסט ייצור של самым свежим.<br/>2. חיבור מחדש בבדיקות ייצור после успешных.<br/>3. Сохранить `portal.manifest.submit.summary.json` и логи בדיקות в папке מקדחה. | `rollback.submit.json`, вывод בדיקות и שחרור תג репетиции. |
| Аудит синтетической валидации | 1. Запустить `npm run probe:portal` ו-`npm run probe:tryit-proxy` ייצור ובמה.<br/>2. Запустить `npm run check:links` ו архивировать `build/link-report.json`.<br/>3. Приложить צילומי מסך/ייצוא панелей Grafana, подтверждающих успех בדיקות. | Логи probes + `link-report.json` со ссылкой на מניפסט טביעת אצבע. |

Эскалируйте пропущенные drills менеджеру Docs/DevRel и на review governance SRE,
так как מפת הדרכים требует детерминированных квартальных доказательств, что alias rollback
и בדיקות פורטל остаются здоровыми.

## PagerDuty и כוננות координация

- סרוויס PagerDuty **Docs Portal Publishing** владеет алертами из
  `dashboards/grafana/docs_portal.json`. Правила `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` ו-`DocsPortal/TLSExpiry` מציגים Docs/DevRel הראשיים
  с Storage SRE как משני.
- При пейджинге укажите `DOCS_RELEASE_TAG`, приложите צילומי מסך затронутых
  панелей Grafana ואפשר לבצע בדיקה/בדיקת קישור לבדיקה.
  начала הקלה.
- הפחתת אמצעים (החזרה או פריסה מחדש) повторно запустите `npm run probe:portal`,
  `npm run check:links` וצילום תמונות Grafana, צילום מדדים
  в пороги. Приложите все הוכחות к инциденту PagerDuty до закрытия.
- Если два алерта срабатывают одновременно (לדוגמה, תפוגה של TLS או צבר הזמנות), сначала
  סירובי טריאז' (остановить פרסום), выполнить rollback, затем закрыть TLS/backlog
  с Storage SRE על גשר.