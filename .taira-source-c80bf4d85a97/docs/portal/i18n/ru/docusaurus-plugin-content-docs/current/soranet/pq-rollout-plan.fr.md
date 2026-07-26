---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: план-развертывания pq
Название: Руководство по постквантовому развертыванию SNNet-16G
Sidebar_label: План развертывания PQ
описание: Операционное руководство по продвижению гибридного рукопожатия SoraNet X25519+ML-KEM с канареечного на стандартное на реле, клиентах и SDK.
---

:::примечание Канонический источник
:::

SNNet-16G завершает постквантовое развертывание транспорта SoraNet. Ручки `rollout_phase` позволяют операторам координировать детерминированное продвижение от этапа A защиты требований к этапу B мажоритарного покрытия и этапу C строгой политики качества без изменения необработанного JSON/TOML для каждой поверхности.

Эта книга охватывает:

- Определения фаз и новые ручки настройки (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) кабелей в кодовой базе (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
— Сопоставление флагов SDK и CLI, чтобы каждый клиент мог следить за развертыванием.
- Ожидания Canary по планированию ретрансляции/клиентов, а также информационные панели управления, которые обеспечивают продвижение по службе (`dashboards/grafana/soranet_pq_ratchet.json`).
— Обработчики отката и ссылки на книгу инструкций по пожарным тренировкам ([Руководство с храповым механизмом PQ](./pq-ratchet-runbook.md)).

## Фазовая карта

| `rollout_phase` | Эффективный этап анонимности | Эффект по умолчанию | Типичное использование |
|-----------------|----------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Этап А) | Требуется хотя бы один охранник PQ на каждый контур, пока флот разогревается. | Исходный уровень и первые недели канарейки. |
| `ramp` | `anon-majority-pq` (Этап B) | Отдавайте предпочтение реле PQ для >= двух третей зоны покрытия; классические реле остаются в резерве. | Канарские эстафеты по регионам; Предварительный просмотр SDK переключается. |
| `default` | `anon-strict-pq` (Этап C) | Внедрите схемы только для PQ и ужесточите сигнализацию о понижении версии. | Окончательное продвижение после завершения телеметрии и управления подписанием. |

Если поверхность также определяет явный `anonymity_policy`, она переопределяет фазу для этого компонента. Если пропустить явный шаг, теперь по умолчанию используется значение `rollout_phase`, поэтому операторы могут переключаться один раз для каждой среды и позволять клиентам наследовать его.

## Справочник по конфигурации

### Оркестратор (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Загрузчик оркестратора разрешает резервный шаг во время выполнения (`crates/sorafs_orchestrator/src/lib.rs:2229`) и предоставляет его через `sorafs_orchestrator_policy_events_total` и `sorafs_orchestrator_pq_ratio_*`. См. `docs/examples/sorafs_rollout_stage_b.toml` и `docs/examples/sorafs_rollout_stage_c.toml` для готовых к применению фрагментов.

### Rust клиент / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь сохраняет анализируемую фазу (`crates/iroha/src/client.rs:2315`), чтобы вспомогательные команды (например, `iroha_cli app sorafs fetch`) могли сообщать о текущей фазе наряду с политикой анонимности по умолчанию.

## Автоматизация

Два помощника `cargo xtask` автоматизируют создание расписания и сбор артефактов.

1. **Создать региональное расписание**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```Длительность принимает суффиксы `s`, `m`, `h` или `d`. Команда выдает `artifacts/soranet_pq_rollout_plan.json` и сводку Markdown (`artifacts/soranet_pq_rollout_plan.md`), чтобы прикрепить их к запросу на изменение.

2. **Сохраняйте артефакты сверления с помощью сигнатур**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Команда копирует файлы, предоставленные в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет дайджесты BLAKE3 для каждого артефакта и записывает `rollout_capture.json` с метаданными и подписью Ed25519 в полезные данные. Используйте тот же закрытый ключ, которым подписывают протоколы противопожарных учений, чтобы руководство могло быстро подтвердить факт захвата.

## Матрица флагов SDK и CLI

| Площадь | Канарейка (Этап А) | Рампа (Этап B) | По умолчанию (этап C) |
|---------|-----------------|----------------|------------------|
| `sorafs_cli` выборка | `--anonymity-policy stage-a` или отдых на фазе | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Конфигурация Оркестратора JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Конфигурация клиента Rust (`iroha.toml`) | `rollout_phase = "canary"` (по умолчанию) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` подписанные команды | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, опционально `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, опционально `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, опционально `.ANON_STRICT_PQ` |
| Помощники оркестратора JavaScript | `rolloutPhase: "canary"` или `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Питон `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Свифт `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Все пакеты SDK переключаются на один и тот же анализатор этапов, используемый оркестратором (`crates/sorafs_orchestrator/src/lib.rs:365`), поэтому многоязычные развертывания остаются синхронизированными с настроенным этапом.

## Контрольный список планирования Canary

1. **Предполетная подготовка (Т минус 2 недели)**

- Подтвердите, что уровень отключения электроэнергии на этапе A составляет =70% по регионам (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запланируйте интервал проверки управления, который утверждает канареечное окно.
   — Обновите `sorafs.gateway.rollout_phase = "ramp"` в промежуточной версии (отредактируйте оркестратор JSON и повторно разверните) и выполните пробный запуск конвейера продвижения.

2. **Эстафета канарейки (день Т)**

   — Повышайте уровень по одному региону за раз, устанавливая `rollout_phase = "ramp"` в оркестраторе и участвующих манифестах ретрансляции.
   - Отслеживайте «События политики для каждого результата» и «Коэффициент отключения» на панели управления PQ Ratchet (которая теперь включает в себя панель развертывания) для удвоенного TTL защитного кэша.
   - Создавайте снимки `sorafs_cli guard-directory fetch` до и после для хранения аудита.

3. **Canary-клиент/SDK (Т плюс 1 неделя)**

   - Переключите `rollout_phase = "ramp"` в конфигурациях клиента или передайте переопределения `stage-b` для назначенных когорт SDK.
   - Запишите различия телеметрии (группа `sorafs_orchestrator_policy_events_total` по `client_id` и `region`) и прикрепите их к журналу инцидентов развертывания.4. **Акция по умолчанию (Т плюс 3 недели)**

   - После проверки управления переключите конфигурации оркестратора и клиента на `rollout_phase = "default"` и запустите подписанный контрольный список готовности в артефактах выпуска.

## Контрольный список управления и фактических данных

| Изменение фазы | Продвижение ворот | Пакет доказательств | Панели мониторинга и оповещения |
|--------------|----------------|-----------------|----------------------|
| Канарейка -> Рампа *(Предварительный просмотр этапа B)* | Уровень провалов Этап A = 0,7 на продвигаемый регион, проверка билета Argon2 p95  По умолчанию *(Этап исполнения)* | Достигнута проработка телеметрии SN16 за 30 дней, `sn16_handshake_downgrade_total` остается неизменной на базовом уровне, `sorafs_orchestrator_brownouts_total` — на нулевом уровне во время клиентской канарейки и репетиции журнала переключения прокси-сервера. | Транскрипция `sorafs_cli proxy set-mode --mode gateway|direct`, выход `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, журнал `sorafs_cli guard-directory verify` и знак пакета `cargo xtask soranet-rollout-capture --label default`. | Та же плата PQ Ratchet, а также платы понижения версии SN16, описанные в `docs/source/sorafs_orchestrator_rollout.md` и `dashboards/grafana/soranet_privacy_metrics.json`. |
| Готовность к экстренному понижению/откату | Срабатывает при повышении счетчиков перехода на более раннюю версию, сбое проверки защитного каталога или в буфере `/policy/proxy-toggle`, записывающем устойчивые события понижения версии. | Контрольный список `docs/source/ops/soranet_transport_rollback.md`, журналы `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, заявки на инциденты и шаблоны уведомлений. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` и два пакета предупреждений (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Сохраните каждый артефакт под `artifacts/soranet_pq_rollout/<timestamp>_<label>/` со сгенерированным `rollout_capture.json`, чтобы пакеты управления содержали табло, трассировки promtool и дайджесты.
- Прикрепите SHA256-дайджесты загруженных доказательств (протоколы в формате PDF, пакет захвата, снимки защиты) к протоколам продвижения, чтобы можно было воспроизвести одобрения парламента без доступа к промежуточному кластеру.
- Ссылайтесь на план телеметрии в рекламном билете, чтобы доказать, что `docs/source/soranet/snnet16_telemetry_plan.md` остается каноническим источником понижения словаря и порогов оповещения.

## Обновление информационных панелей и телеметрии

`dashboards/grafana/soranet_pq_ratchet.json` теперь включает панель аннотаций «План развертывания», которая ссылается на этот сборник сценариев и описывает текущий этап, чтобы проверки управления подтверждали, какой этап активен. Синхронизируйте описание панели с будущими изменениями ручек конфигурации.

Для оповещений убедитесь, что существующие правила используют метку `stage`, чтобы фазы канареечных событий и фазы по умолчанию запускали отдельные пороговые значения политики (`dashboards/alerts/soranet_handshake_rules.yml`).

## Крючки отката

### По умолчанию -> Рампа (Этап C -> Этап B)1. Понизьте версию оркестратора до `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (и отразите ту же фазу в конфигурациях SDK), чтобы возобновить этап B для всего парка.
2. Заставьте клиентов использовать транспортный профиль на `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, записывая расшифровку, чтобы рабочий процесс исправления `/policy/proxy-toggle` оставался доступным для аудита.
3. Запустите `cargo xtask soranet-rollout-capture --label rollback-default`, чтобы заархивировать различия в каталоге Guard, выходные данные promtool и снимки экрана информационной панели под `artifacts/soranet_pq_rollout/`.

### Рампа -> Канарейка (Этап B -> Этап A)

1. Импортируйте снимок защитного каталога, полученный перед повышением уровня с помощью `sorafs_cli guard-directory import --guard-directory guards.json`, и повторно запустите `sorafs_cli guard-directory verify`, чтобы пакет понижения включал хэши.
2. Установите `rollout_phase = "canary"` (или переопределите с помощью `anonymity_policy stage-a`) в конфигурациях оркестратора и клиента, а затем воспроизведите детализацию PQ Ratchet из [Ratchet Runbook PQ] (./pq-ratchet-runbook.md), чтобы подтвердить переход на более раннюю версию конвейера.
3. Перед уведомлением управления прикрепите обновленные снимки экрана телеметрии PQ Ratchet и SN16, а также результаты оповещений к журналу инцидентов.

### напоминания о ограждениях

- Ссылайтесь на `docs/source/ops/soranet_transport_rollback.md` при каждом понижении в должности и записывайте любое временное смягчение как элемент `TODO:` в системе отслеживания развертывания для отслеживания.
- Держите `dashboards/alerts/soranet_handshake_rules.yml` и `dashboards/alerts/soranet_privacy_rules.yml` под прикрытием `promtool test rules` до и после отката, чтобы любое отклонение предупреждений документировалось с помощью пакета захвата.