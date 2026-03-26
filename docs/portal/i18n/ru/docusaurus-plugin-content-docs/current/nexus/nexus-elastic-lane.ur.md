---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Elastic-Lane
Название: لچکدار Lane پروویژنگ (NX-7)
Sidebar_label: لچکدار переулок پروویژنگ
описание: манифесты полосы Nexus, записи каталога, свидетельства развертывания, возможность начальной загрузки и проверка.
---

:::обратите внимание на канонический источник
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ جب تک ترجمہ پورٹل تک نہیں پہنچتا، دونوں کاپیوں کو выравнивается رکھیں۔
:::

# لچکدار переулок پروویژنگ ٹول کٹ (NX-7)

> **Элемент плана действий:** NX-7 — набор инструментов для переулка  
> **Статус:** набор инструментов — манифесты, фрагменты каталога, полезные нагрузки Norito, дымовые тесты.
> Помощник по пакету нагрузочного тестирования, шлюзование задержки слота + манифесты доказательств, а также валидаторы и загрузочные прогоны.
> Создание сценариев и их использование

В число операторов входят помощники `scripts/nexus_lane_bootstrap.sh`, а также генерация манифеста дорожек, фрагменты каталога дорожек/пространства данных, а также доказательства развертывания. خودکار بناتا ہے۔ Nexus полосы движения (публичные и частные) Каталог геометрии

## 1. Предварительные условия

1. псевдоним полосы, пространство данных, набор валидаторов, отказоустойчивость (`f`), политика расчетов и одобрение управления.
2. Проверка валидаторов (идентификаторы учетных записей) и защищенных пространств имен.
3. Репозиторий конфигурации узла. Создание сгенерированных фрагментов кода.
4. Реестр манифестов полос и пути (например, `nexus.registry.manifest_directory` или `cache_directory`).
5. Лейн کے لئے контакты телеметрии/обработчики PagerDuty تاکہ оповещения Лейн کے онлайн ہوتے ہی проводной ہو سکیں۔

## 2. Артефакты переулка بنائیں

Корень репозитория и помощник:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
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

- `--lane-id` или `nexus.lane_catalog` — запись, индекс, совпадение, или совпадение.
- `--dataspace-alias` или `--dataspace-id/hash` запись каталога пространства данных для управления (опустите ہونے или идентификатор полосы по умолчанию استعمال ہوتا ہے).
- `--validator` کو повторите کیا جا سکتا ہے یا `--validators-file` سے پڑھا جا سکتا ہے۔
- Готовые к вставке правила маршрутизации `--route-instruction` / `--route-account` выдают کرتے ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) захват контактов Runbook کرتے ہیں تاکہ информационные панели درست владельцев دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` перехватчик обновления среды выполнения для манифеста и расширения полос для расширенных элементов управления оператора.
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ `--space-directory-out` — это вариант настройки `.to`, который используется по умолчанию. رکھنا ہو۔

Для `--output-dir` можно использовать артефакты и файлы (каталог по умолчанию — ہے), а также кодировку. В качестве примера можно привести:1. `<slug>.manifest.json` — манифест полосы, кворум валидатора, защищенные пространства имен, метаданные перехватчика обновления среды выполнения и многое другое.
2. `<slug>.catalog.toml` — фрагмент TOML, содержащий `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`, правила маршрутизации и правила маршрутизации. Запись в пространстве данных `fault_tolerance` لازمی طور پر set کریں تاکہ Lane-Relay Committee (`3f+1`) درست سائز ہو۔
3. `<slug>.summary.json` — сводка аудита для геометрии (срез, сегменты, метаданные) и необходимые шаги развертывания. `cargo xtask space-directory encode` — точная команда (`space_directory_encode.command` — для выполнения). کرتا ہے۔ Билет на посадку или доказательства, или прикрепите их.
4. `<slug>.manifest.to` - `--encode-space-directory` в случае необходимости. Torii کے `iroha app space-directory manifest publish` поток کے لئے готов ہے۔

`--dry-run` — JSON/фрагменты — можно просмотреть в предварительном просмотре `--force` — перезаписать артефакты کریں۔

## 3. تبدیلیاں لاگو کریں

1. Манифест JSON настроен `nexus.registry.manifest_directory` и используется (каталог кэша или зеркало удаленных пакетов реестра). Репозиторий манифестов конфигурации с версиями и коммитами
2. Фрагмент каталога کو `config/config.toml` (یا مناسب `config.d/*.toml`) или добавление کریں۔ `nexus.lane_count` کم از کم `lane_id + 1` ہونا چاہیے اور نئے Lane کے لئے `nexus.routing_policy.rules` ڈیٹ کریں۔
3. Закодируйте файл (`--encode-space-directory` в файле Space Directory) и опубликуйте манифест. Краткое описание команды (`space_directory_encode.command`) اس سے `.manifest.to` полезная нагрузка بنتا ہے اور аудиторы کے لئے доказательства ریکارڈ ہوتا ہے؛ `iroha app space-directory manifest publish` کے ذریعے отправить کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` обеспечивает вывод трассировки, билет на развертывание и архив. یہ ثابت کرتا ہے کہ نئی сгенерированная геометрия пули/сегменты Куры کے مطابق ہے۔
5. Манифест/каталог: развертывание или переулок, назначенные валидаторы, перезапуск или перезапуск. будущие аудиты Краткое описание JSON Билет میں رکھیں۔

## 4. Пакет дистрибутива реестра

сгенерированный манифест, оверлей, пакет, операторы, хост, конфигурации, редактирование, управление полосами данных, распределение данных, Манифесты помощника Bundler, канонический макет, `nexus.registry.cache_directory`, наложение каталога управления, автономные передачи, архив tarball. نکال سکتا ہے:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Выходы:

1. `manifests/<slug>.manifest.json` — настройка `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں رکھیں۔ ہر `--module` запись ایک определение подключаемого модуля بن جاتی ہے، جس سے swap-outs модуля управления (NX-2) ممکن ہوتے ہیں اور صرف Если вам нужен `config.toml`, вы можете использовать его.
3. `summary.json` — хеши, наложение метаданных, инструкции оператора и многое другое.
4. Дополнительный `registry_bundle.tar.*` — SCP, S3 и трекеры артефактов готовы к работе.

Каталог (архив) Проверка валидатора Синхронизация хостов с воздушным зазором Извлечение извлечения Torii перезапуск Использование манифестов + наложение кэша Пути в реестре

## 5. Дымовые тесты валидатораTorii перезапустите کے بعد نیا Smoke helper چلائیں تاکہ Lane `manifest_ready=true` رپورٹ کرے، metrics میںں ожидаемое количество полос движения نظر آئے، اور герметичный манометр صاف ہو۔ جن переулки کو манифеста درکار ہوں انہیں непустой `manifest_path` ظاہر کرنا چاہیے؛ helper اب path غائب ہونے پر فوراً error ہوتا ہے تاکہ ہر NX-7 развертывание میں подписало манифест доказательств شامل ہو:

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

Самоподписанные среды اگر переулок غائب ہو، запечатан ہو، یا метрики/ожидаемые значения телеметрии سے дрейф کریں تو скрипт ненулевой выход کرتا ہے۔ `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, اور `--max-headroom-events` Доступны телеметрические измерения высоты блока на уровне полосы движения/конечности/отставания/запаса. Операционная оболочка `--max-slot-p95` / `--max-slot-p99` (`--min-slot-samples`) может быть полезна для NX-18, целевого помощника по длительности слота. اندر ہی обеспечить соблюдение کریں۔

Проверка с воздушным зазором (CI) позволяет использовать конечную точку в реальном времени и захватить повтор ответа Torii, например:

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

`fixtures/nexus/lanes/` Есть записанные приборы, помощник начальной загрузки, например, артефакты, файлы, манифесты, манифесты. Создание сценариев и использование lint CI поток `ci/check_nexus_lane_smoke.sh` или `ci/check_nexus_lane_registry_bundle.sh` (псевдоним: `make check-nexus-lanes`) или `make check-nexus-lanes`. Помощник по дыму NX-7. Формат полезной нагрузки. Формат загрузки. Воспроизводимые дайджесты/наложения пакетов.