---
lang: ja
direction: ltr
source: docs/source/soranet/nsc-55-legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b72f94767de6c92f28503526ea37756d4ca5a7c4ca007b3d31939e78296eb7de
source_last_modified: "2025-12-09T06:50:20.541828+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet/nsc-55-legal.md -->

# NSC-55 — rANS 検証 & 特許姿勢

**ステータス:** 完了 — 決定論的テーブル、生成ツール、ポリシーノートがリポジトリに存在。

- 決定論的テーブル: `codec/rans/tables/rans_seed0.toml` は正準の `SignedRansTablesV1`
  成果物を CC0‑1.0 の献呈、チェックサム、seed、commit hash とともに保存する。
  `tools/rans/gen_tables.py` は `cargo run -p xtask -- codec rans-tables` をラップし、
  将来の更新も再現可能にする。
- 特許姿勢: `PATENTS.md` は rANS のベースライン範囲を記録し、特許化された派生
  （Markov モデルのテーブル切替、escape-code メカニズム、Microsoft の US 11,234,023 B2 の機能）を
  明示的に除外する。テーブル公開はこれらのクレーム要素を含まない。
- 輸出メモ: `EXPORT.md` は、決定論的テーブルが暗号を実装しないため EAR99 (NLR) に分類されることを確認する。
- 検証フック: xtask コマンドは TOML アーティファクトを出力し、
  `tools/rans/gen_tables.py --verify` が内蔵検証器で署名/チェックサムを検証する。
  ベンチマークハーネス（`benchmarks/nsc/rans_compare.py` 参照）は JSON/TOML テーブルを読み込み、
  公開ベクターに対してエンコーダを比較する。
- 設定配線: `[streaming.codec]` は決定論的テーブルのパスとエントロピー
  トグル（`rans_tables_path`, `entropy_mode`, `bundle_width`, `bundle_accel`）を保持する。
  パスを上書きする運用者は同一マニフェスト構造の成果物を提供する必要があり、
  パースは無効なエントロピー設定や欠落ファイルを拒否する。bundled entropy は
  現在幅 2–3 のテーブルを同梱し、範囲外は拒否される。

**次のステップ:** 追加の seed/アーティファクトは `tools/rans/gen_tables.py` で再生成し、
CC0 ヘッダを保持した TOML をコミットしてから追加すること。
