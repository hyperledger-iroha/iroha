---
lang: ja
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/da/threat_model.md` を反映します。旧ドキュメントが
退役するまで両方のバージョンを同期してください。
:::

# Sora Nexus Data Availability 脅威モデル

_最終レビュー: 2026-01-19 — 次回レビュー予定: 2026-04-19_

保守サイクル: Data Availability Working Group (<=90 days)。各改訂は
`status.md` に記録し、アクティブな緩和チケットとシミュレーション
アーティファクトへのリンクを含めること。

## 目的と範囲

Data Availability (DA) プログラムは Taikai 配信、Nexus lane の blobs、
ガバナンスアーティファクトが Byzantine/ネットワーク/運用障害下でも
取得可能であることを保証する。本脅威モデルは DA-1 (アーキテクチャと
脅威モデル) のエンジニアリング作業を支え、後続の DA タスク (DA-2
から DA-10) の基準となる。

対象範囲のコンポーネント:
- Torii DA ingest 拡張と Norito metadata writer。
- SoraFS バックの blob ストレージツリー (hot/cold tiers) と
  replication ポリシー。
- Nexus ブロック commitments (wire formats, proofs, light-client APIs)。
- DA payloads 向け PDP/PoTR enforcement hooks。
- オペレータ作業 (pinning, eviction, slashing) と observability pipelines。
- DA オペレータとコンテンツを承認/排除する governance 承認。

この文書の対象外:
- 経済モデルの全体 (DA-7 workstream に記載)。
- 既に SoraFS threat model で扱う SoraFS 基本プロトコル。
- 脅威面以外の client SDK ergonomics。

## アーキテクチャ概要

1. **Submission:** クライアントは Torii DA ingest API で blobs を送信。
   ノードは blobs をチャンク化し、Norito manifests (blob type, lane,
   epoch, codec flags) をエンコードして SoraFS hot tier に保存する。
2. **Advertisement:** Pin intents と replication hints は registry (SoraFS
   marketplace) を介してストレージプロバイダへ伝播し、hot/cold
   retention 目標を示す policy tags を付与する。
3. **Commitment:** Nexus sequencer は blob commitments (CID + optional KZG
   roots) をブロックに含める。ライトクライアントは commitment hash と
   metadata で availability を検証する。
4. **Replication:** ストレージノードは割り当てられた shares/chunks を
   取得し、PDP/PoTR チャレンジを満たし、ポリシーに従って hot/cold
   tiers 間でデータを移動する。
5. **Fetch:** 消費者は SoraFS または DA-aware gateway からデータを取得し、
   proofs を検証し、replicas が消えた場合は repair request を出す。
6. **Governance:** 議会と DA 監督委員会がオペレータ、rent schedules、
   enforcement escalations を承認。ガバナンスアーティファクトは同じ
   DA 経路で保存し透明性を確保する。

## 資産とオーナー

影響度スケール: **Critical** は ledger の安全性/活性を破壊。
**High** は DA backfill またはクライアントを阻害。**Moderate** は
品質劣化だが回復可能。**Low** は影響限定。

| Asset | Description | Integrity | Availability | Confidentiality | Owner |
| --- | --- | --- | --- | --- | --- |
| DA blobs (chunks + manifests) | Taikai/lane/governance blobs in SoraFS | Critical | Critical | Moderate | DA WG / Storage Team |
| Norito DA manifests | Typed metadata describing blobs | Critical | High | Moderate | Core Protocol WG |
| Block commitments | CIDs + KZG roots inside Nexus blocks | Critical | High | Low | Core Protocol WG |
| PDP/PoTR schedules | Enforcement cadence for DA replicas | High | High | Low | Storage Team |
| Operator registry | Approved storage providers & policies | High | High | Low | Governance Council |
| Rent and incentive records | Ledger entries for DA rent & penalties | High | Moderate | Low | Treasury WG |
| Observability dashboards | DA SLOs, replication depth, alerts | Moderate | High | Low | SRE / Observability |
| Repair intents | Requests to rehydrate missing chunks | Moderate | Moderate | Low | Storage Team |

## 敵対者と能力

| Actor | Capabilities | Motivations | Notes |
| --- | --- | --- | --- |
| Malicious client | Malformed blobs、stale manifests の replay、ingest への DoS | Taikai 配信妨害、無効データ注入 | 特権鍵なし |
| Byzantine storage node | レプリカドロップ、PDP/PoTR proof 偽造、共謀 | DA retention 低下、rent 回避、データ人質 | 有効な operator credentials を保有 |
| Compromised sequencer | commitments 省略、ブロック equivocation、metadata 並べ替え | DA submission 隠蔽、不整合 | コンセンサス多数に制約 |
| Insider operator | governance 濫用、retention ポリシー改ざん、credential 流出 | 経済的利益、妨害 | hot/cold tier 基盤にアクセス |
| Network adversary | partition、replication 遅延、MITM 注入 | availability 低下、SLO 劣化 | TLS 破壊不可だが drop/slow は可能 |
| Observability attacker | ダッシュボード/アラート改ざん、インシデント隠蔽 | DA outage 隠蔽 | テレメトリーパイプラインへのアクセスが必要 |

## 信頼境界

- **Ingress boundary:** クライアントから Torii DA 拡張。リクエスト認証、
  rate limiting、payload 検証が必要。
- **Replication boundary:** ストレージノード間の chunks/proofs 交換。相互認証されるが
  Byzantine 挙動の可能性あり。
- **Ledger boundary:** ブロックにコミットされたデータと off-chain storage の境界。
  コンセンサスが整合性を守るが availability には off-chain enforcement が必要。
- **Governance boundary:** Council/Parliament の承認 (operators/budget/slashing)。
  破綻は DA 展開に直接影響。
- **Observability boundary:** metrics/logs の収集とダッシュボード/アラートへのエクスポート。
  改ざんで outage/攻撃が隠れる。

## 脅威シナリオとコントロール

### Ingest 経路の攻撃

**Scenario:** 悪意あるクライアントが不正 Norito payload や大型 blobs を送信し、
リソース枯渇や不正 metadata を紛れ込ませる。

**Controls**
- Norito schema 検証と厳格なバージョン交渉。未知フラグは拒否。
- Torii ingest endpoint での rate limiting と認証。
- SoraFS chunker による chunk size 上限と決定論的エンコード。
- Integrity checksum が一致した後のみ manifest を永続化。
- 決定論的 replay cache (`ReplayCache`) が `(lane, epoch, sequence)` を追跡し、
  ディスクに high-water marks を保存、重複/古い replay を拒否。property/fuzz
  harness が fingerprint の差異や out-of-order submission をカバー。
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Residual gaps**
- Torii ingest は replay cache を admission に組み込み、sequence cursors を
  再起動を跨いで永続化する必要がある。
- Norito DA schemas に専用 fuzz harness (`fuzz/da_ingest_schema.rs`) を追加。
  coverage dashboard で退行があればアラートすべき。

### Replication withholding

**Scenario:** Byzantine storage operator が pin を受け入れるが chunks を落とし、
偽造や共謀で PDP/PoTR を通過する。

**Controls**
- PDP/PoTR challenge schedule を DA payloads に拡張し epoch 毎のカバレッジを確保。
- quorum 閾値を持つ multi-source replication。fetch orchestrator が欠落 shard を検出し repair を起動。
- failed proofs と missing replicas に連動した governance slashing。
- 自動 reconciliation job (`cargo xtask da-commitment-reconcile`) が ingest receipts と
  DA commitments (SignedBlockWire/`.norito`/JSON) を比較し、ガバナンス向け JSON evidence
  bundle を出力し、欠落/不一致で失敗して Alertmanager を発火。

**Residual gaps**
- `integration_tests/src/da/pdp_potr.rs` の simulation harness (tests: `integration_tests/tests/da/pdp_potr_simulation.rs`)
  が collusion/partition を演習し、PDP/PoTR schedule の決定論的検出を確認。DA-5 と
  並行して proof surface を拡張すること。
- cold-tier eviction ポリシーには covert drop 防止の署名付き監査ログが必要。

### Commitment tampering

**Scenario:** Compromised sequencer が DA commitments を省略/改ざんし、fetch failures
や light-client 不整合を発生させる。

**Controls**
- コンセンサスがブロック提案と DA submission キューをクロスチェックし、必要な commitments
  がない提案を拒否。
- light clients は fetch handle を出す前に inclusion proof を検証。
- submission receipts と block commitments の audit trail。
- 自動 reconciliation job (`cargo xtask da-commitment-reconcile`) が ingest receipts と
  commitments を比較し、ガバナンス向け JSON evidence bundle を出力、欠落/不一致で失敗し
  Alertmanager を発火。

**Residual gaps**
- reconciliation job + Alertmanager hook でカバー済み。ガバナンスパケットは JSON evidence を既定で取り込む。

### Network partition and censorship

**Scenario:** adversary が replication network を partition し、割り当て chunks の取得や
PDP/PoTR 応答を阻害する。

**Controls**
- multi-region provider 要件でネットワークパスを多様化。
- challenge window に jitter を導入し、out-of-band repair へフォールバック。
- observability dashboards で replication depth、challenge success、fetch latency を監視し
  アラート閾値を設定。

**Residual gaps**
- Taikai live events の partition simulations が未整備。soak tests が必要。
- repair bandwidth reservation ポリシーが未整備。

### Insider abuse

**Scenario:** registry アクセス権を持つ operator が retention ポリシーを改ざんし、
悪意ある provider を許可したり alerts を抑制する。

**Controls**
- governance actions に multi-party signatures と Norito-notarised records を要求。
- policy changes は monitoring/archival logs にイベントを出力。
- observability pipeline は hash chaining 付き Norito append-only logs を強制。
- 四半期アクセスレビュー自動化 (`cargo xtask da-privilege-audit`) が manifest/replay
  ディレクトリ (＋ operator が指定するパス) を走査し、欠落/非ディレクトリ/world-writable
  を検出して署名付き JSON bundle を出力。

**Residual gaps**
- dashboard の tamper-evidence には署名済み snapshots が必要。

## 残余リスク登録

| Risk | Likelihood | Impact | Owner | Mitigation Plan |
| --- | --- | --- | --- | --- |
| DA-2 sequence cache 前の DA manifest replay | Possible | Moderate | Core Protocol WG | DA-2 で sequence cache + nonce validation を実装し、回帰テストを追加。 |
| >f ノード侵害時の PDP/PoTR collusion | Unlikely | High | Storage Team | cross-provider sampling を含む新 challenge schedule を導出し、simulation harness で検証。 |
| cold-tier eviction の audit gap | Possible | High | SRE / Storage Team | eviction 用に署名ログと on-chain receipts を追加し、ダッシュボード監視。 |
| sequencer omission detection latency | Possible | High | Core Protocol WG | 夜間 `cargo xtask da-commitment-reconcile` で receipts と commitments (SignedBlockWire/`.norito`/JSON) を比較し、欠落/不一致でガバナンスへページング。 |
| Taikai live streams の partition resilience | Possible | Critical | Networking TL | partition drills 実施、repair 帯域予約、failover SOP を文書化。 |
| governance privilege drift | Unlikely | High | Governance Council | 四半期 `cargo xtask da-privilege-audit` (manifest/replay dirs + extra paths) を署名 JSON + dashboard gate で実施し、監査 artefacts を on-chain で固定。 |

## Required Follow-Ups

1. DA ingest Norito schemas と example vectors を公開 (DA-2 に持ち込む)。
2. Torii DA ingest に replay cache を通し、sequence cursors を再起動間で永続化。
3. **Completed (2026-02-05):** PDP/PoTR simulation harness が collusion + partition
   を QoS backlog model と共に実行。`integration_tests/src/da/pdp_potr.rs` と
   `integration_tests/tests/da/pdp_potr_simulation.rs` を参照。
4. **Completed (2026-05-29):** `cargo xtask da-commitment-reconcile` が ingest receipts
   と DA commitments (SignedBlockWire/`.norito`/JSON) を比較し、
   `artifacts/da/commitment_reconciliation.json` を出力。Alertmanager/ガバナンスパケット
   に接続済み (`xtask/src/da.rs`)。
5. **Completed (2026-05-29):** `cargo xtask da-privilege-audit` が manifest/replay
   spool (＋ operator 指定パス) を走査し、欠落/非ディレクトリ/world-writable を検出して
   署名 JSON bundle (`artifacts/da/privilege_audit.json`) を生成し、アクセスレビューの
   自動化ギャップを解消。

**次に見る場所:**

- DA-2 に replay cache と cursor 永続化が入った。`crates/iroha_core/src/da/replay_cache.rs`
  (cache ロジック) と `crates/iroha_torii/src/da/ingest.rs` (Torii 統合) を参照。
- PDP/PoTR streaming simulations は proof-stream harness
  `crates/sorafs_car/tests/sorafs_cli.rs` で実施し、PoR/PDP/PoTR の request flows と
  threat model に記載の failure scenarios をカバー。
- Capacity/repair soak の結果は `docs/source/sorafs/reports/sf2c_capacity_soak.md`、
  Sumeragi の広域 soak matrix は `docs/source/sumeragi_soak_matrix.md` (翻訳版含む)。
- Reconciliation + privilege-audit automation は `docs/automation/da/README.md` と
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit` にあり、
  `artifacts/da/` の既定出力をガバナンスパケットに添付する。

## Simulation Evidence & QoS Modelling (2026-02)

DA-1 follow-up #3 を閉じるため、`integration_tests/src/da/pdp_potr.rs`
( `integration_tests/tests/da/pdp_potr_simulation.rs` でカバー ) に
決定論的 PDP/PoTR シミュレーション harness を実装。3 地域にノードを
割り当て、roadmap の確率に従って partition/collusion を注入し、
PoTR lateness を追跡し、hot tier の repair budget を模した backlog model
に流し込む。デフォルトシナリオ (12 epochs, 18 PDP challenges + 2 PoTR windows/epoch)
の結果は以下の通り。

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metric | Value | Notes |
| --- | --- | --- |
| PDP failures detected | 48 / 49 (98.0%) | Partitions でも検出が発火し、未検出 1 件は正当な jitter に起因。 |
| PDP mean detection latency | 0.0 epochs | 失敗は発生 epoch 内で可視化。 |
| PoTR failures detected | 28 / 77 (36.4%) | ノードが >=2 PoTR windows を逃すと検出。大半は残余リスク登録へ。 |
| PoTR mean detection latency | 2.0 epochs | アーカイブエスカレーションに組み込まれた 2-epoch 遅延閾値と一致。 |
| Repair queue peak | 38 manifests | partitions が 1 epoch あたり 4 repairs を上回ると backlog が急増。 |
| Response latency p95 | 30,068 ms | 30 s challenge window と QoS 用 +/-75 ms jitter を反映。 |
<!-- END_DA_SIM_TABLE -->

これらの出力は DA ダッシュボード試作を駆動し、roadmap に記載の
"simulation harness + QoS modelling" 受け入れ基準を満たす。

自動化は
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
の背後にあり、共有 harness を呼び出して Norito JSON を
`artifacts/da/threat_model_report.json` に出力する。夜間ジョブはこの
ファイルを消費して本書のマトリクスを更新し、検出率/repair queue/QoS
サンプルのドリフトを検知してアラートする。

ドキュメント用に上表を更新するには `make docs-da-threat-model` を実行。
`cargo xtask da-threat-model-report` を呼び出し、
`docs/source/da/_generated/threat_model_report.json` を再生成し、
`scripts/docs/render_da_threat_model_tables.py` で本セクションを再書き込み。
`docs/portal` のミラー (`docs/portal/docs/da/threat-model.md`) も同時に更新され、
両方のコピーが同期する。
