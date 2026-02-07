---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: настройка оркестратора
Название: Despliegue y ajuste del orquestador
Sidebar_label: Настройка ордера
описание: Предопределенные практические значения, советы по регулировке и зрительным точкам для обучения многопрофильного оратора в GA.
---

:::примечание Фуэнте каноника
Рефлея `docs/source/sorafs/developer/orchestrator_tuning.md`. Мы спешим скопировать список документов, которые должны быть удалены.
:::

# Guía de despliegue y ajuste del orquestador

Это основа [ссылка на конфигурацию] (orchestrator-config.md) и
[runbook de despliegue multi-origen](multi-source-rollout.md). Объяснение
как настроить оркестера для каждого этапа деспльега, как переводчика
артефакты табло и те сигналы телеметрии, которые раньше были в списках
расширение трафика. Применение рекомендаций по форме, согласованной в
CLI, SDK и автоматизация для того, чтобы каждый раз видеть политическую ошибку.
получить детерминист.

## 1. База параметров параметров

Часть установки для конфигурации и настройки соединения
de perillas a medida que progresa el despliegue. La tabla siguiente вспоминает лос
значения, рекомендуемые для большинства общин; Лос-Валорес без списков vuelven
а лос предопределено в `OrchestratorConfig::default()` и `FetchOptions::default()`.

| Фаза | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Заметки |
|------|-----------------|-------------------------------|------------------------------------|-------------|------------------------------------------------|-------|
| **Лаборатория / CI** | `3` | `2` | `2` | `2500` | `300` | Ограничение задержки и ограничение скорости срабатывания телеметрии. Подождите, пока вы освободитесь от недействительных вещей. |
| **Постановка** | `4` | `3` | `3` | `4000` | `600` | Refleja лос-ценности производства dejando margen для сверстников-исследователей. |
| **Канарейка** | `6` | `3` | `3` | `5000` | `900` | Igual a los valores por дефекто; Конфигурация `telemetry_region` для того, чтобы панели мониторинга можно было сегментировать канарский трафик. |
| **Общая диспонибилидад** | `None` (используйте все элементы) | `4` | `4` | `5000` | `900` | Инкрементные тени вновь натянуты и упали, чтобы поглотить проходящие моменты, когда аудитория снова переформировалась в детерминизм. |

- `scoreboard.weight_scale` se mantiene en el valor predeterminado `10_000` залп, который требует от системы aguas abajo другого решения. Aumentar la escala no cambia el orden de losprovedores; только распределение кредитов в больших объемах.
- При переходе между фазами сохраняйте пакет JSON и используйте `--scoreboard-out`, чтобы аудитория зарегистрировала соединение с точными параметрами.

## 2. Уход за табло

Табло сочетает в себе реквизиты манифестов, объявления проверяющих и телеметрию.
До авансара:1. **Действительна телеметрия.** Убедитесь, что снимки ссылаются на
   `--telemetry-json` Fueron Capturados Dentro de la Ventana de Gracia configurada. Лас-энтрадас
   больше антигуасов, чем `telemetry_grace_secs`, с `TelemetryStale { last_updated }`.
   Работайте как блокировка и актуализация экспорта телеметрии до продолжения.
2. **Проверка мотивов элегантности.** Сохранение артефактов
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Када-энтрада
   включая блок `eligibility` с точной причиной падения. Никаких собрепискрибас
   прекращение работы или объявление об истечении срока действия; исправить полезную нагрузку вверх по течению.
3. **Пересмотрите камбиос де песо.** Сравните поле `normalised_weight` с
   освободить переднюю часть. Desplazamientos de peso >10% корреляция с камнями
   обдумать объявления о телеметрии и зарегистрироваться в журнале сообщений.
4. **Архив артефактов.** Настройте `scoreboard.persist_path` для каждого случая.
   выбрасывается финальный снимок табло. Дополнение к артефакту в реестре
   Освободите объединение всех манифестов и пакета телеметрии.
5. **Регистрация доказательств, подтверждающих наличие доказательств.** Метаданные `scoreboard.json`
   y el `summary.json`, соответствующий экспоненту `provider_count`,
   `gateway_provider_count` и производная этикетка `provider_mix` для внесения изменений
   нажмите кнопку выброса топлива `direct-only`, `gateway-only` или `mixed`. Лас каптурас де
   шлюз сообщает `provider_count=0` и `provider_mix="gateway-only"`, сейчас это происходит
   ejecuciones mixtas requieren conteos no cero para ambos orígenes. `cargo xtask sorafs-adoption-check`
   impone estos Campos (y Falla Si Los Conteos/etiquetas не случайно), así que ejecútalo
   Siempre Junto с `ci/check_sorafs_orchestrator_adoption.sh` или сценарием захвата для
   производит комплект доказательств `adoption_report.json`. Шлюзы Cuando Haya Torii,
   сохранить `gateway_manifest_id`/`gateway_manifest_cid` в метаданных табло
   для того, чтобы ворота усыновления могли коррелировать с самим собой манифеста с
   мезкла де проверядорес каптурада.

Подробные определения кампуса, консультации
`crates/sorafs_car/src/scoreboard.rs` и структура возобновления вывода CLI для
`sorafs_cli fetch --json-out`.

## Ссылка на флаги CLI и SDK

`sorafs_cli fetch` (версия `crates/sorafs_car/src/bin/sorafs_cli.rs`) и обертка
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) разделяет
неправильная настройка конфигурации ордера. Флаги Лос-Сигуентес США
захватить доказательства показа или воспроизвести канонические светильники:

Referencia Compartida de flags multi-origen (поддерживайте доступ к CLI и документам на английском языке)
sincronía editando Solo este Archivo):- `--max-peers=<count>` ограничивает количество доступных элементов, сохраняя фильтр табло. Выполните настройку для передачи всех доступных элементов и понло в `1` только тогда, когда вы намеренно используете запасной вариант для одного будущего. Отобразить периллу `maxPeers` в SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` возвращается к ограничению повторного использования фрагмента, который соответствует `FetchOptions`. Используйте таблицу развертывания и руководство по настройке рекомендуемых значений; выбросы CLI, которые скопировали доказательства, совпадающие с ценностями из-за дефекта SDK, чтобы сохранить справедливость.
- `--telemetry-region=<label>` этикетка серии Prometheus `sorafs_orchestrator_*` (и элементы OTLP) с региональной этикеткой/информацией для отдельных панелей мониторинга, трафика лаборатории, промежуточного уровня, канарейки и общего доступа.
- `--telemetry-json=<path>` добавляет снимок экрана на табло. Сохраните соединение JSON на табло, чтобы аудиторы могли воспроизвести выброс (и для того, чтобы `cargo xtask sorafs-adoption-check --require-telemetry` мог быть использован для подачи потока OTLP в захват).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) доступны крючки моста наблюдателя. При настройке сервер передает фрагменты через локальный прокси-сервер Norito/Kaigi для клиентов навигации, охраняет кеши и освобождает Kaigi от получения ошибочных сообщений, полученных от Rust.
- `--scoreboard-out=<path>` (дополнительно с `--scoreboard-now=<unix_secs>`) сохраняет снимок удаленности для аудиторов. Продолжайте использовать JSON с артефактами телеметрии и отображать ссылки в билете выпуска.
- Приложение `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` корректирует параметры метаданных объявлений. Флаги США установлены только для удобства; деградация в производстве должна быть связана с артефактами правительства, чтобы каждый раз наносить на пакет политических ошибок.
- `--provider-metrics-out` / `--chunk-receipts-out` сохраняет метрики обслуживания для проверки и полученные фрагменты ссылок для контрольного списка развертывания; дополнительные артефакты для представления доказательств усыновления.

Пример (используйте опубликованное приспособление):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK используется в неправильной конфигурации `SorafsGatewayFetchOptions` ru
клиент Rust (`crates/iroha/src/client.rs`), привязки JS
(`javascript/iroha_js/src/sorafs.js`) и SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantén esas ayudas en
синхронизация с лос-значениями из-за дефекта CLI, чтобы можно было использовать операции
копировать политику в автоматическом режиме без возможности перевода в медицину.

## 3. Настройка политической выборки

`FetchOptions` контролирует соответствие повторных намерений, синхронизацию и проверку.
Аль-Аюстар:- **Reintentos:** Выберите `per_chunk_retry_limit` для увеличения `4`.
  восстановление сил может привести к скрытым последствиям. Предпочтительный мантенер `4`
  Как технология и поверьте вращению доводчиков, чтобы обнаружить лос-де-бахо, полученное.
- **Умбраль де Фаллос:** `provider_failure_threshold` определяет, когда будет проводиться проверка.
  будет отключена для восстановления сеанса. Алинея — доблесть в политике
  reintentos: un umbral menor que el presuuesto de reintentos obliga al orquestador
  изгнание сверстников до того, как все они вернулись.
- **Конкуренция:** Дежа `global_parallel_limit` без настройки (`None`) в меню.
  que un entorno específico no pueda saturar los rangos anunciados. Когда ты видишь
  настроить, гарантировать, что доблесть моря будет равна сумме предположений
  потоки воды, чтобы избежать истощения.
- **Переключение проверки:** `verify_lengths` и `verify_digests` должны быть постоянными.
  навыки производства. Гарантия детерминизма при смешивании сена
  де проверяющие; соло отключалось в режиме фаззинга.

## 4. Этапы транспортировки и анонимности

США `rollout_phase`, `anonymity_policy` и `transport_policy` пункт
представлять позицию конфиденциальности:

- Выберите `rollout_phase="snnet-5"` и разрешите анонимную политику
  неисправны хиты SNNet-5. Запишите с `anonymity_policy_override`
  только когда губернатор издал твердую директиву.
- Mantén `transport_policy="soranet-first"` в стандартной комплектации SNNet-4/5/5a/5b/6a/7/8/12/13 🈺
  (версия `roadmap.md`). США `transport_policy="direct-only"` только для ухудшения качества
  Документы или симулякро-де-кумплименто и надежда на пересмотр Cobertura PQ
  перед продвижением `transport_policy="soranet-strict"` — этот уровень быстро падает
  соло было классическим.
- `write_mode="pq-only"` только для того, чтобы импонировать, когда приходилось вручную писать (SDK,
  orquestador, gobernanza gobernanza) может удовлетворить все необходимые требования PQ. Дуранте
  развертывания поддерживаются `write_mode="allow-downgrade"` для ответов
  Чрезвычайная ситуация может быть приостановлена в прямом направлении в течение телеметрии марки ла
  деградация.
- Выбор охраны и подготовка цепей в зависимости от направления
  де СораНет. Пропорция моментального снимка фирмы `relay_directory` и сохранение
  кэш `guard_set`, чтобы отток стражи был сохранен в этом месте
  вентана де удержания аккордады. Регистрация в кэше
  `sorafs_cli fetch` является частью доказательства развертывания.

## 5. Ганчо деградации и накопления

Орденские подсистемы помогают избежать вмешательства в политику:- **Устранение деградации** (`downgrade_remediation`): мониторинг событий
  `handshake_downgrade_total` да, после того, как `threshold` настроен
  превысить `window_secs`, использовать локальный прокси-сервер `target_mode` (пор
  дефектные метаданные (только метаданные). Mantén los valores predeterminados (`threshold=3`,
  `window=300`, `cooldown=900`) залп, который указывает на посмертные сообщения под покровителем
  отдельно. Документ может переопределить журнал развертывания и гарантировать его
  Лос-приборные панели сиган `sorafs_proxy_downgrade_state`.
- **Политика накопления** (`compliance`): исключения по юрисдикции и
  Манифесто проезжает через списки исключений, административные по губернаторам.
  Nunca вставляет специальные переопределения в пакет конфигурации; в своем Лугаре,
  ходатайство об актуализации фирмы
  `governance/compliance/soranet_opt_outs.json` и отключите созданный JSON.

Для каждой системы сохраните полученный пакет конфигурации и включите его.
В доказательствах освобождения, которые могли бы быть использованы аудиторами, как и прежде
активировать сокращение.

## 6. Телеметрия и информационные панели

Прежде чем активировать устройство, подтвердите, что все эти события активны.
в энторно-цели:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  ты будешь серо после того, как закончишь канарейку.
- `sorafs_orchestrator_retries_total` у
  `sorafs_orchestrator_retry_ratio` — требуется стабилизация для уменьшения 10%
  во время канарейки и ухода за 5% от GA.
- `sorafs_orchestrator_policy_events_total` — подтверждение этапа развертывания
  когда-то активна (метка `stage`) и регистрирует отключения через `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — rastrean el suministro de relés PQ
  Фронт ожиданий политики.
- Objetivos de log `telemetry::sorafs.fetch.*` — доступен для работы с агрегатором журналов
  разделите с охранными устройствами для `status=failed`.

Загрузите приборную панель canónico de Grafana от Desde
`dashboards/grafana/sorafs_fetch_observability.json` (экспортировано на портале)
bajo **SoraFS → Fetch Observability**) для выбора региона/манифеста,
тепловая карта восстановления для проверки, гистограммы задержки фрагментов и
Контадоры атак совпали с тем, что пересмотрели SRE во время выгорания.
Подключитесь к правилам Alertmanager на `dashboards/alerts/sorafs_fetch_rules.yml`
и действует синтаксис Prometheus с `scripts/telemetry/test_sorafs_fetch_alerts.sh`
(помощник автоматически выбрасывается локально `promtool test rules` или Docker).
Передача оповещений требует блокировки маршрутизации, которую необходимо выполнить
Сценарий для операторов может быть дополнительным доказательством билета на развертывание.

### Запись телеметрии

Для элемента дорожной карты **SF-6e** требуется запись телеметрии за 30 дней до этого.
de cambiar el orquestador multi-origen a sus valores GA. США, лос-скрипты
хранилище для захвата набора воспроизводимых артефактов каждый день
вентана:

1. Выброс `ci/check_sorafs_orchestrator_adoption.sh` с переменными
   постоянные конфигурации. Пример:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Эль-помощник воспроизводит `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   опишите `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` и `adoption_report.json` бахо
   `artifacts/sorafs_orchestrator/<timestamp>/`, и я назначу минимальный номер
   provedores elegibles mediante `cargo xtask sorafs-adoption-check`.
2. Когда появятся переменные записи, сценарий также будет создан.
   `burn_in_note.json`, захват этикета, индекс дня, идентификатор дня
   манифесто, функция телеметрии и дайджесты артефактов. Адъюнта
   Это JSON в журнале развертывания, чтобы было очевидно, что захватывается каждый день
   де ла вентана де 30 дней.
3. Импорт актуализированной таблицы Grafana (`dashboards/grafana/sorafs_fetch_observability.json`)
   в рабочем пространстве постановки/производства, этикет с этикетом прожига
   и подтвердите, что каждая панель должна быть выбрана для манифеста/региона в проверке.
4. Эжекута `scripts/telemetry/test_sorafs_fetch_alerts.sh` (о `promtool test rules …`)
   cuando cambie `dashboards/alerts/sorafs_fetch_rules.yml` для документирования, которое
   Маршруты оповещений совпадают с экспортированными показателями во время выгорания.
5. Архивируйте снимок приборной панели, нажмите кнопку проверки оповещений и эл.
   хвост журналов лас-бускедас `telemetry::sorafs.fetch.*` в соединении с лос
   артефакты Оркестадора, которые помогут воспроизвести правительство
   доказательства греха дополнительных показателей систем en vivo.

## 7. Список проверок развертывания

1. Обновите табло в CI, используя кандидатскую конфигурацию и захват.
   Лос-артефакты ниже контроля версий.
2. Выгрузка определенных приспособлений в каждый день (лаборатория, постановка,
   канарейка, производство) и дополнения к артефактам `--scoreboard-out` и `--json-out`
   Аль-регистр развертывания.
3. Просмотрите информационные панели телеметрии с дежурным инженером, гарантируя, что
   todas las métricas anteriores tengan muestras en vivo.
4. Зарегистрируйте руту окончательной конфигурации (обычно через `iroha_config`) и выберите
   зафиксируйте git del registero de gobernanza usado для объявлений и накоплений.
5. Актуализация трекера развертывания и уведомление об оборудовании SDK на вашем оборудовании.
   Новые ценности для дефекта, который поможет интеграциям клиентов
   Алинеадас.

Seguir esta guía mantiene los despliegues del orquestador deterministas y
проверяемые, периоды циклического обратного питания должны быть отрегулированы
presupuestos de reintentos, capacidad de provedores и postura de privacidad.