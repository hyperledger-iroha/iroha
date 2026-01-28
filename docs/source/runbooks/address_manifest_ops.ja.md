---
lang: ja
direction: ltr
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

# アドレスマニフェスト運用ランブック (ADDR-7c)

このランブックはロードマップ項目 **ADDR-7c** を運用に落とし込み、Sora
Nexus のアカウント/エイリアスマニフェストに対する検証・公開・退役の
手順を示します。技術契約は
[`docs/account_structure.md`](../../account_structure.md) §4 を参照し、
テレメトリ要件は `dashboards/grafana/address_ingest.json` を参照してください。

## 1. スコープと入力

| 入力 | ソース | 備考 |
|-------|--------|-------|
| 署名済みマニフェストバンドル（`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`） | SoraFS pin（`sorafs://address-manifests/<CID>/`）と HTTPS ミラー | バンドルはリリース自動化が生成するため、ミラー時にディレクトリ構造を維持してください。 |
| 前回マニフェストの digest + sequence | 直前のバンドル（同じパスパターン） | 単調増加/不変性の証明に必要。 |
| テレメトリへのアクセス | Grafana `address_ingest` ダッシュボード + Alertmanager | Local‑8 退役と不正アドレスのスパイク監視に使用。 |
| ツール | `cosign`, `shasum`, `b3sum`（または `python3 -m blake3`）, `jq`, `iroha` CLI, `scripts/account_fixture_helper.py` | チェックリスト実行前にインストール。 |

## 2. アーティファクト構成

各バンドルは以下の構成に従います。環境間コピー時にファイル名を
変更しないでください。

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

`manifest.json` のヘッダフィールド:

| フィールド | 説明 |
|-------|-------------|
| `version` | スキーマバージョン（現在 `1`）。 |
| `sequence` | 単調増加のリビジョン番号（必ず 1 ずつ増加）。 |
| `generated_ms` | 公開時刻の UTC タイムスタンプ（エポックからのミリ秒）。 |
| `ttl_hours` | Torii/SDK が尊重可能なキャッシュ上限（デフォルト 24）。 |
| `previous_digest` | 直前マニフェスト本文の BLAKE3（hex）。 |
| `entries` | レコード配列（`global_domain`, `local_alias`, `tombstone`）。 |

## 3. 検証手順

1. **バンドルを取得する。**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **チェックサム検証。**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   すべて `OK` であること。一致しない場合は改ざんとして扱います。

3. **Sigstore 検証。**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **不変性証明。** `sequence` と `previous_digest` をアーカイブ済み
   マニフェストと比較します。

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   出力された digest が `previous_digest` と一致する必要があります。sequence
   の欠番は禁止です。逸脱した場合はマニフェストを再発行してください。

5. **TTL の確認。** `generated_ms + ttl_hours` が想定デプロイ期間を
   カバーすること。足りない場合はキャッシュ期限前に再公開します。

6. **エントリの健全性。**
   - `global_domain` エントリは `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }` を必ず含む。
   - `local_alias` エントリは Norm v1 の 12 バイト digest を含める（`iroha tools address convert <address-or-account_id> --format json --expect-prefix 753` で確認。JSON 概要は `input_domain` にドメインを反映し、`--append-domain` は `<ih58>@<domain>` 形式の再エンコードを示す）。
   - `tombstone` エントリは退役対象の selector を正確に参照し、`reason_code`、`ticket`、`replaces_sequence` を含める。

7. **フィクスチャ整合性。** 正規ベクトルを再生成し、Local digest
   テーブルに意図しない変更がないことを確認します。

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **自動検証ガードレール。** マニフェスト検証ツールでバンドル全体
   を再確認します（ヘッダスキーマ、エントリ形状、チェックサム、
   previous‑digest の結線）。

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   `--previous` は直前バンドルを指します。ツールは `sequence` の単調性と
   `previous_digest` の BLAKE3 を再計算します。チェックサム不一致や
   `tombstone` の必須項目欠落で即時失敗するため、署名依頼前に出力を
   変更チケットへ添付してください。

## 4. エイリアス / tombstone の変更フロー

1. **変更提案。** 理由コード（`LOCAL8_RETIREMENT`、`DOMAIN_REASSIGNED` など）と
   対象 selector を記載した governance チケットを起票します。
2. **正規 payload の導出。** 更新対象 alias について以下を実行します。

   ```bash
   iroha tools address convert sora...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **マニフェストのドラフト。** 次の形式の JSON を追加します。

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   Local alias を Global に置換する場合、`tombstone` と `global_domain`
   の両方を追加し、後者には Nexus 識別子を含めます。

4. **バンドル検証。** 署名前にドラフトに対して検証手順を再実行します。
   `true` に維持します。Local‑8 の利用が 0 と確認できるまで `false` に
   しないでください。dev/test でのみ soak が必要な場合に一時的に `false`
   を許可します。

## 5. 監視とロールバック

- ダッシュボード: `dashboards/grafana/address_ingest.json`（
  `torii_address_invalid_total{endpoint,reason}`、
  `torii_address_local8_total{endpoint}`、
  `torii_address_collision_total{endpoint,kind="local12_digest"}`、
  `torii_address_collision_domain_total{endpoint,domain}`）は 30 日間
  グリーンを維持し、Local‑8/Local‑12 の恒久ゲートの前提とします。
- ゲート証跡: `torii_address_local8_total` と `torii_address_collision_total`
  に対する 30 日の Prometheus 範囲クエリ（例: `promtool query range --output=json ...`）を
  エクスポートし、`cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`
  を実行します。JSON と CLI 出力を rollout チケットに添付し、ガバナンスが
  カバレッジとカウンタの平坦性を確認できるようにします。
- アラート（`dashboards/alerts/address_ingest_rules.yml`）:
  - `AddressLocal8Resurgence` — どのコンテキストでも Local‑8 が増えたら
    ページング。リリースブロッカーとして扱い、strict‑mode ロールアウトを
    を一時的に設定します。テレメトリがクリーンになったら `true` に戻します。
  - `AddressLocal12Collision` — 2 つの Local‑12 ラベルが同一 digest に
    なると即発火。マニフェスト昇格を停止し、`scripts/address_local_toolkit.sh`
    で digest マッピングを確認し、Nexus ガバナンスと調整して再発行します。
  - `AddressInvalidRatioSlo` — 無効 IH58/圧縮アドレスの割合が 10 分間 0.1% SLO
    を超えると警告（Local‑8/strict‑mode 拒否は除外）。`torii_address_invalid_total`
    を原因別に調査し、SDK オーナーと調整して strict‑mode を再開します。
- ログ: Torii の `manifest_refresh` ログとガバナンスチケット番号を `notes.md`
  に残してください。
- ロールバック: 直前のバンドルを再発行（同じファイル＋rollback チケット）し、

## 6. 参考

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1（契約）。
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py)（フィクスチャ同期）。
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json)（正規 digest）。
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json)（テレメトリ）。
