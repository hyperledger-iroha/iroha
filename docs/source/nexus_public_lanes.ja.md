---
lang: ja
direction: ltr
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

# Nexus Public Lane Staking (NX-9)

ステータス: 🈺 進行中 → **runtime + オペレータードキュメント整合** (2026年4月)
オーナー: Economics WG / Governance WG / Core Runtime
ロードマップ参照: NX-9 – Public lane staking & reward module

本ノートは、Nexus の public-lane ステーキングプログラムに向けたカノニカルなデータモデル、
命令サーフェス、ガバナンス制御、運用フックをまとめる。目的は、permissionless バリデータが
public lanes に参加し、stake をボンドし、ブロックをサービスし、報酬を得る一方で、
ガバナンスが決定論的な slashing/runbook のレバーを維持することにある。

コードの足場は以下に配置済み:

- データモデル型: `crates/iroha_data_model/src/nexus/staking.rs`
- ISI 定義: `crates/iroha_data_model/src/isi/staking.rs`
- Core executor stub (NX-9 実装まで決定論的 guard エラーを返す):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs は完全な runtime 実装前に Norito payload を配線できる。ステーク命令は
設定済みの staking asset を `stake_account`/`staker` から bonded escrow account
(`nexus.staking.stake_escrow_account_id`) へ引き落としてロックする。Slashes は escrow を
デビットし、設定済み sink (`nexus.staking.slash_sink_account_id`) をクレジットする。Unbonds は
タイマー満了後に元のアカウントへ返金する。

## 1. Ledger state と型

### 1.1 バリデータレコード

`PublicLaneValidatorRecord` は各バリデータのカノニカル状態を追跡する:

| フィールド | 説明 |
|-----------|------|
| `lane_id: LaneId` | バリデータが担当する lane。 |
| `validator: AccountId` | コンセンサスメッセージに署名するアカウント。 |
| `stake_account: AccountId` | self-bond を供給するアカウント (validator のアイデンティティと異なる場合あり)。 |
| `total_stake: Numeric` | self stake + 承認済みデリゲーション。 |
| `self_stake: Numeric` | バリデータが提供する stake。 |
| `metadata: Metadata` | コミッション率、テレメトリID、管轄フラグ、連絡先情報。 |
| `status: PublicLaneValidatorStatus` | ライフサイクル (pending/active/jailed/exiting/etc.)。`PendingActivation` の payload がターゲット epoch を持つ。 |
| `activation_epoch: Option<u64>` | バリデータが active になった epoch (アクティベーション時に設定)。 |
| `activation_height: Option<u64>` | アクティベーション時に記録された block height。 |
| `last_reward_epoch: Option<u64>` | 最後の支払いを行った epoch。 |

`PublicLaneValidatorStatus` のライフサイクル:

- `PendingActivation(epoch)` — ガバナンス指定の activation epoch を待機。タプル payload は
  `epoch_length_blocks` から導出される最短 activation epoch を保持する。
- `Active` — コンセンサスに参加し、報酬を受け取れる。
- `Jailed { reason }` — 一時停止 (downtime, telemetry breach など)。
- `Exiting { releases_at_ms }` — unbonding; 報酬の加算は停止。
- `Exited` — セットから除外。
- `Slashed { slash_id }` — 監査向けの slashing イベント。

アクティベーションのメタデータは単調増加である。`activation_epoch`/`activation_height` は
pending バリデータが最初に active になった時点で設定され、それ以前の epoch/height への
再アクティベーション試行は拒否される。pending バリデータは、スケジュール境界に到達した
最初のブロックで自動昇格し、アクティベーションメトリクスカウンタ
(`nexus_public_lane_validator_activation_total`) が状態変更とともに記録する。

Permissioned 展開は、public-lane の stake が存在しない場合でも genesis peers をアクティブに
維持する。consensus keys が生きている限り、runtime は validator set に genesis peers を
フォールバックし、staking admission が無効または rollout 中でも bootstrap deadlock を回避する。

### 1.2 Stake shares と unbonding

Delegators (および self-bond を追加するバリデータ) は `PublicLaneStakeShare` で表現される:

- `bonded: Numeric` — 現在の bonded 量。
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — クライアント指定の `request_id` を鍵にした pending withdrawals。
- `metadata` は UX/バックオフィスのヒント (例: custody desk 参照番号) を格納。

`PublicLaneUnbonding` は決定論的な引き出しスケジュール (`amount`, `release_at_ms`) を持つ。
Torii は `GET /v1/nexus/public_lanes/{lane}/stake` で live shares と pending withdrawals を公開し、
ウォレットが RPC 拡張なしでタイマー表示できるようにする。

ライフサイクルフック (runtime enforced):

- `PendingActivation(epoch)` は現在の epoch が `epoch` に到達すると自動的に `Active` に移行する。
  アクティベーションは `activation_epoch` と `activation_height` を記録し、auto-activation と
  明示的な `ActivatePublicLaneValidator` 呼び出しの両方で regressions を拒否する。
- `Exiting(releases_at_ms)` は block timestamp が `releases_at_ms` を超えると `Exited` に遷移し、
  stake-share 行をクリアして容量を手動清掃なしで回収する。
- 報酬記録は validator が `Active` でない場合は shares を拒否し、pending/exiting/jailed が
  payouts を累積しないようにする。

### 1.3 報酬レコード

報酬分配は `PublicLaneRewardRecord` と `PublicLaneRewardShare` を使用する:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "xor#wonderland",
  "total_reward": "250.0000",
  "shares": [
    { "account": "validator@lane", "role": "Validator", "amount": "150" },
    { "account": "delegator@lane", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe…",
    "distribution_tx": "0xaabbccdd"
  }
}
```

レコードは監査人とダッシュボードに各 payout の決定論的証跡を提供する。報酬構造は
`RecordPublicLaneRewards` ISI に流れる。

Runtime guards:

- Nexus builds が有効であること。offline/stub builds は報酬記録を拒否する。
- Reward epochs は lane ごとに単調に進む。stale/重複 epochs は拒否。
- Reward assets は設定済みの fee sink (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) と一致し、sink 残高は `total_reward` を完全に賄う必要がある。
- 各 share は正であり、asset の数値仕様を満たすこと。share 合計は `total_reward` に一致すること。

## 2. 命令カタログ

すべての命令は `iroha_data_model::isi::staking` に配置される。Norito encoders/decoders を
derive しているため、SDKs は bespoke codec なしで payloads を送信できる。

### 2.1 `RegisterPublicLaneValidator`

バリデータを登録し、初期 stake をボンドする:

```norito
{
  "lane_id": 1,
  "validator": "validator@lane",
  "stake_account": "validator@lane",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

検証ルール:

- `initial_stake` >= `min_self_stake` (ガバナンスパラメータ)。
- Metadata はアクティベーション前に contact/telemetry hooks を含むこと。
- ガバナンスが承認/却下する。承認まで status は `PendingActivation` で、ターゲット epoch 到達後の
  次の境界で runtime が `Active` に昇格させる。

### 2.2 `BondPublicLaneStake`

追加 stake をボンドする (validator の self-bond または delegator の寄与)。

主要フィールド: `staker`, `amount`, 任意の metadata。Runtime は lane 固有の制限
(`max_delegators`, `min_bond`, `commission caps`) を強制する。

### 2.3 `SchedulePublicLaneUnbond`

Unbonding タイマーを開始する。submitters は決定論的な `request_id`
(推奨: `blake2b(invoice)`)、`amount`、`release_at_ms` を指定する。Runtime は
`amount` <= bonded stake を検証し、`release_at_ms` を設定済みの unbonding period にクランプする。

### 2.4 `FinalizePublicLaneUnbond`

タイマー満了後、この ISI は pending stake を解除して `staker` に返す。executor は request id を
検証し、unlock timestamp が過去であることを確認し、`PublicLaneStakeShare` の更新を emit し、
telemetry を記録する。

### 2.5 `SlashPublicLaneValidator`

ガバナンスはこの命令を用いて stake をデビットし、validator を jail/eject する。

- `slash_id` はイベントを telemetry + incident docs に紐付ける。
- `reason_code` は安定した enum 文字列 (例: `double_sign`, `downtime`, `safety_violation`).
- `metadata` は証拠バンドルのハッシュ、runbook 参照、regulator IDs を格納する。

Slashes はガバナンスポリシーに応じて delegators に波及する (比例または validator-first)。
Runtime ロジックは NX-9 実装時に `PublicLaneRewardRecord` 注釈を emit する。

### 2.6 `RecordPublicLaneRewards`

ある epoch の payout を記録する。フィールド:

- `reward_asset`: 配布 asset (デフォルト `xor#nexus`).
- `total_reward`: minted/transferred の合計。
- `shares`: `PublicLaneRewardShare` のベクタ。
- `metadata`: payout トランザクション、root hashes、または dashboards への参照。

この ISI は `(lane_id, epoch)` 単位で idempotent であり、夜間会計の基盤となる。

## 3. 運用、ライフサイクル、ツール

- **ライフサイクル + モード:** stake-elected lanes は
  `nexus.staking.public_validator_mode = stake_elected` で有効化され、restricted lanes は
  admin-managed のまま (`nexus.staking.restricted_validator_mode = admin_managed`)。Permissioned
  deployments は stake が存在するまで genesis peers を保持する。stake-elected lanes では
  `RegisterPublicLaneValidator` が成功する前に、commit topology に live consensus key を持つ
  登録済み peer が必要。genesis fingerprints と `use_stake_snapshot_roster` は runtime が
  stake snapshots から roster を導出するか、genesis peers にフォールバックするかを決める。
- **Activation/exit operations:** 登録は `PendingActivation` に入り、`epoch_length_blocks`
  の境界に到達した最初のブロックで自動昇格する。オペレーターは境界後に
  `ActivatePublicLaneValidator` を呼び出して強制昇格もできる。退出は `Exiting(release_at_ms)`
  に移行し、block timestamp が `release_at_ms` に到達して初めて容量が解放される。slash 後の
  再登録でも退出が必要で、`Exited` へ遷移して容量が回収される。容量チェックは
  `nexus.staking.max_validators` を使い exit finalizer の後に実行されるため、将来時刻の exit
  はタイマー満了まで新規登録をブロックする。
- **Config knobs:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`, `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, および上記の validator-mode
  switches。`iroha_config::parameters::actual::Nexus` を通して配線し、GA 値が確定したら
  `status.md` に反映する。
- **Torii/CLI quickstart:**
  - `iroha nexus lane-report --summary` は lane catalog entries、manifest readiness、validator modes
    (stake-elected vs admin-managed) を表示し、ステーキング admission が有効か確認できる。
  - `iroha_cli nexus public-lane validators --lane <id> [--summary] [--address-format {ih58,compressed}]`
    は lifecycle/activation 指標 (pending target epoch, `activation_epoch` / `activation_height`,
    exit release, slash id) と bonded/self stake を表示する。
    `iroha_cli nexus public-lane stake --lane <id> [--validator account@domain] [--summary]` は
    `(validator, staker)` ペアの pending-unbond ヒント付きで `/stake` をミラーする。
  - Torii snapshots (dashboards/SDKs 向け):
    - `GET /v1/nexus/public_lanes/{lane}/validators` – metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation epoch/height,
      release timers, bonded stake, last reward epoch.
      `address_format=ih58|compressed` で literal 表示を制御（IH58 推奨、compressed（`snx1`）は Sora 専用の次善）。
    - `GET /v1/nexus/public_lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) と pending unbond timers。`?validator=account@domain` は
      特定バリデータ向けにフィルタし、`address_format` は全 literal に適用。
  - Lifecycle ISIs は標準トランザクションパスを使用 (Torii `/v1/transactions`
    または CLI instruction pipeline)。Norito JSON payload 例:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "validator@nexus" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "validator@nexus",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetry + runbooks:** メトリクスは validator 数、bonded/pending stake、reward totals、slash counters を
  `nexus_public_lane_*` ファミリで公開する。NX-9 acceptance tests と同じデータセットに
  dashboards を接続し、validator deltas と reward/slash evidence を監査可能に保つ。Slashing
  instructions は governance-only のまま。報酬記録は payout totals の証明 (payout batch の hash) が必要。

## 4. Roadmap alignment

- ✅ Runtime と WSV storages が NX-9 の validator lifecycle を実装。activation timing、peer prerequisites、
  delayed exits、slash 後の再登録を回帰テストでカバー。
- ✅ Torii が `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` を Norito JSON で提供し、
  SDKs と dashboards が custom RPC なしで lane 状態を監視可能。
- ✅ Config/telemetry knobs をドキュメント化。混在 deployments でも stake-elected と admin-managed lanes を
  分離し、validator rosters の決定性を維持。

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
