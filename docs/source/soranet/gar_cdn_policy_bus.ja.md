---
lang: ja
direction: ltr
source: docs/source/soranet/gar_cdn_policy_bus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b230f98f025e9d55d6eefa2b7b44348ceef3f20505e723fc703151497aeef777
source_last_modified: "2026-01-03T18:08:02.008828+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/soranet/gar_cdn_policy_bus.md -->

# GAR CDN ポリシー バス

SNNet-15G の enforcement surface は、オペレーターが GAR CDN ポリシーのペイロード (TTL 上書き、パージタグ、モデレーションスラッグ、レート上限、ジオフェンス規則、法的保全) をファイルバックのバス経由で公開できるようになり、PoP が受領バンドルと並行して再現可能なアーティファクトを受け取れる。

## 公開
- `cargo xtask soranet-gar-bus --policy <path> [--pop <label>] [--out-dir <dir>]` を実行し、`GarCdnPolicyV1` の JSON ペイロードを読み込んで、対象ディレクトリ (既定: `artifacts/soranet/gateway/<pop>/gar_bus/`) に `gar_cdn_policy_event.{json,md}` を出力する。
- JSON バンドルはソースパス、公開タイムスタンプ、任意の PoP ラベル、CDN ポリシー全文を記録し、PoP の証跡パケットと整合する。

## ゲートウェイでの適用
- ゲートウェイは `sorafs.gateway.cdn_policy_path` を読み込み、`GatewayPolicy` の GAR 違反と CLI 受領アクション種別 (`ttl_override`, `moderation`) に表れる同じ強制契約 (TTL 上書き、パージタグ、モデレーションスラッグ、レート上限、ジオフェンス/拒否リスト、法的保全) を適用する。
- GAR 違反イベントの更新には、新しいポリシーラベル、観測 TTL、リージョン、レート上限のヒントが含まれ、ダッシュボード/アラートに反映される。
