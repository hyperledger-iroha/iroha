---
lang: ja
direction: ltr
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

# Nexus レーン・コンプライアンスとホワイトリスト・ポリシーエンジン (NX-12)

ステータス: 🈴 実装済み — 本ドキュメントは稼働中のポリシーモデルとコンセンサス・クリティカルな適用を記述する。
ロードマップ項目 **NX-12 — レーン・コンプライアンスとホワイトリスト・ポリシーエンジン** に紐付く。
`crates/iroha_core/src/compliance` に実装され、Torii admission と `iroha_core` トランザクション検証の双方で適用される
データモデル、ガバナンスフロー、テレメトリ、ロールアウト戦略を説明し、
各 lane と dataspace を決定論的な管轄ポリシーに結び付ける。

## 目的

- ガバナンスが各 lane manifest に allow/deny ルール、管轄フラグ、CBDC 送金上限、監査要件を付与できるようにする。
- Torii admission とブロック実行で全トランザクションを評価し、ノード間で決定論的なポリシー強制を保証する。
- Norito の証跡バンドルと規制当局/オペレーター向けに検索可能なテレメトリを備えた、暗号学的に検証可能な監査トレイルを生成する。
- モデルの柔軟性を維持する: 同一の policy engine が私的 CBDC lanes、公的 settlement DS、ハイブリッドなパートナー dataspaces を bespoke forks なしでカバーする。

## 対象外

- AML/KYC 手順や法的エスカレーションのワークフロー定義。これらは本ドキュメントが生成するテレメトリを消費する compliance playbooks に属する。
- IVM の命令単位トグルの導入。エンジンはどの accounts/assets/domains がトランザクションを送信または lane と相互作用できるかのみを制御する。
- Space Directory を廃止すること。マニフェストは DS メタデータの権威的ソースとして残り、コンプライアンス・ポリシーは Space Directory のエントリを参照して補完する。

## ポリシーモデル

### エンティティと識別子

ポリシーエンジンは以下を扱う:

- `LaneId` / `DataSpaceId` — ルールの適用範囲を識別する。
- `UniversalAccountId (UAID)` — cross-lane の ID をまとめる。
- `JurisdictionFlag` — 規制区分を列挙するビットマスク (例: `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` — 影響対象を記述する:
  - `AccountId`, `DomainId`, または `UAID`。
  - プレフィクスベースのセレクタ (`DomainPrefix`, `UaidPrefix`) によるレジストリ一致。
  - Space Directory のマニフェスト向け `CapabilityTag` (例: FX-cleared の DS のみ)。
  - `privacy_commitments_any_of` の gating により、ルール一致前に Nexus privacy commitments の宣言を必須化
    (NX-10 の manifest surface を反映し、`LanePrivacyRegistry` の snapshots で強制される)。

### LaneCompliancePolicy

ポリシーは Norito エンコードされた構造体としてガバナンスで公開される:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` は `ParticipantSelector`、任意の管轄 override、capability tags、理由コードを組み合わせる。
- `DenyRule` は allow 構造を反映するが、先に評価される (deny wins)。
- `TransferLimit` は資産/bucket 単位の上限を定義する:
  - `max_notional_xor` と `max_daily_notional_xor`。
  - `asset_limits[{asset_id, per_tx, per_day}]`。
  - `relationship_limits` (例: CBDC retail vs wholesale)。
- `AuditControls` は以下を設定する:
  - Torii が拒否を監査ログに必ず保存するかどうか。
  - 成功した判断を Norito digests にサンプリングするかどうか。
  - `LaneComplianceDecisionRecord` の保持期間。

### 保存と配布

- 最新の policy hashes は Space Directory の manifest に validator keys と並んで保存される。
  `LaneCompliancePolicyReference` (policy id + version + hash) が manifest フィールドとなり、
  validators と SDKs が正規の policy blob を取得できるようにする。
- `iroha_config` は `compliance.policy_cache_dir` を公開し、Norito payload と detached signature を保存する。
  ノードは更新適用前に署名検証を行い、改ざんを防ぐ。
- ポリシーは Torii が使用する Norito admission manifests にも埋め込み、CI/SDKs が
  validators に接続せずにポリシー評価を再現できるようにする。

## ガバナンスとライフサイクル

1. **提案** — ガバナンスが `ProposeLaneCompliancePolicy` を Norito payload、管轄の正当化、
   アクティベーション epoch とともに提出する。
2. **レビュー** — compliance reviewers が `LaneCompliancePolicyReviewEvidence` に署名する
   (監査可能で `governance::ReviewEvidenceStore` に保存される)。
3. **アクティベーション** — 遅延ウィンドウ後に validators が `ActivateLaneCompliancePolicy`
   を呼び出してポリシーを取り込む。Space Directory の manifest は新しい policy reference と
   原子的に更新される。
4. **改訂/撤回** — `AmendLaneCompliancePolicy` は差分メタデータを持ち、法的再現のために
   旧バージョンを保持する。`RevokeLaneCompliancePolicy` は policy id を `denied` に固定し、
   置き換えが有効化されるまで Torii が当該 lane のトラフィックを拒否する。

Torii が公開する API:

- `GET /v1/lane-compliance/policies/{lane_id}` — 最新の policy reference を取得。
- `POST /v1/lane-compliance/policies` — governance 専用 endpoint (ISI の proposal helpers を反映)。
- `GET /v1/lane-compliance/decisions` — `lane_id`, `decision`, `jurisdiction`, `reason_code`
  のフィルタを備えた監査ログのページング取得。

CLI/SDK コマンドはこれらの HTTP サーフェスをラップし、オペレーターがレビューを
自動化して成果物 (署名済み policy blob + reviewer attestations) を取得できるようにする。

## 執行パイプライン

1. **アドミッション (Torii)**
   - `Torii` は lane manifest の変更時、またはキャッシュ署名の期限切れ時にアクティブポリシーを取得する。
   - `/v1/pipeline` キューに入る各トランザクションは `LaneComplianceContext`
     (participant ids, UAID, dataspace manifest metadata, policy id, `crates/iroha_core/src/interlane/mod.rs`
     に記載された最新の `LanePrivacyRegistry` snapshot) とともにタグ付けされる。
   - UAID を持つ権限者は、ルーティング対象 dataspace の Space Directory manifest を有効化済みでなければならない。
     UAID が dataspace に紐付いていない場合、Torii はポリシールールを評価する前にトランザクションを拒否する。
   - `compliance::Engine` は `deny` ルール、次に `allow` ルールを評価し、最後に transfer limits を適用する。
     失敗したトランザクションは監査用に理由と policy id を含む型付きエラー
     (`ERR_LANE_COMPLIANCE_DENIED`) を返す。
   - アドミッションは高速なプリフィルタであり、コンセンサス検証が state snapshots を使って同じルールを再確認する。
2. **実行 (iroha_core)**
   - ブロック構築中に `iroha_core::tx::validate_transaction_internal` が
     `StateTransaction` snapshots (`lane_manifests`, `lane_privacy_registry`, `lane_compliance`) を用いて
     同じ lane governance/UAID/privacy/compliance の検証を再実行する。Torii のキャッシュが古くても
     コンセンサス・クリティカルな適用を維持する。
   - lane manifests やコンプライアンスポリシーを変更するトランザクションも同じ検証パスを通り、
     admission だけのバイパスは存在しない。
3. **非同期フック**
   - RBC gossip と DA fetchers は policy id をテレメトリに付与し、遅延判断を正しいルール版に紐付ける。
   - `iroha_cli` と SDK helpers は `LaneComplianceDecision::explain()` を公開し、
     自動化が人間可読の診断を生成できるようにする。

エンジンは決定論的かつ純粋であり、manifest/policy を取得した後に外部システムへアクセスしない。
これにより CI fixtures とマルチノード再現が容易になる。

## 監査とテレメトリ

- **メトリクス**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (activation delay 未満であるべき)。
- **ログ**
  - 構造化レコードは `policy_id`, `version`, `participant`, `UAID`, 管轄フラグ、および違反トランザクションの Norito hash を記録する。
  - `LaneComplianceDecisionRecord` は Norito でエンコードされ、`AuditControls` が耐久保存を要求した場合に
    `world.compliance_logs::<lane_id>::<ts>::<nonce>` 配下へ保存される。
- **Evidence bundles**
  - `cargo xtask nexus-lane-audit` は `--lane-compliance <path>` モードを追加し、ポリシー、
    レビュー署名、メトリクス snapshot、最新の監査ログを JSON + Parquet 出力に統合する。
    フラグは次の形式の JSON payload を期待する:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    CLI は各 `policy` blob が記録内の `lane_id` に一致することを埋め込み前に検証し、
    古い/不一致のエビデンスが規制パケットやロードマップのダッシュボードに混入するのを防ぐ。
  - `--markdown-out` (既定: `artifacts/nexus_lane_audit.md`) は、遅延している lanes、非ゼロ backlog、
    保留中 manifests、不足する compliance evidence を強調する可読サマリを出力する。
    これにより annex パケットは machine-readable な成果物と迅速なレビュー面の両方を含められる。

## ロールアウト計画

1. **P0 — 観測のみ**
   - ポリシー型、ストレージ、Torii endpoints、メトリクスを出荷する。
   - Torii は `audit` モード (enforcement なし) で評価し、データを収集する。
2. **P1 — Deny/allow enforcement**
   - deny ルールが発火した際に Torii と実行でハード失敗を有効化する。
   - 全 CBDC lanes にポリシーを要求する。public DS は audit モードを継続可能。
3. **P2 — 制限と管轄 override**
   - 送金上限と管轄フラグの enforcement を有効化する。
   - テレメトリを `dashboards/grafana/nexus_lanes.json` に投入する。
4. **P3 — 完全自動化**
   - 監査エクスポートを `SpaceDirectoryEvent` 消費者と統合する。
   - ポリシー更新をガバナンス runbooks と release automation に結び付ける。

## 受け入れとテスト

- `integration_tests/tests/nexus/compliance.rs` の統合テストは次をカバーする:
  - allow/deny の組み合わせ、管轄 override、transfer limits;
  - manifest/policy のアクティベーション競合; と
  - Torii と `iroha_core` の判断一致 (multi-node 実行)。
- `crates/iroha_core/src/compliance` の単体テストは、純粋な評価エンジン、キャッシュ無効化タイマー、メタデータ解析を検証する。
- Docs/SDK 更新 (Torii + CLI) は、ポリシー取得、ガバナンス提案の送信、エラーコード解釈、監査エビデンス収集を示す必要がある。

NX-12 のクローズには、上記成果物と `status.md`/`roadmap.md` の更新が必要であり、
ステージングクラスタで enforcement が有効になった段階で反映する。
