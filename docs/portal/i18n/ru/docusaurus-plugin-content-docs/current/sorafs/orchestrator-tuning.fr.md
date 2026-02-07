---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: настройка оркестратора
Название: Déploiement et réglage de l’orchestrateur
Sidebar_label: Правила оркестра
описание: Значения по умолчанию, советы по настройке и точки аудита для управления многоисточниковым оркестром в GA.
---

:::note Источник канонический
Reflète `docs/source/sorafs/developer/orchestrator_tuning.md`. Оставьте две копии, которые соответствуют тому, что документация унаследована до сих пор.
:::

# Руководство по развертыванию и настройке оркестратора

Это руководство по [ссылки на конфигурацию] (orchestrator-config.md) и др.
[runbook de déploiement с несколькими источниками] (multi-source-rollout.md). Я объясню
комментарий ajuster l’orchestrateur pour chaquephase de déploiement, комментарий переводчика
артефакты табло и знаки телеметрии, которые находятся на месте
перед движением транспорта. Appliquez ces ces recommands de manière conherente dans
CLI, SDK и автоматизация для того, чтобы ничего не было похоже на политическую тему.
принеси детерминист.

## 1. Базовые параметры игры

Часть модели конфигурации, часть и настройка маленького ансамбля регулировок
в меху и в меру развертывания. Le tableau ci-dessous захватывает ценности
рекомендуемые этапы для курантов; les valeurs non listées retombent
по умолчанию значения `OrchestratorConfig::default()` и `FetchOptions::default()`.

| Фаза | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Заметки |
|-------|-----------------|-------------------------------|------------------------------------|-------------|------------------------------------------------|-------|
| **Лаборатория / CI** | `3` | `2` | `2` | `2500` | `300` | Серьезный плафон задержки и окно благодати быстро становятся доказательствами жестокой телеметрии. Следите за повторными попытками, чтобы увидеть все инвалиды и все остальные. |
| **Постановка** | `4` | `3` | `3` | `4000` | `600` | Отразите ценность производства, рекламируя его на просторах для исследований сверстников. |
| **Канари** | `6` | `3` | `3` | `5000` | `900` | Соответствуют значениям по умолчанию; Определено `telemetry_region` для обеспечения возможности поворотных панелей на приборных панелях для канарского движения. |
| **Общая недоступность** | `None` (пользователь всех соответствующих критериям) | `4` | `4` | `5000` | `900` | Расширение результатов повторных попыток и проверок для поглощения переходных событий рекламируется в целях сохранения детерминизма посредством аудита. |

- `scoreboard.weight_scale` сохраняет значение по умолчанию `10_000`, если система имеет необходимое другое разрешение. Augmenter l’échelle не меняет pas l’ordre des fournisseurs; это простое и плотное распределение кредитов.
- При переходе к другому этапу сохраните пакет JSON и используйте `--scoreboard-out` для того, чтобы провести аудит регистрации точных параметров.

## 2. Гигиена таблоНа табло сочетаются события манифеста, объявления профессиональных инструкторов и телеметрия.
Авангард:

1. **Проверка телеметрии.** Убедитесь, что снимки являются ссылками.
   `--telemetry-json` — это захват в настроенном окне. Первые блюда
   плюс старые `telemetry_grace_secs` повторяют `TelemetryStale { last_updated }`.
   Traitez cela comme un stop dur et rafraîchissez l’export de télémétrie avant de continue.
2. **Проверьте основания для получения прав.** Сохраните артефакты через
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Чак-закуска
   Транспортируйте блок `eligibility` по точной причине. Не контурнез
   Срок действия электронных карт или объявлений истекает; исправить полезную нагрузку amont.
3. **Revoir les écarts de poids.** Сравните чемпион `normalised_weight` с
   предыдущий выпуск. Вариации >10% соответствуют изменениям
   добровольцы объявлений или телеметрии и грузополучатели в журнале развертывания.
4. **Архивируйте артефакты.** Настройте `scoreboard.persist_path` для выполнения каждого отдельного действия.
   Сделайте финальный снимок табло. Attachez l’artefact au dossier de Release
   с манифестом и комплектом телеметрии.
5. **Отправитель preuve du mix Fournisseurs.** Метаданные `scoreboard.json` _et_ le
   `summary.json` соответствующий разоблачитель doivent `provider_count`,
   `gateway_provider_count` и полученный этикет `provider_mix` afin que les relecteurs
   можно проверить выполнение `direct-only`, `gateway-only` или `mixed`. Лес
   захватывает шлюз doivent rapporter `provider_count=0` плюс `provider_mix="gateway-only"`,
   tandis que les exécutions mixtes requièrent des Comptes Non Nuls for les Deux Sources.
   `cargo xtask sorafs-adoption-check` наложить эти чемпионы (и отобразить эти поля/ярлыки)
   расходящиеся), alors exécutez-le toujours avec `ci/check_sorafs_orchestrator_adoption.sh`
   или ваш сценарий захвата для получения пакета доказательств `adoption_report.json`.
   Шлюзы Torii, которые подразумеваются, сохраняются `gateway_manifest_id`/`gateway_manifest_cid`
   в метаданных табло для ворот усыновления, которые можно отправить в конверт
   манифест с захваченным миксом четырех нянек.

Для определения полей в деталях, представьте себе
`crates/sorafs_car/src/scoreboard.rs` и структура резюме, представленная в CLI
`sorafs_cli fetch --json-out`.

## Ссылка на флаги CLI и SDK

`sorafs_cli fetch` (voir `crates/sorafs_car/src/bin/sorafs_cli.rs`) и оболочка
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) часть мема
поверхность конфигурации оркестратора. Utilisez les flags suivants lors de la
захватить превентивные меры по развертыванию или обновить канонические светильники:

Разделенная ссылка на флаги с несколькими источниками (gardez l'aide CLI и синхронизированные документы в
éditant uniquement ce fichier):- `--max-peers=<count>` ограничивает количество квалифицированных специалистов, которые проходят через фильтр табло. Laissez Vide для стримера для всех, кто имеет право на участие в программе, меттез `1` является уникальным для добровольных тренировок в качестве резервного моноисточника. Отразите ключ `maxPeers` в SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` передает ограничение повторных попыток по фрагменту, примененному к `FetchOptions`. Используйте таблицу развертывания руководства по настройке для рекомендуемых значений; CLI-исполнения, которые собирают превентивные меры, соответствующие значениям по умолчанию для SDK, для гарантии равенства.
- `--telemetry-region=<label>` этикетка для серии Prometheus `sorafs_orchestrator_*` (и реле OTLP) с меткой региона/среды, относящейся к панелям мониторинга, предназначенным для различных лабораторий, промежуточных объектов, канари и GA.
- `--telemetry-json=<path>` добавляет ссылку на снимок на табло. Сохраняйте JSON на табло, чтобы аудиторы могли продолжить выполнение (и чтобы `cargo xtask sorafs-adoption-check --require-telemetry` подтвердил, что поток OTLP поддерживает захват).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) активные крючки моста наблюдателя. В соответствии с определением, оркестратор распределяет фрагменты через прокси-сервер Norito/Kaigi local, где клиенты перемещаются по клиентам, охраняют кэши и комнаты. Kaigi распознает мемы, полученные в Rust.
- `--scoreboard-out=<path>` (при наличии `--scoreboard-now=<unix_secs>`) сохраняется моментальный снимок права доступа для аудиторов. Associez toujours le JSON сохраняет артефакты телеметрии и манифестные ссылки в билете выпуска.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` приложение детерминированных настроек для метадонических сообщений. Используйте уникальные флаги для повторений; Понижение уровня производства должно пройти через артефакты управления, которые нужно применить к набору политических мемов.
- `--provider-metrics-out` / `--chunk-receipts-out` сохраняет метрики здоровья для фурниссера и фрагменты ссылок в контрольном списке развертывания; Прикрепите два артефакта в склад доказательств усыновления.

Пример (с использованием опубликованного приспособления):

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

SDK позволяет настроить память через `SorafsGatewayFetchOptions` в этом файле.
клиент Rust (`crates/iroha/src/client.rs`), привязки файлов JS
(`javascript/iroha_js/src/sorafs.js`) и SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Gardez ces helpers alignés
с заданными по умолчанию значениями CLI для того, чтобы операторы могли использовать более мощные копировальные устройства.
политика в рамках автоматизации без кушеток ad hoc.

## 3. Политическая корректировка выборки

`FetchOptions` контролирует повторные попытки, совпадение и проверку. Лорс дю
изменить:- **Повторные попытки:** дополнение `per_chunk_retry_limit` к `4` аккроîт le temps
  При восстановлении возникает риск маскировки профессиональных поваров. Преферес
  Гардер `4` как плафон и комптер на вращении четырех ступеней для заливки
  разоблачители плохих исполнителей.
- **Seuil d’échec :** `provider_failure_threshold` détermine quand un fournisseur
  отключено для отдыха во время сеанса. Выровняйте эту ценность в политике
  повторные попытки: un seuil inférieur au Budget de retries Force l’orchestrateur à
  выбрасывайте одноранговый узел перед тем, как все повторные попытки будут неожиданными.
- **Согласие:** laissez `global_parallel_limit` не определено (`None`) в течение нескольких минут
  какая особая среда не может быть насыщена анонсированными пляжами. Лорск
  défini, уверяю вас в том, что ценность таких потоков будет меньше, чем у некоторых бюджетов потоков
  Fournisseurs afin d’Eviter la голодание.
- **Переключатели проверки:** `verify_lengths` и `verify_digests` doivent rester.
  деятельность в производстве. Ils garantissent le déterminisme lorsque des Flottes
  смеси де фурниссеров и активных веществ; ne les desactivez que dans des environnements
  de fuzzing isolés.

## 4. Транспорт и анонимная постановка

Используйте поля `rollout_phase`, `anonymity_policy` и `transport_policy` для
представитель положения конфиденциальности:

- Préférez `rollout_phase="snnet-5"` и не допускайте анонимной политики по умолчанию
  suivre les jalons SNNet-5. Заменить через уникальность `anonymity_policy_override`
  Лорск-ла-управление получил подписанную директиву.
- Gardez `transport_policy="soranet-first"` как базовый вариант SNNet-4/5/5a/5b/6a/7/8/12/13 🈺
  (голос `roadmap.md`). Используйте уникальный уникальный вариант `transport_policy="direct-only"`
  понижает статус документов или упражнений по соответствию и посещает ревю де
  кувертюра PQ avant de promouvoir `transport_policy="soranet-strict"` — ce niveau
  échouera Rapidement Si Seuls des Relais Classics продолжает существовать.
- `write_mode="pq-only"` не делает аппликацию, похожую на шемин d'écriture
  (SDK, оркестратор, инструменты управления) может удовлетворить требования PQ. Дюрант
  развертывания, следите за `write_mode="allow-downgrade"`, чтобы получить срочные ответы
  puissent s’appuyer sur des Routes направляет подвеску, которая сигнализирует о телеметрии
  деградация.
- Выбор охранников и подготовка цепей для оснащения
  репертуар SoraNet. Снимок с подписью `relay_directory` и др.
  сохранить кэш `guard_set` до тех пор, пока не произойдет смена охранников в окне
  de retention convenue. Регистрация зарегистрированного кэша по `sorafs_cli fetch`
  fait party de l’évidence de развертывания.

## 5. Способы понижения версии и соответствия

Две су-системы оркестра, помогающие уважать политику без политики
вмешательство Мануэль:- **Устранение понижения версии** (`downgrade_remediation`): наблюдение за событиями.
  `handshake_downgrade_total` и после завершения настройки `threshold`.
  `window_secs`, принудительно использовать локальный прокси в `target_mode` (по умолчанию только метаданные).
  Сохраните значения по умолчанию (`threshold=3`, `window=300`, `cooldown=900`) в дальнейшем
  si les postmortems montrent un autre schéma. Documentez toute override dans le
  журнал развертывания и обеспечения соответствия информационных панелей
  `sorafs_proxy_downgrade_state`.
- **Политика соответствия** (`compliance`): исключения юрисдикции и
  манифестный пропуск по спискам отказов от участия в управлении. Нинтегрез
  jamais d’overrides ad hoc в пакете конфигурации; требуй многого
  неправильно подписан `governance/compliance/soranet_opt_outs.json` и перераспределено
  общий формат JSON.

Для двух систем сохраните полученный пакет конфигурации и включите его.
aux preuves de Release, когда самые сильные аудиторы возвращаются к баскулам.

## 6. Телеметрия и информационные панели

Прежде чем начать развертывание, подтвердите, что все знаки будут активны в данный момент
l’environnement cible:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  doit être à zéro после окончания Канари.
- `sorafs_orchestrator_retries_total` и др.
  `sorafs_orchestrator_retry_ratio` — стабилизатор doivent se sous 10% подвеска le
  canari et rester sous 5% после обеда.
- `sorafs_orchestrator_policy_events_total` — подтверждение того, что этап развертывания присутствует
  наиболее активен (метка `stage`) и зарегистрируйте отключения питания через `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — соответствующий дополнительный выход реле PQ
  внимание к политике.
- Кабели журнала `telemetry::sorafs.fetch.*` — отправляются в агрегатор
  журналы разделены с сохраненными исследованиями для `status=failed`.

Приборная панель Chargez le Grafana canonique depuis
`dashboards/grafana/sorafs_fetch_observability.json` (экспорт в порту
**SoraFS → Fetch Observability**) в зависимости от выбранного региона/манифеста, тепловой карты
повторные попытки по четырем параметрам, гистограммы задержки фрагментов и счетчиков
корреспондент по блокировке, где SRE изучает выгорания. Раккордез-ле-регль
Alertmanager в `dashboards/alerts/sorafs_fetch_rules.yml` и валидный синтаксис
Prometheus с `scripts/telemetry/test_sorafs_fetch_alerts.sh` (помощник при выполнении
Локализация `promtool test rules` или через Docker). Передача дел в случае необходимости
même bloc de routage que le script imprime afin que les opérateurs puissent joindre
l’évidence au Ticket de Rollout.

### Рабочий процесс записи телеметрии

Пункт дорожной карты **SF-6e** требует прожига телеметрии за 30 дней до
basculer l’orchestrateur с несколькими источниками vers ses valeurs GA. Используйте сценарии du
ссылка для захвата набора воспроизводимых артефактов в течение дня
фенетр:

1. Выполните `ci/check_sorafs_orchestrator_adoption.sh` с записанными переменными.
   определения. Пример:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Помощник радуется `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   écrit `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` и `adoption_report.json` су
   `artifacts/sorafs_orchestrator/<timestamp>/`, и назначьте минимальное число
   Fournisseurs éligibles через `cargo xtask sorafs-adoption-check`.
2. Укажите переменные записи, которые присутствуют в скрипте, а также
   `burn_in_note.json`, захват метки, индекс дня, идентификатор манифеста,
   источник телеметрии и дайджесты артефактов. Joignez в журнале JSON
   развертывание, которое позволит получить удовлетворительный результат в течение дня
   Фенетр на 30 дней.
3. Импортируйте таблицу Grafana неправильно (`dashboards/grafana/sorafs_fetch_observability.json`).
   в пространстве труда, постановка/продюсирование, taguez-le avec le label de burn-in
   и проверьте, что на каждой панели прикреплены таблички для манифеста/тестового региона.
4. Exécutez `scripts/telemetry/test_sorafs_fetch_alerts.sh` (или `promtool test rules …`)
   Chaque fois que `dashboards/alerts/sorafs_fetch_rules.yml` изменение, скоро документирование
   что маршрутизация оповещений соответствует экспортируемым меткам подвески le burn-in.
5. Архивируйте снимки приборной панели, вылазку на проверку оповещений и очередь журналов.
   des recherches `telemetry::sorafs.fetch.*` с артефактами оркестра для
   que la gouvernance puisse rejouer l’evidence sans extraire de métriques des system live.

## 7. Контрольный список внедрения

1. Обновите табло в CI с кандидатом конфигурации и захватом файлов.
   артефакты под контролем версии.
2. Выполните выборку определенного оборудования в окружающей среде (лаборатория, постановка,
   canari, производство) и прикрепите артефакты `--scoreboard-out` и `--json-out`
   или регистрация развертывания.
3. Пройдитесь по обзорным панелям телеметрии с инженерным оборудованием, и
   vérifiant que toutes les métriques ci-dessus ont des échantillons живы.
4. Зарегистрируйте окончательную настройку конфигурации (с помощью `iroha_config`) и сохраните ее.
   зафиксируйте использование реестра управления для объявлений и соответствия.
5. Отслеживайте развертывание и информируйте новое оборудование SDK
   по умолчанию все интеграционные клиенты остаются согласованными.

Suivre ce Guide maintient les déploiements de l’orchestrateur deterministes et
проверяемые, tout en fournissant des boucles de rétroaction claires pour ajuster
бюджеты на повторные попытки, возможности четырех специалистов и конфиденциальность.