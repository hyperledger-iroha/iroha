---
lang: ja
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

# データ可用性マニフェスト ガバナンスパケット (テンプレート)

議会パネルが補助金、takedown、または保持期間の変更 (roadmap DA-10) のために
DA マニフェストをレビューする際にこのテンプレートを使用する。Markdown を
ガバナンスチケットにコピーし、プレースホルダーを埋め、下記で参照する署名済み
Norito ペイロードと CI アーティファクトと一緒に完成版を添付する。

```markdown
## マニフェスト メタデータ
- マニフェスト名 / バージョン: <string>
- Blob クラスとガバナンスタグ: <taikai_segment / da.taikai.live>
- BLAKE3 ダイジェスト (hex): `<digest>`
- Norito ペイロードハッシュ (任意): `<digest>`
- ソース envelope / URL: <https://.../manifest_signatures.json>
- Torii ポリシー snapshot ID: `<unix timestamp or git sha>`

## 署名検証
- マニフェスト取得元 / storage チケット: `<hex>`
- 検証コマンド/出力: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (ログ抜粋を添付?)
- `manifest_blake3` (ツール報告): `<digest>`
- `chunk_digest_sha3_256` (ツール報告): `<digest>`
- 評議会署名者のマルチハッシュ:
  - `<did:...>` / `<ed25519 multihash>`
- 検証タイムスタンプ (UTC): `<2026-02-20T11:04:33Z>`

## 保持期間検証
| 項目 | 期待値 (ポリシー) | 観測値 (マニフェスト) | エビデンス |
|------|-------------------|------------------------|------------|
| ホット保持 (秒) | <例: 86400> | <値> | `<torii.da_ingest.replication_policy dump | CI link>` |
| コールド保持 (秒) | <例: 1209600> | <値> |  |
| 必要レプリカ数 | <値> | <値> |  |
| ストレージクラス | <hot / warm / cold> | <値> |  |
| ガバナンスタグ | <da.taikai.live> | <値> |  |

## コンテキスト
- リクエスト種別: <補助金 | Takedown | マニフェストローテーション | 緊急凍結>
- 起票チケット / コンプライアンス参照: <link or ID>
- 補助金 / レント影響: <expected XOR change or "n/a">
- モデレーション異議申し立てリンク (ある場合): <case_id or link>

## 決定サマリ
- パネル: <インフラ | モデレーション | トレジャリー>
- 投票結果: `<for>/<against>/<abstain>` (クォーラム `<threshold>` 達成?)
- 有効化 / ロールバック 高度またはウィンドウ: `<block/slot range>`
- フォローアップ:
  - [ ] Treasury / rent ops に通知
  - [ ] 透明性レポートを更新 (`TransparencyReportV1`)
  - [ ] バッファ監査をスケジュール

## エスカレーションと報告
- エスカレーション区分: <補助金 | Compliance | 緊急凍結>
- 透明性レポートのリンク / ID (更新時): <`TransparencyReportV1` CID>
- Proof-token バンドルまたは ComplianceUpdate 参照: <path or ticket ID>
- レント / リザーブ台帳の差分 (該当時): <`ReserveSummaryV1` snapshot link>
- テレメトリ snapshot URL: <Grafana permalink or artefact ID>
- 議会議事録向けメモ: <summary of deadlines / obligations>

## 添付資料
- [ ] 署名済み Norito マニフェスト (`.to`)
- [ ] 保持値を示す JSON サマリ / CI アーティファクト
- [ ] Proof token または compliance パケット (takedown 用)
- [ ] バッファ テレメトリ snapshot (`iroha_settlement_buffer_xor`)
```

各パケットは投票の Governance DAG エントリ配下に保管し、後続のレビューが
マニフェストダイジェストを参照できるよう、全体のセレモニーを繰り返さずに済むようにする。
