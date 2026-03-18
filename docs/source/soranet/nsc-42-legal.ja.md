---
lang: ja
direction: ltr
source: docs/source/soranet/nsc-42-legal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 190eadf5c923a138ed3a50bdf38b1ba739e94732a9e2637ee9d5ee0e86ec4b78
source_last_modified: "2025-12-09T06:50:20.541828+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet/nsc-42-legal.md -->

# NSC-42 — コーデック法務サインオフ

**ステータス:** 完了 — リポジトリは NSC-42 意見で求められる法務スキャフォールドを提供する。

- `NOTICE` はエントロピー符号化の特許姿勢（CABAC, trellis, rANS）を文書化し、
  CABAC 配置にはプールライセンス（Via Licensing Alliance / Access Advance）が必要であることを明示する。
- `PATENTS.md` は SEP 義務、trellis のクレーム回避姿勢、rANS のベースライン制約を要約する。
- `EXPORT.md` は CABAC/trellis/rANS の EAR99 分類を記録し、これらの圧縮ツールが
  Category 5 Part 2 の "encryption items" に該当しないことを文書化する。
- `tools/rans/gen_tables.py` と `codec/rans/tables/rans_seed0.toml` は NSC-55 参照の
  決定論的 CC0 ライセンス rANS テーブルを提供し、seed/commit/checksum の provenance を保持する。
- ビルドゲート: `ENABLE_CABAC=1` を環境に設定した場合のみ CABAC を有効化し、
  `norito` クレートに `norito_enable_cabac` cfg を追加する。trellis スキャンは
  クレーム回避設計が公開されるまで無効のまま。
- ランタイムゲート: `[streaming.codec]` は CABAC/trellis 前提条件を強制する
  （`disabled` 以外の cabac モードは build cfg を要求、trellis リストは
  クレーム回避プロファイルが着地するまで空）。同時にエントロピートグル
  （`entropy_mode`, `bundle_width`, `bundle_accel`, `rans_tables_path`）を検証付きで保持し、
  NSC-55 完了後に bundled rANS を安全に展開できるようにする。bundled プロファイルは
  CABAC と同様に `ENABLE_RANS_BUNDLES=1` をビルド時に要求する。bundled entropy は
  テーブル幅 2–3 に固定され、範囲外はパース時に拒否される。

**フォローアップ:** なし — NSC-55 がこれらの成果物を継承し、決定論的テーブルの検証に集中する。
