---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T15:38:30.655980+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

:::note 正規ソース
ミラー `docs/source/sorafs/runbooks/sorafs_node_ops.md`。リリース間で両方のコピーを揃えておいてください。
:::

## 概要

この Runbook では、オペレータが Torii 内に埋め込まれた `sorafs-node` デプロイメントを検証する手順を説明します。各セクションは、SF-3 の成果物 (ピン/フェッチ ラウンド トリップ、再起動回復、クォータ拒否、PoR サンプリング) に直接マッピングされます。

## 1. 前提条件

- `torii.sorafs.storage` でストレージ ワーカーを有効にします。

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Torii プロセスが `data_dir` への読み取り/書き込みアクセス権を持っていることを確認します。
- 宣言が記録されたら、ノードが `GET /v1/sorafs/capacity/state` 経由で予想される容量をアドバタイズしていることを確認します。
- スムージングが有効な場合、ダッシュボードには生の GiB 時間/PoR カウンターと平滑化された GiB 時間/PoR カウンターの両方が表示され、スポット値とともにジッターのない傾向が強調表示されます。

### CLI ドライラン (オプション)

HTTP エンドポイントを公開する前に、バンドルされた CLI を使用してストレージ バックエンドの健全性をチェックできます。【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

このコマンドは、Norito JSON サマリーを出力し、チャンク プロファイルまたはダイジェストの不一致を拒否するため、Torii 配線の前に CI スモーク チェックを行うのに役立ちます。【crates/sorafs_node/tests/cli.rs#L1】

Torii が有効になると、HTTP 経由で同じアーティファクトを取得できます。

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

両方のエンドポイントは組み込みストレージ ワーカーによって提供されるため、CLI スモーク テストとゲートウェイ プローブは同期を保ちます。【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. ピン→フェッチの往復

1. マニフェスト + ペイロード バンドルを作成します (たとえば、`iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`)。
2. Base64 エンコードを使用してマニフェストを送信します。

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   リクエスト JSON には `manifest_b64` と `payload_b64` が含まれている必要があります。 A successful response returns `manifest_id_hex` and the payload digest.
3. 固定されたデータをフェッチします。

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` フィールドを Base64 デコードし、元のバイトと一致することを確認します。

## 3. 復旧訓練を再開する

1. 上記のように少なくとも 1 つのマニフェストを固定します。
2. Torii プロセス (またはノード全体) を再起動します。
3. フェッチ要求を再送信します。ペイロードは引き続き取得可能である必要があり、返されたダイジェストは再起動前の値と一致する必要があります。
4. `GET /v1/sorafs/storage/state` を調べて、再起動後に永続化されたマニフェストが `bytes_used` に反映されていることを確認します。

## 4. クォータ拒否テスト

1. `torii.sorafs.storage.max_capacity_bytes` を一時的に小さい値 (たとえば、単一マニフェストのサイズ) に下げます。
2. マニフェストを 1 つ固定します。リクエストは成功するはずです。
3. 同様のサイズの 2 番目のマニフェストを固定しようとします。 Torii は、HTTP `400` と `storage capacity exceeded` を含むエラー メッセージを含むリクエストを拒否する必要があります。
4. 終了したら、通常の容量制限に戻します。

## 5. PoR サンプリング プローブ

1. マニフェストを固定します。
2. PoR サンプルをリクエストします。

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. 応答に要求されたカウントを含む `samples` が含まれていること、および保存されているマニフェスト ルートに対して各プルーフが検証されていることを確認します。

## 6. 自動化フック

- CI / スモーク テストでは、以下に追加されたターゲット チェックを再利用できます。

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```これには、`pin_fetch_roundtrip`、`pin_survives_restart`、`pin_quota_rejection`、および `por_sampling_returns_verified_proofs` が含まれます。
- ダッシュボードは以下を追跡する必要があります。
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` および `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` 経由で PoR 成功/失敗カウンターが表示される
  - `sorafs_node_deal_publish_total{result=success|failure}` を介した決済公開試行

これらの訓練に従うことで、ノードがより広範なネットワークに容量をアドバタイズする前に、組み込みストレージ ワーカーがデータを取り込み、再起動に耐え、設定されたクォータを尊重し、決定論的な PoR 証明を生成できることが保証されます。