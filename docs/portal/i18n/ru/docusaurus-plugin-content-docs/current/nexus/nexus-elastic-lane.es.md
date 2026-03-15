---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
Название: Provisionamiento de Lane Elastico (NX-7)
Sidebar_label: Обеспечение эластичной полосы движения
описание: Flujo de bootstrap для создания манифестов дорожки Nexus, ввода каталога и доказательств развертывания.
---

:::обратите внимание на Фуэнте каноника
Эта страница отражает `docs/source/nexus_elastic_lane.md`. Вам нужно будет скопировать информацию о том, что адрес локализации находится на портале.
:::

# Комплект подготовки эластичной дорожки (NX-7)

> **Элемент дорожной карты:** NX-7 — инструмент для подготовки эластичной дорожки  
> **Эстадо:** полный комплект инструментов — общие манифесты, фрагменты каталога, полезные нагрузки Norito, pruebas de humo,
> и помощник в пакете нагрузочного теста сейчас объединяет шлюзование задержки по слоту + манифесты доказательств для того, чтобы корриды грузов валидаторов
> Se publiquen грех писать сценарии Medida.

Это помощник для нового помощника `scripts/nexus_lane_bootstrap.sh`, который автоматизирует создание манифеста полосы, фрагментов каталога полосы/пространства данных и доказательств развертывания. Цель - облегчить альта-де-новые полосы Nexus (публичные или частные) без редактирования нескольких архивов вручную и повторения геометрии каталога вручную.

## 1. Prerrequisitos

1. Апробация управления для псевдонима полосы движения, пространства данных, в сочетании с валидаторами, терпимостью к падению (`f`) и политикой урегулирования.
2. Последний список валидаторов (идентификаторы документов) и список защищенных пространств имен.
3. Получите доступ к хранилищу конфигурации узла, чтобы можно было подключить сгенерированные фрагменты.
4. Внесите изменения в реестр манифестов полос (версии `nexus.registry.manifest_directory` и `cache_directory`).
5. Контакты телеметрии/ручки PagerDuty для этой полосы, чтобы оповещения были подключены, когда эта полоса находится в сети.

## 2. Общие артефакты на полосе

Вызов помощника из хранилища:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Flags clave:

- `--lane-id` может совпадать с индексом нового входа в `nexus.lane_catalog`.
- `--dataspace-alias` и `--dataspace-id/hash` управляют входом в каталог пространства данных (из-за неисправности в США идентификатор дорожки, если его пропустить).
- `--validator` можно повторить или прочитать из `--validators-file`.
- `--route-instruction` / `--route-account` выдает регулярные списки для ознакомления.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) захватывает контакты Runbook, чтобы панели мониторинга были исправлены владельцами.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` объединяет перехватчик обновления среды выполнения в манифесте, когда полоса требует расширения элементов управления.
- `--encode-space-directory` автоматически вызывает `cargo xtask space-directory encode`. Объедините с `--space-directory-out`, когда архив `.to` закодирован, чтобы он отличался от стандартного.

Скрипт создает три артефакта в месте `--output-dir` (из-за дефекта фактического каталога), но это дополнительная строка, когда вы умеете кодировать:1. `<slug>.manifest.json` — манифест полосы, содержащий кворум валидаторов, защищенные пространства имен и дополнительные метаданные для привязки обновления во время выполнения.
2. `<slug>.catalog.toml` — фрагмент TOML с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и различными правилами запроса информации. Убедитесь, что `fault_tolerance` настроен на входе в пространство данных для измерения коммита реле полосы движения (`3f+1`).
3. `<slug>.summary.json` - возобновляется аудитория, описывающая геометрию (срез, сегменты, метаданные) для требуемых шагов развертывания и точной команды `cargo xtask space-directory encode` (бахо `space_directory_encode.command`). Дополнением является JSON к регистрационному билету в качестве доказательства.
4. `<slug>.manifest.to` - излучается, когда `--encode-space-directory` активирован; список для гриппа `iroha app space-directory manifest publish` от Torii.

Используйте `--dry-run` для предварительного просмотра JSON/фрагментов без записи архивов и `--force` для описания существующих артефактов.

## 3. Применение камбиосов

1. Скопируйте манифест JSON в настроенный `nexus.registry.manifest_directory` (и в кэше каталога, и в реестре, отражающем удаленные пакеты). Соберите архив и манифесты, если они будут версии в вашем репозитории конфигурации.
2. Прикрепите фрагмент каталога `config/config.toml` (или соответствующий `config.d/*.toml`). Asegura que `nexus.lane_count` sea all menos `lane_id + 1` и актуализируется Cualquier `nexus.routing_policy.rules` que deba apuntar al nuevo Lane.
3. Кодифицируйте (если опустите `--encode-space-directory`) и опубликуйте манифест в Space Directory, используя команду, записанную в сводке (`space_directory_encode.command`). Это создаст полезную нагрузку `.manifest.to`, которую Torii проверит и зарегистрирует доказательства для аудиторов; envia с `iroha app space-directory manifest publish`.
4. Извлеките `irohad --sora --config path/to/config.toml --trace-config` и заархивируйте след трассировки в билете развертывания. Это означает, что новая геометрия совпадает с сформированными пулями/сегментами Куры.
5. Перейдите к валидаторам, назначенным по всем направлениям, в которых указаны списки манифеста/каталога esten desplegados. Сохраните сводку JSON в билете для будущих аудиторий.

## 4. Создайте пакет распределения реестра

Упакуйте сгенерированный манифест и наложение, чтобы операторы могли распространять данные управления полосами без редактирования конфигураций на каждом хосте. Помощник по объединению копий появляется в каноническом макете, создает дополнительное наложение каталога управления для `nexus.registry.cache_directory` и может создавать архив для переноса в автономном режиме:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Салидас:

1. `manifests/<slug>.manifest.json` — копия этих архивов в конфигурированном `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - это было в `nexus.registry.cache_directory`. Когда вы вводите `--module`, вы можете преобразовать определение модуля ввода, иметь возможность заменять модуль управления (NX-2), чтобы актуализировать наложение кэша и открыть его для редактирования `config.toml`.
3. `summary.json` — включает хэши, метаданные наложения и инструкции по эксплуатации.
4. Дополнительный `registry_bundle.tar.*` — список для SCP, S3 или трекеров артефактов.Синхронизация всего каталога (или архива) с каждым валидатором, дополнительными хостами с воздушным зазором и копированием манифестов + наложение кэша на руты реестра до обновления Torii.

## 5. Pruebas de humo para validadores

После установки Torii воспользуйтесь новым помощником по дымообразованию, чтобы проверить отчет о полосе движения `manifest_ready=true`, который покажет показатели, показывающие, что полосы движения и датчик герметичности являются свободными. Лейс-полосы, которые требуют манифеста, должны быть показаны `manifest_path` без отпуска; Помощник сразу же упал, когда произошел сбой, чтобы каждый раз, когда NX-7 включал в себя доказательства наличия фирмы:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Agrega `--insecure`, когда вы получаете самоподписанные сертификаты. Скрипт закончился с кодом без сертификата, если эта полоса фальта, эта метрика/телеметрия запечатана, если опреснено де лос valores esperados. Используйте ручки `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` и `--max-headroom-events` для управления телеметрией на полосе (альтура блока/финалидад/отставание/запас) в пределах ваших рабочих ограничений и в сочетании с `--max-slot-p95` / `--max-slot-p99` (как `--min-slot-samples`) для обеспечения выполнения задач по длительности слота NX-18 без использования помощника.

Для проверки с воздушным зазором (o CI) можно воспроизвести ответ Torii, захваченный и извлеченный из конечной точки в реальном времени:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Захваченные устройства `fixtures/nexus/lanes/` отражают созданные артефакты для помощника начальной загрузки, чтобы новые манифесты могли быть использованы при написании сценариев в медиде. CI запускает поток сообщений через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (псевдоним: `make check-nexus-lanes`) для демонстрации того, как помощник по дыму NX-7 находится в открытом состоянии с опубликованным форматом полезной нагрузки и для обеспечения безопасности дайджестов/наложений пакетов воспроизводимые по Мантенгану.