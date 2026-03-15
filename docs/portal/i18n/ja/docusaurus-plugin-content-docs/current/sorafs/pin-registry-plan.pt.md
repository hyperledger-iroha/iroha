---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: SoraFS によるピン レジストリの実装計画
サイドバーラベル: Plano do Pin レジストリ
説明: SF-4 の実装計画、レジストリのマキナ、ファチャダ Torii、ツールの監視。
---

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/sorafs/pin_registry_plan.md`。マンテンハは、記録を記録するための記録として、永続的な活動を記録します。
:::

# SoraFS (SF-4) を使用してピン レジストリを実装する計画

O SF-4 entrega o contrato do Pin Registry e os servicos de apoio que armazenam
Torii に関する API のマニフェストでの妥協、マニフェストでの政治的不法行為、
ゲートウェイとオルケストラドール。 Este documento amplia o plano de validacao com
具体的な実装方法、オンチェーンでのロジックのコブリンド、OS サービスの実行
ホスト、OS フィクスチャ、OS の操作が必要です。

## エスコポ

1. **Maquina de estados do registry**: Norito パラ マニフェストのレジストリ定義、
   エイリアス、cadias sucessoras、epocas de retencao および metadados de Governmenta。
2. **コントラートの実装**: 自動 CRUD の決定性を実現するためのオペラ
   dos ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **サービスの機能**: エンドポイント gRPC/REST sustentados pelo registry que Torii
   os SDK のコンソメムには、paginacao や atestacao が含まれます。
4. **ツールとフィクスチャ**: CLI のヘルパー、テストおよびドキュメントのパラメータの管理
   マニフェスト、エイリアス、エンベロープ デ ガバナンカ シンクロニザドス。
5. **Telemetria e ops**: レジストリに関するメトリクス、アラート、ランブック。

## モデロ デ ダドス

### レジストロス セントライス (Norito)

|構造体 |説明 |カンポス |
|----------|----------|----------|
| `PinRecordV1` |マニフェストの登録。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` | Mapeia エイリアス -> CID のマニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーの修正プログラムまたはマニフェストの説明。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーを確認してください。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |政府の政治のスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

実装に関するリファレンス: veja `crates/sorafs_manifest/src/pin_registry.rs` para OS
esquemas Norito em Rust eos ヘルパーは、レジストリをサポートします。あ
マニフェストのツールの検証 (チャンカー レジストリの検索、ポリシー ゲートの固定)
Torii は、CLI と同様に、同一性を維持します。

タレファス:
- Norito または `crates/sorafs_manifest/src/pin_registry.rs` を終了します。
- Gerar codigo (Rust + outros SDK) マクロ Norito。
- ドキュメントを更新 (`sorafs_architecture_rfc.md`) することで、すぐに安全性を確認できます。

## コントラートを実装する

|タレファ |レスポンス |メモ |
|------|------|------|
|スマート コントラクトをモジュールとしてレジストリ (スレッド/SQLite/オフチェーン) を実装します。 |コアインフラ/スマートコントラクトチーム | Fornecer ハッシュ決定論、evitar ponto flutuante。 |
|エントリ ポイント: `submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |コアインフラ | Aprobeitar `ManifestValidator` は検証を計画します。 `RegisterPinManifest` (DTO は Torii を実行します) を介して別名アゴラ フルイを結合します。`bind_alias` は成功を収めるのに役立ちます。 |
| Transicoes de estado: imor sucessao (マニフェスト A -> B)、epocas de retencao、unicidade de alias。 |ガバナンス評議会 / コアインフラ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` の別名、保持の制限、および承認/再任のチェックの制限。マルチホップの管理と永続的な簿記の管理を行います。 |
|ガバナドパラメータ: carregar `ManifestPolicyV1` de config/estado de Governmenta;イベント・デ・ガバナンカ経由で許可が得られます。 |ガバナンス評議会 | Fornecer CLI パラアチュアリザコ・デ・ポリティカ。 |
|イベントの送信: テレメトリア用イベント Norito (`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`)。 |可観測性 |イベントの定義とログ記録。 |精巣:
- Testes unitarios para cada エントリーポイント (positivo + rejeicao)。
- Testes de propriedades para a cadeia de sucessao (sem ciclos, epocas monotonicamente crescentes)。
- Fuzz de validacao gerando は、aleatorios (limitados) を示します。

## ファチャダ デ サービス (Integracao Torii/SDK)

|コンポネ |タレファ |レスポンス |
|-----------|----------|------|
|サービコ Torii | `/v1/sorafs/pin` (送信)、`/v1/sorafs/pin/{cid}` (検索)、`/v1/sorafs/aliases` (リスト/バインド)、`/v1/sorafs/replication` (注文/受領書) をエクスポートします。フォルネサー パギナカオ + フィルトラジェム。 |ネットワーキング TL / コア インフラ |
|アテスタカオ |レジストリに altura/hash を含めます。 Norito の SDK を追加してください。 |コアインフラ |
| CLI | Estender `sorafs_manifest_stub` または新しい CLI `sorafs_pin` com `pin submit`、`alias bind`、`order issue`、`registry export`。 |ツーリングWG |
| SDK |クライアントのバインディング (Rust/Go/TS) の一部は Norito;アディショナー精巣デインテグラカオ。 | SDK チーム |

オペラコ:
- エンドポイントの GET に関するキャッシュ/ETag の追加機能。
- Fornecer レート制限 / 認証は、政治家が行う Torii と同様に com と一致します。

## フィクスチャと CI

- フィクスチャのディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` マニフェスト/エイリアス/オーダーの再生成のスナップショット `cargo run -p iroha_core --example gen_pin_snapshot`。
- CI の Etapa: `ci/check_sorafs_fixtures.sh` スナップショットとファルハセ フーバー差分、CI アリニャドスのマンテンド OS フィクスチャを再生成します。
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) は、別名重複を再確認し、別名を保護/別名を保持し、チャンカーの互換性のないハンドルを処理し、レプリカの感染を検証し、安全性を確認します (ポンテイロス)。デスコンヘシドス/プレアプロバドス/レティラドス/オートリファレンス); veja casos `register_manifest_rejects_*` パラ デタルヘス デ コベルトゥーラ。
- 別名、保護者が後継者をチェックするためのテスト ユニタリオス アゴラ コブレム バリダカオ `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。マルチホップを確実に実行し、マキナ デ スタドス チェガールを実行します。
- JSON ゴールデンパライベント米国ペロスパイプラインの監視。

## テレメトリアと観察

メトリカ (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) は、ダッシュボードをエンドツーエンドで永続的に管理します。

ログ:
- イベント Norito のストリーム パラ オーディトリアス デ ガバナンカ (アッシナドス?)。

アラート:
- SLA を超えた複製の順序。
- 別名 abaixo do limiar を有効にしてください。
- Violacoes de retencao (マニフェスト nao renovado antes de expirar)。

ダッシュボード:
- O JSON は、Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totais の ciclo de vida dos マニフェスト、cobertura de alias、saturacao do backlog、razao de SLA、overlays de latencia とlack、on-call での定期的な見直しを行います。

## ランブックとドキュメント

- Atualizar `docs/source/sorafs/migration_ledger.md` には、ステータスのレジストリが含まれています。
- 操作方法: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publicado) コブリンド メトリクス、アラート、デプロイ、バックアップ、および回復方法。
- 政府の政策: 政治パラメタ、承認ワークフロー、紛争処理。
- API のエンドポイントに関する参照ページ (ドキュメント Docusaurus)。

## 依存性とシーケンス

1. 完全なデータ検証計画 (ManifestValidator の統合)。
2. 最終的なエスケマ Norito + 政治的デフォルト。
3. contrato + servico、conectar テレメトリアを実装します。
4. 再生設備、統合されたロッドスイート。
5. Atualizar のドキュメント/Runbook とロードマップを完全に作成します。

CADA チェックリストは、SF-4 を参照して、エステ プランノ クアンド フーバー プログレスを実行します。
com atestacao でエンドポイントのリストを作成する REST の詳細:

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` retornam マニフェスト com
  バインディングのエイリアス、オブジェクトの複製、デリバドの実行
  ハッシュ・ドゥ・ウルティモ・ブロコ。
- `GET /v1/sorafs/aliases` および `GET /v1/sorafs/replication` カタログの説明
  別名、バックログ、レプリカのコンパニオンの順序、一貫性
  ステータスのフィルター。

CLI カプセル化 essas Chamadas (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) パラ・ケ・オペラドール・ポッサム・オートマティザー・オーディトリアス・ドゥ
レジストリとバイショニベルの API を組み合わせます。