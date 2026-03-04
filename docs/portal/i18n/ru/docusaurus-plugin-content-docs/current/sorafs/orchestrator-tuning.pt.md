---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: настройка оркестратора
title: Внедрение и настройка оркестратора
Sidebar_label: Настройка ордера
описание: Практические площадки, ориентация регулировки и контрольно-пропускные пункты зрительного зала для перемещения или управления несколькими источниками в GA.
---

:::примечание Fonte canônica
Эспелья `docs/source/sorafs/developer/orchestrator_tuning.md`. Подождите, пока появятся две копии документов, которые являются альтернативной документацией.
:::

# Инструкция по развертыванию и настройке ордера

Это руководство содержит [ссылку на конфигурацию] (orchestrator-config.md) и нет
[runbook de развертывания multi-origem](multi-source-rollout.md). Объяснение
как настройка или организатор для каждого этапа развертывания, как интерпретатор ОС
Artefatos do табло и quais sinais de telemetria devem estar prontos antes
усилителя или трафика. Применение в соответствии с рекомендациями по форме, согласованной с CLI, н.у.
SDK и автоматизация для того, чтобы каждый раз делать политическую выборку детерминированной.

## 1. База параметров соединения

Часть шаблона конфигурации для сопоставления и настройки совместного компонента
Ручки доступны для быстрого развертывания. Таблица abaixo captura os valores
рекомендуются для общего пользования; наши ценности не перечислены вольтам аос
падры `OrchestratorConfig::default()` и `FetchOptions::default()`.

| Фаза | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Заметки |
|------|-----------------|-------------------------------|------------------------------------|-------------|------------------------------------------------|-------|
| **Лаборатория / CI** | `3` | `2` | `2` | `2500` | `300` | Ограничение задержки и возможность быстрого ускорения телеметрии. Мантенья повторяет попытку, чтобы объявить недействительные манифесты больше всего. |
| **Постановка** | `4` | `3` | `3` | `4000` | `600` | Я расскажу о планах производства, которые можно использовать для исследования сверстников. |
| **Канарейка** | `6` | `3` | `3` | `5000` | `900` | Igual AOS Padrões; Определено `telemetry_region` для того, чтобы панели мониторинга могли быть разделены или канарского трафика. |
| **Общая диспонибилизация** | `None` (используйте все элегантные) | `4` | `4` | `5000` | `900` | Увеличьте пределы повторных попыток и нажмите, чтобы поглотить проходящие переходы, когда аудитории продолжают реформировать или детерминизм. |

- `scoreboard.weight_scale` постоянно не восстанавливается `10_000` и остается в нисходящей системе exija вне внутреннего разрешения. Не поднимайтесь выше и не меняйте порядок действий; apenas emite uma distribuição de créditos mais densa.
- При переносе между фазами сохраняйте пакет JSON и используйте `--scoreboard-out` для создания трех регистров аудиторий или совместно с другими параметрами.

## 2. Табло гигиены

На табло сочетаются все необходимые условия для манифеста, объявлений о доставке и телеметрии.
До авансара:1. **Действительно для телеметрии.** Гарантия, что снимки будут ссылаться на
   `--telemetry-json` позволяет захватывать данные, полученные при настройке. Энтрадас
   больше противогазов, чем `telemetry_grace_secs`, не соответствует `TelemetryStale { last_updated }`.
   Сделайте это как жесткий блок и начните экспортировать телеметрию до следующего.
2. **Inspectione razões de elegibilidade.** Сохранение артефактов через
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Када-энтрада
   traz um bloco `eligibility` как причина exata da falha. Não sobrescreva
   прекращение работы или объявление об истечении срока действия; Коррия или полезная нагрузка вверх по течению.
3. **Пересмотр песо.** Сравните версию `normalised_weight` с версией.
   передний. Изменения >10% были согласованы с преднамеренными изменениями в объявлениях
   Или телеметрия и точная регистрация без журнала развертывания.
4. **Архивировать артефакты.** Настройте `scoreboard.persist_path` для каждого выполнения.
   эмита или финальный снимок сделать табло. Приложение или артефакт для регистрации выпуска
   вместе с манифестом и пакетом телеметрии.
5. **Регистрация доказательств сочетания доказательств.** Метаданные `scoreboard.json` _e_ o
   `summary.json` соответствует экспорту `provider_count`, `gateway_provider_count`
   Производная метка `provider_mix` для того, чтобы внесенные изменения были выполнены для выполнения
   `direct-only`, `gateway-only` или `mixed`. Отчет шлюза Capturas `provider_count=0`
   и `provider_mix="gateway-only"`, при выполнении ошибок exigem contagens нет
   ноль пунктов в качестве шрифтов. `cargo xtask sorafs-adoption-check` imõe esses Campos
   (e falha quando contagens/labels расходятся), então Execute-o Semper junto com
   `ci/check_sorafs_orchestrator_adoption.sh` или ваш сценарий захвата для производства
   или комплект доказательств `adoption_report.json`. Шлюзы Quando Torii эстиверем
   envolvidos, mantenha `gateway_manifest_id`/`gateway_manifest_cid` с метаданными
   Табло для того, чтобы ворота или входные билеты соответствовали конверту манифеста
   как смесь захватывающих средств.

Для определения деталей кампуса, веха
`crates/sorafs_car/src/scoreboard.rs` и резюме резюме CLI для
`sorafs_cli fetch --json-out`.

## Ссылка на флаги CLI и SDK

`sorafs_cli fetch` (версия `crates/sorafs_car/src/bin/sorafs_cli.rs`) и обертка
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) компартильхам
одна из лучших настроек ордера. Используйте флаги os seguintes ao
Запишите доказательства развертывания или воспроизведения канонических светильников:

Справочная информация по сопоставлению флагов нескольких источников (узнайте, как связаться с CLI и документацией).
sincronizados editando apenas este arquivo):- `--max-peers=<count>` Ограничение количества проверок может быть сокращено до фильтра на табло. Дейксе необходимо настроить для потоковой передачи всех элементов, элегантных и определенных `1`, которые будут доступны, когда вы намеренно выполняете упражнения или откатываете уникальный шрифт. Используйте ручку `maxPeers` без SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` запускается для ограничения количества повторных попыток для фрагмента приложения `FetchOptions`. Используйте таблицу развертывания с инструкциями по настройке рекомендуемых значений; выполнения CLI, которые должны быть подтверждены соответствующим образом с помощью SDK, чтобы обеспечить безопасность.
- `--telemetry-region=<label>` вращается как серия Prometheus `sorafs_orchestrator_*` (и версии OTLP) с меткой региона/окружения для отдельных панелей мониторинга для лабораторной, промежуточной, канарской и общедоступной систем.
- `--telemetry-json=<path>` инъекция или снимок табло референтного игрока. Сохраняйте или JSON на табло, чтобы аудиторы могли воспроизвести выполнение (и чтобы `cargo xtask sorafs-adoption-check --require-telemetry` подтверждал, что поток OTLP питается).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) имеют крючки для крепления наблюдательного мостика. Когда вы определитесь, организатор передает фрагменты данных на локальный прокси-сервер Norito/Kaigi для клиентов навигации, охраняет кэши и передает Kaigi сообщения, полученные от Rust.
- `--scoreboard-out=<path>` (дополнительно для `--scoreboard-now=<unix_secs>`) сохраняется или создается снимок элегантности для аудиторов. Всегда экспериментируйте или сохраняйте JSON с артефактами телеметрии и ссылочными манифестами без билета выпуска.
- Приложение `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` регулирует детерминированность метаданных объявлений. Используйте флаги esses apenas para ensaios; Понижение уровня производства позволяет использовать артефакты управления для того, чтобы это не было приложением или лучшим политическим пакетом.
- `--provider-metrics-out` / `--chunk-receipts-out` возвращает метрики для проверки и получения фрагментов, ссылающихся на контрольный список развертывания; приложение для регистрации документов, подтверждающих правонарушение.

Пример (используется или публикуется):

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

ОС SDK можно настроить через `SorafsGatewayFetchOptions` без клиента.
Rust (`crates/iroha/src/client.rs`), без привязок JS
(`javascript/iroha_js/src/sorafs.js`) и без SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). У Мантенья есть помощники
синхронизация с панелями CLI для того, чтобы операторы могли копировать политику между собой
автоматический перевод текста на медицину.

## 3. Отрегулируйте политическую выборку

`FetchOptions` управляет повторными попытками, согласованием и проверкой. Ао настройка:- **Повторные попытки:** Элевар `per_chunk_retry_limit` увеличивает скорость `4`.
  восстановление сил, маска для лица может быть повреждена. Префира мантер `4` как
  Это и подтверждение ротации поставщиков для экспорта фракций.
- **Limiar de falhas:** `provider_failure_threshold` определяет, что и как доказать
  desabilitado pelo restante da sessão. Alinhe esse valor à politica de retry:
  um limiar menor que o orçamento de retry força o orquestrador ejetar um peer
  перед этим все повторные попытки.
- **Конкорренция:** Deixe `global_parallel_limit` был определен (`None`) и менялся
  Особое окружение не сообщается о более насыщенных диапазонах. Когда я определился,
  Гарантия, что доблесть будет меньше à soma dos orçamentos destreams dosprovores para
  эвитарное голодание.
- **Переключатели проверки:** `verify_lengths` и `verify_digests` всегда остаются неизменными.
  навыки в производстве. Eles garantem determinismo quando há frotas mistas de
  проведоры; Desative-os apenas emambientes isolados de fuzzing.

## 4. Транспортный и анонимный терминал

Используйте OS Campos `rollout_phase`, `anonymity_policy` и `transport_policy` для
представляют собой позицию конфиденциальности:

- Prefira `rollout_phase="snnet-5"` и разрешение на анонимную политику
  сопровождал Маркоса в SNNet-5. Замена через `anonymity_policy_override` apenas
  Когда управление эмитиром было направлено на управление.
- Mantenha `transport_policy="soranet-first"` в базовой комплектации SNNet-4/5/5a/5b/6a/7/8/12/13 estiverem 🈺
  (веджа `roadmap.md`). Используйте `transport_policy="direct-only"` для перехода на более раннюю версию
  Документы или упражнения по соблюдению требований и проверка пересмотра соглашений PQ заранее
  Промоувер `transport_policy="soranet-strict"` — это очень быстрое открытие
  relés classicos permanecerem.
- `write_mode="pq-only"`, поэтому он должен быть установлен, когда каждый раз пишет (SDK,
  организатор, инструменты управления) могут удовлетворить все необходимые требования PQ. Дуранте
  развертывание, сообщение `write_mode="allow-downgrade"` для экстренного реагирования
  Вы можете использовать ротацию прямо в режиме телеметрии или понизить версию.
- Выбор охраны и организация цепей зависят от направления SoraNet.
  Сохранение моментального снимка `relay_directory` и сохранение кэша `guard_set`
  для того, чтобы постоянно сменить охранников на линии удержания аккордады. Впечатление
  цифровой кэш регистрируется для `sorafs_cli fetch` для проверки развертывания.

## 5. Способы перехода на более раннюю версию и соблюдения требований

Dois subsistemas dois orquestrador ajudam a impor a politica sem intervenção manual:- **Устранение понижения версии** (`downgrade_remediation`): мониторинг событий.
  `handshake_downgrade_total` и `threshold` сконфигурированы превосходно.
  `window_secs`, используется локальный прокси для `target_mode` (только метаданные для входа).
  Mantenha ospadrões (`threshold=3`, `window=300`, `cooldown=900`) и это
  пересмотренные инциденты, произошедшие в результате последующих событий. Documente qualquer переопределить нет
  журнал развертывания и гарантия, что панели мониторинга сопровождают `sorafs_proxy_downgrade_state`.
- **Политика соответствия** (`compliance`): исключения из юрисдикции и манифеста.
  Доступны списки отказа от участия в управлении государством. Nunca insira отменяет специальное решение
  нет пакета конфигурации; Когда вы это сделаете, попросите об окончательном завершении
  `governance/compliance/soranet_opt_outs.json` и повторно имплантированный JSON.

Для систем, сохранение или включение полученного пакета конфигурации
нас доказывают, что освобождение для того, чтобы аудиторы могли быть растеряны как сокращение времени
асионадас.

## 6. Телеметрия и информационные панели

Перед расширением или развертыванием подтвердите, что следующие этапы еще не активны.
окружающее пространство:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  мне нужно было начать с нуля, чтобы сделать канарейский вывод.
- `sorafs_orchestrator_retries_total` е
  `sorafs_orchestrator_retry_ratio` — необходимо стабилизировать 10 % в течение длительного времени.
  o канарейка и постоянная скидка 5% в год.
- `sorafs_orchestrator_policy_events_total` — подтверждение того, что этап развертывания начался успешно
  это состояние (метка `stage`) и регистрация отключений через `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — сопровождение или дополнительная информация о PQ em
  отношение к политическим ожиданиям.
- Alvos de log `telemetry::sorafs.fetch.*` — мы получаем отправку в качестве агрегатора журналов.
  compartilhado com buscas salvas для `status=failed`.

Carregue или приборная панель Grafana canônico em
`dashboards/grafana/sorafs_fetch_observability.json` (экспортируется без портала).
**SoraFS → Получить наблюдаемость**) для выбора региона/манифеста, или
тепловая карта повторных попыток для проверки, гистограммы задержки фрагментов и ОС
свяжитесь с нами для переписки по поводу проверки SRE во время выгорания. Подключитесь
как указано в Alertmanager в `dashboards/alerts/sorafs_fetch_rules.yml` и подтверждено
sintaxe do Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh` (помощник
выполнить локально `promtool test rules` или Docker). Как пассажи де оповещения
exigem или mesmo bloco de roteamento que o script imprime para que operadores possam
добавьте доказательства в виде билета на выдачу.

### Поток записи телеметрии

В пункте дорожной карты **SF-6e** предусмотрена запись телеметрии за 30 дней до этого.
Альтернативный или мультиоригинальный организатор для своих домов GA. Используйте сценарии ОС, сделайте
хранилище для захвата набора воспроизводимых артефактов каждый день
Джанела:

1. Выполните `ci/check_sorafs_orchestrator_adoption.sh` com в качестве варианта прожига.
   конфигурации. Пример:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```О помощник по воспроизведению `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   Грава `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` и `adoption_report.json` в
   `artifacts/sorafs_orchestrator/<timestamp>/`, и введите минимальный номер
   provores elegíveis через `cargo xtask sorafs-adoption-check`.
2. Когда представлены варианты записи, или сценарий также выводится
   `burn_in_note.json`, захват этикетки, индекса диаметра или идентификатора манифеста,
   шрифт телеметрии и дайджесты артефатосов. Приложение содержит JSON в журнале
   развертывание для получения ясного изображения, которое будет удовлетворительным для каждого дня от 30 дней.
3. Импортируйте настроенную панель управления Grafana (`dashboards/grafana/sorafs_fetch_observability.json`).
   для рабочей области подготовки/производства, метки для записи и подтверждения
   que Cada Painel Exibe Amostras Para o Manifest/Região Em Teste.
4. Выполните `scripts/telemetry/test_sorafs_fetch_alerts.sh` (или `promtool test rules …`).
   всегда, что `dashboards/alerts/sorafs_fetch_rules.yml` нужно для документирования, которое
   o ротационные оповещения соответствуют экспортированным показателям во время работы или выгорания.
5. Архив или снимок панели мониторинга, а также тестовые оповещения и хвосты журналов.
   das buscas `telemetry::sorafs.fetch.*` junto aos artefatos do orquestrador para
   что управление может воспроизвести доказательства, которые являются дополнительными показателями системы
   в производстве.

## 7. Контрольный список внедрения

1. Восстановите табло в CI, используя конфигурацию возможных данных и систему захвата.
   артефатос требует контроля версии.
2. Выполните выборку определенных приборов в каждой окружающей среде (лаборатория, сцена,
   canary, produção) и приложение к artefatos `--scoreboard-out` и `--json-out` и т. д.
   регистр развертывания.
3. Пересмотрите информационные панели телеметрии с инженерами завода, гарантируя, что
   todas as métricas acima tenham amostras ao vivo.
4. Зарегистрируйтесь или зарегистрируйтесь в последней конфигурации (через `iroha_config`) и o
   зафиксируйте git do registero degovança usado для объявлений и соблюдения требований.
5. Настройте трекер развертывания и уведомления на оборудовании SDK в любое время.
   подсказки для интеграции клиентов-клиентов.

Следите за тем, чтобы развертывание было детерминированным и организованным.
пассивная аудитория, активная циклическая обратная связь для регулировки
условия повторных попыток, возможности проверки и конфиденциальность.