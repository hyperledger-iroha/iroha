<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi Формальная модель (TLA+ / Apalache)

Этот каталог содержит ограниченные формальные модели безопасности и жизнеспособности Sumeragi.

## Область применения

`Sumeragi.tla` фиксирует путь фиксации:
- прогрессия фаз (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- пороги голосования и кворума (`CommitQuorum`, `ViewQuorum`),
- кворум взвешенной доли (`StakeQuorum`) для защиты фиксации в стиле NPoS,
- Причинность эритроцитов (`Init -> Chunk -> Ready -> Deliver`) с доказательствами заголовка/дайджеста,
- GST и слабые предположения о справедливости в отношении честных действий по прогрессу.

`SumeragiFrontierRecovery.tla` демонстрирует целенаправленный урок Тайры по зависанию вокруг одного
ожидающий блокирования прилегающей границы:
- доказательства принятия решения о голосовании ниже или при наличии кворума,
- отставание в очереди голосов и локальный слив,
- отсутствует по сравнению с состоянием локальной полезной нагрузки,
- свежее и устаревшее право собственности на восстановление границы,
- маркер кворума-перепланирования/регулировка окна,
- будущие границы/доказательства нового взгляда, которые могут вновь закрепить локальную границу,
- детерминированная фиксация после GST, повторная передача, ограниченное вращение представления и
  результаты падения с нулевыми доказательствами.

Обе модели намеренно абстрагируют форматы проводной связи, ECDSA/подпись.
проверка и полная информация о сети.

## Файлы- `Sumeragi.tla`: модель и свойства протокола.
- `Sumeragi_fast.cfg`: меньший набор параметров, совместимый с CI.
- `Sumeragi_deep.cfg`: больший набор параметров напряжения.
- `SumeragiFrontierRecovery.tla`: целенаправленная модель восстановления границ.
- `SumeragiFrontierRecovery_fast.cfg`: меньший набор пограничных параметров, совместимых с CI.
- `SumeragiFrontierRecovery_deep.cfg`: увеличенный набор границ невыполненной работы/окна/представления.
- `SumeragiFrontierRecovery_wide.cfg`: ручной набор с более широкой границей.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: мутация устаревшего владельца с ожидаемым сбоем.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: мутация очереди голосования с ожидаемым сбоем.

## Свойства

Инварианты:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Временное свойство:
- `EventuallyCommit` (`[] (gst => <> committed)`), с кодировкой справедливости после GST
  в рабочем режиме в `Next` (тайм-аут/защита от сбоев включена)
  прогрессивные действия). Это позволяет проверить модель с помощью Apalache 0.52.x, который
  не поддерживает операторы справедливости `WF_` внутри проверенных временных свойств.

Инварианты восстановления границ:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, что исключает терминал
  состояние после GST, когда `pending /\ voteBacked /\ ~committed` не имеет восстановления,
  фиксация, повторная передача, вращение или переход с ограниченным отбрасыванием.Граница восстановления временного имущества:
- `PostGstVoteBackedFrontierEventuallyResolves`: после GST все неразрешенные
  ожидающее пограничное состояние, поддерживаемое голосованием, в конечном итоге достигает фиксации, полезная нагрузка
  восстановление, повторная передача кворума, повторная привязка к будущим границам или ограниченное представление
  вращение.
- `RecoveredPayloadEventuallyAdvances`: приграничное государство, поддерживаемое голосованием,
  восстановленная полезная нагрузка не может оставаться в ожидании вечно без фиксации,
  повторная передача, повторное привязывание или вращение.
- `QuorumRetransmitEventuallyLeavesPending`: после запуска повторной передачи кворума
  для приграничного государства, поддержанного голосованием, ожидающая оболочка должна в конечном итоге очиститься.
- `FutureFrontierEvidenceEventuallyReanchors`: более поздние рубежи/доказательства нового взгляда
  должен либо очистить ожидающую обертку, либо использоваться в качестве пограничной привязки.

## Карта Успения

Пограничная модель намеренно конечна. Это реализация
поверхности, которые он абстрагирует:| Концепция модели | Поверхность реализации |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | Обработка `PendingBlock` и локальные проверки полезной нагрузки в `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, а также материализация BlockCreated/граничной собственности в `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Подсчет голосов за фиксацию и входной контроль голосов осуществляется `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` и `reschedule_ignores_quorum_timeout_vote_queue_backlog` в `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | Состояние активного/устаревшего владельца границы в `frontier_slot_has_active_owner_state_for_view(...)`, выход устаревшего владельца в `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` и замена очистки в `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Изменение графика кворума на основе голосования в `reschedule_stale_pending_blocks_with_now(...)`; покрытие регрессии включает `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Точный пограничный ремонт кузова и устаревший допуск на ремонт RBC в `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` и `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Выбор цели повторной передачи кворума, `rebroadcast_pending_block_updates(...)`, и детерминированные вызовы изменения представления в `reschedule_stale_pending_blocks_with_now(...)`. |
| `futureFrontierEvidence` | Будущее новое представление/доказательство кворума более высокого уровня в `on_pacemaker_propose_ready(...)`, охваченное `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## Бег

Из корня репозитория:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Бегун явно устанавливает Apalache `--length` для каждого режима:| Режим | Длина | Использование по назначению |
| --- | ---: | --- |
| `fast` | 10 | Проверка пути фиксации CI |
| `deep` | 10 | Увеличенная проверка пути фиксации |
| `frontier-fast` | 10 | CI пограничный контроль |
| `frontier-deep` | 12 | Большой пограничный контроль |
| `frontier-wide` | 14 | Ручная/ночная проверка стресса на границе |

`APALACHE_LENGTH=<n>` переопределяет значение по умолчанию для каждого режима при локальном исследовании
контрпример или расширение ограниченного доказательства.

### Воспроизводимая локальная настройка (Docker не требуется)

Установите закрепленную локальную цепочку инструментов Apalache, используемую в этом репозитории:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Раннер автоматически обнаруживает эту установку по адресу:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
После установки `ci/check_sumeragi_formal.sh` должен работать без дополнительных переменных окружения:

```bash
bash ci/check_sumeragi_formal.sh
```

Мутации ожидаемого отказа намеренно находятся за пределами нормального CI. Они должны
терпят неудачу в Apalache и полезны при изменении модели:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Если Apalache нет в `PATH`, вы можете:

- установите `APALACHE_BIN` в путь к исполняемому файлу или
- использовать резервный вариант Docker (включен по умолчанию, когда доступен `docker`):
  - изображение: `APALACHE_DOCKER_IMAGE` (по умолчанию `ghcr.io/apalache-mc/apalache:0.52.2`)
  - требуется работающий демон Docker
  - отключить резервный вариант с помощью `APALACHE_ALLOW_DOCKER=0`.

Примеры:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Примечания- Эта модель дополняет (не заменяет) исполняемые тесты модели Rust в
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  и
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Проверки ограничены постоянными значениями в файлах `.cfg`.
- PR CI запускает эти проверки в `.github/workflows/pr.yml` через
  `ci/check_sumeragi_formal.sh`.
