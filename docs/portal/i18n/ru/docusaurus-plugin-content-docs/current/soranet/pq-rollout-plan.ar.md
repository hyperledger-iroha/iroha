---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: план-развертывания pq
Название: План постквантового запуска SNNet-16G
Sidebar_label: План запуска PQ
Описание: Операционное руководство по обновлению гибридного подтверждения связи SoraNet X25519+ML-KEM с канареечного до стандартного через реле, клиенты и SDK.
---

:::note Стандартный источник
Эта страница отражает `docs/source/soranet/pq_rollout_plan.md`. Сохраняйте две копии идентичными до тех пор, пока старые документы не будут удалены.
:::

SNNet-16G завершает запуск постквантовой передачи SoraNet. Переключатели `rollout_phase` позволяют операторам координировать детерминированное обновление от требования защиты на этапе A до мажоритарного покрытия на этапе B, а затем строгого режима PQ на этапе C без изменения необработанного JSON/TOML для каждой поверхности.

Эта книга охватывает:

- Новые определения фаз и ключи инициализации (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`), связанные в кодовой базе (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Выравнивание флагов SDK и CLI, чтобы каждый клиент мог отслеживать развертывание.
- Планирование ожиданий для ретрансляционных/клиентских канареек и плат управления, которые контролируют обновление (`dashboards/grafana/soranet_pq_ratchet.json`).
- Крючки для отката и ссылки на руководство по пожарным тренировкам ([Руководство по трещотке PQ] (./pq-ratchet-runbook.md)).

## Карта этапов

| `rollout_phase` | Этап сокрытия фактической личности Эффект по умолчанию | Обычное использование |
|-----------------|---------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (Этап А) | Во время нагрева системы убедитесь, что для каждого контура имеется хотя бы один защитный PQ. | базовый уровень и первые канареечные недели. |
| `ramp` | `anon-majority-pq` (Этап B) | Смещение выбора в сторону реле PQ для достижения покрытия >= двух третей; Классические реле остаются запасным вариантом. | канарейки по регионам для эстафет; включает предварительный просмотр SDK. |
| `default` | `anon-strict-pq` (Этап C) | Принудительно использовать только цепи PQ и усилить сигналы понижения версии. | Окончательное обновление после завершения телеметрии и одобрения руководства. |

Если поверхность также устанавливает явный `anonymity_policy`, она переопределяет фазу для этого компонента. Удаление явной фазы делает ее зависимой от значения `rollout_phase`, поэтому операторы могут переключать фазу один раз для каждой среды и позволять клиентам наследовать ее.

## Справочник по конфигурации

### Оркестратор (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Загрузчик оркестратора разрешает резервный вариант времени выполнения (`crates/sorafs_orchestrator/src/lib.rs:2229`) и отображает его через `sorafs_orchestrator_policy_events_total` и `sorafs_orchestrator_pq_ratio_*`. Готовые примеры см. в `docs/examples/sorafs_rollout_stage_b.toml` и `docs/examples/sorafs_rollout_stage_c.toml`.

### Rust-клиент / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` теперь записывает решенную фазу (`crates/iroha/src/client.rs:2315`), чтобы вспомогательные команды (такие как `iroha_cli app sorafs fetch`) могли сообщать о текущей фазе с политикой анонимизации по умолчанию.

## Автоматизация

`cargo xtask` Два инструмента автоматизируют создание таблиц и сбор артефактов.

1. **Создать региональную таблицу**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Периоды принимают суффиксы `s`, `m`, `h` или `d`. Выдается команда `artifacts/soranet_pq_rollout_plan.json`, и к запросу на изменение можно прикрепить сводку Markdown (`artifacts/soranet_pq_rollout_plan.md`).

2. **Сохраните артефакты для репетиции с подписями**

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

   Команда копирует предоставленные файлы в `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, вычисляет дайджесты BLAKE3 для каждого артефакта и записывает `rollout_capture.json`, содержащий метаданные и подпись Ed25519, в полезную нагрузку. Используйте тот же закрытый ключ, которым подписывают журналы пожарных учений, чтобы руководство могло быстро проверить.## Массив флагов для SDK и CLI

| Поверхность | Канарейка (Этап А) | Рампа (Этап B) | По умолчанию (этап C) |
|---------|--------------------------------|----------------|-------------------|
| `sorafs_cli` выборка | `--anonymity-policy stage-a` или фазовая зависимость | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Конфигурация Оркестратора JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Конфигурация клиента Rust (`iroha.toml`) | `rollout_phase = "canary"` (по умолчанию) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` подписанные команды | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, опционально `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, опционально `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, опционально `.ANON_STRICT_PQ` |
| Помощники оркестратора JavaScript | `rolloutPhase: "canary"` или `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Питон `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Свифт `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Все переключатели в SDK соответствуют тому же синтаксическому анализатору этапа, который используется в оркестраторе (`crates/sorafs_orchestrator/src/lib.rs:365`), обеспечивая соответствие многоязычных развертываний настроенному этапу.

## список планирования для canary

1. **Предполетная подготовка (Т минус 2 недели)**

- Убедитесь, что уровень отключения электроэнергии на этапе A составляет менее 1 % в течение предыдущих двух недель и что покрытие PQ составляет >=70 % для каждой области (`sorafs_orchestrator_pq_candidate_ratio`).
   - Временной интервал для проверки управления, соответствующий канареечному окну.
   — Обновлен `sorafs.gateway.rollout_phase = "ramp"` при промежуточном этапе (редактирование JSON оркестратора и повторное развертывание) и пробный запуск пути обновления.

2. **Эстафета канарейки (день Т)**

   — Обновляйте одну зону за раз, устанавливая `rollout_phase = "ramp"` для оркестратора и манифестов участвующих ретрансляторов.
   - Мониторинг «Событий политики для каждого результата» и «Коэффициента отключения» на панели PQ Ratchet (которая теперь включает в себя панель развертывания) для удвоенного TTL защитного кэша.
   - Делайте снимки `sorafs_cli guard-directory fetch` до и после загрузки для хранения аудита.

3. **Клиент/SDK canary (Т плюс 1 неделя)**

   - Инвертируйте `rollout_phase = "ramp"` в настройках клиента или передайте переопределения `stage-b` в указанные пакеты SDK.
   - Зафиксируйте различия телеметрии (`sorafs_orchestrator_policy_events_total`, сгруппированные по `client_id` и `region`) и прикрепите их к журналу развертывания.

4. **Акция по умолчанию (Т плюс 3 недели)**

   — После одобрения руководства измените настройки оркестратора и клиента на `rollout_phase = "default"` и включите контрольный список режима ожидания сайта в версии артефактов.

## Список управления и доказательств| Изменение фазы | Обновление портала | Пакет доказательств Панели мониторинга и оповещения |
|--------------------------|----------------|-----------------|------------------------------------|
| Канарейка -> Рампа *(Предварительный просмотр этапа B)* | Уровень отключения на этапе A составляет менее 1% за 14 дней, `sorafs_orchestrator_pq_candidate_ratio` >= 0,7 для каждого обновленного региона, проверьте билет Argon2 p95  По умолчанию *(применение этапа C)* | Проверьте работоспособность в течение 30 дней для телеметрии SN16, `sn16_handshake_downgrade_total` остается на базовом уровне, ` sorafs_orchestrator_brownouts_total` равен нулю во время работы канареечных клиентов и репетиции аутентификации для переключения прокси-сервера. | Текст `sorafs_cli proxy set-mode --mode gateway|direct`, вывод `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, регистр `sorafs_cli guard-directory verify` и подписанный пакет `cargo xtask soranet-rollout-capture --label default`. | Та же плата PQ Ratchet с платами понижения версии SN16, описанная в `docs/source/sorafs_orchestrator_rollout.md` и `dashboards/grafana/soranet_privacy_metrics.json`. |
| Готовность к экстренному понижению/откату | Срабатывает, когда счетчики перехода на более раннюю версию увеличиваются, проверка защитного каталога завершается неудачей или буфер `/policy/proxy-toggle` записывает постоянные события понижения версии. | Контрольный список `docs/source/ops/soranet_transport_rollback.md`, записи `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, заявки на инциденты и шаблоны уведомлений. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` и оба пакета предупреждений (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Сохраните каждый артефакт под `artifacts/soranet_pq_rollout/<timestamp>_<label>/` с результирующим `rollout_capture.json`, чтобы пакеты управления содержали табло, трассировки promtool и дайджесты.
- Прикрепляйте SHA256-дайджесты загруженных доказательств (протоколы в формате PDF, пакет захвата, защитные снимки) для обновления стенограмм, чтобы можно было повторно выполнять утверждения парламента без доступа к промежуточной среде.
- Обратитесь к плану телеметрии в билете на обновление, чтобы доказать, что `docs/source/soranet/snnet16_telemetry_plan.md` является стандартным источником словаря перехода на более раннюю версию и ограничений предупреждений.

## Обновления информационной панели и телеметрии

`dashboards/grafana/soranet_pq_ratchet.json` теперь включает панель обратной связи «План развертывания», которая ссылается на этот сборник инструкций и отображает текущий этап, чтобы проверки управления могли подтвердить, какой этап активен. Синхронизируйте описание платы с будущими изменениями ручек.

Для оповещений убедитесь, что в текущих правилах используется метка `stage`, чтобы на этапах canary и по умолчанию поднимались отдельные ограничения политики (`dashboards/alerts/soranet_handshake_rules.yml`).

## Хуки для отката

### По умолчанию -> Рампа (Этап C -> Этап B)

1. Опустите оркестратор через `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (и синхронизируйте тот же этап через настройки SDK), чтобы вернуться на этап B на уровне парка.
2. Заставьте клиентов обеспечить безопасность передачи файлов через `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` с захватом текста, чтобы рабочий процесс обработки `/policy/proxy-toggle` оставался контролируемым.
3. Запустите `cargo xtask soranet-rollout-capture --label rollback-default`, чтобы заархивировать различия в каталоге Guard, выходные данные promtool и снимки информационных панелей под `artifacts/soranet_pq_rollout/`.

### Рампа -> Канарейка (Этап B -> Этап A)1. Импортируйте снимок защитного каталога, сделанный перед обновлением, через `sorafs_cli guard-directory import --guard-directory guards.json` и перезагрузите `sorafs_cli guard-directory verify`, чтобы включить уменьшенные хэши пакетов.
2. Установите `rollout_phase = "canary"` (или переопределите с помощью `anonymity_policy stage-a`) для конфигураций оркестратора и клиента, затем перезапустите детализацию PQ Ratchet из [PQ Ratchet Runbook] (./pq-ratchet-runbook.md), чтобы подтвердить путь к переходу на более раннюю версию.
3. Прикрепите обновленные снимки PQ Ratchet и телеметрии SN16 с результатами оповещений в журнал инцидентов, прежде чем уведомлять руководство.

### напоминания о ограждениях

- Обратитесь к `docs/source/ops/soranet_transport_rollback.md` при возникновении простоя и запишите все временные меры по устранению последствий как элемент `TODO:` в средстве отслеживания развертывания, чтобы продолжить.
- Держите `dashboards/alerts/soranet_handshake_rules.yml` и `dashboards/alerts/soranet_privacy_rules.yml` под прикрытием `promtool test rules` до и после любого отката, чтобы документировать отклонения в предупреждениях с помощью пакета захвата.