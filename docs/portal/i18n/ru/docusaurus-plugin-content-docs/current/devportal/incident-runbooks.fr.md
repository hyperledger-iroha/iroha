---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks d'incident и инструкции по откату

## Объектив

Пункт дорожной карты **DOCS-9** требует создания сборников действий и плана повторения для этого
les операторы du portail puissent recuperer des echecs de livraison sans deviner. Эта заметка
три инцидента и сигнал форта - скорость развертывания, деградация репликации и т. д.
Pannes d'analytics - и документируйте триместры, которые подтверждают откат псевдонимов
и синтетическая проверка работает во время боя в бою.

### Материальное соединение

- [`devportal/deploy-guide`](./deploy-guide) - рабочий процесс упаковки, подписания и продвижения псевдонима.
- [`devportal/observability`](./observability) — теги выпуска, аналитика и зондирование ссылок ci-dessous.
- `docs/source/sorafs_node_client_protocol.md`
  и др [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - телеметрия регистрации и отслеживания событий.
- `docs/portal/scripts/sorafs-pin-release.sh` и помощники `npm run probe:*`
  ссылки в контрольных списках.

### Телеметрия и детали оснастки

| Сигнал / Выход | Объектив |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (встречен/пропущен/ожидается) | Обнаруживайте блокировки репликации и нарушения SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Определите объем отставания и задержку завершения сортировки. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Montre les echecs представляет собой шлюз, который обеспечивает скорость развертывания. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Проверяет синтетические данные, которые позволяют выпускать релизы и действительные откаты. |
| `npm run check:links` | кассы Gate de liens; используйте смягчение последствий апре-шака. |
| `sorafs_cli manifest submit ... --alias-*` (обернутый номиналом `scripts/sorafs-pin-release.sh`) | Механизм продвижения/возврата псевдонима. |
| Плата `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Объедините отказы/псевдонимы/TLS/репликацию телеметрии. Предупреждения PagerDuty ссылаются на предварительные сведения. |

## Runbook — скорость развертывания или дефекты артефактов

### Условия снижения

- Предварительный/производственный эхо зондов (`npm run probe:portal -- --expect-release=...`).
- Оповещения Grafana о `torii_sorafs_gateway_refusals_total` или
  `torii_sorafs_manifest_submit_total{status="error"}` после развертывания.
- Руководство по контролю качества для кассет маршрутов или панелей прокси. Попробуйте сразу после обеда.
  продвижение псевдонима.

### Немедленное заключение

1. **Собрать развертывания:** отметить конвейер CI с `DEPLOY_FREEZE=1` (вход рабочего процесса
   GitHub) или приостановить работу Дженкинса для создания нечастого артефакта.
2. **Захват артефактов:** телезарядное устройство `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, и вылазка из зондов, которые вы строите и выполняете в ближайшее время
   ссылка на откат файлов дайджеста точна.
3. **Уведомление сторон о предстоящем:** хранилище SRE, ведущая документация/разработка и т. д.
   управление для повышения осведомленности (surtout si `docs.sora` оказывает влияние).

### Процедура отката

1. Идентификатор последнего известного удачного манифеста (LKG). Рабочий процесс производства продуктов су
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Укажите псевдоним в манифесте с помощником по доставке:

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

3. Зарегистрируйте резюме отката в заявке об инциденте с дайджестами.
   манифест LKG и др. манифест в электронном виде.

### Проверка1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. И18НИ00000040Х.
3. `sorafs_cli manifest verify-signature ...` и `sorafs_cli proof verify ...`
   (смотрите руководство по развертыванию) для подтверждения соответствия манифеста
   архив toujours au CAR.
4. `npm run probe:tryit-proxy`, чтобы гарантировать, что прокси-сервер Try-It принесет доход.

### После инцидента

1. Реакция уникального конвейера развертывания после предотвращения возникновения причины расизма.
2. Mettre a Jour les Entrees «Извлеченные уроки» в [`devportal/deploy-guide`](./deploy-guide)
   с новыми точками, если это так.
3. Создание дефектов для набора тестов и проверок (зонд, проверка ссылок и т. д.).

## Runbook — деградация репликации

### Условия снижения

- Оповещение: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  зажим_мин(сумма(torii_sorafs_replication_sla_total{outcome=~"выполнено|пропущено"}), 1)  15 минут.
- Проверка конфиденциальности обнаруживает случаи отсутствия внимания на заброшенных мероприятиях.
- `npm run probe:tryit-proxy` отражается по путям `/probe/analytics`.

### Ответ1. Проверка входных данных сборки: `DOCS_ANALYTICS_ENDPOINT` и др.
   `DOCS_ANALYTICS_SAMPLE_RATE` в артефакте выпуска (`build/release.json`).
2. Повторный исполнитель `npm run probe:portal` с `DOCS_ANALYTICS_ENDPOINT` pointant vers le
   сборщик промежуточных данных для подтверждения того, что трекер получает полезные нагрузки.
3. Если сборщики запишут, определите `DOCS_ANALYTICS_ENDPOINT=""` и восстановите его.
   que le следящий контур суда; грузоотправитель в случае отключения электроэнергии в рамках графика времени.
4. Подтвердите, что `scripts/check-links.mjs` продолжит отпечаток пальца `checksums.sha256`.
   (панели аналитики не должны блокировать проверку карты сайта).
5. Чтобы восстановить сборщик файлов, используйте lancer `npm run test:widgets` для выполнения файлов.
   модульные тесты помощника по аналитике перед публикацией.

### После инцидента

1. Mettre a jour [`devportal/observability`](./observability) с новыми ограничениями
   Коллекционер или требования к отбору проб.
2. Уведомление об управлении с помощью аналитики, связанной с потерей или редактированием
   вне политики.

## Тренировка триместров устойчивости

Lancer les deux Drills во время **премьер-марди-де-шак триместра** (январь/апрель/июль/октябрь)
или немедленно после всех изменений в инфраструктуре. Stocker les artefacts sous
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Дрель | Этапы | Преуве |
| ----- | ----- | -------- |
| Повторение отката псевдонима | 1. Обновите откат «Скорости развертывания» с файлом манифеста производства плюс недавние.2. Ответьте на вопрос, что зонды прошли.3. Регистратор `portal.manifest.submit.summary.json` и журналы проверок в досье бурения. | `rollback.submit.json`, выполните поиск и отпустите тег повторения. |
| Синтетический аудит | 1. Lancer `npm run probe:portal` и `npm run probe:tryit-proxy` в производстве и постановке.2. Lancer `npm run check:links` и архиватор `build/link-report.json`.3. Объединение снимков экрана/экспорта panneaux Grafana, подтверждающее успех зондирования. | Журналы зондирования + `link-report.json`, ссылающийся на отпечаток пальца манифеста. |

Отрабатывайте упражнения для менеджеров Docs/DevRel и проверяйте управление SRE, автомобиль
Дорожная карта определяет предварительный триместр, определяющий откат псевдонимов и зондов
портал restent sains.

## Координация PagerDuty и дежурство- Служба PagerDuty **Публикация на портале документов** позволяет получать отложенные оповещения.
  `dashboards/grafana/docs_portal.json`. Правила `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry` на первой странице Docs/DevRel
  avec Storage SRE на втором месте.
- На этой странице добавьте `DOCS_RELEASE_TAG`, а также скриншоты Panneaux.
  Grafana влияет и на вылазку/проверку соединения в предварительных заметках об инциденте
  начало смягчения последствий.
- Послеоперационное смягчение (откат или повторное развертывание), повторный исполнитель `npm run probe:portal`,
  `npm run check:links` и захват моментальных снимков Grafana для монтажа метрик
  доходы в les seuils. Присоединяйтесь к доказательствам инцидента PagerDuty
  авангардное разрешение.
- Если два оповещения уменьшаются в темпе мема (например, истечение срока действия TLS плюс невыполненная работа), триггер
  отказы в премьере (арретер публикации), исполнитель процедуры отката, puis
  предатель TLS/отставание с хранилищем SRE на мосту.