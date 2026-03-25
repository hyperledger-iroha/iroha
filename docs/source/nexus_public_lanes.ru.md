---
lang: ru
direction: ltr
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

# Стейкинг публичных lanes Nexus (NX-9)

Статус: 🈺 В процессе → **runtime + операторская документация согласована** (Апр 2026)
Владельцы: Economics WG / Governance WG / Core Runtime
Ссылка на roadmap: NX-9 – Public lane staking & reward module

Эта заметка фиксирует каноническую модель данных, поверхность инструкций, governance-контроли и
операционные хуки программы стейкинга публичных lanes Nexus. Цель — позволить permissionless
валидаторам присоединяться к публичным lanes, бондить stake, обслуживать блоки и получать награды,
при этом governance сохраняет детерминированные рычаги slashing/runbook.

Кодовый каркас находится в:

- Типы модели данных: `crates/iroha_data_model/src/nexus/staking.rs`
- ISI определения: `crates/iroha_data_model/src/isi/staking.rs`
- Заглушка core executor (возвращает детерминированную guard-ошибку до появления логики NX-9):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs могут начать подключение Norito payloads до полной реализации runtime; инструкции staking
теперь блокируют настроенный staking asset, списывая его с `stake_account`/`staker` на escrow account
(`nexus.staking.stake_escrow_account_id`). Slashes дебетуют escrow и кредитуют настроенный sink
(`nexus.staking.slash_sink_account_id`), а unbonds возвращают средства на исходный счет после истечения таймера.

## 1. Состояние ledger и типы

### 1.1 Записи валидаторов

`PublicLaneValidatorRecord` хранит каноническое состояние каждого валидатора:

| Поле | Описание |
|------|----------|
| `lane_id: LaneId` | Lane, которую обслуживает валидатор. |
| `validator: AccountId` | Аккаунт, подписывающий сообщения консенсуса. |
| `stake_account: AccountId` | Аккаунт, который предоставляет self-bond (может отличаться от идентичности валидатора). |
| `total_stake: Numeric` | Собственный stake + одобренные делегации. |
| `self_stake: Numeric` | Stake, предоставленный валидатором. |
| `metadata: Metadata` | Комиссия %, ids телеметрии, флаги юрисдикции, контактные данные. |
| `status: PublicLaneValidatorStatus` | Жизненный цикл (pending/active/jailed/exiting/etc.). Payload `PendingActivation` кодирует целевой epoch. |
| `activation_epoch: Option<u64>` | Epoch, когда валидатор стал активным (фиксируется при активации). |
| `activation_height: Option<u64>` | Высота блока, зафиксированная при активации. |
| `last_reward_epoch: Option<u64>` | Epoch последней выплаты. |

`PublicLaneValidatorStatus` перечисляет фазы жизненного цикла:

- `PendingActivation(epoch)` — ожидание governance-определенного epoch активации; кортеж хранит самый ранний
  epoch активации, вычисленный как `current_epoch + 1` (genesis bootstrap uses `current_epoch`)
  (epochs выводятся из `epoch_length_blocks`).
- `Active` — участвует в консенсусе и может получать награды.
- `Jailed { reason }` — временно приостановлен (downtime, телеметрийное нарушение и т.д.).
- `Exiting { releases_at_ms }` — unbonding; награды перестают начисляться.
- `Exited` — удален из набора.
- `Slashed { slash_id }` — событие slashing, записанное для аудита.

Метаданные активации монотонны: `activation_epoch`/`activation_height` задаются при первом переводе
pending валидатора в active, и попытки реактивации на более раннем epoch/height отклоняются.
Pending валидаторы продвигаются автоматически в начале первого блока, чей epoch достигает заданной
границы, а счетчик метрик активации (`nexus_public_lane_validator_activation_total`) фиксирует
промоушен вместе со сменой статуса.

Permissioned deployments сохраняют активный roster genesis peers даже до появления stake публичных
валидаторов: пока у peers есть активные consensus keys, runtime использует genesis peers как набор
валидаторов. Это предотвращает bootstrap deadlock, пока staking admission отключен или еще в rollout.

### 1.2 Доли stake и unbonding

Delegators (и валидаторы, пополняющие собственный bond) моделируются через `PublicLaneStakeShare`:

- `bonded: Numeric` — активная bonded сумма.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — ожидающие withdrawals, ключом служит
  клиентский `request_id`.
- `metadata` хранит UX/back-office подсказки (например, номера custody desk).

`PublicLaneUnbonding` задает детерминированный график вывода (`amount`, `release_at_ms`). Torii теперь
публикует активные shares и pending withdrawals через `GET /v1/nexus/public_lanes/{lane}/stake`, чтобы
кошельки могли показывать таймеры без bespoke RPCs.

Hooks жизненного цикла (enforced runtime):

- Записи `PendingActivation(epoch)` автоматически переходят в `Active`, когда текущий epoch достигает `epoch`.
  Активация записывает `activation_epoch` и `activation_height`, а регрессии отклоняются как для авто-активации,
  так и для явных вызовов `ActivatePublicLaneValidator`.
- Записи `Exiting(releases_at_ms)` переходят в `Exited`, когда timestamp блока превышает `releases_at_ms`,
  очищая строки stake-share и освобождая емкость без ручной очистки.
- Запись наград отклоняет shares валидаторов, если валидатор не `Active`, чтобы pending/exiting/jailed
  валидаторы не начисляли выплаты.

### 1.3 Записи наград

Распределение наград использует `PublicLaneRewardRecord` и `PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU",
  "total_reward": "250.0000",
  "shares": [
    { "account": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw", "role": "Validator", "amount": "150" },
    { "account": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe…",
    "distribution_tx": "0xaabbccdd"
  }
}
```

Записи дают аудиторам и дашбордам детерминированные доказательства каждой выплаты. Структура награды
передается в ISI `RecordPublicLaneRewards`.

Runtime guards:

- Nexus builds должны быть включены; offline/stub builds отклоняют запись наград.
- Reward epochs увеличиваются монотонно по lane; stale или дубликатные epochs отклоняются.
- Reward assets должны совпадать с настроенным fee sink (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`), а баланс sink должен полностью покрывать `total_reward`.
- Каждая доля должна быть положительной и соответствовать числовой спецификации asset; сумма долей
  должна равняться `total_reward`.

## 2. Каталог инструкций

Все инструкции находятся под `iroha_data_model::isi::staking`. Они имеют Norito encoders/decoders,
чтобы SDKs могли отправлять payloads без bespoke codecs.

### 2.1 `RegisterPublicLaneValidator`

Регистрирует валидатора и бондит начальный stake:

```norito
{
  "lane_id": 1,
  "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
  "stake_account": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

Правила валидации:

- `initial_stake` >= `min_self_stake` (параметр governance).
- Metadata ДОЛЖНА включать контакт/telemetry hooks до активации.
- Governance утверждает/отклоняет запись; до этого статус `PendingActivation`, и runtime продвигает
  валидатора в `Active` на границе следующего epoch после достижения целевого epoch
  (`current_epoch + 1` (genesis bootstrap uses `current_epoch`) при регистрации).

### 2.2 `BondPublicLaneStake`

Бондит дополнительный stake (self-bond валидатора или вклад делегатора).

Ключевые поля: `staker`, `amount`, опциональная metadata для statements. Runtime должен применять
lane-specific лимиты (`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

Запускает таймер unbonding. Отправители задают детерминированный `request_id`
(рекомендация: `blake2b(invoice)`), `amount` и `release_at_ms`. Runtime должен проверить,
что `amount` <= bonded stake, и ограничить `release_at_ms` настроенным периодом unbonding.

### 2.4 `FinalizePublicLaneUnbond`

После истечения таймера этот ISI разблокирует pending stake и возвращает его `staker`.
Executor валидирует request id, убеждается, что unlock timestamp в прошлом, публикует обновление
`PublicLaneStakeShare` и записывает телеметрию.

### 2.5 `SlashPublicLaneValidator`

Governance использует эту инструкцию для списания stake и jail/eject валидаторов.

- `slash_id` связывает событие с телеметрией + инцидентными документами.
- `reason_code` — стабильная строковая enum (например, `double_sign`, `downtime`, `safety_violation`).
- `metadata` хранит хэши bundles доказательств, runbook ссылки или regulator IDs.

Slashes распространяются на delegators согласно политике governance (пропорциональный ущерб или
validator-first). Runtime логика будет добавлять аннотации `PublicLaneRewardRecord` после реализации NX-9.

### 2.6 `RecordPublicLaneRewards`

Записывает выплату за epoch. Поля:

- `reward_asset`: распределяемый asset (default `xor#nexus`).
- `total_reward`: общий minted/transferred объем.
- `shares`: вектор `PublicLaneRewardShare`.
- `metadata`: ссылки на payout transactions, root hashes или dashboards.

Этот ISI идемпотентен для `(lane_id, epoch)` и является основой ночного учета.

## 3. Операции, жизненный цикл и tooling

- **Lifecycle + modes:** stake-elected lanes включаются через
  `nexus.staking.public_validator_mode = stake_elected`, а restricted lanes остаются admin-managed
  (`nexus.staking.restricted_validator_mode = admin_managed`). Permissioned deployments держат genesis peers
  активными, пока нет stake; для stake-elected lanes мы все еще требуем зарегистрированный peer с активным
  consensus key в commit topology до успешного `RegisterPublicLaneValidator`. Genesis fingerprints и
  `use_stake_snapshot_roster` определяют, будет ли runtime выводить roster из stake snapshots или
  использовать genesis peers.
- **Activation/exit operations:** регистрации попадают в `PendingActivation` с целью
  `current_epoch + 1` (genesis bootstrap uses `current_epoch`) и авто-продвигаются в первом блоке, чей epoch достигает границы
  (`epoch_length_blocks`). Операторы также могут вызвать
  `ActivatePublicLaneValidator` после границы для принудительного продвижения. Выходы переводят
  валидаторов в `Exiting(release_at_ms)` и освобождают емкость только когда timestamp блока достигает
  `release_at_ms`; повторная регистрация после slash все равно требует выхода, чтобы запись стала `Exited`
  и емкость была освобождена. Проверки емкости используют `nexus.staking.max_validators` и выполняются
  после финализатора выхода, поэтому future-dated exits блокируют новые регистрации до истечения таймера.
- **Config knobs:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`,
  `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold` и переключатели режимов выше.
  Протягивать их через `iroha_config::parameters::actual::Nexus` и отражать в `status.md`, когда
  GA значения будут утверждены.
- **Torii/CLI quickstart:**
  - `iroha app nexus lane-report --summary` показывает записи каталога lanes, readiness manifests и режимы
    валидаторов (stake-elected vs admin-managed), чтобы операторы могли подтвердить включение staking admission.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary]`
    показывает lifecycle/activation метки (pending target epoch, `activation_epoch` / `activation_height`,
    exit release, slash id) вместе с bonded/self stake.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator i105...] [--summary]`
    отражает `/stake` endpoint с подсказками pending-unbond для пары `(validator, staker)`.
  - Torii snapshots для dashboards и SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` – metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation epoch/height,
      release timers, bonded stake, last reward epoch.
      `canonical I105 literal rendering` управляет отображением literals (I105 предпочтительно; I105 — второй по предпочтению, только для Sora).
    - `GET /v1/nexus/public_lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) плюс pending unbond timers. `?validator=i105...`
      фильтрует ответ для dashboards, сфокусированных на одном валидаторе; `canonical I105 rendering`
      применяется ко всем literals.
  - Lifecycle ISIs используют стандартный транзакционный путь (Torii
    `/v1/transactions` или CLI instruction pipeline). Примеры Norito JSON payloads:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetry + runbooks:** метрики показывают количество валидаторов, bonded и pending stake, totals rewards
  и slash counters под семейством `nexus_public_lane_*`. Подключать дашборды к тем же данным, что и в
  NX-9 acceptance tests, чтобы validator deltas и reward/slash evidence оставались auditable. Slashing
  инструкции остаются governance-only; запись наград должна подтверждать payout totals (hash payout batch).

## 4. Соответствие roadmap

- ✅ Runtime и WSV storages реализуют жизненный цикл валидаторов NX-9; regressions покрывают сроки активации,
  prerequisites peers, delayed exits и re-registration после slashes.
- ✅ Torii публикует `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` в Norito JSON,
  чтобы SDKs и dashboards могли мониторить состояние lane без custom RPCs.
- ✅ Config и telemetry knobs документированы; смешанные deployments сохраняют изоляцию между stake-elected
  и admin-managed lanes, чтобы roster валидаторов оставался детерминированным.

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
