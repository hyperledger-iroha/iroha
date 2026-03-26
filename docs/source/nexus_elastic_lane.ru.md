---
lang: ru
direction: ltr
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

# Инструментарий эластичного провижининга lane (NX-7)

> **Пункт roadmap:** NX-7 — инструменты эластичного провижининга lane  
> **Статус:** Tooling завершен — генерирует manifests, catalog snippets, Norito payloads, smoke tests,
> и helper для load-test bundle теперь соединяет gating по latencia slots + manifests доказательств,
> чтобы валидаторные load runs можно было публиковать без bespoke scripting.

Этот гайд проводит операторов через helper `scripts/nexus_lane_bootstrap.sh`, который автоматизирует
генерацию manifests для lane, snippets каталога lane/dataspace и доказательств rollout. Цель —
упростить запуск новых Nexus lanes (public или private) без ручного редактирования множества файлов
или повторного вывода геометрии каталога.

## 1. Пререквизиты

1. Одобрение управления для alias lane, dataspace, набора валидаторов, fault tolerance (`f`) и
   policy settlement.
2. Финальный список валидаторов (account IDs) и список защищенных namespace.
3. Доступ к репозиторию конфигураций узлов, чтобы добавить сгенерированные snippets.
4. Пути для registry manifests lane (см. `nexus.registry.manifest_directory` и `cache_directory`).
5. Контакты телеметрии/PagerDuty для lane, чтобы алерты можно было сразу подключить.

## 2. Генерация артефактов lane

Запустите helper из корня репозитория:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
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

- `--lane-id` должен совпадать с индексом новой записи в `nexus.lane_catalog`.
- `--dataspace-alias` и `--dataspace-id/hash` управляют записью в каталоге dataspace (по умолчанию
  используется lane id, если опущено).
- `--validator` можно повторять или передавать через `--validators-file`.
- `--route-instruction` / `--route-account` генерируют готовые правила routing.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) фиксируют контакты runbook,
  чтобы дашборды показывали корректных owners.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` добавляют hook runtime-upgrade в manifest,
  когда lane требует расширенных операторских контролей.
- `--encode-space-directory` автоматически вызывает `cargo xtask space-directory encode`. Используйте
  `--space-directory-out`, если хотите записать `.to` в другой путь.

Скрипт создает три артефакта внутри `--output-dir` (по умолчанию текущий каталог), плюс опциональный
четвертый при включенном encoding:

1. `<slug>.manifest.json` — manifest lane с quorum валидаторов, защищенными namespace и опциональной
   metadata hook runtime-upgrade.
2. `<slug>.catalog.toml` — TOML snippet с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и
   запрошенными routing правилами. Убедитесь, что `fault_tolerance` задан в записи dataspace для
   sizing комитета lane-relay (`3f+1`).
3. `<slug>.summary.json` — audit summary с геометрией (slug, сегменты, metadata), шагами rollout и
   точной командой `cargo xtask space-directory encode` (в `space_directory_encode.command`).
   Приложите этот JSON к тикету onboarding как доказательство.
4. `<slug>.manifest.to` — создается при `--encode-space-directory`; готов для
   `iroha app space-directory manifest publish` в Torii.

Используйте `--dry-run` для предпросмотра JSON/snippets без записи файлов и `--force` для
перезаписи существующих артефактов.

## 3. Применение изменений

1. Скопируйте manifest JSON в настроенный `nexus.registry.manifest_directory` (и в cache directory,
   если registry зеркалит remote bundles). Сделайте commit файла, если manifests версионируются в
   вашем repo конфигураций.
2. Добавьте snippet каталога в `config/config.toml` (или `config.d/*.toml`). Убедитесь, что
   `nexus.lane_count` не меньше `lane_id + 1`, и обновите `nexus.routing_policy.rules`, которые
   должны ссылаться на новую lane.
3. Сгенерируйте encoding (если вы не использовали `--encode-space-directory`) и опубликуйте manifest
   в Space Directory командой из summary (`space_directory_encode.command`). Это создает payload
   `.manifest.to`, ожидаемый Torii, и фиксирует доказательства для аудиторов; отправьте через
   `iroha app space-directory manifest publish`.
4. Запустите `irohad --sora --config path/to/config.toml --trace-config` и архивируйте вывод trace
   в тикете rollout. Это доказывает, что геометрия совпадает со сгенерированными slug/kura сегментами.
5. Перезапустите валидаторы, назначенные на lane, после deploy изменений manifest/catalog. Храните
   summary JSON в тикете для будущих аудитов.

## 4. Сборка registry distribution bundle

После готовности manifest, catalog snippet и summary упакуйте их для распространения валидаторам.
Новый bundler копирует manifests в layout, ожидаемый
`nexus.registry.manifest_directory` / `cache_directory`, выпускает governance catalog overlay, чтобы
модули можно было менять без правок основного config, и опционально архивирует bundle:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Результаты:

1. `manifests/<slug>.manifest.json` — копируйте в `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — положите в `nexus.registry.cache_directory` для override или
   замены governance modules (`--module ...` override кешированный catalog). Это путь plug-in модуля
   для NX-2: замените определение модуля, перезапустите bundler и разошлите cache overlay без
   изменений `config.toml`.
3. `summary.json` — содержит SHA-256 / Blake2b digests для каждого manifest плюс overlay metadata.
4. Опциональный `registry_bundle.tar.*` — готов для Secure Copy / хранения артефактов.

Если деплой зеркалит bundles на air-gapped хосты, синхронизируйте весь output dir (или tarball).
Online узлы могут монтировать manifest directory напрямую, а offline узлы используют tarball,
распаковывают и копируют manifests + cache overlay в настроенные пути.

## 5. Smoke tests валидаторов

После перезапуска Torii запустите smoke helper, чтобы убедиться, что lane сообщает
`manifest_ready=true`, метрики показывают ожидаемое количество lanes, и gauge sealed чист. Lanes,
которые требуют manifest, теперь должны показывать непустой `manifest_path` — helper быстро
заваливается, если доказательство отсутствует, чтобы NX-7 change controls включали ссылки на
подписанные bundles автоматически:

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
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Добавьте `--insecure` при тестировании self-signed окружений. Скрипт завершается с non-zero, если
lane отсутствует, запечатана, или метрики/телеметрия отклоняются от ожидаемых значений. Используйте
новые knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` и
`--max-headroom-events`, чтобы удерживать телеметрию height/finality/backlog/headroom в
операционных пределах. Сочетайте с `--max-slot-p95/--max-slot-p99` (и `--min-slot-samples`) для
SLO длительности слота NX-18 прямо в helper. Передавайте `--allow-missing-lane-metrics` только если
staging кластеры еще не экспонируют эти gauges (в проде оставляйте defaults).

Этот же helper теперь enforce телеметрию scheduler load-test. Используйте `--min-teu-capacity`,
чтобы доказать, что каждая lane репортит `nexus_scheduler_lane_teu_capacity`, ограничьте утилизацию
слота через `--max-teu-slot-commit-ratio` (сравнивает `nexus_scheduler_lane_teu_slot_committed` с
capacity), и держите счетчики deferral/truncation на нуле через `--max-teu-deferrals` и
`--max-must-serve-truncations`. Эти knobs превращают требование NX-7 о "deeper validator load tests"
в повторяемый CLI чек: helper падает, когда lane откладывает PQ/TEU работу или когда committed TEU
на слот превышает заданный headroom, а CLI печатает сводку по lane, чтобы evidence пакеты
фиксировали те же числа, что и CI.

Для air-gapped валидаций (или CI) можно воспроизвести захваченный ответ Torii вместо обращения к
live узлу:

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
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Фикстуры в `fixtures/nexus/lanes/` повторяют артефакты, созданные bootstrap helper, чтобы новые
manifests можно было lint без bespoke scripting. CI гоняет тот же поток через
`ci/check_nexus_lane_smoke.sh` и также запускает `ci/check_nexus_lane_registry_bundle.sh`
(alias: `make check-nexus-lanes`), чтобы подтвердить совместимость smoke helper NX-7 с опубликованным
форматом payload и обеспечить воспроизводимость digests/overlays.

Когда lane переименована, захватите telemetry события `nexus.lane.topology` (например,
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) и подайте их в smoke helper.
Новый флаг `--telemetry-file/--from-telemetry` принимает newline-delimited лог, а
`--require-alias-migration old:new` проверяет, что событие `alias_migrated` зафиксировало rename:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
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
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

Фикстура `telemetry_alias_migrated.ndjson` включает канонический пример rename, чтобы CI могла
проверить telemetry parsing без обращения к live узлу.

## 6. Validator load tests (доказательства NX-7)

Roadmap **NX-7** требует, чтобы операторы lane фиксировали воспроизводимый load run до того, как
lane отмечена production ready. Цель — нагрузить lane достаточно, чтобы проверить slot duration,
settlement backlog, DA quorum, oracles, scheduler headroom и TEU metrics, а затем архивировать
результат так, чтобы аудиторы могли воспроизвести его без bespoke tooling. Новый helper
`scripts/nexus_lane_load_test.py` объединяет smoke checks, slot-duration gating и slot bundle
manifest в один набор артефактов, чтобы load runs можно было публиковать напрямую в governance
тикеты.

### 6.1 Подготовка workload

1. Создайте директорию run и снимите канонические fixtures для тестируемой lane:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   Fixtures повторяют `fixtures/nexus/lane_commitments/*.json` и дают генератору нагрузки
   детерминированный seed (зафиксируйте seed в `artifacts/.../README.md`).
2. Baseline lane перед запуском:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   Храните stdout/stderr в директории run, чтобы thresholds smoke были аудируемыми.
3. Снимите telemetry лог, который позже используется в `--telemetry-file` (evidence alias migration)
   и `validate_nexus_telemetry_pack.py`:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. Запустите workload lane (профиль k6, replay harness или федеративные ingestion tests) и держите
   seed workload + диапазон slots под рукой; metadata используется валидатором telemetry manifest в
   разделе 6.3.

5. Упакуйте evidence run новым helper. Передайте захваченные payloads status/metrics/telemetry,
   aliases lanes и любые alias migration события. Helper пишет `smoke.log`, `slot_summary.json`,
   slot bundle manifest и `load_test_manifest.json`, связывая все для review governance:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   Команда применяет те же gate'ы quorum DA, oracle, settlement buffer, TEU и slot-duration, что и
   в этом гайде, и создает manifest, готовый к прикреплению без bespoke scripting.

### 6.2 Инструментированный run

Пока workload нагружает lane:

1. Снимите Torii status + metrics:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. Посчитайте slot-duration quantiles и архивируйте summary:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. Экспортируйте lane-governance snapshot в JSON + Parquet для долгосрочных аудитов:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   JSON/Parquet snapshot теперь фиксирует TEU utilization, уровни scheduler trigger, счетчики
   RBC chunk/byte и статистику транзакционного графа по lane, чтобы evidence rollout показывала и
   backlog, и pressure исполнения.

4. Запустите smoke helper снова на пике нагрузки, чтобы оценить thresholds под стрессом (пишите
   вывод в `smoke_during.log`), и повторите после завершения workload (`smoke_after.log`).

### 6.3 Telemetry pack и governance manifest

Директория run должна включать telemetry pack (`prometheus.tgz`, OTLP stream, structured logs и
любые outputs harness). Валидируйте pack и проставьте metadata, ожидаемую governance:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

В конце приложите захваченный telemetry log и потребуйте alias migration evidence при переименовании
lane во время теста:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

Архивируйте следующие артефакты для governance тикета:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + содержимое pack (`prometheus.tgz`, `otlp.ndjson`, и т.д.)
- `nexus.lane.topology.ndjson` (или релевантный telemetry slice)

Теперь run можно ссылать внутри Space Directory manifests и governance trackers как канонический
NX-7 load test для lane.

## 7. Telemetry и governance follow-ups

- Обновите lane dashboards (`dashboards/grafana/nexus_lanes.json` и связанные overlays) с новым
  lane id и metadata. Сгенерированные keys (`contact`, `channel`, `runbook`, и т.д.) упрощают
  предварительное заполнение labels.
- Подключите правила PagerDuty/Alertmanager для новой lane перед включением admission. `summary.json`
  повторяет checklist из `docs/source/nexus_operations.md`.
- Зарегистрируйте manifest bundle в Space Directory, когда набор валидаторов будет live. Используйте
  тот же manifest JSON, сгенерированный helper, подписанный по governance runbook.
- Следуйте `docs/source/sora_nexus_operator_onboarding.md` для smoke tests (FindNetworkStatus,
  Torii reachability) и фиксируйте evidence набором артефактов выше.

## 8. Пример dry-run

Чтобы предварительно посмотреть артефакты без записи файлов:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --dry-run
```

Команда печатает JSON summary и TOML snippet в stdout, позволяя быструю итерацию во время
планирования.

---

Для дополнительного контекста см:

- `docs/source/nexus_operations.md` — операционный checklist и требования телеметрии.
- `docs/source/sora_nexus_operator_onboarding.md` — детальный onboarding flow, который ссылается на
  новый helper.
- `docs/source/nexus_lanes.md` — геометрия lanes, slugs и storage layout, используемый инструментом.
