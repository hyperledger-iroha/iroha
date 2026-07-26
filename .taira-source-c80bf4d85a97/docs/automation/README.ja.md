---
lang: ja
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# ドキュメント自動化のベースライン

このディレクトリは、AND5/AND6（Android Developer Experience + Release
Readiness）や DA-1（Data-Availability の脅威モデル自動化）のロードマップ項目が
監査可能なドキュメント証跡を求める際に参照する自動化の範囲をまとめたものです。
コマンド参照と期待される成果物をリポジトリ内に配置しておくことで、CI パイプラインや
ダッシュボードがオフラインでもコンプライアンスレビューの前提条件を確保できます。

## ディレクトリ構成

| パス | 目的 |
|------|------|
| `docs/automation/android/` | Android ドキュメントとローカライズの自動化ベースライン（AND5）。i18n スタブ同期ログ、パリティサマリ、AND6 承認前に必要な SDK 公開証跡を含む。 |
| `docs/automation/da/` | `cargo xtask da-threat-model-report` と夜間ドキュメント更新で参照される Data-Availability 脅威モデル自動化の出力。 |

各サブディレクトリでは、証跡を生成するコマンドと、チェックイン対象のファイル構成
（通常は JSON サマリ、ログ、またはマニフェスト）を記載しています。自動化実行によって
公開ドキュメントが重要に変化した場合、担当チームは該当フォルダに新しい成果物を置き、
status/roadmap の該当エントリからコミットを参照します。

## 利用手順

1. **自動化を実行**：サブディレクトリの README に記載されたコマンドを実行します
   （例：`ci/check_android_fixtures.sh`、`cargo xtask da-threat-model-report`）。
2. **生成された JSON/ログ成果物をコピー**：`artifacts/…` から該当する
   `docs/automation/<program>/…` に移し、監査がガバナンスの議事録と突合できるよう
   ISO-8601 のタイムスタンプをファイル名に付けます。
3. **コミットを参照**：ロードマップのゲートを閉じる際に `status.md`/`roadmap.md` で
   コミットを参照し、レビュー担当者が使用した自動化ベースラインを確認できるようにします。
4. **ファイルは軽量に**：構造化メタデータ、マニフェスト、サマリを想定し、大きな
   バイナリの塊は避けます。大容量のダンプはオブジェクトストレージに置き、署名付き参照を
   ここに記録してください。

これらの自動化ノートを一元化することで、AND6 が要求する
「監査可能な docs/automation ベースラインの用意」を満たし、DA 脅威モデルの夜間レポートと
手動スポットチェックのための確定的な置き場を提供します。
