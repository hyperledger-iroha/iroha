---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Elastic-Lane
Название: Эластичное выделение пер. (NX-7)
sidebar_label: Эластичное выделение lane
описание: Bootstrap-процесс для создания манифестов полосы Nexus, каталога записей и развертывания доказательств.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_elastic_lane.md`. Держите копии синхронизированными, пока волна переводов не попадет на портал.
:::

# Набор инструментов для эластичного выделения lane (NX-7)

> **Пункт roadmap:** NX-7 - инструменты эластичного выделения lane  
> **Статус:** инструменты завершены — генерируют манифесты, фрагменты каталога, полезные нагрузки Norito, дымовые тесты,
> Помощник для пакета нагрузочного тестирования теперь связывает гейтинг по задержке слота + манифестные доказательства, чтобы прогнать нагрузку валидаторов
> можно было публиковать без кастомных скриптов.

Этот гайд проводит операторов через новый помощник `scripts/nexus_lane_bootstrap.sh`, который автоматизирует генерацию полос манифестов, фрагментов каталога дорожек/пространства данных и развертывание доказательств. Цель - легко поднять новые полосы Nexus (общедоступные или частные) без ручного редактирования нескольких файлов и без ручного пересчета геологического каталога.

## 1. Предварительные требования

1. Управление-одобрение для псевдонимов, пространства данных, набора валидаторов, отказоустойчивости (`f`) и политики урегулирования.
2. Финальный список валидаторов (account IDs) и список защищенных namespaces.
3. Доступ к реконструкции узлов конфигурации, чтобы добавить сгенерированные фрагменты.
4. Пути для реестра манифестов lane (см. `nexus.registry.manifest_directory` и `cache_directory`).
5. Свяжитесь с ручками телеметрии/PagerDuty для полосы, чтобы оповещения были подключены сразу после онлайна.

## 2. Сгенерируйте артефакты lane

Запустите helper из корня репозитория:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Ключевые флаги:

- `--lane-id` должен совпадать с index новой записи в `nexus.lane_catalog`.
- `--dataspace-alias` и `--dataspace-id/hash` управляют пространством данных записи в каталоге (по умолчанию используется id Lane).
- `--validator` можно повторять или читать из `--validators-file`.
- `--route-instruction` / `--route-account` выводят готовые к вставке правила маршрутизации.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) фиксируют контакты Runbook, чтобы информационные панели показывали правильные владельцы.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` включает в манифест перехватчик runtime-upgrade, когда полоса требует расширения операторских контроллеров.
- `--encode-space-directory` автоматически вызывает `cargo xtask space-directory encode`. Используйте вместе с `--space-directory-out`, если хотите добавить кодированный `.to` в другое место.

Скрипт создаёт три артефакта в `--output-dir` (по умолчанию настоящего каталога) и опционально четвёртый при включенном кодировании:1. `<slug>.manifest.json` — манифест с кворумом валидаторов, защищенными пространствами имен и опциональными метаданными хуками runtime-upgrade.
2. `<slug>.catalog.toml` - TOML-фрагмент с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и запрошенными стандартными маршрутизациями. Убедитесь, что `fault_tolerance` задано в записи dataspace, чтобы увеличить комитет Lane-Relay (`3f+1`).
3. `<slug>.summary.json` - аудит-сводка с геометрией (слизень, сегменты, метаданные), требуемыми шагами развертывания и точной командой `cargo xtask space-directory encode` (под `space_directory_encode.command`). Приложите этот JSON к тикету регистрации в качестве доказательства.
4. `<slug>.manifest.to` - появление при `--encode-space-directory`; готов к потоку Torii `iroha app space-directory manifest publish`.

Используйте `--dry-run` для просмотра JSON/фрагментов без записи файлов и `--force` для перезаписи существующих документов.

## 3. Применить изменения

1. Скопируйте JSON-манифест в настроенный `nexus.registry.manifest_directory` (и в каталог кэша, если реестр зеркалит удаленные пакеты). Зафиксируйте файл, если версии манифеста составлены в конфигурационном репозитории.
2. Добавьте фрагмент каталога в `config/config.toml` (или в соответствующий `config.d/*.toml`). Убедитесь, что `nexus.lane_count` не меньше `lane_id + 1`, и обновите `nexus.routing_policy.rules`, которые должны следовать по новой полосе.
3. Закодируйте (если пропустили `--encode-space-directory`) и опубликуйте манифест в Space Directory, используя команду из сводки (`space_directory_encode.command`). Он создает `.manifest.to`, который ожидает Torii, и фиксирует документы для аудиторов; отформатировать через `iroha app space-directory manifest publish`.
4. Запустите `irohad --sora --config path/to/config.toml --trace-config` и заархивируйте вывод трассировки в развертывании тикета. Это подтверждается, что новая геометрия соответствует сгенерированным сегментам пули/Кура.
5. Перезапустите валидаторы, назначенные на полосу, после развертывания изменений манифеста/каталога. Сохраните сводку JSON в заявке для будущих аудитов.

## 4. Соберите пакетное распределение реестра

Упакуйте сгенерированный манифест и наложение, чтобы операторы могли распространять управление данными по каналам без правок конфигов на каждом хосте. Помощник по производству копирует манифесты в стандартном формате, создает оверлей каталога опционального управления для `nexus.registry.cache_directory` и может собрать tarball для офлайн-передач:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Результаты:

1. `manifests/<slug>.manifest.json` - скопируйте их в настроенный `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - кладите в `nexus.registry.cache_directory`. каждая запись `--module` становится подключаемым определением модуля, что приводит к замене модуля управления (NX-2) через наложение кэша обновления вместо редактирования `config.toml`.
3. `summary.json` - содержит хеши, метаданные оверлеи и инструкции для операторов.
4. Опционально `registry_bundle.tar.*` — готово для SCP, S3 или трекеров.

Синхронизируйте весь каталог (или архив хоста) на каждом валидаторе, распакуйте на изолированных сайтах и ​​скопируйте манифесты + наложение кеша в их реестре пути перед перезапуском Torii.

##5. Дымовые тесты валидаторовПосле перезапуска Torii запустите новый дымовой помощник, чтобы проверить, что полоса сообщает `manifest_ready=true`, метрики отображают ожидаемое количество полос и датчик запечатан в чистоте. Дорожки, которым нужны манифесты, теперь имеют непустой `manifest_path`; helper теперь мгновенно падает при отсутствии пути, чтобы каждое развертывание NX-7 основывалось на подтверждении подписанного манифеста:

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

Добавьте `--insecure` при тестировании самоподписанных окружений. Скрипт завершается ненулевым кодом, если полоса отсутствует, запечатана или метрики/телеметрия расходуются с ожидаемыми значениями. Используйте `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` и `--max-headroom-events`, чтобы удерживать телеметрию по полосе (высота блока/финальность/отставание/запас) в допустимых пределах, и совмещайте их с `--max-slot-p95` / `--max-slot-p99` (и `--min-slot-samples`) Для соблюдения требований NX-18 по долговечности слота, не выходя из помощника.

Для тестов с воздушным зазором (или CI) можно проигрывать сохраненный ответ Torii вместо обращения к живой конечной точке:

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

Записанные фикстуры в `fixtures/nexus/lanes/` отражают артефакты, созданные помощником начальной загрузки, чтобы новые манифесты можно было линтить без кастомных скриптов. CI гонит тот же поток через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (псевдоним: `make check-nexus-lanes`), чтобы убедиться, что Smoke Helper NX-7 совместим с опубликованным форматом полезной нагрузки и что пакет дайджестов/наложений продолжает воспроизводиться.