---
lang: ja
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

# Taikai アンカー系譜パケット テンプレート (SN13-C)

ロードマップ項目 **SN13-C - Manifests & SoraNS anchors** は、各エイリアスローテーションが
決定的な証跡バンドルを出荷することを要求する。このテンプレートをロールアウト
アーティファクトディレクトリ (例
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) にコピーし、ガバナンスへ
提出する前にプレースホルダーを置き換える。

## 1. メタデータ

| フィールド | 値 |
|------------|----|
| Event ID | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Alias namespace / name | `<sora / docs>` |
| 証跡ディレクトリ | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| オペレーター連絡先 | `<name + email>` |
| GAR / RPT チケット | `<governance ticket or GAR digest>` |

## バンドル helper (任意)

spool アーティファクトをコピーし、残りのセクションを埋める前に JSON サマリ
(任意で署名) を出力する:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

この helper は `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`,
envelopes, sentinels を Taikai spool ディレクトリ
(`config.da_ingest.manifest_store_dir/taikai`) から取り出すため、証跡フォルダには
以下で参照するファイルが既に揃う。

## 2. 系譜レジャーと hint

このウィンドウ向けに Torii が書き出した on-disk 系譜レジャーと hint JSON を
両方添付する。ファイルは
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` と
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json` から
直接取得される。

| アーティファクト | ファイル | SHA-256 | メモ |
|------------------|----------|---------|------|
| 系譜レジャー | `taikai-trm-state-docs.json` | `<sha256>` | 直前のマニフェスト digest/ウィンドウを証明する。 |
| 系譜 hint | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS アンカーへ送る前に取得。 |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. アンカーペイロードのキャプチャ

Torii がアンカーサービスへ送信した POST ペイロードを記録する。ペイロードには
`envelope_base64`, `ssm_base64`, `trm_base64` と inline の `lineage_hint` オブジェクトが
含まれる。監査ではこのキャプチャを用いて SoraNS に送った hint を証明する。Torii は
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json` を
Taikai spool ディレクトリ (`config.da_ingest.manifest_store_dir/taikai/`) に自動で書き出すため、
オペレーターは HTTP ログを追跡せずに直接コピーできる。

| アーティファクト | ファイル | SHA-256 | メモ |
|------------------|----------|---------|------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | `taikai-anchor-request-*.json` (Taikai spool) からコピーした raw リクエスト。 |

## 4. マニフェスト digest の確認

| フィールド | 値 |
|------------|----|
| 新しいマニフェスト digest | `<hex digest>` |
| 直前のマニフェスト digest (hint 由来) | `<hex digest>` |
| ウィンドウ開始 / 終了 | `<start seq> / <end seq>` |
| 受領タイムスタンプ | `<ISO8601>` |

上で記録した ledger/hint の hash を参照し、置き換えられたウィンドウを
reviewers が検証できるようにする。

## 5. メトリクス / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (alias 別): `<file path + hash>`

Prometheus/Grafana の export または `curl` 出力を提示し、カウンタの増加と
この alias の `/status` 配列を示す。

## 6. 証跡ディレクトリ用マニフェスト

証跡ディレクトリ (spool ファイル、payload キャプチャ、メトリクス snapshot) の
決定的なマニフェストを生成し、ガバナンスがアーカイブを展開せずに各 hash を
検証できるようにする。

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| アーティファクト | ファイル | SHA-256 | メモ |
|------------------|----------|---------|------|
| 証跡マニフェスト | `manifest.json` | `<sha256>` | ガバナンスパケット / GAR に添付する。 |

## 7. チェックリスト

- [ ] 系譜レジャーをコピー + hash 作成。
- [ ] 系譜 hint をコピー + hash 作成。
- [ ] Anchor POST payload をキャプチャし hash 作成。
- [ ] マニフェスト digest テーブルを記入。
- [ ] メトリクス snapshot をエクスポート (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] `scripts/repo_evidence_manifest.py` でマニフェストを生成。
- [ ] hash と連絡先情報を付けてガバナンスへアップロード。

このテンプレートを各エイリアスローテーションで維持すると、SoraNS の
ガバナンスバンドルは再現可能になり、系譜 hint が GAR/RPT 証跡に直接結び付く。
