---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードプラン
タイトル: ノードの実装計画 SoraFS
Sidebar_label: ノードの実装計画
説明: アルマセナミエント SF-3 の管理者は、適切なコンヒト、タレアス、およびコベルトゥーラ デ プルエバスを扱うことができます。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/sorafs_node_plan.md` のページを参照してください。スフィンクスの引退を報告するために、記録を保存する必要があります。
:::

SF-3 entrega el primer crate ejectable `sorafs-node` que convierte un proceso Iroha/Torii en un proveedor de almacenamiento SoraFS。米国エステ計画は、[政府の政策](node-storage.md)、[政治的管理](provider-admission-policy.md)、[法的な規制、市場の安全保障](node-storage.md)、およびラ[法的規制] almacenamiento](storage-capacity-marketplace.md) al secuenciar entregables。

## アルカンス オブジェティボ (人 M1)

1. **チャンクの統合。** Envuelve `sorafs_car::ChunkStore` は、チャンクのバックエンド永続的なガード バイトを制御し、データ設定ディレクトリの PoR マニフェストを表示します。
2. **ゲートウェイのエンドポイント。** ピン、チャンクのフェッチ、プロセス Torii の遠隔測定に関するエンドポイント HTTP Norito を公開します。
3. **構成のプロメリア** 構成 `SoraFsStorage` の構造の集合体 (フラグ ハビリタド、容量、ディレクトリ、合意の制限) `iroha_config`、`iroha_core` のケーブル接続`iroha_torii`。
4. **制限/計画** ディスコ/パラレリズムの制限を無視し、オペレーターやバックプレッシャーに対する要求を無視してください。
5. **テレメトリ** ピンの保存、チャンクのフェッチの遅延、PoR の計算結果の使用量のメトリクス/ログを出力します。

## デグローセ・デル・トラバホ

### A. モジュールの構造

|タレア |責任者 |メモ |
|------|---------------|------|
| `crates/sorafs_node` モジュールをクリアします: `config`、`store`、`gateway`、`scheduler`、`telemetry`。 |ストレージの装備 | Torii の統合に関する再利用可能なヒントを再エクスポートします。 |
| `StorageConfig` の実装 `SoraFsStorage` (usuario → real →defaults)。 |ストレージ設備・設定WG | Asegura que las capas Norito/`iroha_config` permanezcan deterministas。 |
| `NodeHandle` と Torii は、パラエンビア ピン/フェッチを使用します。 |ストレージの装備 |アルマセナミエントとプロメリアの非同期カプセル化。 |

### B. アルマセン デ チャンクの永続化

|タレア |責任者 |メモ |
|------|---------------|------|
|ディスコ環境でのバックエンドの構築 `sorafs_car::ChunkStore` は、ディスコでのマニフェストのインデックスを構築します (`sled`/`sqlite`)。 |ストレージの装備 |レイアウト決定者: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
| Mantener メタデータ PoR (64 KiB/4 KiB のファイル) `ChunkStore::sample_leaves` を使用。 |ストレージの装備 | Soporta は tras reinicios を再生します。腐敗前の急速な被害。 |
|最初から完全な再生を実装します (マニフェストの再ハッシュ、不完全なポダーピン)。 |ストレージの装備 | Torii のブロックはリプレイを完了しました。 |

### C. エンドポイントとゲートウェイ

|エンドポイント |コンポルタミエント |タレアス |
|----------|----------------|----------|
| `POST /sorafs/pin` | Acepta `PinProposalV1`、検証マニフェスト、エンコラ摂取、マニフェストに対する CID の応答。 |チャンク ストアを介して、チャンカーの有効性、インポーネ クオータ、ストリームデータを確認できます。 |
| `GET /sorafs/chunks/{cid}` + ランゴについて相談 |チャンクのヘッダー `Content-Chunker` のバイトを処理します。ランゴの特別な能力を調べます。 |米国スケジューラ + プレスプエストス デ ストリーム (ヴィンキュラー ア キャパシダ デ ランゴ SF-2d)。 |
| `POST /sorafs/por/sample` |プルエバスの開発バンドルを明示するための PoR の出力。 | Reusa el muestreo del chunk ストア、ペイロード Norito JSON に応答します。 |
| `GET /sorafs/telemetry` |再開: 容量、PoR のエクスポート、フェッチのエラーの内容。 |ダッシュボード/オペレーターのプロポーシオナデータ。 |

`sorafs_node::por` の実行時のアクセス許可: `PorChallengeV1`、`PorProofV1` および `AuditVerdictV1` の `CapacityMeter` のメトリクスに関するトラッカー レジストラの登録reflejen los veredictos de gobernanza sin lógica Torii Personalizada.【crates/sorafs_node/src/scheduler.rs#L147】

実装上の注意:

- ペイロード `norito::json` に対するスタック Axum de Torii。
- Agrega esquemas Norito para respuestas (`PinResultV1`、`FetchErrorV1`、estructuras de telemetria)。- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` 大量のバックログを説明し、期間中/制限期間中はタイムスタンプを失い、証明済みの期間中はタイムスタンプを取得し、`sorafs_node::NodeHandle::por_ingestion_status`、Torii を実行します。レジストラ ロス ゲージ `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` パラダッシュボード.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. スケジュール管理と管理の集中

|タレア |詳細 |
|------|----------|
|クオタ・デ・ディスコ |ディスコでのバイトのセギミエント。レチャザ ヌエボス ピン アル スーパー `max_capacity_bytes`。今後の政治における追放を証明します。 |
|フェッチの同意 |グローバル (`max_parallel_fetches`) は、SF-2d の制限を証明するために使用されます。 |
|コーラ・ド・ピン |ペンディエンテスの摂取制限。 Expone エンドポイント Norito の詳細な説明。 |
|カデンシア PoR | Worker en segundo plano impulsado por `por_sample_interval_secs`。 |

### E. テレメトリのログ記録

メトリカス (Prometheus):

- `sorafs_pin_success_total`、`sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result` に関するヒストグラム)
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

ログ/イベント:

- 遠隔測定 Norito 摂取のための構造 (`StorageTelemetryV1`)。
- 利用率が 90% を超えていることを警告します。

### F. プルエバス戦略

1. **Pruebas Unitarias.** チャンク ストアの永続化、計算、スケジューラの不変性 (バージョン `crates/sorafs_node/src/scheduler.rs`)。
2. **統合プルーバ** (`crates/sorafs_node/tests`)。ピン → 往復のフェッチ、回復、回復、検査、PoR の検証。
3. **Torii の統合を実行します。** `assert_cmd` 経由で Torii を有効にし、HTTP エンドポイントを取り出します。
4. **Hoja de ruta de caos.** Futuros は、ディスコ、IO レント、レティーロの練習をシミュレートします。

## 依存関係

- SF-2b の承認政治 — 発表前に承認の封筒を確認する必要があります。
- 容量 SF-2c のマーケットプレイス — 容量の宣言を行うブエルタの遠隔測定。
- 広告 ​​SF-2d の拡張機能 — ランゴの容量と管理費のストリームを消費します。

## サリダデルヒトの基準

- `cargo run -p sorafs_node --example pin_fetch` ロケールと対照的なフィクスチャ機能。
- Torii は `--features sorafs-storage` と統合を完了するためにコンパイルしました。
- ドキュメント ([ノードの設定](node-storage.md)) デフォルトの設定 + CLI の実際の操作。担当のオペレーターのランブック。
- テレメトリアはステージングのダッシュボードに表示されます。 PoR の容量飽和に関するアラートを設定します。

## ドキュメントと操作の強化

- [ノード参照](node-storage.md) のデフォルト設定、CLI の使用、およびトラブルシューティングの実際。
- Mantener el [ノードの運用手順書](node-operations.md) は、SF-3 の進化に合わせて実装されています。
- エンドポイント `/sorafs/*` の API リファレンスを公開し、OpenAPI のハンドラーを Torii リストに表示するためのポータル ポータルと接続を公開します。