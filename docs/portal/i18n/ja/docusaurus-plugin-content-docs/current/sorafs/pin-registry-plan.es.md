---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: SoraFS のピン レジストリの実装計画
Sidebar_label: プラン デル ピン レジストリ
説明: SF-4 の実装計画は、レジストリの牧草地、ファチャダ Torii、ツールと監視を実行します。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/sorafs/pin_registry_plan.md`。満天のコピアス シンクロニザダス ミエントラス ラ ドキュメンタシオン ヘレダダ シガ アクティバ。
:::

# SoraFS (SF-4) のピン レジストリの実装を計画する

SF-4 entrega el contrato del Pin Registry y los servicios de soporte que almacenan
マニフェストの侵害、ピンの指数 API の不正侵入、Torii、ゲートウェイ
オルケスタドールよ。検証に関する計画に関する文書の作成
実装コンクリート、オンチェーンのロジック、ロス サービス、ロス
必要なオペラティボスの試合。

## アルカンス

1. **レジストリのマキナ**: Norito パラマニフェストの定義済みレジストリ、
   エイリアス、cadenas sucesoras、epocas de retencion y metadatos de gobernanza。
2. **コントラートの実装**: 自動制御の CRUD 決定操作
   ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **サービスの機能**: エンドポイント gRPC/REST レジストリ クエリの消費
   Torii には SDK がありません。ページやアテスタシオンも含まれます。
4. **ツールとフィクスチャ**: CLI のヘルパー、管理者のプルエバとドキュメントのベクトル
   マニフェスト、エイリアス、エンベロープ デ ゴベルナンザ シンクロニザドス。
5. **テレメトリと運用**: レジストリのメトリクス、アラートおよびランブック。

## モデロ デ ダトス

### 中央レジストロス (Norito)

|構造 |説明 |カンポス |
|-----------|---------------|----------|
| `PinRecordV1` |マニフェストの登録。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` | Mapea エイリアス -> CID のマニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーのマニフェストに関する指示。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーに対する非難。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |ゴベルナンザの政治のスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

実装のリファレンス: バージョン `crates/sorafs_manifest/src/pin_registry.rs` パラロス
esquemas Norito en Rust y los helpers de validacion que respaldan estos registro.ラ
マニフェストの検証ツールの検証 (チャンカー レジストリのルックアップ、ポリシー ゲーティングのピン留め)
パラ ケ エル コントラート、ラス ファチャダス Torii と CLI のコンパルタン不変条件の同一性。

タレアス:
- ロス エスケマス Norito と `crates/sorafs_manifest/src/pin_registry.rs` を終了します。
- 一般的な codigo (Rust + otros SDK) のマクロ Norito。
- ドキュメントの実際の内容 (`sorafs_architecture_rfc.md`) は、リストを確認する必要があります。

## コントラートの実装

|タレア |責任者 |メモ |
|------|--|------|
|レジストリ (スレッド/sqlite/オフチェーン) またはスマート コントラクトのモジュールを実装します。 |コアインフラ/スマートコントラクトチーム | Proveer ハッシュ決定者、evitar punto flotante。 |
|エントリ ポイント: `submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |コアインフラ | Aprovechar `ManifestValidator` 計画の検証。 `RegisterPinManifest` (Torii の DTO) を介して別名 ahora fluye をバインディングし、`bind_alias` を実行して実際の操作を実行します。 |
| Transiciones de estado: 不正な承継 (マニフェスト A -> B)、保持期間、別名による単一殺害。 |ガバナンス評議会 / コアインフラ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` での別名、不正行為/過去の生存期間の検証の制限。マルチホップの成功を検出し、安全な複製を管理します。 |
|パラメータ: cargar `ManifestPolicyV1` desde config/estado de gobernanza;イベントス・デ・ゴベルナンザ経由で実際の許可を取得します。 |ガバナンス評議会 | Proveer CLI パラ現実政治。 |
|イベントの送信: テレメトリに関するイベント Norito の送信 (`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`)。 |可観測性 |イベントの定義とログ記録。 |プルエバ:
- unitarios para cada エントリポイント (positivo + rechazo) をテストします。
- 成功のためのプロピエダのテスト (シン・シクロス、エポカス・モノトニカメント・クリエンテス)。
- Fuzz de validaciongenerando は aleatorios (acotados) を示します。

## サービスの機能 (Integracion Torii/SDK)

|コンポネ |タレア |責任者 |
|----------|----------|-----|
|サービスTorii |エクスポナー `/v2/sorafs/pin` (送信)、`/v2/sorafs/pin/{cid}` (検索)、`/v2/sorafs/aliases` (リスト/バインド)、`/v2/sorafs/replication` (注文/受領)。プローバー・パギナシオン+フィルトラード。 |ネットワーキング TL / コア インフラ |
|アテスタシオン | altura/hash del registry en respuestas を含めます。アテスタシオン Norito の SDK を構築します。 |コアインフラ |
| CLI |エクステンダー `sorafs_manifest_stub` または新しい CLI `sorafs_pin` と `pin submit`、`alias bind`、`order issue`、`registry export`。 |ツーリングWG |
| SDK |クライアントの一般的なバインディング (Rust/Go/TS) の設計 Norito。アグリガーは統合テストを行います。 | SDK チーム |

操作:
- エンドポイントの GET に関するキャッシュ/ETag の統合機能。
- Proveer のレート制限 / 認証は、Torii の政治と一致しています。

## フィクスチャと CI

- フィクスチャのディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` は、`cargo run -p iroha_core --example gen_pin_snapshot` のマニフェスト/エイリアス/順序再生成のスナップショット ファームを保護します。
- CI のパス: `ci/check_sorafs_fixtures.sh` スナップショットとフォールラ SI ヘイの差分、CI ラインのフィクスチャを管理します。
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) エイリアス重複の除去、不正行為/エイリアス保持のガード、チャンカーの塩漬け処理の処理、レプリカのコンテオ検証、および継承の失敗 (パンテロス)デスコノシドス/プレアプロバドス/レティラドス/オートリファレンス);バージョン casos `register_manifest_rejects_*` パラ詳細。
- `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` のエイリアス、ガード、保持、および後継者のチェックをテストします。マルチホップの成功を検出し、マキナ デ スタドスを検出します。
- JSON ゴールデンパライベント、監視パイプライン。

## テレメトリアと観察

メトリカ (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) がエンドツーエンドでダッシュボードに存在するテレメトリ。

ログ:
- イベント Norito のオーディオ構造 (firmados?) のストリーム。

アラート:
- SLA を超えて複製を行う手順。
- 別名有効期限。
- Violaciones de retencion (期限切れになるまでの期限を明示しません)。

ダッシュボード:
- Grafana `docs/source/grafana_sorafs_pin_registry.json` のマニフェストのラストレア合計、エイリアスのコベルチュラ、バックログの飽和率、SLA の比率、レイテンシアとスラックのオーバーレイ、オンコールでのリビジョンの調整。

## ランブックとドキュメント

- Actualizar `docs/source/sorafs/migration_ledger.md` には、レジストリの実際の情報が含まれています。
- オペラドールの操作: `docs/source/sorafs/runbooks/pin_registry_ops.md` (公開) メトリカス、アラート、デスリーグ、バックアップと回復のフルホス。
- 政治政策: 政治パラメタ、不正行為のワークフロー、論争の管理に関する記述。
- API のエンドポイントに関する参照ページ (Docusaurus ドキュメント)。

## 依存関係と優先順位

1. 検証計画の完全な領域 (ManifestValidator の統合)。
2. 最終的なエスケマ Norito + 政治的デフォルト。
3. コントラート + サービス、コネクター テレメトリアを実装します。
4. 再生フィクスチャ、統合スイートの修正。
5. ドキュメント/ランブックとロードマップのマーク項目を完全に把握します。

SF-4 のチェックリストは、エステ プランの進捗状況を参照します。
REST のアホラ エントレガ エンドポイントの一覧表示:

- `GET /v2/sorafs/pin` y `GET /v2/sorafs/pin/{digest}` デブエルベンマニフェストコン
  エイリアスのバインディング、レプリケーションの順序、およびオブジェクトの作成のデリバティブ
  ハッシュデルアルティモブロック。
- `GET /v2/sorafs/aliases` y `GET /v2/sorafs/replication` 指数関数カタログ
  エイリアス アクティビティ、バックログ、レプリケーションの順序、ページの一貫性
  フィルトロス・デ・エスタド。

La CLI envuelve estas llamadas (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) パラ ケ ロス オペラドール プエダン オートマティザール オーディトリアス デル
レジストリは、Bajo Nivel の API に登録されています。