---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4dd795704b314435fc5d94be48c36aea7205396e71d3d81c49fdcc25137d09d8
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-elastic-lane
title: Эластичное выделение lane (NX-7)
sidebar_label: Эластичное выделение lane
description: Bootstrap-процесс для создания манифестов lane Nexus, записей каталога и доказательств rollout.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_elastic_lane.md`. Держите обе копии синхронизированными, пока волна переводов не попадет в портал.
:::

# Набор инструментов для эластичного выделения lane (NX-7)

> **Пункт roadmap:** NX-7 - инструменты эластичного выделения lane  
> **Статус:** инструменты завершены - генерируют манифесты, фрагменты каталога, Norito payloads, smoke tests,
> а helper для load-test bundle теперь связывает гейтинг по slot latency + манифесты доказательств, чтобы прогоны нагрузки валидаторов
> можно было публиковать без кастомных скриптов.

Этот гайд проводит операторов через новый helper `scripts/nexus_lane_bootstrap.sh`, который автоматизирует генерацию манифестов lane, фрагментов каталога lane/dataspace и доказательств rollout. Цель - легко поднимать новые Nexus lanes (public или private) без ручного редактирования нескольких файлов и без ручного пересчета геометрии каталога.

## 1. Предварительные требования

1. Governance-одобрение для alias lane, dataspace, набора валидаторов, fault tolerance (`f`) и политики settlement.
2. Финальный список валидаторов (account IDs) и список защищенных namespaces.
3. Доступ к репозиторию конфигураций узлов, чтобы добавить сгенерированные фрагменты.
4. Пути для реестра манифестов lane (см. `nexus.registry.manifest_directory` и `cache_directory`).
5. Контакты телеметрии/PagerDuty handles для lane, чтобы алерты были подключены сразу после онлайна.

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
- `--dataspace-alias` и `--dataspace-id/hash` управляют записью dataspace в каталоге (по умолчанию используется id lane).
- `--validator` можно повторять или читать из `--validators-file`.
- `--route-instruction` / `--route-account` выводят готовые к вставке правила маршрутизации.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) фиксируют контакты runbook, чтобы dashboards показывали правильных владельцев.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` добавляют hook runtime-upgrade в манифест, когда lane требует расширенных операторских контролей.
- `--encode-space-directory` автоматически вызывает `cargo xtask space-directory encode`. Используйте вместе с `--space-directory-out`, если хотите положить кодированный `.to` в другое место.

Скрипт создает три артефакта в `--output-dir` (по умолчанию текущий каталог) и опционально четвертый при включенном encoding:

1. `<slug>.manifest.json` - манифест lane с quorum валидаторов, защищенными namespaces и опциональными метаданными hook runtime-upgrade.
2. `<slug>.catalog.toml` - TOML-фрагмент с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и запрошенными правилами маршрутизации. Убедитесь, что `fault_tolerance` задан в записи dataspace, чтобы размерить lane-relay комитет (`3f+1`).
3. `<slug>.summary.json` - аудит-сводка с геометрией (slug, сегменты, метаданные), требуемыми шагами rollout и точной командой `cargo xtask space-directory encode` (под `space_directory_encode.command`). Приложите этот JSON к тикету onboarding как доказательство.
4. `<slug>.manifest.to` - появляется при `--encode-space-directory`; готов для потока Torii `iroha app space-directory manifest publish`.

Используйте `--dry-run`, чтобы посмотреть JSON/фрагменты без записи файлов, и `--force` для перезаписи существующих артефактов.

## 3. Примените изменения

1. Скопируйте JSON манифеста в настроенный `nexus.registry.manifest_directory` (и в cache directory, если registry зеркалит удаленные bundles). Закоммитьте файл, если манифесты версионируются в конфигурационном репозитории.
2. Добавьте фрагмент каталога в `config/config.toml` (или в подходящий `config.d/*.toml`). Убедитесь, что `nexus.lane_count` не меньше `lane_id + 1`, и обновите `nexus.routing_policy.rules`, которые должны указывать на новую lane.
3. Закодируйте (если пропустили `--encode-space-directory`) и опубликуйте манифест в Space Directory, используя команду из summary (`space_directory_encode.command`). Это создает `.manifest.to`, который ожидает Torii, и фиксирует доказательства для аудиторов; отправьте через `iroha app space-directory manifest publish`.
4. Запустите `irohad --sora --config path/to/config.toml --trace-config` и архивируйте вывод trace в тикете rollout. Это подтверждает, что новая геометрия соответствует сгенерированным slug/Kura сегментам.
5. Перезапустите валидаторы, назначенные на lane, после развертывания изменений манифеста/каталога. Сохраните summary JSON в тикете для будущих аудитов.

## 4. Соберите bundle распределения реестра

Упакуйте сгенерированный манифест и overlay, чтобы операторы могли распространять данные governance по lanes без правок конфигов на каждом хосте. Helper упаковки копирует манифесты в канонический layout, создает опциональный governance catalog overlay для `nexus.registry.cache_directory` и может собрать tarball для офлайн-передачи:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Выходы:

1. `manifests/<slug>.manifest.json` - копируйте их в настроенный `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - кладите в `nexus.registry.cache_directory`. Каждая запись `--module` становится подключаемым определением модуля, позволяя замену governance-модуля (NX-2) через обновление cache overlay вместо редактирования `config.toml`.
3. `summary.json` - содержит хеши, метаданные overlay и инструкции для операторов.
4. Опционально `registry_bundle.tar.*` - готово для SCP, S3 или трекеров артефактов.

Синхронизируйте весь каталог (или архив) на каждый валидатор, распакуйте на air-gapped хостах и скопируйте манифесты + cache overlay в их пути реестра перед перезапуском Torii.

## 5. Smoke tests валидаторов

После перезапуска Torii запустите новый smoke helper, чтобы проверить, что lane сообщает `manifest_ready=true`, метрики показывают ожидаемое количество lanes и gauge sealed чист. Lanes, которым нужны манифесты, обязаны иметь непустой `manifest_path`; helper теперь мгновенно падает при отсутствии пути, чтобы каждый деплой NX-7 включал доказательство подписанного манифеста:

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

Добавьте `--insecure` при тестировании self-signed окружений. Скрипт завершается ненулевым кодом, если lane отсутствует, sealed, или метрики/телеметрия расходятся с ожидаемыми значениями. Используйте `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` и `--max-headroom-events`, чтобы удерживать телеметрию по lane (высота блока/финальность/backlog/headroom) в допустимых рамках, и сочетайте их с `--max-slot-p95` / `--max-slot-p99` (и `--min-slot-samples`) для соблюдения целей NX-18 по длительности слота, не выходя из helper.

Для air-gapped проверок (или CI) можно проигрывать сохраненный ответ Torii вместо обращения к живому endpoint:

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

Записанные fixtures в `fixtures/nexus/lanes/` отражают артефакты, создаваемые bootstrap helper, чтобы новые манифесты можно было lint-ить без кастомных скриптов. CI гоняет тот же поток через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`), чтобы убедиться, что smoke helper NX-7 остается совместимым с опубликованным форматом payload и что digests/overlays bundle остаются воспроизводимыми.
