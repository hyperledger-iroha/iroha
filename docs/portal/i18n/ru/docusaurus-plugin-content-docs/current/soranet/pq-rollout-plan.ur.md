---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: план-развертывания pq
Название: Пособие по постквантовому развертыванию SNNet-16G
sidebar_label: План развертывания PQ
описание: Практическое руководство по продвижению гибридного рукопожатия SoraNet X25519+ML-KEM от канареечного до стандартного в реле, клиентах и SDK.
---

:::обратите внимание на канонический источник
Эта страница отражает `docs/source/soranet/pq_rollout_plan.md`. Синхронизируйте обе копии до тех пор, пока старый набор документации не будет удален.
:::

SNNet-16G завершает постквантовое развертывание транспорта SoraNet. Ручки `rollout_phase` позволяют операторам координировать детерминированное продвижение, которое варьируется от текущих требований к защите на этапе A до мажоритарного покрытия на этапе B и строгого режима PQ на этапе C, без редактирования необработанного JSON/TOML каждой поверхности.

Эта книга охватывает следующее:

- Определения фаз и новые ручки конфигурации (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`), которые встроены в кодовую базу (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
— Сопоставление флагов SDK и CLI, чтобы каждый клиент мог отслеживать развертывание.
- Ретрансляционное/клиентское канареечное планирование ожиданий и информационные панели управления, обеспечивающие продвижение по службе (`dashboards/grafana/soranet_pq_ratchet.json`).
- Обработчики отката и ссылки на Runbook Fire-Drill ([PQ Ratchet Runbook](./pq-ratchet-runbook.md)).

## Фазовая карта

| `rollout_phase` | Эффективный этап анонимности | Эффект по умолчанию | Типичное использование
|------------------|--------------------------|----------------|--------------|
| `canary` | `anon-guard-pq` (Этап А) | Требуется как минимум один предохранитель PQ для каждого контура, пока квартира не отапливается. | Базовый уровень и начальные канареечные недели. |
| `ramp` | `anon-majority-pq` (Этап B) | выбор смещения в сторону реле PQ, чтобы покрытие >= двух третей; Классические реле остаются запасным вариантом. | Порегиональная эстафета канареек; Предварительный просмотр SDK переключается. |
| `default` | `anon-strict-pq` (Этап C) | Обеспечьте соблюдение цепей только для PQ и ужесточите сигналы тревоги о понижении версии. | Окончательное повышение после завершения утверждения телеметрии и управления. |

Если поверхность также явно устанавливает `anonymity_policy`, она переопределяет фазу для этого компонента. Если явного этапа нет, теперь он подчиняется значению `rollout_phase`, так что операторы меняют этап один раз в каждой среде, а клиенты наследуют его.

## Справочник по конфигурации

### Оркестратор (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Загрузчик Orchestrator разрешает резервную стадию во время выполнения (`crates/sorafs_orchestrator/src/lib.rs:2229`) и отображает ее через `sorafs_orchestrator_policy_events_total` и `sorafs_orchestrator_pq_ratio_*`. См. готовые фрагменты в разделах `docs/examples/sorafs_rollout_stage_b.toml` и `docs/examples/sorafs_rollout_stage_c.toml`.

### Rust-клиент / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь анализирует запись фазы (`crates/iroha/src/client.rs:2315`), чтобы вспомогательные команды (например, `iroha_cli app sorafs fetch`) могли сообщать о текущей фазе с политикой анонимности по умолчанию.

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
   ```Длительность принимает суффикс `s`, `m`, `h` или `d`. Команда выдает `artifacts/soranet_pq_rollout_plan.json` и сводку Markdown (`artifacts/soranet_pq_rollout_plan.md`), которую можно отправить с запросом на изменение.

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

   Команда копирует предоставленные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет дайджесты BLAKE3 для каждого артефакта и записывает `rollout_capture.json`, который включает подпись Ed25519 в метаданные и полезные данные. Используйте тот же закрытый ключ, которым подписывают протоколы противопожарных учений, чтобы руководство могло быстро подтвердить данные.

## Матрица флагов SDK и CLI

| Поверхность | Канарейка (Этап А) | Рампа (Этап B) | По умолчанию (этап C) |
|---------|------------------|-------------|
| `sorafs_cli` выборка | `--anonymity-policy stage-a` или фазовая зависимость `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Конфигурация Оркестратора JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Конфигурация клиента Rust (`iroha.toml`) | `rollout_phase = "canary"` (по умолчанию) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` подписанные команды | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, опционально `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, опционально `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, опционально `.ANON_STRICT_PQ` |
| Помощники оркестратора JavaScript | `rolloutPhase: "canary"` или `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Питон `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Свифт `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Все переключатели SDK сопоставлены с тем же анализатором этапа, который использует оркестратор (`crates/sorafs_orchestrator/src/lib.rs:365`), поэтому многоязычные развертывания синхронизируются с настроенным этапом.

## Контрольный список планирования Canary

1. **Предполетная подготовка (Т минус 2 недели)**

- Подтвердите, что уровень отключения электроэнергии на этапе A составляет =70% (`sorafs_orchestrator_pq_candidate_ratio`).
   - Запланируйте интервал проверки управления, который одобрит канареечное окно.
   — Обновите `sorafs.gateway.rollout_phase = "ramp"` в промежуточном режиме (переразвертывание путем редактирования JSON оркестратора) и пробный запуск конвейера продвижения.

2. **Эстафета канарейки (день Т)**

   — Продвигайте по одному региону за раз, устанавливая `rollout_phase = "ramp"` в оркестраторе и участвующих манифестах ретрансляции.
   - Отслеживайте «События политики по каждому результату» и «Коэффициент отключения» на панели управления PQ Ratchet для удвоенного TTL защитного кэша (теперь на панели мониторинга отображается панель развертывания).
   - Сделайте снимки `sorafs_cli guard-directory fetch` до и после для хранения аудита.

3. **Клиент/SDK canary (Т плюс 1 неделя)**

   - Измените `rollout_phase = "ramp"` в конфигурациях клиента или задайте переопределения `stage-b` для выбранных когорт SDK.
   - Запишите различия телеметрии (группа `sorafs_orchestrator_policy_events_total` по `client_id` и `region`) и прикрепите их к журналу инцидентов развертывания.

4. **Акция по умолчанию (Т плюс 3 недели)**— После завершения управления переключите конфигурации оркестратора и клиента на `rollout_phase = "default"` и поменяйте подписанный контрольный список готовности для выпуска артефактов.

## Контрольный список управления и доказательств

| Изменение фазы | Продвижение ворот | Пакет доказательств | Панели мониторинга и оповещения |
|--------------|--------------|
| Канарейка -> Рампа *(Предварительный просмотр этапа B)* | Уровень отключения на этапе A = 0,7 на продвигаемый регион, проверка билета Argon2 p95  По умолчанию *(применение этапа C)* | Завершена 30-дневная прогонка телеметрии SN16, `sn16_handshake_downgrade_total` не меняется в базовом состоянии, `sorafs_orchestrator_brownouts_total` нулевое во время клиентского канарейки и журнал репетиции переключения прокси-сервера. | Расшифровка `sorafs_cli proxy set-mode --mode gateway|direct`, выходные данные `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, журнал `sorafs_cli guard-directory verify` и подписанный пакет `cargo xtask soranet-rollout-capture --label default`. | Та же плата PQ Ratchet и панели понижения версии SN16, как описано в `docs/source/sorafs_orchestrator_rollout.md` и `dashboards/grafana/soranet_privacy_metrics.json`. |
| Готовность к экстренному понижению/откату | Триггер возникает, когда счетчики перехода на более раннюю версию резко увеличиваются, проверка защитного каталога завершается сбоем или буфер `/policy/proxy-toggle` непрерывно записывает события понижения версии. | Контрольный список `docs/source/ops/soranet_transport_rollback.md`, журналы `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, заявки на инциденты и шаблоны уведомлений. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` и оба пакета предупреждений (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Сохраните каждый артефакт в `artifacts/soranet_pq_rollout/<timestamp>_<label>/` с сгенерированным `rollout_capture.json`, чтобы пакеты управления включали табло, трассировки promtool и дайджесты.
- Прикрепите SHA256-дайджесты загруженных доказательств (протоколы в формате PDF, пакет захватов, снимки охраны) к протоколам продвижения, чтобы можно было воспроизвести одобрения парламента без доступа к промежуточному кластеру.
- Ссылайтесь на план телеметрии в рекламном билете, чтобы доказать, что `docs/source/soranet/snnet16_telemetry_plan.md` является каноническим источником для перехода на более ранние словари и пороговые значения оповещений.

## Обновления информационной панели и телеметрии

`dashboards/grafana/soranet_pq_ratchet.json` теперь поставляется с панелью аннотаций «План развертывания», которая ссылается на этот сборник сценариев и отображает текущую фазу, чтобы проверки управления могли подтвердить активную фазу. Синхронизируйте описание панели с будущими изменениями в регуляторах конфигурации.

Для оповещения обязательно используйте метку существующих правил `stage`, чтобы фазы канареечных событий и фазы по умолчанию запускали отдельные пороговые значения политики (`dashboards/alerts/soranet_handshake_rules.yml`).

## Крючки отката

### По умолчанию -> Рампа (Этап C -> Этап B)1. Понизьте уровень оркестратора через `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (и отразите ту же фазу в конфигурациях SDK), чтобы этап B был повторно применен ко всему парку.
2. Заставьте клиентов использовать безопасный транспортный профиль через `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` и запишите расшифровку, чтобы рабочий процесс исправления `/policy/proxy-toggle` можно было проверить.
3. Запустите `cargo xtask soranet-rollout-capture --label rollback-default`, чтобы заархивировать различия в каталоге Guard, выходные данные promtool и снимки экрана информационной панели в `artifacts/soranet_pq_rollout/`.

### Рампа -> Канарейка (Этап B -> Этап A)

1. Импортируйте снимок защитного каталога, полученный перед повышением уровня, через `sorafs_cli guard-directory import --guard-directory guards.json` и снова запустите `sorafs_cli guard-directory verify`, чтобы включить хэши в пакет понижения.
2. Установите `rollout_phase = "canary"` в конфигурациях Оркестратора и клиента (или переопределите `anonymity_policy stage-a`), а затем повторите детализацию PQ Ratchet из [PQ Ratchet Runbook] (./pq-ratchet-runbook.md), чтобы подтвердить переход на более раннюю версию конвейера.
3. Прикрепите результаты оповещений к журналу инцидентов с обновленными скриншотами телеметрии PQ Ratchet и SN16, а затем уведомите руководство.

### Напоминания о ограждениях

- Обращайтесь к `docs/source/ops/soranet_transport_rollback.md` при каждом понижении в должности и регистрируйте временное снижение как `TODO:` в системе отслеживания развертывания для последующих действий.
- Поместите `dashboards/alerts/soranet_handshake_rules.yml` и `dashboards/alerts/soranet_privacy_rules.yml` в покрытие `promtool test rules` до и после отката к документу с пакетом захвата отклонений предупреждений.