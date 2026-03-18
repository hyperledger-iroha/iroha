---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# انسیڈنٹ رن بکس اور رول بیک ڈرلز

## مقصد

روڈمیپ آئٹم آئٹم **DOCS-9** قابل عمل playbooks اور репетиция, когда вам нужно будет подготовиться к работе
Если вы хотите, чтобы это произошло, вы можете сделать это в любое время. یہ نوٹ تین ہائی سگنل
Проблемы — проблемы с развертыванием, ухудшение репликации, сбои в аналитике — проблемы с развертыванием
Ежеквартальные тренировки, например, откат псевдонимов и синтетическая проверка
اب بھی конец в конец کام کرتے ہیں۔

### متعلقہ مواد

- [`devportal/deploy-guide`](./deploy-guide) — рабочий процесс упаковки, подписания и продвижения псевдонимов.
- [`devportal/observability`](./observability) — теги выпуска, аналитика, зонды جن کا نیچے حوالہ ہے۔
- `docs/source/sorafs_node_client_protocol.md`
  اور [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — телеметрия реестра и пороги эскалации.
- `docs/portal/scripts/sorafs-pin-release.sh` и `npm run probe:*` помощники
  جو چیک لسٹس میں ریفرنس ہیں۔

### Инструменты для телеметрии

| Сигнал / Инструмент | مقصد |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (встречен/пропущен/ожидается) | остановки репликации и обнаружение нарушений SLA کرتا ہے۔ |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | глубина отставания, задержка завершения, сортировка, количественная оценка, количество |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | сбои на стороне шлюза |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | синтетические зонды جو выпускает ворота کرتے ہیں اور откаты подтверждают کرتے ہیں۔ |
| `npm run check:links` | сломанные ворота; ہر смягчение последствий или смягчение последствий |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعے) | механизм продвижения/возврата псевдонимов |
| Плата `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | отказы/псевдонимы/TLS/телеметрия репликации — совокупность данных Оповещения PagerDuty на панелях с доказательствами и ссылками на них |

## Runbook — возможность развертывания и артефакт артефакта

### شروع ہونے کی شرائط

- Сбой проб предварительного просмотра/производства (`npm run probe:portal -- --expect-release=...`).
- Grafana оповещения на `torii_sorafs_gateway_refusals_total` یا
  Развертывание `torii_sorafs_manifest_submit_total{status="error"}` کے بعد۔
- Продвижение псевдонимов вручную QA کے فوراً بعد неработающие маршруты یا Попробуйте сбои прокси-сервера نوٹ کرے۔

### فوری روک تھام

1. **Заморозка развертываний:** Конвейер CI: `DEPLOY_FREEZE=1` с отметкой (входные данные рабочего процесса GitHub)
   Приостановка работы Дженкинса и артефакты
2. **Артефакты фиксируют:** неудачная сборка `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, выходной сигнал пробника может быть отключен откат
   Дайджесты и ссылки
3. **Заинтересованные стороны:** SRE хранилища, руководитель отдела документации/разработчиков, а также ответственный за управление.
   (خصوصاً جب `docs.sora` متاثر ہو)۔

### رول بیک طریقہ کار

1. Манифест последнего известного удачного варианта (LKG) рабочий процесс производства
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` میں اسٹور کرتا ہے۔
2. Помощник по доставке (псевдоним, указанный в манифесте, связанный с привязкой):

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

3. Сводка отката, запись об инциденте и LKG, дайджесты неудавшихся манифестов.

### توثیق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. И18НИ00000040Х.
3. `sorafs_cli manifest verify-signature ...` или `sorafs_cli proof verify ...`
   (руководство по развертыванию) تاکہ تصدیق ہو کہ repromoted манифест, заархивированный CAR کے ساتھ match کرتا ہے۔
4. `npm run probe:tryit-proxy` Для промежуточного прокси-сервера Try-It### واقعے کے بعد

1. Основная причина возникновения конвейера развертывания или конвейера развертывания
2. [`devportal/deploy-guide`](./deploy-guide) Записи «Извлеченные уроки»: количество баллов, количество баллов, количество баллов.
3. Неудачный набор тестов (зонд, проверка ссылок, проверка) или наличие дефектов.

## Runbook — ریپلیکیشن میں گراوٹ

### شروع ہونے کی شرائط

- الرٹ: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  зажим_мин(сумма(torii_sorafs_replication_sla_total{outcome=~"выполнено|пропущено"}), 1)  15 минут для приема и загрузки.
- Проверка конфиденциальности удалила события.
- `npm run probe:tryit-proxy` `/probe/analytics` пути при неудаче ہو۔

### جوابی اقدامات

1. Входные данные во время сборки проверяют наличие: `DOCS_ANALYTICS_ENDPOINT` или `DOCS_ANALYTICS_SAMPLE_RATE`.
   артефакт сбоя выпуска (`build/release.json`)
2. `DOCS_ANALYTICS_ENDPOINT` — промежуточный сборщик в точке, где находится `npm run probe:portal` — выдает полезные нагрузки трекера
3. Коллекторы отключены или установлены `DOCS_ANALYTICS_ENDPOINT=""`, чтобы восстановить систему отслеживания короткого замыкания. Хронология инцидента в окне отключения میں ریکارڈ کریں۔
4. Подтвердите отпечаток пальца `scripts/check-links.mjs` или `checksums.sha256`.
   (перебои в работе аналитики или проверка карты сайта *блокировка* или отсутствие проверки)۔
5. Восстановление сборщика. Запуск `npm run test:widgets`. Запуск модульных тестов помощника по аналитике. Запуск повторной публикации.

### واقعے کے بعد1. [`devportal/observability`](./observability) Ограничения сборщика и требования к выборке данных
2. Политика использования аналитических данных. Удаление или редактирование. Уведомление об управлении.

## سہ ماہی استقامت کی مشقیں

Учения за **четвертую четверть месяца** (январь/апрель/июль/октябрь)
یا کسی بڑے изменение инфраструктуры کے فوراً بعد۔ артефакты
`artifacts/devportal/drills/<YYYYMMDD>/` کے تحت محفوظ کریں۔

| مشق | مراحل | شواہد |
| ----- | ----- | -------- |
| Откат псевдонима کی مشق | 1. Создание производственного манифеста и откат «Ошибочное развертывание» или откат2. зонды проходят процесс производства и повторную привязку3. `portal.manifest.submit.summary.json` Журналы зондирования и сверла | `rollback.submit.json`, выход пробника, репетиция или тег выпуска. |
| مصنوعی توثیق کا آڈٹ | 1. Производство и постановка `npm run probe:portal` или `npm run probe:tryit-proxy` چلائیں۔2. `npm run check:links` Архив `build/link-report.json` 3. Grafana панели کے скриншоты/экспорт прикрепить کریں جو подтверждение успеха проверки کریں۔ | Журналы зонда + `link-report.json` для отпечатков пальцев манифеста. |

Пропущенные тренировки: менеджер по документации/DevRel, проверка управления SRE, эскалация, дорожная карта, план действий.
откат псевдонима اور портальные зонды کے صحت مند رہنے کا детерминированный, ежеквартальные данные موجود ہو۔

## PagerDuty — координация по вызову

- Служба PagerDuty **Публикация портала документов** `dashboards/grafana/docs_portal.json` سے پیدا ہونے والے alerts کی مالک ہے۔
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, `DocsPortal/TLSExpiry` Основная страница Docs/DevRel.
  Вторичный SRE хранилища ہوتا ہے۔
- страница آنے پر `DOCS_RELEASE_TAG` کریں، متاثرہ Grafana панели کے скриншоты прикрепить کریں، اور смягчение последствий شروع کرنے سے پہلے
  выходные данные проверки/проверки канала; заметки об инцидентах; ссылка.
- смягчение последствий (откат и повторное развертывание) Для `npm run probe:portal`, `npm run check:links` можно использовать Grafana
  захват снимков, метрики и пороговые значения, параметры تمام доказательства инцидента PagerDuty کے ساتھ прикрепить کریں
  Как решить проблему
- Оповещения о срабатывании пожарной сигнализации (например, о сроке действия TLS и журнале невыполненной работы), а также об отказах и сортировке.
  (публикация) Процедура отката Использование хранилища SRE Создание моста для TLS/незавершенной работы