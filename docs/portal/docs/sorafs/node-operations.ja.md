---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a8347c1f10b45cc72fadd830e246b9fedcf7804cf43631ea85998743bc1f044
source_last_modified: "2025-11-12T17:23:43.290245+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: node-operations
title: ノード運用ランブック
sidebar_label: ノード運用ランブック
description: Torii 内に埋め込まれた `sorafs-node` デプロイを検証します。
---

:::note 正規ソース
このページは `docs/source/sorafs/runbooks/sorafs_node_ops.md` を反映しています。Sphinx の旧ドキュメントセットが退役するまで、両方のコピーを同期してください。
:::

## 概要

このランブックは、Torii に埋め込まれた `sorafs-node` デプロイを検証する手順を
オペレーター向けにまとめたものです。各セクションは SF-3 の成果物に直接
対応します: pin/fetch 往復、再起動復旧、クォータ拒否、PoR サンプリング。

## 1. 前提条件

- `torii.sorafs.storage` でストレージワーカーを有効化します:

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

- Torii プロセスが `data_dir` に読み書きできる権限を持つことを確認します。
- 宣言が記録された後、`GET /v1/sorafs/capacity/state` でノードが期待どおりの
  容量を広告していることを確認します。
- スムージングを有効にすると、ダッシュボードは生の GiB·hour/PoR カウンタと
  平滑化されたカウンタの両方を表示し、スポット値と並べてジッタのない
  トレンドを強調します。

### CLI ドライラン (任意)

HTTP エンドポイントを公開する前に、同梱 CLI でストレージバックエンドを
サニティチェックできます。【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

これらのコマンドは Norito JSON サマリーを出力し、チャンクプロファイルや digest の
不一致を拒否するため、Torii 連携前の CI スモークチェックに有用です。【crates/sorafs_node/tests/cli.rs#L1】

### PoR 証明リハーサル

オペレーターは、ガバナンスが発行した PoR アーティファクトを Torii に
アップロードする前にローカルで再生できます。CLI は同じ `sorafs-node` の
ingest パスを再利用するため、ローカル実行で HTTP API が返すのと同じ検証
エラーを表面化できます。

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

このコマンドは JSON サマリー (manifest digest、provider id、proof digest、
サンプル数、任意の verdict 結果) を出力します。`--manifest-id=<hex>` を
指定して保存済み manifest がチャレンジ digest と一致することを確認し、
`--json-out=<path>` でサマリーを元のアーティファクトと一緒に監査証跡として
保存してください。`--verdict` を付けると、HTTP API を呼ぶ前にオフラインで
チャレンジ → 証明 → verdict の一連のループをリハーサルできます。

Torii が稼働したら、同じアーティファクトを HTTP から取得できます:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

両方のエンドポイントは埋め込みストレージワーカーによって提供されるため、
CLI スモークテストとゲートウェイのプローブは同期したままです。【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → Fetch 往復

1. manifest + payload バンドルを作成します (例:
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`)。
2. base64 エンコードで manifest を送信します:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   リクエスト JSON には `manifest_b64` と `payload_b64` を含める必要があります。
   成功すると `manifest_id_hex` と payload digest が返ります。
3. pin 済みデータを fetch します:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` フィールドを base64 デコードし、元のバイト列と一致することを
   確認します。

## 3. 再起動復旧ドリル

1. 上記の手順で少なくとも 1 つの manifest を pin します。
2. Torii プロセス (またはノード全体) を再起動します。
3. fetch リクエストを再送します。payload は引き続き取得でき、返却される
   digest は再起動前の値と一致する必要があります。
4. `GET /v1/sorafs/storage/state` を確認し、`bytes_used` が再起動後も
   永続化された manifest を反映していることを確かめます。

## 4. クォータ拒否テスト

1. `torii.sorafs.storage.max_capacity_bytes` を一時的に小さな値
   (例: 1 つの manifest のサイズ) に下げます。
2. manifest を 1 件 pin し、リクエストが成功することを確認します。
3. 同程度のサイズの manifest をもう 1 件 pin します。Torii は HTTP `400` と
   `storage capacity exceeded` を含むエラーメッセージで拒否する必要があります。
4. 終了後に通常の容量制限へ戻します。

## 5. PoR サンプリングのプローブ

1. manifest を pin します。
2. PoR サンプルをリクエストします:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. 返却された `samples` に要求した件数が含まれ、各 proof が保存済み manifest
   ルートに対して検証できることを確認します。

## 6. 自動化フック

- CI / スモークテストは次のターゲットチェックを再利用できます:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  これらは `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`,
  `por_sampling_returns_verified_proofs` をカバーします。
- ダッシュボードは以下を追跡します:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` と `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` で公開される PoR 成功/失敗カウンタ
  - `sorafs_node_deal_publish_total{result=success|failure}` による settlement 公開試行

これらのドリルを実施することで、埋め込みストレージワーカーがデータを
取り込み、再起動に耐え、設定されたクォータを尊重し、ノードがネットワークに
容量を広告する前に決定的な PoR 証明を生成できることを保証します。
