---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Журналы происшествий и упражнения по откату

## Предложение

Пункт дорожной карты **DOCS-9** содержит сборники сценариев, которые можно использовать больше всего в плане планирования для того, чтобы
Операторы портала могут восстановить данные, полученные от отправления, сем adivinhacao. Esta nota cobre tres
инциденты на высоком уровне - развертывает фальш, деградацию репликации и какие-либо аналитики - и
Документы, посвященные триместрам, которые подтверждают откат псевдонима и синтетическую валидацию
continuam funcionando от начала до конца.

### Связи с материалами

- [`devportal/deploy-guide`](./deploy-guide) — рабочий процесс упаковки, подписания и продвижения псевдонима.
- [`devportal/observability`](./observability) — теги выпуска, аналитика и зонды, на которые ссылаются abaixo.
- `docs/source/sorafs_node_client_protocol.md`
  е [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - телеметрия по реестру и ограничениям эскалонаменто.
- `docs/portal/scripts/sorafs-pin-release.sh` и помощники `npm run probe:*`
  контрольные списки контрольных номеров.

### Телеметрия и набор инструментов

| Синал / Инструмент | Предложение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (встречен/пропущен/ожидается) | Обнаружение блоков реплик и нарушений SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Количественное отставание и задержка завершения сортировки. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Чаще всего приходится делать шлюз, который часто затем развертывается. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Синтетические зонды, которые позволяют освободить и подтвердить откат. |
| `npm run check:links` | Ворота-де-линкс-кебрадос; usado apos cada mitigacao. |
| `sorafs_cli manifest submit ... --alias-*` (используется от `scripts/sorafs-pin-release.sh`) | Механизм продвижения/обратного использования псевдонима. |
| Плата `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Совокупная телеметрия отказов/псевдонимов/TLS/репликаций. Оповещения о вызове PagerDuty являются доказательствами. |

## Runbook — развертывание ошибочного или разрушенного артефакта

### Condicoes de disparo

- Пробники предварительного просмотра/производства изображения (`npm run probe:portal -- --expect-release=...`).
- Оповещения Grafana в `torii_sorafs_gateway_refusals_total` или
  `torii_sorafs_manifest_submit_total{status="error"}` в связи с развертыванием.
- Руководство по обеспечению качества, обратите внимание на ротацию или прокси-сервер. Попробуйте немедленно.
  рекламный псевдоним.

### Немедленное сообщение

1. **Congelar развертывает:** marcar o конвейер CI com `DEPLOY_FREEZE=1` (входной рабочий процесс
   GitHub) или приостановить работу Дженкинса, чтобы не получить результат.
2. **Захват артефактов:** baixar `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, и эти датчики собирают данные для того, чтобы
   o откат ссылок или дайджесты exatos.
3. **Уведомление заинтересованных сторон:** SRE хранилища, ведущий специалист по документации/разработке, дежурный офицер
   управление для повышения осведомленности (особенно, когда `docs.sora` оказало воздействие).

### Процедура отката

1. Идентификация последнего известного товара (LKG). Рабочий процесс производства вооружений
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Повторно укажите псевдоним в манифесте или помощнике по доставке:

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

3. Зарегистрируйтесь или возобновите откат, нет билета, сделайте инцидент, связанный с дайджестами ОС, сделайте
   манифест LKG и манифест с ошибкой.

### Валидакао1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. И18НИ00000040Х.
3. `sorafs_cli manifest verify-signature ...` и `sorafs_cli proof verify ...`.
   (подсказка или инструкция по развертыванию) для подтверждения продолжения повторного объявления
   batendo в составе автомобильного архива.
4. `npm run probe:tryit-proxy` для гарантии того, что прокси-сервер будет готов к использованию.

### Поз-инцидент

1. Реативность или трубопровод развертывания возможностей для создания причины.
2. Преенча в виде вступления «Извлеченные уроки» в [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos, se hover.
3. Обнаружение дефектов в наборе тестов (зонд, проверка ссылок и т. д.).

## Runbook — Ухудшение репликации

### Condicoes de disparo

- Оповещение: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  зажим_мин(сумма(torii_sorafs_replication_sla_total{outcome=~"выполнено|пропущено"}), 1) 15 минут.
- Проверка конфиденциальности после неожиданных событий.
- `npm run probe:tryit-proxy` содержит пути `/probe/analytics`.

### Репоста1. Проверьте входные данные сборки: `DOCS_ANALYTICS_ENDPOINT` e.
   `DOCS_ANALYTICS_SAMPLE_RATE` артефато не выпускается (`build/release.json`).
2. Повторно выполните `npm run probe:portal` com `DOCS_ANALYTICS_ENDPOINT` apontando para o.
   сборщик промежуточных данных для подтверждения того, что трекер отправляет полезные данные.
3. Если сборщики ОС отключены, определите `DOCS_ANALYTICS_ENDPOINT=""` и перестройте их.
   что произошло короткое замыкание на трекере; зарегистрируйте сообщение об отключении электроэнергии прямо сейчас
   делай темп, делай инцидент.
4. Подтвердите, что `scripts/check-links.mjs` и отпечаток пальца `checksums.sha256`
   (Чтобы аналитика *nao* разработала блокировку валидной карты сайта).
5. Когда напряжение коллектора, проехал `npm run test:widgets` для выполнения модульных испытаний.
   Сделайте помощника по аналитике перед переизданием.

### Поз-инцидент

1. Установите [`devportal/observability`](./observability) с новыми ограничениями для коллектора.
   или необходимые материалы.
2. Обратите внимание на то, что данные об аналитическом форуме потеряны или изменены.
   да политика.

## Тренировки триместра по устойчивости

Проведение учений в течение **первого третьего-летнего триместра** (январь/апрель/июль/выход)
или немедленно после того, как возникла большая инфраструктура. Армазенские артефатос эм
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Дрель | Пассос | Эвиденсия |
| ----- | ----- | -------- |
| Откат псевдонима | 1. Повторите или откатите «Развертывание файла», используя или последний созданный манифест.2. Повторно выполните сбор средств, которые проходят зонды.3. Регистратор `portal.manifest.submit.summary.json` и журналы тестов на пасту для сверления. | `rollback.submit.json`, указанные зонды, и тег выпуска do ensaio. |
| Синтетическая аудитория валидации | 1. Rodar `npm run probe:portal` и `npm run probe:tryit-proxy` против производства и постановки.2. Родар `npm run check:links` и архив `build/link-report.json`.3. Скриншоты/экспорт изображений Anexar Grafana подтверждают успешность зондирования. | Журналы зондирования + ссылки `link-report.json` или отпечатки пальцев отображаются. |

Эскалон проводит тренировки для менеджеров по документации/разработке и пересмотру управления SRE,
Pois или Roadmap Exige Evidencia Trimestral Determinista de que o Rollback de alias e os
зонды продолжают работу портала.

## Coordenacao PagerDuty и дежурство по вызову- Служба PagerDuty **Docs Portal Publishing** и отправляет оповещения, отправленные частично
  `dashboards/grafana/docs_portal.json`. Как указано в `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, и `DocsPortal/TLSExpiry` страницы или основные страницы Docs/DevRel
  com Storage SRE как вторичное.
- Когда отображается страница, включая `DOCS_RELEASE_TAG`, скриншоты приложений Grafana
  afetados e linke на указание зонда/проверки ссылок на заметки о происшествии до начала
  митигакао.
- Depois da mitigacao (откат или повторное развертывание), повторно выполните `npm run probe:portal`,
  `npm run check:links`, снимки состояния Grafana недавно добавлены в качестве показателей
  Пороги де Вольта АОС. Приложение к доказательствам инцидента с PagerDuty
  анте де резольвер.
- Если оповещения не совпадают с основным темпом (например, истекает время большей очереди TLS),
  сортировка отказов в первую очередь (во время публикации), выполнение процедуры отката и
  depois resolva TLS/backlog com Storage SRE на мосту.