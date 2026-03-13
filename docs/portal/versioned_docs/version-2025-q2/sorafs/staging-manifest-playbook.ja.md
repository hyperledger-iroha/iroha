---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-ja
slug: /sorafs/staging-manifest-playbook-ja
---

:::note 正規ソース
ミラー `docs/source/sorafs/runbooks/staging_manifest_playbook.md`。リリース間で両方のコピーを揃えておいてください。
:::

## 概要

このプレイブックでは、変更を運用環境にプロモートする前に、ステージング Torii デプロイメントで議会承認のチャンカー プロファイルを有効にする手順を説明します。 SoraFS ガバナンス憲章が承認されており、正規フィクスチャがリポジトリで利用可能であることを前提としています。

## 1. 前提条件

1. 正規のフィクスチャと署名を同期します。

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii が起動時に読み取るアドミッション エンベロープ ディレクトリ (パスの例): `/var/lib/iroha/admission/sorafs` を準備します。
3. Torii 設定で検出キャッシュとアドミッションの強制が有効になっていることを確認します。

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. 入学許可封筒の発行

1. 承認されたプロバイダー アドミッション エンベロープを、`torii.sorafs.discovery.admission.envelopes_dir` によって参照されるディレクトリにコピーします。

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii を再起動します (または、ローダーをオンザフライ リロードでラップした場合は SIGHUP を送信します)。
3. 許可メッセージのログを追跡します。

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. ディスカバリーの伝播を検証する

1. によって生成された署名付きプロバイダー広告ペイロード (Norito バイト) を投稿します。
   プロバイダーパイプライン:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. 検出エンドポイントにクエリを実行し、広告が正規のエイリアスで表示されることを確認します。

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   `profile_aliases` に最初のエントリとして `"sorafs.sf1@1.0.0"` が含まれていることを確認します。

## 4. マニフェストの実行とエンドポイントの計画

1. マニフェスト メタデータを取得します (アドミッションが強制されている場合はストリーム トークンが必要です)。

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON 出力を調べて、次のことを確認します。
   - `chunk_profile_handle` は `sorafs.sf1@1.0.0` です。
   - `manifest_digest_hex` は決定論レポートと一致します。
   - `chunk_digests_blake3` は、再生成されたフィクスチャと位置合わせされます。

## 5. テレメトリチェック

- Prometheus が新しいプロファイル メトリックを公開していることを確認します。

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ダッシュボードには、想定されるエイリアスの下にステージング プロバイダーが表示され、プロファイルがアクティブである間は電圧低下カウンターがゼロに維持される必要があります。

## 6. 展開の準備

1. URL、マニフェスト ID、テレメトリ スナップショットを含む短いレポートをキャプチャします。
2. 予定されている運用アクティベーション ウィンドウと並行して、Nexus ロールアウト チャネルでレポートを共有します。
3. 関係者が承認したら、生産チェックリスト (`chunker_registry_rollout_checklist.md` のセクション 4) に進みます。

このプレイブックを常に最新の状態に保つことで、すべてのチャンカー/アドミッションのロールアウトがステージングと運用全体で同じ決定的な手順に従うことが保証されます。
