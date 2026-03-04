---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks по инцидентам и упражнениям по откату

## Предложение

Пункт дорожной карты **DOCS-9** представляет собой список действий, который является практическим планом для этого.
Оперативники портала могут восстановиться после провала входа в мир без приключений. Esta nota
кубре трех инцидентов на высоком уровне: падение падений, деградация репликации и
аналитические материалы и документальные данные о триместрах упражнений, которые нужно выполнить, чтобы откатить назад
псевдоним и валидация синтетики, которые функционируют от начала до конца.

### Связи с материалами

- [`devportal/deploy-guide`](./deploy-guide) - упаковка, подпись и продвижение псевдонима.
- [`devportal/observability`](./observability) — теги выпуска, аналитические и зондирующие ссылки.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - телеметрия регистратуры и зон эскаламиенто.
- `docs/portal/scripts/sorafs-pin-release.sh` и помощники `npm run probe:*`
  референсиадос в контрольных списках.

### Телеметрия и отсек для инструментов

| Сенал / Инструмент | Предложение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (встречен/пропущен/ожидается) | Обнаружение блокировок репликации и нарушений SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Cuantifica la глубокая невыполненная работа и задержка завершения для сортировки. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Муэстра проваливается из шлюза, когда меню обнаруживается при неправильном развертывании. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Синтетические зонды, которые позволяют выпускать и выполнять откаты. |
| `npm run check:links` | Gate de enlaces rotos; если США будут смягчены каждый раз. |
| `sorafs_cli manifest submit ... --alias-*` (используется от `scripts/sorafs-pin-release.sh`) | Механизм продвижения/возврата псевдонима. |
| Плата `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Совокупная телеметрия отказов/псевдонимов/TLS/репликации. Справочные оповещения PagerDuty представляют собой панели в качестве доказательств. |

## Runbook — ошибка или неправильный артефакт

### Условия ухода за кожей

- Выполнены тесты предварительного просмотра/производства (`npm run probe:portal -- --expect-release=...`).
- Оповещения Grafana и `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` после развертывания.
- Руководство по контролю качества: обнаружение рутинных операций или падений прокси-сервера. Попробуйте немедленно после этого.
  продвижение псевдонима.

### Немедленный спор

1. **Уточняйте:** Маркируйте конвейер CI с `DEPLOY_FREEZE=1` (вход рабочего процесса
   GitHub) или приостановите работу Дженкинса, чтобы не избавиться от артефактов.
2. **Захват артефактов:** удаление `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, и я проверил зонды сборки, которые упали, чтобы они
   откат ссылки на дайджесты точных.
3. **Уведомление заинтересованных сторон:** SRE хранилища, руководитель документации/разработчика и официальный представитель охраны.
   gobernanza для повышения осведомленности (особенно, когда `docs.sora` сильно пострадал).

### Процедура отката

1. Идентификационный манифест последнего знания (LKG). Рабочий процесс производства лос-гарда
   бахо `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Повторно укажите псевдоним в этом манифесте с помощью помощника по отправке:

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

3. Зарегистрируйте возобновление отката в билете инцидента с дайджестами.
   манифест LKG и манифест падения.

### Проверка1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. И18НИ00000040Х.
3. `sorafs_cli manifest verify-signature ...` и `sorafs_cli proof verify ...`
   (ver la guia de despliegue) для подтверждения того, что манифест повторно рекламируется
   совпадение с архивом CAR.
4. `npm run probe:tryit-proxy` для проверки промежуточной проверки прокси-сервера Try-It.

### После инцидента

1. Реабилитация трубопровода спасения в одиночку после возникновения причины роста.
2. Сообщение о «Извлеченных уроках» на [`devportal/deploy-guide`](./deploy-guide)
   с новыми заметками, если это применимо.
3. Обнаружены дефекты для проверки ошибок (зонд, проверка ссылок и т. д.).

## Runbook — деградация репликации

### Условия ухода за кожей

- Оповещение: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  зажим_мин(сумма(torii_sorafs_replication_sla_total{outcome=~"выполнено|пропущено"}), 1) 15 минут.
- Проверка конфиденциальности обнаружила непредвиденное событие в удаленных событиях.
- `npm run probe:tryit-proxy` попадает в пути `/probe/analytics`.

### Респуэста1. Проверка входных данных сборки: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` в артефакте выпуска (`build/release.json`).
2. Повторно извлеките `npm run probe:portal` с `DOCS_ANALYTICS_ENDPOINT` apuntando al.
   сборщик промежуточных данных для подтверждения того, что трекер излучает полезные нагрузки.
3. Если коллекционеры уже не знают, установите `DOCS_ANALYTICS_ENDPOINT=""` и перестройте
   из-за короткого замыкания трекера; регистрация аварийного отключения в ла
   линия времени происшествия.
4. Подтвердите, что `scripts/check-links.mjs` имеет отпечатки пальцев `checksums.sha256`.
   (las caidas de analitica *no* deben bloquear la validacion del sitemap).
5. Когда коллектор будет восстановлен, вставьте `npm run test:widgets` для его извлечения.
   модульные тесты помощника по аналитике перед публикацией.

### После инцидента

1. Актуализировать [`devportal/observability`](./observability) с новыми ограничениями
   сборщик необходимых вещей.
2. Сообщите об этом правительству, если вы потеряете его или издадите аналитические данные
   де политика.

## Тренировки триместральной устойчивости

Учения Ejecuta ambos в течение **первого марта календарного триместра** (вторник/апрель/июль/октябрь)
o немедленно после того, как мэр инфраструктуры станет мэром. Артефакты Бахо Guarda
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Дрель | Пасос | Эвиденсия |
| ----- | ----- | -------- |
| Откат псевдонима | 1. Повторите откат «Despliegue Fallido», используя полученный манифест производства.2. Повторно сообщите о производстве, которое было выполнено.3. Регистратор `portal.manifest.submit.summary.json` и журналы датчиков на ковре сверла. | `rollback.submit.json`, отключите датчики и отпустите тег del ensayo. |
| Синтетическая аудитория валидации | 1. Ejecutar `npm run probe:portal` y `npm run probe:tryit-proxy` против производства и постановки.2. Извлеките `npm run check:links` и архивируйте `build/link-report.json`.3. Дополнительные снимки экрана/экспорт панелей Grafana для подтверждения выхода из зонда. | Журналы проверки + ссылка `link-report.json` на отпечаток пальца манифеста. |

Проведите обучение менеджера по документации/DevRel и пересмотру управления SRE,
я знаю, что дорожная карта exige evidencia trimestral determinista de que elrollback de alias y los
зонды дель портала являются полезными.

## Координация дежурства пейджера и дежурства по вызову- Служба PagerDuty **Публикация портала документов** предоставляется из-за общих предупреждений
  `dashboards/grafana/docs_portal.json`. Лас-реглас `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry` страница основной документации/DevRel
  с хранилищем SRE как вторичное.
- На этой странице есть `DOCS_RELEASE_TAG`, а также дополнительные скриншоты панелей Grafana.
  afectados y enlaza la salida de Probe/проверка ссылок в примечаниях к инцидентам перед
  начальное смягчение.
- В случае смягчения последствий (отката или повторного развертывания) повторно извлеките `npm run probe:portal`,
  `npm run check:links`, снимки снимков Grafana фрески Mostrando las metricas
  de nuevo dentro de umbrales. Дополнение ко всем доказательствам инцидента с PagerDuty
  до разрешения.
- Если оповещения не совпадают со временем (например, по истечении срока действия TLS в большом отставании), сортировка
  отказы Primero (публикация задержания), ejecuta el procedimiento de откат, luego
  Освободите элементы TLS/отставания с SRE хранилища на мосту.