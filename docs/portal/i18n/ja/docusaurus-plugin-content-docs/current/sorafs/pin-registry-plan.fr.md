---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: SoraFS のピン レジストリの実装計画
サイドバー_ラベル: プラン デュ パン レジストリ
説明: SF-4 の実装計画、レジストリのマシン設計、ファサード Torii、ツールおよび観察可能性の確認。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/pin_registry_plan.md` を参照します。 Gardez les deux は、アクティブなドキュメントの保存と同期をコピーします。
:::

# SoraFS (SF-4) のピン レジストリの実装計画

SF-4 の制御ピン レジストリと在庫管理サービス
マニフェストの約束、固定および公開の政治的なアップリケント
API à Torii、補助ゲートウェイおよび補助オーケストレーター。計画を立てて文書を作成する
具体的な実装に関する検証、論理的な検証
オンチェーン、ホテルのサービス、設備および運営の管理。

## ポルテ

1. **レジストリのマシンデータ** : マニフェスト、エイリアスを入力する登録 Norito
   継承の連鎖、維持の時代、そして統治の時代。
2. **Implementation du contrat** : CRUD 決定のサイクル ド ヴィーの操作
   ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **サービスの外観** : Torii のレジストリ コンソメをサポートするエンドポイント gRPC/REST
   SDK やページネーション、認証など。
4. **ツールとフィクスチャ** : ヘルパー CLI、テストおよびドキュメントの作成ツール
   ガーダーマニフェスト、エイリアス、および統治同期のエンベロープ。
5. **Télémétrie et ops** : レジストリのメトリクス、アラート、およびランブック。

## ドネのモデル

### プリンシポー登録 (Norito)

|構造体 |説明 |チャンピオン |
|----------|---------------|----------|
| `PinRecordV1` |マニフェストの基準。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` | Mappe エイリアス -> CID のマニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーのマニフェストの指示。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーの受信者に対する告発。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |統治の政治のスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

実装の参照: `crates/sorafs_manifest/src/pin_registry.rs` を実行する
スキーマ Norito en Rust および les helpers de validation qui soutiennent ces enregistrements。
ツール マニフェストの検証を反映 (チャンカー レジストリのルックアップ、ピン ポリシー ゲーティング)
コントラット、ファサード Torii および CLI の不変要素の同一性を確認します。

タシュ :
- スキーマ Norito と `crates/sorafs_manifest/src/pin_registry.rs` のファイナライザー。
- マクロ Norito を介した一般的なコード (Rust + オートレ SDK)。
- 日々のドキュメントの管理 (`sorafs_architecture_rfc.md`) スキーマを適切に管理します。

## コントラットの実装

|ターシュ |所有者 |メモ |
|------|----------|------|
|レジストリの在庫 (スレッド/SQLite/オフチェーン) とスマート コントラクトのモジュールを実装します。 |コアインフラ/スマートコントラクトチーム |ハッシュ決定を行う必要はなく、フロッタントを使用することもできます。 |
|エントリ ポイント: `submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |コアインフラ |計画の検証を行う `ManifestValidator` を作成します。 Le binding d'alias passe maintenant par `RegisterPinManifest` (DTO Torii exposé) Tandis que `bind_alias` dédiéreste prévu pour des misses à jour successives。 |
|移行: 継承 (マニフェスト A -> B)、保持期間の強制、別名の統一。 |ガバナンス評議会 / コアインフラ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` の別名、保持およびチェックの制限および承認/撤回の警告。相続マルチホップの検出と複製の簿記管理。 |
|パラメータ: 充電器 `ManifestPolicyV1` 設定/管理者設定。 permettre les misses à jour via événements de gouvernance。 |ガバナンス評議会 |政治の日々を過ごすために CLI を使いましょう。 |
| Émission d'événements : émettre des événements Norito pour la télémétrie (`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`)。 |可観測性 |スキーマの定義 + ロギング。 |テスト:
- ユニテアがチャックのエントリーポイントを注ぐテスト (ポジティフ + リジェット)。
- 継承のテスト・デ・プロプリエテス（パ・ド・サイクル、エポック・モノトーン）。
- マニフェスト aléatoires (bornes) の一般的な検証のファズ。

## ファサード サービス (統合 Torii/SDK)

|構成材 |ターシュ |所有者 |
|----------|------|----------|
|サービス Torii | Exposer `/v1/sorafs/pin` (送信)、`/v1/sorafs/pin/{cid}` (検索)、`/v1/sorafs/aliases` (リスト/バインド)、`/v1/sorafs/replication` (注文/受領)。 Fournir ページネーション + フィルター。 |ネットワーキング TL / コア インフラ |
|証明書 |対応するレジストリのオート/ハッシュを含めます。 SDK の構造証明書 Norito コンソメ。 |コアインフラ |
| CLI | Étendre `sorafs_manifest_stub` ou une nouvelle CLI `sorafs_pin` avec `pin submit`、`alias bind`、`order issue`、`registry export`。 |ツーリングWG |
| SDK |バインディング クライアント (Rust/Go/TS) の一般的なスキーマ Norito ;統合のテストを行います。 | SDK チーム |

操作:
- キャッシュ/ETag を使用してエンドポイントを取得します。
- Fournir レート制限 / 認証一貫性 avec les politiques Torii。

## 備品と CI

- 備品のドシエ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` マニフェスト/エイリアス/注文登録の署名付きスナップショット (`cargo run -p iroha_core --example gen_pin_snapshot` 経由) をストックします。
- CI の作成: `ci/check_sorafs_fixtures.sh` スナップショットと相違点の確認、CI の調整結果の確認。
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) ハッピー パスと重複の拒否、承認/保持の保護、一致しないチャンカーの処理、レプリカの検証と継承の検証 (ポイントツール) inconnus/pre-approuvés/retirés/auto-reférences) ; voir les cas `register_manifest_rejects_*` クーベルチュールの詳細を注ぎます。
- `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` の検証単位のテスト、保持および成功のチェック。後継マルチホップの検出はマシンのテストに参加します。
- JSON は、パイプラインの監視を可能にするゴールデン ユーティリティを提供します。

## 遠隔測定と観察可能性

メトリック (Prometheus) :
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- 既存のプロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) は、ダッシュボードのスコープをエンドツーエンドで保持します。

ログ:
- Flux d'événements Norito structurés pour les Audits de gouvernance (signés ?)。

警告:
- SLA を無視して複製を行う命令。
- 有効期限 d'alias < seuil。
- 保持違反 (明示的な非更新期限切れ)。

ダッシュボード:
- JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` は、マニフェストのサイクル全体、バックログの飽和度、SLA 比率、レイテンスとスラックのオーバーレイ、およびオンコールのレビューを確認するためのテストに適しています。

## ランブックとドキュメント

- 法定登録簿 `docs/source/sorafs/migration_ledger.md` を含む。
- ガイド操作: `docs/source/sorafs/runbooks/pin_registry_ops.md` (公開情報) の特徴、警告、展開、革新性と再現性。
- 統治ガイド: 政治パラメータの決定、承認のワークフロー、訴訟の提起。
- Chaque エンドポイントの API 参照ページ (ドキュメント Docusaurus)。

## 依存性と順序付け

1. 計画の検証を終了する (ManifestValidator の統合)。
2. スキーマ Norito + 政治的デフォルトのファイナライザ。
3. コントラット + サービスの実装、テレメトリの分岐。
4. 備品を再作成し、統合を実行します。
5. 完了したロードマップの項目を、時間ごとのドキュメント/ランブックとマーケールに記載します。

チェック項目のチェックリスト SF-4 は、計画を参照し、進捗状況を登録します。
REST のエンドポイントの一覧表示の概要:

- `GET /v1/sorafs/pin` および `GET /v1/sorafs/pin/{digest}` のマニフェストの平均値
  バインディングの別名、複製の順序、およびハッシュの証明書を取得するオブジェクト
  デュ・デルニエ・ブロック。
- `GET /v1/sorafs/aliases` および `GET /v1/sorafs/replication` 公開ファイル カタログ
  エイリアス アクションとバックログの順序で複製を実行し、一貫したページ番号を付けます
  法律のフィルタリング。

La CLI カプセル化の申請 (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) 監査を自動化する操作を許可する
レジストリにはタッチャー補助 API がありません。