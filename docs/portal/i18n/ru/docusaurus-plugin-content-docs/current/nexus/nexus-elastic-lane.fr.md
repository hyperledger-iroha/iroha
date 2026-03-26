---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Elastic-Lane
Название: Provisionnement de Lane Elastic (NX-7)
Sidebar_label: Обеспечение эластичной полосы движения
описание: Рабочий процесс начальной загрузки для создания манифестов полосы Nexus, входов в каталог и предварительных развертываний.
---

:::note Источник канонический
Представление этой страницы `docs/source/nexus_elastic_lane.md`. Gardez les deux copys alignees jusqu'a ce que la смутный перевод прибывает на портал.
:::

# Комплект подготовки эластичной дорожки (NX-7)

> **Элемент дорожной карты:** NX-7 — инструмент для обеспечения эластичности полос  
> **Статут:** комплект инструментов — род манифестов, фрагменты каталога, полезные нагрузки Norito, дымовые тесты,
> и помощник по сборке нагрузочного теста, поддерживающий ограничение задержки по слоту + предварительные манифесты, которые запускают валидаторы заряда
> puissent etre publies без написания сценариев.

Это руководство сопровождает операторов с помощью нового помощника `scripts/nexus_lane_bootstrap.sh`, который автоматизирует генерацию манифестов полос, фрагментов каталога дорожек/пространства данных и превентивных мер по развертыванию. L'objectif - это облегчение создания новых дорожек Nexus (publiques ou privees) без редактирования основных дополнительных файлов и повторного получения в соответствии с основной геометрией каталога.

## 1. Предварительное условие

1. Одобрение управления для псевдонимов полосы движения, пространства данных, ансамбля валидаторов, толерантности к панелям (`f`) и политики урегулирования.
2. Последний список проверяющих (IDs de compte) и список протеже пространств имен.
3. Доступ к конфигурации узлов для доступа к общим фрагментам.
4. Chemins pour le реестр манифестов дорожного движения (voir `nexus.registry.manifest_directory` и `cache_directory`).
5. Контактная телеметрия/управление PagerDuty для полос, которые будут оповещены о том, что полоса находится на линии.

## 2. Генератор артефактов полосы движения

Lancez le helper depuis la racine du depot:

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

Флаги cle:

- `--lane-id` соответствует индексу нового входа в `nexus.lane_catalog`.
- `--dataspace-alias` и `--dataspace-id/hash` управляют входом в каталог пространства данных (по умолчанию, идентификатор дорожки, если она отсутствует).
- `--validator` может повторить или повторить `--validators-file`.
- `--route-instruction` / `--route-account` содержит правила маршрутизации, требующие коллера.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) захват контактов из Runbook для информационных панелей, предназначенных для владельцев хороших сайтов.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` позволяет перехватить обновление среды выполнения в манифесте, когда требуется полоса управления оператором Etendus.
- `--encode-space-directory` вызвать автоматический вызов `cargo xtask space-directory encode`. Объедините `--space-directory-out`, когда вы захотите, чтобы этот файл `.to` закодировал все параметры, которые указаны по умолчанию.

Сценарий создает три артефакта в `--output-dir` (по умолчанию для курантского репертуара), а также четырехвариантную опцию, когда кодировка активна:1. `<slug>.manifest.json` — манифест содержимого полосы, кворума валидаторов, протежных пространств имен и метадоннальных опций обновления среды выполнения перехвата.
2. `<slug>.catalog.toml` — фрагмент TOML с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и все необходимые правила маршрутизации. Убедитесь, что `fault_tolerance` определяет входное пространство данных для измерения комитационного реле полосы (`3f+1`).
3. `<slug>.summary.json` - резюме аудита, декривирующего геометрию (заготовка, сегменты, метадоны) плюс этапы развертывания и точные команды `cargo xtask space-directory encode` (sous `space_directory_encode.command`). Используйте JSON в качестве билета на посадку в предварительном порядке.
4. `<slug>.manifest.to` - emis quand `--encode-space-directory` est активен; pret pour le flux `iroha app space-directory manifest publish` de Torii.

Используйте `--dry-run` для предварительного просмотра JSON/фрагментов без подписи файлов и `--force` для удаления существующих артефактов.

## 3. Внесение изменений

1. Скопируйте манифест JSON в конфигурацию `nexus.registry.manifest_directory` (и в каталог кэша, а также в реестр, который будет отображаться в удаленных пакетах). Committez le fichier si les манифесты, содержащие версии в вашем репозитории конфигурации.
2. Добавьте фрагмент каталога `config/config.toml` (или `config.d/*.toml`). Уверяем вас, что `nexus.lane_count` так же, как и `lane_id + 1`, и вы встретитесь со всеми правилами `nexus.routing_policy.rules`, которые делают указатель на новую полосу.
3. Зашифруйте (если вы используете `--encode-space-directory`) и опубликуйте манифест в пространстве каталога с помощью команды захвата в сводке (`space_directory_encode.command`). Cela produit le payload `.manifest.to`, сопровождающий Torii, и зарегистрируйте предварительную регистрацию для аудитов; суметез через `iroha app space-directory manifest publish`.
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` и архивирует трассировку вылазки в билете вывоза. Cela prouve que la nouvelle geometry соответствует aux сегментам Kura du Slug Genere.
5. Redemarrez les validateurs назначает полосу для развертывания манифеста/каталога изменений. Сохраните сводку JSON в билете для будущих проверок.

## 4. Создайте пакет дистрибутива реестра.

Упакуйте родовой манифест и наложите его на тех операторов, которые могут распространить данные по управлению полосами без редактирования конфигураций на каждой странице. Помощник по объединению копий файлов манифестов в канонический макет, создаст опцию наложения каталога управления для `nexus.registry.cache_directory` и может хранить архив для автономной передачи:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Sorties :

1. `manifests/<slug>.manifest.json` — копирование файлов в `nexus.registry.manifest_directory` configure.
2. `cache/governance_catalog.json` — вставьте в `nexus.registry.cache_directory`. Вход в `--module` отличается от определения разветвляемого модуля, позволяющего заменять модули управления (NX-2), и позволяет использовать наложение кэша в редакторе `config.toml`.
3. `summary.json` — включая хеши, метадоны наложения и инструкции для оператора.
4. Опция `registry_bundle.tar.*` — для SCP, S3 или трекеров артефактов.Синхронизация всего репертуара (или архива) с чаком валидатора, экстракция с воздушным зазором и копирование манифестов + наложение кэша на символы реестра до восстановления Torii.

## 5. Дымовые тесты валидаторов

После восстановления Torii, новый помощник по дыму для проверки связи по полосе `manifest_ready=true`, которые выставляют счетчики на число полос, и что машина запечатана. Полосы, необходимые для манифестов, не выставляют `manifest_path`; le helper echoue des qu'il manque le chemin afin que chaque deploiement NX-7, включая предварительную подпись манифеста:

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

Ajoutez `--insecure` lorsque vous testez des environnements самоподписанный. Сортировка сценария с ненулевым кодом, если полоса движения запечатана, или метрики/телеметрии, производные от значений посещаемости. Используйте ручки `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` и `--max-headroom-events` для поддержания телеметрии по полосе (высокий блок/конечный результат/отставание/запас) в ваших рабочих диапазонах и т. д. `--max-slot-p95` / `--max-slot-p99` (плюс `--min-slot-samples`) для установки объектов на время слота NX-18 без выхода из помощника.

Для проверки с воздушным зазором (или CI) вы можете получить ответ Torii, захваченный вместо допрашивающего конечной точки в реальном времени:

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

Приборы регистрируются с помощью `fixtures/nexus/lanes/`, отражая артефакты, созданные в качестве помощника начальной загрузки, чтобы новые манифесты могли быть использованы без необходимости написания сценариев. CI выполняет поток мемов через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (псевдоним: `make check-nexus-lanes`), чтобы убедиться, что помощник дыма NX-7 соответствует формату публичной полезной нагрузки и гарантирует, что дайджесты/оверлеи будут воспроизводиться в пакете.