---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Elastic-Lane
Название: Provisionamento de Lane Elastico (NX-7)
Sidebar_label: Обеспечение эластичной полосы движения
описание: Fluxo de bootstrap для записи манифестов полосы Nexus, ввода каталога и доказательств развертывания.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/nexus_elastic_lane.md`. Мантенья представил, как копиас alinhadas ели, что или прочистили локализацию, проходящую через портал.
:::

# Комплект подготовки эластичной дорожки (NX-7)

> **Пункт плана действий:** NX-7 — инструмент для подготовки эластичных полос  
> **Статус:** полный набор инструментов — манифесты, фрагменты каталога, полезные нагрузки Norito, дымовые тесты,
> e o помощник в сборке нагрузочного теста, агора костюмирует шлюзование задержки для слота + манифесты доказательств, которые служат для доставки грузов валидаторов
> Возможно, вы публикуете сценарии, написанные вслепую Медидой.

Это новый помощник `scripts/nexus_lane_bootstrap.sh`, который автоматизирует создание манифестов полос, фрагментов каталога полос/пространства данных и доказательств развертывания. Целью и облегчением создания новых дорожек Nexus (публичных или частных) является ручное редактирование различных архивов, а не повторная обработка геометрии каталога вручную.

## 1. Предварительные условия

1. Утверждение управления по псевдониму полосы движения, пространства данных, совместно с валидаторами, толерантностью к фактам (`f`) и политике урегулирования.
2. Укажите окончательный список валидаторов (идентификаторы содержимого) и список защищенных пространств имен.
3. Доступ к хранилищу конфигураций узла для подключения фрагментов фрагментов.
4. Зарегистрируйтесь в реестре манифестов полосы движения (veja `nexus.registry.manifest_directory` и `cache_directory`).
5. Контакты телеметрии/манипуляторы PagerDuty для полосы движения, для того, чтобы оповещения были подключены к той полосе, которая находится в сети.

## 2. Гир артефатос де Лейн

Выполните команду, помогающую выбрать репозиторий:

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

Флаги чеканятся:

- `--lane-id` соответствует индексу нового входа в `nexus.lane_catalog`.
- `--dataspace-alias` и `--dataspace-id/hash` управляют входом в каталог в пространство данных (для США или по указанной полосе, если ее пропустить).
- `--validator` можно повторить или прочитать `--validators-file`.
- `--route-instruction` / `--route-account` излучает защитную полость для воротника.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) захватывает контакты Runbook для тех информационных панелей, которые наиболее часто используются владельцами.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` — дополнительные средства или подключение обновления среды выполнения к манифесту, когда запрашивается полоса управления, установленная оператором.
- `--encode-space-directory` автоматически отключается `cargo xtask space-directory encode`. Объедините com `--space-directory-out`, когда вы запросите, что или архив `.to` кодифицирован и будет использоваться по умолчанию.

В сценарии, создающем три артефата из `--output-dir` (для первоначального или исходного руководства), можно выполнить дополнительный четырехкратный вариант кодирования, который уже привычен:1. `<slug>.manifest.json` — манифест полосы соперничества или кворума валидаторов, защищенные пространства имен и дополнительные метаданные для перехвата обновления во время выполнения.
2. `<slug>.catalog.toml` — фрагмент TOML с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и quisquer regras de roteamento solicitadas. Гарантия, что `fault_tolerance` определен для входа в пространство данных для определения размера реле полосы движения (`3f+1`).
3. `<slug>.summary.json` — резюме расшифрованной геометрии (слизняк, сегменты, метаданные), которые требуются для развертывания и команда exato `cargo xtask space-directory encode` (em `space_directory_encode.command`). Приложение представляет собой JSON в качестве билета на посадку в качестве доказательства.
4. `<slug>.manifest.to` - испущено, когда `--encode-space-directory` esta habilitado; быстро от `iroha app space-directory manifest publish` до Torii.

Используйте `--dry-run` для визуализации JSON/фрагментов в виде архивных файлов и `--force` для обнаружения существующих артефактов.

##3. Аппликация в виде муданок

1. Скопируйте манифест JSON для настройки `nexus.registry.manifest_directory` (например, для каталога кэша или для удаленных пакетов реестра). В комите или архиве содержатся манифесты версий, не принадлежащих вашему репозиторию конфигурации.
2. Приложение или фрагмент каталога `config/config.toml` (или нет `config.d/*.toml`). Гарантия, что `nexus.lane_count` будет заменена `lane_id + 1`, и вы сможете выполнить `nexus.routing_policy.rules`, который вам нужно будет перейти на новую полосу.
3. Закодируйте (вызовите `--encode-space-directory`) и опубликуйте или укажите без каталога пространства, используя команду захвата без сводки (`space_directory_encode.command`). Продукт или полезная нагрузка `.manifest.to`, которую Torii ожидает и регистрирует доказательства для аудиторов; завидую через `iroha app space-directory manifest publish`.
4. Выполните `irohad --sora --config path/to/config.toml --trace-config` и получите указание не отслеживать билет развертывания. Это доказывает, что новая геометрия соответствует пулям/сегментам Куры.
5. Reinicie os validadores atribuidos ao Lane Quando, как если бы они были внесены в декларацию/каталог имплантатов. Мантенья или сводка JSON без билета для будущих аудиторий.

## 4. Подключите пакет дистрибуции реестра

Добавьте манифест или наложение, чтобы операторы могли распространять данные управления полосами для редактирования конфигураций на каждом хосте. Помощник по объединению копий манифестов для канонического макета создаст дополнительное наложение на каталог управления для `nexus.registry.cache_directory` и может отправить архив для переноса в автономном режиме:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Саидас:

1. `manifests/<slug>.manifest.json` — скопируйте этот архив для настройки `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - соедините с `nexus.registry.cache_directory`. При вводе `--module` с определенным модулем подключаемого модуля разрешена замена модулей управления (NX-2), а также настройка или наложение кэша на все редактируемые `config.toml`.
3. `summary.json` — включает хэши, метаданные для наложения и инструкции для операций.
4. Дополнительный `registry_bundle.tar.*` — сразу для SCP, S3 или трекеров-артефатов.

Синхронизируйте внутренний каталог (или архив) для каждой проверки, экстрагируйте хосты с воздушным зазором и скопируйте манифесты + наложение кэша для своих файлов реестра до повторного использования или Torii.## 5. Дымовые тесты валидаторов

После повторного запуска Torii запустите новый помощник по дыму для проверки отчета о полосе движения `manifest_ready=true`, если метрики покажут заражение на бегущих полосах и увидят датчик запечатанного состояния в неопределенном состоянии. Дорожки, которые exigem манифестирует разработку um `manifest_path` nao vazio; Помощник должен немедленно прийти к выводу, что нужно развернуть NX-7, включая доказательства явного убийства:

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

Adicione `--insecure` или тестовая среда, подписанная самостоятельно. Сценарий, указанный как код, находящийся на нуле, если он находится в открытом состоянии, запечатан или имеет метрику/телеметрию, отличающуюся от ожидаемых значений. Используйте кнопки операционной системы `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` и `--max-headroom-events` для параметров телеметрии по полосе (блокирование/завершение/отставание/запас) вместе с вашими ограничениями в работе и объединением с `--max-slot-p95` / `--max-slot-p99` (также `--min-slot-samples`) для импорта в качестве метаданных о сроке службы слота NX-18, который может помочь.

Чтобы подтвердить голос с воздушным зазором (или CI), можно воспроизвести ответ Torii, захваченный перед любым доступом к конечной точке в реальном времени:

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

Наши светильники серьезно связаны с `fixtures/nexus/lanes/`, отражая артефатос-продукты, которые используют помощник начальной загрузки для того, чтобы новые манифесты были доступны для написания сценариев в Медиде. Выполнение CI или основного потока через `ci/check_nexus_lane_smoke.sh` и `ci/check_nexus_lane_registry_bundle.sh` (псевдоним: `make check-nexus-lanes`) для проверки того, что помощник дыма NX-7 продолжает сравниваться с опубликованным форматом полезной нагрузки и гарантирует, что дайджесты/оверлеи будут воспроизводиться в связке.