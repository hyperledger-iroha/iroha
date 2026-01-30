---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 78437d302e0ac816784faef9dd7d29ac81054cbd7321a35901cebb5a579f4764
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Ранбуки инцидентов и тренировки rollback

## Назначение

Пункт дорожной карты **DOCS-9** требует прикладных playbook'ов и плана репетиций, чтобы
операторы портала могли восстанавливаться после сбоев доставки без догадок. Эта заметка
охватывает три высокосигнальных инцидента - неудачные деплои, деградацию репликации и
сбои аналитики - и документирует квартальные тренировки, доказывающие, что rollback alias
и синтетическая валидация продолжают работать end to end.

### Связанные материалы

- [`devportal/deploy-guide`](./deploy-guide) — workflow упаковки, подписи и promotion alias.
- [`devportal/observability`](./observability) — release tags, analytics и probes, упомянутые ниже.
- `docs/source/sorafs_node_client_protocol.md`
  и [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — телеметрия реестра и пороги эскалации.
- `docs/portal/scripts/sorafs-pin-release.sh` и helpers `npm run probe:*`
  упомянуты в чеклистах.

### Общая телеметрия и инструменты

| Сигнал / Инструмент | Назначение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | Выявляет остановки репликации и нарушения SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Измеряет глубину backlog и задержку завершения для triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Показывает сбои со стороны gateway, часто следующие за плохим deploy. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Синтетические probes, которые gate релизы и проверяют rollbacks. |
| `npm run check:links` | Gate битых ссылок; используется после каждой mitigation. |
| `sorafs_cli manifest submit ... --alias-*` (через `scripts/sorafs-pin-release.sh`) | Механизм promotion/revert alias. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | Агрегирует refusals/alias/TLS/replication телеметрию. Алерты PagerDuty ссылаются на эти панели как на доказательства. |

## Ранбук - Неудачный деплой или плохой артефакт

### Условия срабатывания

- Пробы preview/production падают (`npm run probe:portal -- --expect-release=...`).
- Grafana alerts на `torii_sorafs_gateway_refusals_total` или
  `torii_sorafs_manifest_submit_total{status="error"}` после rollout.
- QA вручную замечает сломанные маршруты или сбои proxy Try it сразу после
  promotion alias.

### Немедленное сдерживание

1. **Заморозить деплой:** отметить CI pipeline `DEPLOY_FREEZE=1` (input GitHub workflow)
   или приостановить Jenkins job, чтобы новые артефакты не выходили.
2. **Зафиксировать артефакты:** скачать `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, и вывод probes из failing build,
   чтобы rollback ссылался на точные digests.
3. **Уведомить стейкхолдеров:** storage SRE, lead Docs/DevRel и governance duty officer
   (особенно если затронут `docs.sora`).

### Процедура rollback

1. Определить last-known-good (LKG) manifest. Production workflow хранит их в
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

3. Записать summary rollback в incident ticket вместе с digest LKG и неудачного manifest.

### Валидация

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` и `sorafs_cli proof verify ...`
   (см. deploy guide), чтобы убедиться, что repromoted manifest совпадает с архивным CAR.
4. `npm run probe:tryit-proxy` чтобы убедиться, что Try-It staging proxy вернулся.

### После инцидента

1. Включить pipeline деплоя только после понимания root cause.
2. Дополнить раздел "Lessons learned" в [`devportal/deploy-guide`](./deploy-guide)
   новыми выводами, если есть.
3. Завести defects для провалившихся тестов (probe, link checker, и т.д.).

## Ранбук - Деградация репликации

### Условия срабатывания

- Алерт: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` в течение 10 минут.
- `torii_sorafs_replication_backlog_total > 10` в течение 10 минут (см.
  `pin-registry-ops.md`).
- Governance сообщает о медленной доступности alias после release.

### Triage

1. Проверить dashboards [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops), чтобы
   понять, локализован ли backlog в storage class или в fleet провайдеров.
2. Перепроверить логи Torii на `sorafs_registry::submit_manifest`, чтобы определить,
   не падают ли submissions.
3. Выборочно проверить здоровье реплик через `sorafs_cli manifest status --manifest ...`
   (показывает исходы репликации по провайдерам).

### Mitigation

1. Перевыпустить manifest с более высоким числом реплик (`--pin-min-replicas 7`) через
   `scripts/sorafs-pin-release.sh`, чтобы scheduler распределил нагрузку на больший набор
   провайдеров. Зафиксировать новый digest в incident log.
2. Если backlog привязан к одному провайдеру, временно отключить его через replication scheduler
   (описано в `pin-registry-ops.md`) и отправить новый manifest, принуждающий остальных
   провайдеров обновить alias.
3. Когда свежесть alias важнее parity репликации, rebind alias на warm manifest уже в staging
   (`docs-preview`), затем опубликовать follow-up manifest после очистки backlog SRE.

### Recovery & closure

1. Мониторить `torii_sorafs_replication_sla_total{outcome="missed"}` и убедиться, что
   счетчик стабилизировался.
2. Сохранить вывод `sorafs_cli manifest status` как evidence, что каждая реплика снова в норме.
3. Создать или обновить replication backlog post-mortem с дальнейшими шагами
   (масштабирование провайдеров, tuning chunker, и т.д.).

## Ранбук - Отключение аналитики или телеметрии

### Условия срабатывания

- `npm run probe:portal` проходит, но dashboards перестают принимать события
  `AnalyticsTracker` более чем на 15 минут.
- Privacy review фиксирует неожиданный рост dropped events.
- `npm run probe:tryit-proxy` падает на путях `/probe/analytics`.

### Response

1. Проверить build-time inputs: `DOCS_ANALYTICS_ENDPOINT` и
   `DOCS_ANALYTICS_SAMPLE_RATE` в release artifact (`build/release.json`).
2. Перезапустить `npm run probe:portal` с `DOCS_ANALYTICS_ENDPOINT`, направленным
   на staging collector, чтобы подтвердить, что tracker продолжает слать payloads.
3. Если collectors недоступны, установить `DOCS_ANALYTICS_ENDPOINT=""` и rebuild,
   чтобы tracker short-circuit; зафиксировать окно outage в incident timeline.
4. Проверить, что `scripts/check-links.mjs` продолжает fingerprint `checksums.sha256`
   (аналитические сбои *не* должны блокировать проверку sitemap).
5. После восстановления collector запустить `npm run test:widgets`, чтобы прогнать
   unit tests analytics helper перед republish.

### После инцидента

1. Обновить [`devportal/observability`](./observability) с новыми ограничениями
   collector или требованиями sampling.
2. Выпустить governance notice, если данные analytics были потеряны или отредактированы
   вне политики.

## Квартальные учения по устойчивости

Запускайте оба drill в **первый вторник каждого квартала** (Jan/Apr/Jul/Oct)
или сразу после любого крупного изменения инфраструктуры. Храните артефакты в
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Учение | Шаги | Доказательства |
| ----- | ----- | -------- |
| Репетиция alias rollback | 1. Повторить rollback "Failed deployment" с самым свежим production manifest.<br/>2. Re-bind на production после успешных probes.<br/>3. Сохранить `portal.manifest.submit.summary.json` и логи probes в папке drill. | `rollback.submit.json`, вывод probes и release tag репетиции. |
| Аудит синтетической валидации | 1. Запустить `npm run probe:portal` и `npm run probe:tryit-proxy` против production и staging.<br/>2. Запустить `npm run check:links` и архивировать `build/link-report.json`.<br/>3. Приложить screenshots/exports панелей Grafana, подтверждающих успех probes. | Логи probes + `link-report.json` со ссылкой на fingerprint manifest. |

Эскалируйте пропущенные drills менеджеру Docs/DevRel и на review governance SRE,
так как roadmap требует детерминированных квартальных доказательств, что alias rollback
и portal probes остаются здоровыми.

## PagerDuty и on-call координация

- Сервис PagerDuty **Docs Portal Publishing** владеет алертами из
  `dashboards/grafana/docs_portal.json`. Правила `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry` пейджат primary Docs/DevRel
  с Storage SRE как secondary.
- При пейджинге укажите `DOCS_RELEASE_TAG`, приложите screenshots затронутых
  панелей Grafana и добавьте вывод probe/link-check в заметки инцидента до
  начала mitigation.
- После mitigation (rollback или redeploy) повторно запустите `npm run probe:portal`,
  `npm run check:links` и сохраните свежие Grafana snapshots, показывающие возврат метрик
  в пороги. Приложите все evidence к инциденту PagerDuty до закрытия.
- Если два алерта срабатывают одновременно (например TLS expiry и backlog), сначала
  triage refusals (остановить publishing), выполнить rollback, затем закрыть TLS/backlog
  с Storage SRE на bridge.
