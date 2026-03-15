---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
title: SoraFS Pin レジストリ
Sidebar_label: Pin レジストリ
説明: SF-4 の定義 レジストリ ステート マシン Torii ファサード ツール 可観測性 ہے۔
---

:::note メモ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS ピン レジストリ نفاذی منصوبہ (SF-4)

SF-4 ピン レジストリの確認と確認の確認と約束の明示の確認
ポリシーのピン留め Torii ゲートウェイ オーケストレーター API の API の固定
検証計画 実装タスク オンチェーン ロジック
ホスト側のサービスと備品の詳細

## और देखें

1. **レジストリ ステート マシン**: Norito で定義されたレコード、マニフェスト、エイリアス、後継チェーン
   保持エポックとガバナンス メタデータ。
2. *******: ピンのライフサイクル*** 決定論的な CRUD 操作 (`ReplicationOrder`、
   `Precommit`、`Completion`、エビクション)。
3. **ファサード**: gRPC/REST エンドポイントとレジストリ、Torii、SDK、および SDK
   ページネーション 認証 شامل ہے۔
4. **ツールとフィクスチャ**: CLI ヘルパー、テスト ベクトル、ドキュメント、マニフェスト、エイリアス
   ガバナンス封筒 ہم آہنگ رہیں۔
5. **テレメトリ操作**: レジストリ、メトリクス、アラート、ランブック。

## और देखें

### بنیادی ریکارڈز (Norito)

|構造体 | | और देखしているの
|----------|----------|----------|
| `PinRecordV1` |正規のマニフェスト エントリ。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` |エイリアス -> マニフェスト CID マッピング。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーはマニフェスト ピンを作成します。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーの承認。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |ガバナンス ポリシーのスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

実装リファレンス: `crates/sorafs_manifest/src/pin_registry.rs` دیکھیں جہاں Rust Norito スキーマ
検証ヘルパー موجود ہیں۔検証マニフェスト ツール (チャンカー レジストリ ルックアップ、ピン ポリシー ゲーティング)
Torii ファサード、CLI、不変条件、不変条件

タスク:
- `crates/sorafs_manifest/src/pin_registry.rs` スキーマ Norito スキーマ
- Norito マクロとコード生成 (Rust + SDK)
- スキーマのドキュメント (`sorafs_architecture_rfc.md`) のドキュメント (`sorafs_architecture_rfc.md`) のドキュメント

## ऩुल॰

|ああ | और देखेंにゅう |
|-----|-------------|------|
|レジストリ ストレージ (スレッド/sqlite/オフチェーン) スマート コントラクト モジュール|コアインフラ/スマートコントラクトチーム |決定論的ハッシュ 浮動小数点 浮動小数点|
|エントリ ポイント: `submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |コアインフラ |検証計画 `ManifestValidator` ستعمال کریں۔エイリアス バインディング `RegisterPinManifest` (Torii DTO) سے گزرتی ہے جبکہ مخصوص `bind_alias` آئندہ اپڈیٹس کے और देखें |
|状態遷移: 継承 (マニフェスト A -> B)、保持エポック、エイリアスの一意性|ガバナンス評議会 / コアインフラ |エイリアスの一意性、保持制限、前任者の承認/廃止チェック、`crates/iroha_core/src/smartcontracts/isi/sorafs.rs` などマルチホップ連続検出 レプリケーション ブックキーピング|
|管理対象パラメーター: `ManifestPolicyV1` 構成/管理状態の管理ガバナンスイベント ذریعے اپڈیٹس کی اجازت دیں۔ |ガバナンス評議会 |ポリシーの更新情報 CLI فراہم کریں۔ |
|イベント送信: テレメトリ Norito イベント (`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`) |可観測性 |イベント スキーマ + ログ記録|

テスト:
- エントリ ポイントの単体テスト (肯定 + 拒否)。
- 継承チェーンの特性テスト (サイクルなし、単調エポックなし)。
- ランダムなマニフェスト (制限付き)、ファズの検証。

## ファサード (Torii/SDK انضمام)|ああ |ああ | और देखें
|------|-----|---------------|
| Torii サービス | `/v2/sorafs/pin` (送信) `/v2/sorafs/pin/{cid}` (検索) `/v2/sorafs/aliases` (リスト/バインド) `/v2/sorafs/replication` (注文/受領)ページネーション + フィルタリング|ネットワーキング TL / コア インフラ |
|証明書 |応答 レジストリの高さ/ハッシュ شامل کریں؛ Norito 構成証明構造体 SDK は、次のものを消費します。 |コアインフラ |
| CLI | `sorafs_manifest_stub` インターフェイス `sorafs_pin` CLI インターフェイス `pin submit`、`alias bind`、`order issue`、 `registry export` ہو۔ |ツーリングWG |
| SDK | Norito スキーマとクライアント バインディング (Rust/Go/TS) が生成する統合テストのテスト| SDK チーム |

操作:
- エンドポイントの取得 (キャッシュ/ETag レイヤー)
- Torii ポリシー レート制限 / 認証

## 試合日程 CI

- フィクスチャ ディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` 署名済みマニフェスト/エイリアス/注文スナップショット محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے 再生成 ہوتے ہیں۔
- CI ステップ: `ci/check_sorafs_fixtures.sh` スナップショットの再生成、差分、失敗、CI フィクスチャの位置合わせ
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) ハッピー パス、重複エイリアスの拒否、エイリアス承認/リテンション ガード、チャンカー ハンドルの不一致、レプリカ数の検証、継承ガードの失敗 (不明/事前承認/廃止/自己ポインタ) `register_manifest_rejects_*` ケース دیکھیں۔
- 単体テスト、`crates/iroha_core/src/smartcontracts/isi/sorafs.rs` 別名検証、リテンション ガード、後続チェックマルチホップ連続検出 ステート マシン ステート マシン
- 可観測性パイプラインのゴールデン JSON イベント

## テレメトリと可観測性

メトリック (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー テレメトリ (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) エンドツーエンド ダッシュボード スコープ スコープ

ログ:
- ガバナンス監査構造化 Norito イベント ストリーム (署名済み?)。

アラート:
- SLA は保留中の複製命令です。
- エイリアスの有効期限のしきい値。
- 保持違反 (マニフェスト更新 پہلے نہ ہو)。

ダッシュボード:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` マニフェスト ライフサイクルの合計、エイリアス カバレッジ、バックログの飽和度、SLA 比率、レイテンシとスラック オーバーレイ、欠品率、オンコール レビュー、データ

## ランブックとドキュメント

- `docs/source/sorafs/migration_ledger.md` レジストリ ステータスの更新情報
- オペレーター ガイド: `docs/source/sorafs/runbooks/pin_registry_ops.md` (分析) メトリクス、アラート、導入、バックアップ、回復フロー
- ガバナンス ガイド: 政策パラメーター、承認ワークフロー、紛争処理、その他
- API リファレンス ページ (Docusaurus ドキュメント)。

## 依存関係とシーケンス

1. 検証計画タスク (ManifestValidator 統合)。
2. Norito スキーマ + ポリシーのデフォルト
3. 契約 + サービス テレメトリ ワイヤー
4. フィクスチャは統合スイートを再生成します
5. ドキュメント/Runbook ロードマップ項目の説明

SF-4 チェックリスト آئٹم میں پیش رفت پر اس منصوبے کا حوالہ ہونا چاہیے۔
REST ファサードの認証済みエンドポイントのリスト:

- `GET /v2/sorafs/pin` は `GET /v2/sorafs/pin/{digest}` をマニフェストします。
  エイリアス バインディング、レプリケーション順序、ブロック ハッシュ、認証オブジェクト、認証オブジェクト
- `GET /v2/sorafs/aliases` `GET /v2/sorafs/replication` エイリアス カタログ
  レプリケーション順序バックログ 一貫性のあるページネーション ステータス フィルター

CLI は、ラップを呼び出します (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) 演算子 API レジストリ監査 レジストリ監査