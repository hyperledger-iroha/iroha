---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードプラン
タイトル: 実装計画 SoraFS
Sidebar_label: 不要な実装計画
説明: SF-3 の在庫ルートのトランスフォーマーは、実用的なアクション、テスト、テストに使用できます。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/sorafs_node_plan.md` を参照します。 Gardez les deux は、スフィンクスの歴史に関する文書の同期をコピーします。
:::

SF-3 のプレミア クレート実行可能ファイル `sorafs-node` は、プロセス Iroha/Torii と在庫 SoraFS の変換に使用されます。 [在庫管理ガイド](node-storage.md)、[プロバイダーの承認ポリシー](provider-admission-policy.md) および [在庫管理市場のルート管理](storage-capacity-marketplace.md) を使用して、順序を設定します。住みやすいもの。

## ポルテ シブル (Jalon M1)

1. **チャンク ストアの統合。** エンベロープ `sorafs_car::ChunkStore` は、バックエンドの永続的なチャンクのバイト、マニフェスト、および PoR のレパートリー構成を保存します。
2. **エンドポイント ゲートウェイ。** エンドポイントのエクスポーザー HTTP Norito は、ピンの管理、チャンクの取得、プロセス Torii の PoR および在庫管理を実行します。
3. **設定の詳細** 設定 `SoraFsStorage` の構造 (有効化、容量、レパートリー、一致制限のフラグ) は、`iroha_config`、`iroha_core`、および `iroha_torii` に依存します。
4. **割り当て/規則** アップリケは、安全なバックプレッシャーを制限するための制限およびパラレル リスムを定義します。
5. **分析。** ピンの成功率、チャンクの取得遅延、PoR の容量使用率および結果の評価/ログの分析。

## 旅の分解

### A. 構造宣言とモジュール

|ターシュ |責任者 |メモ |
|------|--|------|
| `crates/sorafs_node` の平均モジュール: `config`、`store`、`gateway`、`scheduler`、`telemetry`。 |ストレージを装備 |統合 Torii を再エクスポートするタイプの再利用可能ファイル。 |
|実装者 `StorageConfig` は `SoraFsStorage` をマップします (ユーザー → 実際の → デフォルト)。 |装備ストレージ / 構成 WG | Garantir que les couches Norito/`iroha_config` 残りの決定。 |
| Fournir une façade `NodeHandle` que Torii は、注ぎ口ピン/フェッチを利用します。 |ストレージを装備 |非同期の在庫とプロムベリーの内部カプセル化。 |

### B. チャンクストア永続化

|ターシュ |責任者 |メモ |
|------|--|------|
|バックエンドディスクエンベロープ `sorafs_car::ChunkStore` のマニフェストインデックス (`sled`/`sqlite`) を構築します。 |ストレージを装備 |レイアウト決定者: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
| `ChunkStore::sample_leaves` 経由で Metadonnées PoR (arbres 64 KiB/4 KiB) を保守します。 |ストレージを装備 |再婚後のリプレイをサポート。破損時のフェイルファースト。 |
| Implémenter le play d'intégrité au démarrage (マニフェストの再ハッシュ、不完全なピンのパージ)。 |ストレージを装備 | Torii の最後の再生のブロック。 |

### C. エンドポイントゲートウェイ

|エンドポイント |コンポートメント |タシュ |
|----------|--------------|------|
| `POST /sorafs/pin` | `PinProposalV1` を受け入れ、マニフェストを有効にし、ファイルの取り込みに対応し、マニフェストの CID を返信します。 |ル・チャンク・ストア経由で、チャンカーのプロフィール、クォータのアップリケ、ドンネのストリーマーを検証します。 |
| `GET /sorafs/chunks/{cid}` + 範囲の要求 |チャンクの平均ヘッダーのバイト数を検索 `Content-Chunker` ;仕様の容量を尊重してください。 |スケジューラー + ストリームの予算 (SF-2d の範囲の容量) を利用します。 |
| `POST /sorafs/por/sample` | PoR は、マニフェストとプレビューの束ねを実行するために必要な作業を行います。 |チャンク ストアの再利用、ペイロードの平均保存 Norito JSON。 |
| `GET /sorafs/telemetry` |履歴書: 能力、PoR の成功、フェッチのエラーの計算。 | Fournir les données はダッシュボード/オペレーターを注ぎます。 |

La plomberie ランタイムは、`sorafs_node::por` を介して相互作用 PoR を信頼します。`PorChallengeV1`、`PorProofV1` および `AuditVerdictV1` は、メトリック `CapacityMeter` の評決を反映するトラッカーを登録します。 gouvernance sans logique Torii の仕様。【crates/sorafs_node/src/scheduler.rs#L147】

実装上の注意:- スタック Axum de Torii のペイロード `norito::json` を使用します。
- Ajouter des schémas Norito pour les réponses (`PinResultV1`、`FetchErrorV1`、structs de télémétrie)。

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` は、`sorafs_node::NodeHandle::por_ingestion_status`、および Torii を介して、プロバイダーごとにバックログのバックログのメンテナンス/成功/成功のタイムスタンプを公開します。また、Torii はゲージを登録します`torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` 注ぎますダッシュボード.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. スケジューラとアプリケーションのクォータ

|ターシュ |詳細 |
|-----|----------|
|クォータディスク | Suivre les bytes sur disque ;リジェテル レ ヌーボー ピン オーデラ デ `max_capacity_bytes`。政治的先物をフックに投資する。 |
|フェッチの同時実行 |セマフォ グローバル (`max_parallel_fetches`) に加えて、SF-2d 範囲のプロバイダーの上限に基づく予算。 |
|ファイルデピン |リミッターは、注意を払って摂取する仕事を制限します。エンドポイントのステータス Norito ファイルの詳細を公開します。 |
|ケイデンスPoR | `por_sample_interval_secs` のパイロット好きの労働者。 |

### E. テレメトリーとロギング

メトリック (Prometheus) :

- `sorafs_pin_success_total`、`sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (ヒストグラム平均ラベル `result`)
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

ログ/イベント:

- 摂取管理のテレメトリー Norito 構造 (`StorageTelemetryV1`)。
- 使用率 > 90 % が問題を解決するための警告。

### F. テスト戦略

1. **ユニットをテストします。** チャンク ストアの永続性、クォータの計算、スケジューラーの不変量 (`crates/sorafs_node/src/scheduler.rs`)。
2. **統合テスト** (`crates/sorafs_node/tests`)。ピン → fetch aller-retour、reprise après redémarrage、rejet deuota、verification de preuve d'échantillonnage PoR。
3. **統合 Torii のテスト。** 実行者 Torii は在庫活動を実行し、`assert_cmd` 経由でエンドポイント HTTP を実行します。
4. **ロードマップのカオス。** 将来の訓練をシミュレートし、長い間、プロバイダーを抑制します。

## 依存関係

- 入場ポリティーク SF-2b — 入場前に出版物を公開するための封筒を保証するもの。
- Marketplace de capacité SF-2c — 容量の宣言を行うためのラッチャー。
- 広告 ​​SF-2d の拡張 — 消費者向けの生産能力と管理能力の予算。

## 出撃基準

- `cargo run -p sorafs_node --example pin_fetch` フィクスチャ ロケールの機能。
- Torii は `--features sorafs-storage` をビルドし、統合テストをパスします。
- ドキュメント ([在庫ガイド](node-storage.md)) 構成の平均的なデフォルトと CLI の例を示します。ランブックオペレーターの責任者。
- ステージングのダッシュボード上に表示されるテレメトリー。アラートは、容量の飽和と PoR のチェックを設定します。

## Livrables のドキュメントと操作

- 定期的な [在庫参照](node-storage.md) 構成のデフォルト値、CLI の使用法およびトラブルシューティングの記録。
- Garder le [runbook d'operations du nœud](node-operations.md) SF-3 の進化を実現するための目標を達成します。
- エンドポイント `/sorafs/*` とポータル開発およびマニフェストの信頼できる API の公開者 OpenAPI ハンドラー Torii を配置します。