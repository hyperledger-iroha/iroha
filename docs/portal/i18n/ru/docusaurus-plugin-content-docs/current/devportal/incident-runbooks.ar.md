---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# كتيبات الوادث وتمارين откат

## غرض

Он был создан в **DOCS-9** в здании, созданном в рамках проекта **DOCS-9**. مشغلو البوابة من
Это произошло в фильме "День Сэнсэй". تغطي هذه الملاحظة ثلاثة حوادث عالية الاشارة—فشل النشر،
Он отвечает на вопрос о том, как это сделать, — Уинстон Тэрри, Сэнсэй, откатывает لalias.
Он сказал, что это будет конец до конца.

### مواد ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) — псевдоним упаковки и подписи.
- [`devportal/observability`](./observability) — теги выпуска والتحليلات والprobes المذكورة ادناه.
- `docs/source/sorafs_node_client_protocol.md`
  و [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — телеметрия السجل وحدود التصعيد.
- `docs/portal/scripts/sorafs-pin-release.sh` и `npm run probe:*` помощники
  Он сказал, что это не так.

### القياس عن بعد والادوات المشتركة

| Сигнал / Инструмент | غرض |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (встречен/пропущен/ожидается) | В соответствии с соглашением об уровне обслуживания. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | В журнале невыполненной работы по сортировке الاكتمال لاغراض. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Откройте шлюз для установки и развертывания системы. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | зонды اصطناعية تقوم بعمل ворота للleases وتتحقق من откаты. |
| `npm run check:links` | بوابة الروابط المكسورة؛ Это поможет смягчить последствия. |
| `sorafs_cli manifest submit ... --alias-*` (название `scripts/sorafs-pin-release.sh`) | Псевдоним آلية ترقية/اعادة. |
| Плата `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Отказ телеметрии/псевдоним/TLS/репликация. Приложение PagerDuty создано в Кейптауне. |

## Runbook — в случае артефакта

### شروط الاطلاق

- Предварительная/производственная версия зондов (`npm run probe:portal -- --expect-release=...`).
- Проверьте Grafana или `torii_sorafs_gateway_refusals_total` او
  `torii_sorafs_manifest_submit_total{status="error"}` Развертывание.
- QA يلاحظ مسارات مكسورة او فشل proxy Попробуйте مباشرة بعد ترقية alias.

### حتواء فوري

1. **Просмотр:** выберите `DEPLOY_FREEZE=1` для конвейера CI (входной рабочий процесс на GitHub).
   На работе Дженкинса были найдены артефакты.
2. **Общие артефакты:** حمل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, тестирует сборку и откат сборки
   الى переваривает الدقيقة.
3. **Настройка хранилища:** хранилище SRE и руководство для Docs/DevRel и приложения для хранения данных.
   (خصوصا عند تاثير `docs.sora`).

### اجراء откат

1. Декларация о том, что الاخير المعروف انه جيد (LKG). Рабочий процесс
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Создайте псевдоним بهذا манифеста عبر helper الشحن:

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

3. Выполните откат для просмотра дайджестов манифеста LKG и манифеста.

### تحقق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. И18НИ00000040Х.
3. `sorafs_cli manifest verify-signature ...` и `sorafs_cli proof verify ...`.
   (Название фильма) Заявление об открытии автомобиля в автомобиле CAR.
4. `npm run probe:tryit-proxy` запускает прокси-сервер Try-It для промежуточного запуска.

### ما بعد الحادثة

1. Подключите трубопровод к трубопроводу, расположенному в помещении.
2. Сообщение «Извлеченные уроки» в [`devportal/deploy-guide`](./deploy-guide)
   Он сделал это.
3. Проверка дефектов (зонд, проверка ссылок, проверка).

## Runbook – تدهور النسخ

### شروط الاطلاق- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  зажим_мин(сумма(torii_sorafs_replication_sla_total{outcome=~"выполнено|пропущено"}), 1) <
  0,95` продолжительность 10 дней.
- `torii_sorafs_replication_backlog_total > 10` через 10 дней (от англ.
  `pin-registry-ops.md`).
- Выпущено издание под псевдонимом بعد.

### сортировка

1. Панели мониторинга [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
   отставание в работе с поставщиками услуг.
2. Установите флажок Torii и `sorafs_registry::submit_manifest` в случае необходимости.
   материалы تفشل.
3. Установите флажок `sorafs_cli manifest status --manifest ...` (подключенный поставщик).

### Смягчение последствий

1. Создайте манифест манифеста в приложении (`--pin-min-replicas 7`)
   `scripts/sorafs-pin-release.sh` используется планировщиком для всех поставщиков.
   Дайджест новостей для журнала «Спорт».
2. Наличие невыполненной работы с поставщиком услуг и созданием планировщика репликации.
   (Каждый `pin-registry-ops.md`) Отображает псевдоним провайдера جديد باقي.
3. Пользователь имеет псевдоним اهم من parity النسخ, اعد ربط alias الى Manifest دافئ.
   постановочный (`docs-preview`) и манифест, созданный в невыполненной работе SRE.

### التعافي والاغلاق

1. Установите флажок `torii_sorafs_replication_sla_total{outcome="missed"}`.
2. Установите `sorafs_cli manifest status`, чтобы установить копию реплики.
3. Анализ и посмертное исследование невыполненной работы по результатам анализа.
   (поставщики услуг, чанкеры и другие).

## Runbook - انقطاع التحليلات او القياس عن بعد

### شروط الاطلاق

- `npm run probe:portal` на приборных панелях تتوقف عن ابتلاع احداث
  `AnalyticsTracker` заработал 15 дней.
- Проверка конфиденциальности.
- `npm run probe:tryit-proxy` в случае с `/probe/analytics`.

### Новости

1. Проверьте входы: `DOCS_ANALYTICS_ENDPOINT` и
   `DOCS_ANALYTICS_SAMPLE_RATE` — артефакт артефакта (`build/release.json`).
2. Установите `npm run probe:portal` на `DOCS_ANALYTICS_ENDPOINT`.
   Коллекционер и постановка полезных нагрузок для трекера и трекера.
3. Коллекторы اذا كانت متوقفة, اضبط `DOCS_ANALYTICS_ENDPOINT=""` для восстановления
   Отслеживание короткого замыкания; Это сообщение отображается на временной шкале.
4. Установите флажок `scripts/check-links.mjs` и отпечаток пальца `checksums.sha256`.
   (انقطاعات التحليلات يجب *الا* تمنع التحقق на карте сайта).
5. Сборщик данных, `npm run test:widgets` для модульных тестов, помощник по аналитике
   قبل اعادة النشر.

### ما بعد الحادثة

1. Создайте [`devportal/observability`](./observability) для сбора данных
   متطلبات выборка.
2. Он сказал, что его ждет Сэнсэй Бэнкс в Вашингтоне.

## تمارين المرونة الربع سنوية

День независимости США ** в честь праздника ** (январь/апрель/июль/октябрь)
Он был убит Трэвисом Кейптауном в фильме «Старый мир». خزّن артефакты تحت
`artifacts/devportal/drills/<YYYYMMDD>/`.| تمرين | خطوات | دليل |
| ----- | ----- | -------- |
| تمرين откат للalias | 1. Выполните откат и нажмите «Ошибочное развертывание», чтобы отобразить манифест.2. Используйте датчики для проверки датчиков.3. Воспользуйтесь датчиками `portal.manifest.submit.summary.json` для проверки работоспособности. | `rollback.submit.json`, датчики и тег выпуска للتمرين. |
| تدقيق التحقق الاصطناعي | 1. Установите `npm run probe:portal` и `npm run probe:tryit-proxy` в промежуточном режиме.2. Установите `npm run check:links` и `build/link-report.json`.3. Скриншоты/экспорт можно загрузить с помощью зондов Grafana. | Проверьте датчики + `link-report.json` для проверки отпечатка пальца в манифесте. |

Доступ к документации Docs/DevRel и разработке SRE, а также локальной сети разработчиков. تتطلب
Нажмите кнопку «Откат» и откат псевдонимов и зонды для проверки.

## Работа с PagerDuty и по вызову

- خدمة PagerDuty **Docs Portal Publishing**
  `dashboards/grafana/docs_portal.json`. القواعد `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry` для страниц в Docs/DevRel.
  Доступ к Storage SRE.
- عند النداء, ارفق `DOCS_RELEASE_TAG`, а также скриншоты Grafana المتاثرة،
  Вы можете выполнить зондирование/проверку ссылок для предотвращения угроз и смягчения последствий.
- Устранение последствий (откат и повторное развертывание), а также добавление `npm run probe:portal`,
  `npm run check:links`, снимки экрана доступны для Grafana.
  Это не так. Откройте приложение PagerDuty и выберите его.
- После завершения срока действия TLS (истечение срока действия TLS и отставание) возникают отказы.
  (Публикация) и откат, а также TLS/backlog для Storage SRE и моста.