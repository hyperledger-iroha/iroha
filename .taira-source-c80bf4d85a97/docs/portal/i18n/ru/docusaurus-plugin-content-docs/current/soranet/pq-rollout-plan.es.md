---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: план-развертывания pq
Название: План постквантового развертывания SNNet-16G
Sidebar_label: План развертывания PQ
описание: Операционное руководство по продвижению гибридного рукопожатия SoraNet X25519+ML-KEM от канареечного до стандартного в реле, клиентах и SDK.
---

:::примечание Канонический источник
Эта страница отражает `docs/source/soranet/pq_rollout_plan.md`. Синхронизируйте обе копии.
:::

SNNet-16G завершает постквантовое развертывание транспорта SoraNet. Элементы управления `rollout_phase` позволяют операторам координировать детерминированное продвижение от текущего требования защиты этапа A к мажоритарному покрытию этапа B и строгому режиму PQ этапа C без редактирования необработанного JSON/TOML для каждой поверхности.

Эта книга охватывает:

- Определения фаз и новые ручки конфигурации (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`), встроенные в кодовую базу (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
— Сопоставление флагов SDK и CLI, чтобы каждый клиент мог следить за развертыванием.
- Ожидания ретранслятора/клиента Canary, а также информационные панели управления, которые отслеживают продвижение по службе (`dashboards/grafana/soranet_pq_ratchet.json`).
— Обработчики отката и ссылки на книгу инструкций по пожарным тренировкам ([Руководство с храповым механизмом PQ](./pq-ratchet-runbook.md)).

## Фазовая карта

| `rollout_phase` | Эффективный этап анонимности | Эффект по умолчанию | Типичное использование |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Этап А) | Требуется как минимум один охранник PQ на цепь, пока флот прогревается. | Исходный уровень и первые недели канарейки. |
| `ramp` | `anon-majority-pq` (Этап B) | Выбор смещения в сторону реле PQ для покрытия >= двух третей; Классические реле остаются запасным вариантом. | Канарские острова по регионам эстафеты; предварительный просмотр переключается в SDK. |
| `default` | `anon-strict-pq` (Этап C) | Применяйте схемы только для PQ и усиливайте сигналы тревоги о понижении версии. | Окончательное продвижение по завершении телеметрии и управления. |

Если поверхность также определяет явный `anonymity_policy`, это переопределяет фазу для этого компонента. Пропустить явный этап теперь относится к значению `rollout_phase`, так что операторы могут изменить этап только один раз для каждой среды и позволить клиентам наследовать его.

## Справочник по конфигурации

### Оркестратор (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Загрузчик оркестратора разрешает резервную стадию во время выполнения (`crates/sorafs_orchestrator/src/lib.rs:2229`) и предоставляет ее через `sorafs_orchestrator_policy_events_total` и `sorafs_orchestrator_pq_ratio_*`. См. `docs/examples/sorafs_rollout_stage_b.toml` и `docs/examples/sorafs_rollout_stage_c.toml` для фрагментов, готовых к применению.

### Rust-клиент / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь записывает фазу анализа (`crates/iroha/src/client.rs:2315`), чтобы помощники (например, `iroha_cli app sorafs fetch`) могли сообщать о текущей фазе вместе с политикой анонимности по умолчанию.

## Автоматизация

Два помощника `cargo xtask` автоматизируют формирование расписания и захват артефактов.

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
   ```Длительность принимает суффиксы `s`, `m`, `h` или `d`. Команда выводит `artifacts/soranet_pq_rollout_plan.json` и сводку Markdown (`artifacts/soranet_pq_rollout_plan.md`), которую можно отправить с запросом на изменение.

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

   Команда копирует предоставленные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет дайджесты BLAKE3 для каждого артефакта и записывает `rollout_capture.json` с метаданными и подписью Ed25519 в полезные данные. Используйте тот же закрытый ключ, которым подписывают протоколы противопожарных учений, чтобы руководство могло быстро проверить запись.

## Массив флагов SDK и CLI

| Поверхность | Канарейка (Этап А) | Рампа (Этап B) | По умолчанию (этап C) |
|---------|-------------------|----------------|-------------------|
| `sorafs_cli` выборка | `--anonymity-policy stage-a` или фаза доверия | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Конфигурация Оркестратора JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Конфигурация клиента Rust (`iroha.toml`) | `rollout_phase = "canary"` (по умолчанию) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` подписанные команды | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, опционально `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, опционально `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, опционально `.ANON_STRICT_PQ` |
| Помощники оркестратора JavaScript | `rolloutPhase: "canary"` или `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Питон `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Свифт `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Все переключатели SDK сопоставляются с тем же синтаксическим анализатором этапа, который используется оркестратором (`crates/sorafs_orchestrator/src/lib.rs:365`), поэтому многоязычные развертывания синхронизируются с настроенным этапом.

## Контрольный список планирования Canary

1. **Предполетная подготовка (Т минус 2 недели)**

- Подтвердите, что уровень отключения на этапе A составил =70% на регион (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запрограммируйте слот проверки управления, который утверждает канареечное окно.
   — Обновите `sorafs.gateway.rollout_phase = "ramp"` в промежуточной версии (отредактируйте JSON оркестратора и повторно разверните) и выполните пробный запуск конвейера продвижения.

2. **Эстафета канарейки (день Т)**

   — Повышайте уровень по одному региону за раз, устанавливая `rollout_phase = "ramp"` в оркестраторе и участвующих манифестах ретрансляции.
   - Отслеживайте «События политики по каждому результату» и «Коэффициент отключения» на панели управления PQ Ratchet (которая теперь включает в себя панель развертывания) для удвоения TTL защитного кэша.
   — Вырезать снимки `sorafs_cli guard-directory fetch` до и после выполнения для хранения аудита.

3. **Клиент/SDK canary (Т плюс 1 неделя)**

   - Измените на `rollout_phase = "ramp"` в конфигурациях клиента или передайте переопределения `stage-b` для определенных когорт SDK.
   - Запишите различия телеметрии (`sorafs_orchestrator_policy_events_total`, сгруппированные по `client_id` и `region`) и прикрепите их к журналу инцидентов развертывания.

4. **Акция по умолчанию (Т плюс 3 недели)**- После подписания управления измените конфигурации оркестратора и клиента на `rollout_phase = "default"` и поменяйте подписанный контрольный список готовности на артефакты выпуска.

## Контрольный список управления и доказательств

| Изменение фазы | Продвижение ворот | Пакет доказательств | Панели мониторинга и оповещения |
|--------------|----------------|-----------------|---------------------|
| Канарейка -> Рампа *(Предварительный просмотр этапа B)* | Уровень отключения на этапе A = 0,7 на продвигаемый регион, проверка билета Argon2 p95  По умолчанию *(применение этапа C)* | Завершена проработка телеметрии SN16 в течение 30 дней, `sn16_handshake_downgrade_total` остается неизменной в базовом состоянии, `sorafs_orchestrator_brownouts_total` — нулевой во время клиентской канарейки, а также зарегистрирована репетиция переключения прокси-сервера. | Транскрипция `sorafs_cli proxy set-mode --mode gateway|direct`, выходные данные `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, журнал `sorafs_cli guard-directory verify` и подписанный пакет `cargo xtask soranet-rollout-capture --label default`. | Та же плата PQ Ratchet плюс панели понижения версии SN16, описанные в `docs/source/sorafs_orchestrator_rollout.md` и `dashboards/grafana/soranet_privacy_metrics.json`. |
| Готовность к экстренному понижению/откату | Срабатывает, когда счетчики перехода на более раннюю версию увеличиваются, проверка защитного каталога завершается сбоем или буфер `/policy/proxy-toggle` записывает устойчивые события понижения версии. | Контрольный список `docs/source/ops/soranet_transport_rollback.md`, журналы `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, заявки на инциденты и шаблоны уведомлений. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` и оба пакета предупреждений (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Сохраните каждый артефакт под `artifacts/soranet_pq_rollout/<timestamp>_<label>/` со сгенерированным `rollout_capture.json`, чтобы пакеты управления содержали табло, трассировки promtool и дайджесты.
- Прикрепляет дайджесты SHA256 загруженных доказательств (протоколы в формате PDF, пакет захвата, снимки сохранения) к протоколам продвижения, чтобы можно было воспроизвести одобрения парламента без доступа к промежуточному кластеру.
- Ссылайтесь на план телеметрии в рекламном билете, чтобы доказать, что `docs/source/soranet/snnet16_telemetry_plan.md` по-прежнему является каноническим источником понижения словарного запаса и порогов оповещения.

## Обновления информационной панели и телеметрии

`dashboards/grafana/soranet_pq_ratchet.json` теперь включает панель аннотаций «План развертывания», которая ссылается на этот сборник сценариев и отображает текущий этап проверок управления, чтобы подтвердить, какой этап активен. Синхронизируйте описание панели с будущими изменениями ручек конфигурации.

Для оповещений убедитесь, что существующие правила используют тег `stage`, чтобы фазы канареечных событий и фазы по умолчанию запускали отдельные пороговые значения политики (`dashboards/alerts/soranet_handshake_rules.yml`).

## Крючки отката

### По умолчанию -> Рампа (Этап C -> Этап B)1. Понизьте уровень оркестратора с помощью `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (и отразите тот же этап в конфигурациях SDK), чтобы вернуть этап B всему автопарку.
2. Принудить клиентов перейти к безопасному транспортному профилю через `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, захватив расшифровку, чтобы рабочий процесс исправления `/policy/proxy-toggle` оставался доступным для аудита.
3. Запустите `cargo xtask soranet-rollout-capture --label rollback-default`, чтобы заархивировать различия в каталоге Guard, выходные данные promtool и снимки экрана информационной панели под `artifacts/soranet_pq_rollout/`.

### Рампа -> Канарейка (Этап B -> Этап A)

1. Импортируйте снимок защитного каталога, сделанный перед повышением уровня с помощью `sorafs_cli guard-directory import --guard-directory guards.json`, и снова запустите `sorafs_cli guard-directory verify`, чтобы демонстрационный пакет включал хэши.
2. Установите `rollout_phase = "canary"` (или переопределите его с помощью `anonymity_policy stage-a`) в конфигурациях оркестратора и клиента, а затем повторите детализацию PQ Ratchet из [Ratchet Runbook PQ] (./pq-ratchet-runbook.md), чтобы протестировать конвейер перехода на более раннюю версию.
3. Прикрепите обновленные снимки экрана телеметрии PQ Ratchet и SN16, а также результатов оповещений к журналу инцидентов, прежде чем уведомлять руководство.

### Напоминания о ограждениях

- Ссылайтесь на `docs/source/ops/soranet_transport_rollback.md` каждый раз, когда происходит понижение в должности, и записывайте любое временное смягчение как элемент `TODO:` в системе отслеживания развертывания для дальнейшей работы.
- Держите `dashboards/alerts/soranet_handshake_rules.yml` и `dashboards/alerts/soranet_privacy_rules.yml` под покрытием `promtool test rules` до и после отката, чтобы дрейф предупреждений документировался вместе с пакетом захвата.