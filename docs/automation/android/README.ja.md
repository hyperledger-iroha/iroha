---
lang: ja
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Android ドキュメント自動化ベースライン (AND5)

ロードマップ項目 AND5 は、AND6（CI & Compliance）を開始する前に
ドキュメント、ローカライズ、公開の自動化が監査可能であることを
求めています。このフォルダは、AND5/AND6 が参照するコマンド、成果物、
証跡レイアウトを記録し、
`docs/source/sdk/android/developer_experience_plan.md` と
`docs/source/sdk/android/parity_dashboard_plan.md` に記載された計画を反映します。

## パイプラインとコマンド

| タスク | コマンド | 期待される成果物 | 注記 |
|------|----------|------------------|------|
| ローカライズスタブ同期 | `python3 scripts/sync_docs_i18n.py`（必要に応じて `--lang <code>` を指定） | `docs/automation/android/i18n/<timestamp>-sync.log` に保存したログと翻訳済みスタブのコミット | `docs/i18n/manifest.json` を翻訳スタブと同期。ログには触れた言語コードとベースラインに取り込んだコミットが記録される。 |
| Norito フィクスチャ + パリティ検証 | `ci/check_android_fixtures.sh`（`python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json` を実行） | 生成された JSON サマリを `docs/automation/android/parity/<stamp>-summary.json` にコピー | `java/iroha_android/src/test/resources` の payload、マニフェストハッシュ、署名済みフィクスチャ長を検証。`artifacts/android/fixture_runs/` にある cadence 証跡と一緒に添付する。 |
| サンプルマニフェストと公開証明 | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]`（テスト + SBOM + provenance を実行） | provenance バンドルのメタデータと `docs/source/sdk/android/samples/` から生成される `sample_manifest.json` を `docs/automation/android/samples/<version>/` に保存 | AND5 サンプルアプリとリリース自動化を連結。生成したマニフェスト、SBOM ハッシュ、provenance ログをベータレビュー用に保存する。 |
| パリティダッシュボードのフィード | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` の後に `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` のスナップショットまたは Grafana のエクスポート JSON を `docs/automation/android/parity/<stamp>-metrics.prom` にコピー | AND5/AND7 のガバナンスが無効送信カウンタとテレメトリ採用状況を検証できるようにダッシュボード計画へ供給する。 |

## 証跡の取得

1. **すべてにタイムスタンプを付ける。** UTC タイムスタンプ
   (`YYYYMMDDTHHMMSSZ`) でファイル名を付け、パリティダッシュボード、
   ガバナンス議事録、公開ドキュメントが同じ実行を参照できるようにします。
2. **コミットを参照する。** 各ログには実行時のコミットハッシュと関連設定
   （例：`ANDROID_PARITY_PIPELINE_METADATA`）を含めます。機密性のため
   伏字が必要な場合は注記を追加し、安全な保管庫へのリンクを付けます。
3. **最小限の文脈を保管する。** 構造化されたサマリ（JSON、`.prom`、`.log`）のみを
   管理します。重い成果物（APK バンドル、スクリーンショット）は
   `artifacts/` またはオブジェクトストレージに置き、署名済みハッシュをログに記録します。
4. **ステータス更新。** `status.md` で AND5 のマイルストーンが進んだら、
   対応ファイル（例：`docs/automation/android/parity/20260324T010203Z-summary.json`）を
   引用し、監査が CI ログを掘らずにベースラインを追跡できるようにします。

このレイアウトに従うことで AND6 が求める「監査可能な docs/automation ベースライン」を
満たし、Android ドキュメントプログラムを公開済み計画と整合させます。
