---
lang: ja
direction: ltr
source: docs/source/soracles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4f46b6cd6b0a1007a286930473f5a36b8b2284a6ffe0b77342e1034b8a777b28
source_last_modified: "2025-12-14T09:58:32.061422+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soracles.md -->

# Soracles — バリデータ支援型オラクル層

このドキュメントは、バリデータ運用のオラクル層に対する正準スキーマと
決定論的スケジューリング規則をまとめる。ロードマップ項目 OR-1〜OR-3、
OR-6、OR-12 を、Norito/JSON レイアウト、委員会/リーダー導出、
コネクタのハッシュ/ケイデンス/PII 伏せ字化、gossip のリプレイ面を
固定することで完了させる。

- **コード参照:** `crates/iroha_data_model/src/oracle/mod.rs`
- **フィクスチャ:** `fixtures/oracle/*.json`

## データモデル

### フィード設定 (`FeedConfig`)
- `feed_id: FeedId` — 正規化された名前（`Name` により UTS‑46/NFC を強制）。
- `feed_config_version: u32` — フィードごとの単調増加。
- `providers: Vec<OracleId>` — バリデータに紐づくオラクル鍵の抽選対象。
- `connector_id/version: string/u32` — フィードに固定されたオフチェーンコネクタ。
- `cadence_slots: NonZeroU64` — `slot % cadence == 0` のときフィードが有効。
- `aggregation: AggregationRule` — `MedianMad(u16)` または `Percentile(u16)`;
  Norito JSON では `{ "aggregation_rule": "<Variant>", "value": <u16> }` で表現。
- `outlier_policy: OutlierPolicy` — `Mad(u16)` または `Absolute { max_delta }`;
  JSON レイアウトは `{ "outlier_policy": "<Variant>", "value": ... }`。
- `min_signers/committee_size: u16` — 安全境界（byzantine `f` を上回る必要）。
- `risk_class: RiskClass` — `Low | Medium | High`（ガバナンス/クオラムに影響）。
  JSON ではユニットバリアントに `{ "risk_class": "<Variant>", "value": null }` を使う。
- 上限: `max_observers`, `max_value_len`, `max_error_rate_bps`,
  `dispute_window_slots`, `replay_window_slots`。

### 観測とレポート
- `ObservationBody` — `{feed_id, feed_config_version, slot, provider_id,
  connector_id/version, request_hash: Hash, outcome, timestamp_ms?}`。
  - `ObservationOutcome` — `Value(ObservationValue)` または
    `Error(ObservationErrorCode)`。
  - `ObservationValue` — fixed-point `{mantissa, scale}`。
  - `ObservationErrorCode` — `ResourceUnavailable | AuthFailed | Timeout |
    Missing | Other(u16)`。
    - 障害区分: 正常系 (`ResourceUnavailable | Timeout | Other`)、設定ミス (`AuthFailed`)、
      欠落 (`Missing`, payload なし/解析エラー)。
  - ハッシュ/署名ヘルパー: `ObservationBody::hash()` と `Observation::hash()`。
- `ReportBody` — `{feed_id, feed_config_version, slot, request_hash, entries[],
  submitter}`。`entries` は `oracle_id` でソート。
  - `ReportEntry` — `{oracle_id, observation_hash, value, outlier}`。
  - ハッシュ/署名ヘルパー: `ReportBody::hash()` と `Report::hash()`。
- `FeedEventOutcome` — `Success { value, entries } | Error { code } | Missing`。
  `FeedEvent` `{feed_id, feed_config_version, slot, outcome}` として発行される。

### コネクタ要求 (OR-6)
- 正準スキーマ: `{feed_id, feed_config_version, slot, connector_id/version,
  method, endpoint, query: BTreeMap, headers: BTreeMap<String,
  RedactedHeaderValue>, body_hash}`。
- ヘッダは `Plain` または `Hashed`。API キーなど秘匿すべき値は `Hashed` を推奨。
  `validate_redaction` は `authorization`/`cookie`/`x-api-*` などの機密ヘッダ名を
  hashed でない場合に拒否する。`body_hash` はリクエスト payload をハッシュ化して
  オンチェーンに保持する。
- `ConnectorRequest::hash()` は観測/レポート/イベントが広告する正準 `request_hash`
  を生成する（XOR/USD サンプル:
  `hash:26A12D920ACC7312746C7534926D971D58FF443C1345B9C14DAF5C3C5E3E6A69#D88C`）。
- `ConnectorResponse` は payload hash と任意のエラーコードを保持し、
  オペレーターが内容を漏らさずにコネクタ応答を保管できる。
- PII フィルタリング: 社会的識別子などには `KeyedHash { pepper_id, digest }`
  を使用し、`digest = Hash::new(pepper || payload)` を導出する。
  `KeyedHash::verify` は監査用に keyed hash を検証しつつ、平文をオフチェーンに保つ。

### フィクスチャ
`fixtures/oracle/` に例がある:
- `feed_config_price_xor_usd.json`
- `connector_request_price_xor_usd.json`
- `observation_price_xor_usd.json`
- `report_price_xor_usd.json`
- `feed_event_price_xor_usd.json`
- `feed_config_social_follow.json`
- `connector_request_social_follow.json`
- `observation_social_follow.json`
- `report_social_follow.json`
- `feed_event_social_follow.json`
- **Twitter binding registry:** Twitter フォローフィードは `RecordTwitterBinding`
  を通じて keyed-hash の証明を保持し、`RevokeTwitterBinding` による手動クリーンアップを
  サポートする。証明は `HMAC(pepper, twitter_user_id||epoch)` → `{uaid, status, tweet_id,
  challenge_hash, expires_ms}` として保存され、PII を平文で保持しない。検索は
  `FindTwitterBindingByHash` を用いた keyed hash で行い、取り消しは型付きイベントを
  サブスクライバへ通知する。

#### バイラル・インセンティブフロー (SOC-2)

Twitter フォローの証明がレジストリに入ると、バイラルインセンティブ契約は
keyed-hash binding を使う小さな ISI セットを公開する:

- `ClaimTwitterFollowReward { binding_hash }` は、設定済みの報酬をバイラル
  インセンティブプールから UAID に紐づくアカウントへ支払う（binding ごとに 1 回、
  UAID/日および binding ごとに上限）。`viral_daily_counters` / `viral_binding_claims`
  を更新し、`governance.viral_incentives` で守られた日次予算を消費する。
  既存の escrow が同じ binding にある場合、報酬経路で escrow を UAID へ解放し、
  送信者の一回限りボーナスを記録する。
- `SendToTwitter { binding_hash, amount }` は、有効な `Following` 証明があれば
  直ちに UAID アカウントへ送金し、なければ `viral_escrows` に預ける。
  その後 binding が現れると `ClaimTwitterFollowReward` が escrow を解放する。
- `CancelTwitterEscrow { binding_hash }` は binding が現れない場合に送信者が
  未処理の escrow を回収できる。`halt` スイッチと設定の deny-list が適用される。

ガバナンスは `iroha_config::parameters::Governance` の `ViralIncentives` セクションで、
インセンティブプール、escrow アカウント、報酬/ボーナス額、UAID/日上限、
binding 上限、日次予算、deny-list を設定する。プロモーション制御には
`promo_starts_at_ms` / `promo_ends_at_ms`（送金/請求の両方に対するウィンドウ制限）と、
キャンペーン全体の `campaign_cap` が追加される。キャンペーン上限は全期間の報酬と
送信者ボーナスを追跡する。期間外または上限超過の試行は、状態変更前に決定論的に
拒否される。`ViralRewardApplied` イベントはプロモーション有効フラグ、halt フラグ、
キャンペーン支出スナップショット、設定済み上限を含み、
`iroha_social_campaign_*`/`iroha_social_promo_active`/`iroha_social_halted` メトリクスと
対応ダッシュボード（`dashboards/grafana/social_follow_campaign.json`）を駆動する。
`halt` フラグは Twitter binding レジストリを変更せず、バイラルフローのみ凍結する。

ツール向け:
- CLI は `iroha social claim-twitter-follow-reward|send-to-twitter|cancel-twitter-escrow`
  を提供し、Norito JSON の `KeyedHash` payload（binding hash）と、送金時は
  `Numeric` 金額を受け取り、対応する命令を構築/送信する。
- JS SDK は `buildClaimTwitterFollowRewardInstruction`, `buildSendToTwitterInstruction`,
  `buildCancelTwitterEscrowInstruction` で同等のヘルパーを提供し、同じ keyed-hash
  形状と数量を受け取って Norito-ready の命令オブジェクトを返す。

これらのフィクスチャは正準ハッシュ表記（`hash:...#...`）、大文字署名、
決定論的 ed25519 キーから導出した i105 プロバイダ ID（例:
`soraゴヂアヌメネヒョタルアキュカンコプヱガョラツゴヸナゥヘガヮザネチョヷニャヒュニョメヺェヅヤアキャヅアタタナイス`）を
使用する。

社会/PII を含むフィードでは、`ObservationValue::from_hash`, `from_keyed_hash`,
`from_uaid` がハッシュ化された識別子から決定論的な fixed-point 値を導出し、
ソーシャルフォローキットが PII 安全性を保ちながらバリデータ間の決定性を維持する。
これらのヘルパーは scale を 0 に保ち、hash からオンチェーン値へ変換する際に
負のエンコードを避けるため mantissa の上位ビットをクリアする。

SDK/CLI の参照キットは `crates/iroha_data_model/src/oracle/mod.rs::kits` にある。
同じフィクスチャを読み込み、XOR/USD 価格フィードと Twitter フォロー binding フィード向けの
`OracleKit` バンドルを提供し、コードサンプルとテストを正準 JSON payload に合わせる。
- 追加の参照キット:
  - `twitter_follow_binding` フィード: `feed_config_twitter_follow.json`,
    `connector_request_twitter_follow.json`,
    `observation_twitter_follow.json`, `report_twitter_follow.json`,
    `feed_event_twitter_follow.json` は、Twitter フォローオラクルの keyed-hash UAID
    マッピングを記録する（pepper `pepper-social-v1`, request hash
    `hash:25BD3C09F859A100396B5FD39066A3AEC5FB2EE6458D8FEDD0E403A35B5B3745#8EF1`）。
  - 既存の `price_xor_usd` フィクスチャは、現在のコネクタハッシュ
    (`hash:59CFEA4268FB255E2FDB550B37CB83DF836D62744F835F121E5731AB62679BDB#844C`) と
    observation/report/event digest に合わせて更新され、SDK 間の整合性を保っている。

### 集計ヘルパー

`aggregate_observations` は `ReportBody` と `FeedEventOutcome` を構築し、
上限と outlier ポリシーを強制する:

```rust
let output = aggregate_observations(
    &feed_config,
    slot,
    request_hash,
    submitter_oracle_id,
    &observations,
)?;
assert!(matches!(output.outcome, FeedEventOutcome::Success(_)));
```

検証内容:
- feed/config/connector の固定とケイデンス (`validate_observation_meta`).
- slot と request-hash の整合、プロバイダ所属、重複検出。
- 上限: `max_observers`, `max_value_len`, 重複 oracle エントリ（`validate_report_caps`）。
- `OutlierPolicy::Mad` または `OutlierPolicy::Absolute` による outlier マーク。
  `AggregationRule` による中央値/パーセンタイル集計。

エラーは `OracleAggregationError` として返され、オンチェーン admission 経路に接続する。
すべての観測が同じエラーコードの場合は `FeedEventOutcome::Error`、
観測が無い場合は `FeedEventOutcome::Missing` となる。

ホスト側の利用者: `iroha_core::oracle::OracleAggregator` と `ObservationAdmission` が
同じヘルパーを使ってノード内検証、リプレイガード、レポート/アウトカム生成を行う。

## 委員会とリーダー選定 (OR-2)

委員会抽選は `(feed_id, feed_config_version, epoch, validator_set_root, providers[])`
にスコープされた決定論的プロセスである。ヘルパー
`derive_committee(feed_id, version, epoch, root, providers, committee_size)` は
`CommitteeDraw { seed, members[] }` を返し、`members` は Blake2b-256 で
`(seed || provider_id)` をハッシュしたスコアの低い順に選ばれる。
重複プロバイダはスコアリング前に除外される。

リーダーはスロット単位で決定論的に導出される:

```rust
let draw = derive_committee(...);
let leader = draw.leader_for_slot(slot);
```

リーダーのインデックスは `Hash(seed || slot)` から導出されるため、
各バリデータは調整なしに特定 slot の submitter を同一に計算できる。
