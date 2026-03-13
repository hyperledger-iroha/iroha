---
lang: ja
direction: ltr
source: docs/source/sorafs_chunk_range_smoketest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa4b01b4d7d5340555c8f27b3f733de4740328058177ef26c97afbc146b3c012
source_last_modified: "2025-11-04T12:07:36.779163+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs_chunk_range_smoketest.md -->

# SoraFS チャンク範囲スモークテスト計画（ドラフト）

## 目的

- デプロイ後にゲートウェイのチャンク範囲対応を素早く検証する。
- 小さなシナリオでオーケストレーター、ストリームトークン、証明の動作を確認する。

## ワークフロー概要

1. 組み込み CLI (`iroha app sorafs storage token issue`) を使ってストリームトークンを発行し、
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --json-out manifest_report.json`
   を実行してマニフェストのチャンク計画を出力する（レポートには `chunk_fetch_specs` が含まれる）。
2. `iroha app sorafs fetch --manifest manifest.to --plan manifest_report.json --manifest-id <manifest_digest_hex> --gateway-provider "name=primary,provider-id=<hex32>,base-url=https://gateway.example,stream-token=<base64>" --max-peers 4 --retry-budget 3` を実行する。
3. CLI はチャンクと証明を検証し、サマリーメトリクスを記録し、必要なら組み立て済みペイロード（`--output`）や JSON レポート（`--json-out`）を出力する。
4. 期待されるダイジェスト／メトリクスしきい値と照合する。

### ストリームトークンの発行

`iroha app sorafs toolkit pack` のマニフェストレポートと、ゲートウェイレジストリ／アドミッション記録の
プロバイダ ID を使い、対象ゲートウェイ向けのスコープ付きトークンを発行する。CLI は Torii の
`/v2/sorafs/storage/token` エンドポイントを呼び出し、`X-SoraFS-Client` / `X-SoraFS-Nonce`
ヘッダを自動で付与する。

```bash
MANIFEST_REPORT_JSON="manifest_report.json"
MANIFEST_ID_HEX="$(jq -r '.manifest_digest_hex' "${MANIFEST_REPORT_JSON}")"
PROVIDER_ID_HEX="<provider-id-hex>"

iroha app sorafs storage token issue \
  --manifest-id "${MANIFEST_ID_HEX}" \
  --provider-id "${PROVIDER_ID_HEX}" \
  --client-id smoke-ci \
  --ttl-secs 900 \
  --max-streams 4 \
  --rate-limit-bytes 134217728 \
  --requests-per-minute 12
```

`--nonce` を省略すると、コマンドは生成した nonce（監査ログ用）を表示し、トークン本文・署名・エンコード
済みペイロードを含む JSON を出力する。

```jsonc
{
  "token": {
    "body": {
      "token_id": "4f9f1c7d2e5e4a86c4d5ae9919cf0da2",
      "manifest_cid_hex": "b41600aac9ff0d3d0fc9f48e3a5659c7c7a9",
      "provider_id_hex": "00112233445566778899aabbccddeeff",
      "profile_handle": "chunk_profile:ldx4",
      "max_streams": 4,
      "ttl_epoch": 1739210400,
      "rate_limit_bytes": 134217728,
      "issued_at": 1739209500,
      "requests_per_minute": 12,
      "token_pk_version": 1
    },
    "signature_hex": "29f71a5d3f4b56c77d44cba2c5ff5c8181ae",
    "encoded": "AABbZGF0YURyb3BTYW1wbGVQYXlsb2FkCg=="
  },
  "token_base64": "AABbZGF0YURyb3BTYW1wbGVQYXlsb2FkCg=="
}
nonce: 4c1f8e9d97c84d61b9d3c8c5
```

`iroha app sorafs fetch` の `--gateway-provider` に渡す `stream-token=<base64>` には
`token.encoded` または `token_base64` を指定できる。フラグ内の `provider-id` は、発行時に
指定した `provider_id_hex` と一致していなければゲートウェイが拒否する。

## 期待される出力

```json
{
  "manifest_id": "d1d6c1b12ba2cd505c6c738bcb12de9dcda6658fcfaa739bf482ba4bf664f7f3",
  "manifest_cid": "6c4347...c7a9",
  "chunk_count": 128,
  "fetched_bytes": 33554432,
  "provider_count": 1,
  "providers": ["primary"],
  "provider_reports": [
    { "provider_id": "primary", "alias": "primary", "successes": 128, "failures": 0, "disabled": false }
  ],
  "chunk_receipts": [
    { "chunk_index": 0, "provider_id": "primary", "alias": "primary", "attempts": 1 }
    // ...
  ]
}
```

## メトリクスしきい値

- 成功チャンク率 >= 99%
- 平均レイテンシ < 50 ms（ホットキャッシュ想定。環境に応じて調整）
- 証明失敗 = 0

## CI 統合

- Buildkite ステップ `ci/sorafs-chunk-range-smoketest` を追加し、デプロイ直後に CLI をステージング
  ゲートウェイへ実行する。`docs/source/sorafs_ci_templates.md` の共有テンプレートを参照し、
  パイプラインシークレット（`SORA_FS_GATEWAY_URL`, `SORA_FS_STREAM_TOKEN`）で
  マニフェスト／トークンのフィクスチャを渡す。
- CLI の JSON 出力をアーカイブしてビルドサマリーへ添付し、再実行せずに
  レイテンシ／スループットを確認できるようにする。
- `docs/source/sorafs_gateway_*`、`crates/sorafs_*`、`ci/sorafs_*` を触る PR では
  ステップをマージゲートにする。

## コールドキャッシュのしきい値

- コールドキャッシュ（初回実行やキャッシュフラッシュ）の場合は、レイテンシのガードを
  `avg_latency_ms < 150`、`p95_latency_ms < 250` に緩める。証明失敗の許容値は 0 のまま。
- CLI に `--profile cold` を追加し、レポートにプロファイルラベルを含める。
  CI ジョブは適切な閾値セットを選択できるようにする。
- JSON アーティファクトにキャッシュ状態（`"cache_state": "cold"`）を記録する。

## マルチゲートウェイ・シナリオ

- オーケストレーター MVP が入ったら、マニフェストが広告するプロバイダ一覧
  （`manifest.providers[]`）を順に検証するよう拡張する。
  オーケストレーターの重み付きスケジューラを使い、各ゲートウェイから取得した際の
  レイテンシ／失敗メトリクスを記録する。
- すべてのゲートウェイ結果を統合した JSON レポートを生成し、
  ゲートウェイごとの成功サマリーと共に出力する。CI ワークフローでは、
  どれか 1 つでも成功率の閾値を下回った場合に失敗扱いとし、
  それぞれのレイテンシ劣化も明示する。
