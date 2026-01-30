---
lang: ur
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0065888efeec93e8d83c37055f5c5232c6917424f64252a74cb1217a6dcceaf6
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: staging-manifest-playbook
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sorafs/runbooks/staging_manifest_playbook.md` を反映しています。Sphinx セットが完全に退役するまで、Docusaurus 版とレガシー Markdown の両方を揃えてください。
:::

## 概要

このプレイブックは、議会承認の chunker プロファイルをステージングの Torii デプロイで有効化し、変更を本番へ昇格する前に確認する手順を示します。SoraFS ガバナンス憲章が承認済みで、リポジトリ内に正規 fixtures があることを前提にしています。

## 1. 前提条件

1. 正規 fixtures と署名を同期します:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii が起動時に読む admission envelope のディレクトリを用意します（例）: `/var/lib/iroha/admission/sorafs`。
3. Torii の設定で discovery キャッシュと admission enforcement を有効にします:

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

## 2. Admission envelope の公開

1. 承認済みの provider admission envelope を `torii.sorafs.discovery.admission.envelopes_dir` が指すディレクトリへコピーします:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii を再起動します（オンザフライのリロードをラップしている場合は SIGHUP を送信）。
3. admission メッセージをログで確認します:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Discovery 伝播の確認

1. プロバイダーパイプラインで生成した署名済み provider advert ペイロード（Norito バイト）を投稿します:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. discovery エンドポイントを照会し、advert が正規 alias で表示されることを確認します:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   `profile_aliases` の先頭に `"sorafs.sf1@1.0.0"` が含まれていることを確認します。

## 4. Manifest と Plan のエンドポイント検証

1. manifest メタデータを取得します（admission が有効な場合は stream token が必要）:

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON 出力を確認し、次を検証します:
   - `chunk_profile_handle` が `sorafs.sf1@1.0.0`。
   - `manifest_digest_hex` が determinism レポートと一致。
   - `chunk_digests_blake3` が再生成した fixtures と一致。

## 5. テレメトリ確認

- Prometheus が新しいプロファイルメトリクスを公開していることを確認します:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ダッシュボードは staging provider を想定 alias で表示し、プロファイルがアクティブな間は brownout カウンタが 0 のままになっていること。

## 6. ロールアウト準備

1. URL、manifest ID、テレメトリスナップショットを含む短いレポートを作成します。
2. レポートを Nexus rollout チャンネルに共有し、予定されている本番アクティベーションウィンドウを添えます。
3. ステークホルダーの承認後、`chunker_registry_rollout_checklist.md` の Section 4 にある本番チェックリストへ進みます。

このプレイブックを更新しておくことで、chunker/admission のロールアウトが staging と production の間で同じ決定的手順を踏むことを保証できます。
