---
lang: ja
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# SDK バインディングとフィクスチャのガバナンス

ロードマップの WP1-E は「docs/bindings」をクロス言語バインディングの状態を
管理する正規の場所として指定しています。本ドキュメントは、バインディングの
在庫、再生成コマンド、ドリフト防止策、証跡の格納先を記録し、GPU パリティの
ゲート（WP1-E/F/G）とクロス SDK のケイデンス評議会が参照できる単一の基準を
提供します。

## 共有ガードレール
- **カノニカル・プレイブック：** `docs/source/norito_binding_regen_playbook.md` に
  Android/Swift/Python/将来のバインディング向けのローテーション方針、必要証跡、
  エスカレーション手順を記載しています。
- **Norito スキーマのパリティ：** `scripts/check_norito_bindings_sync.py`
  （`scripts/check_norito_bindings_sync.sh` 経由で実行、CI では
  `ci/check_norito_bindings_sync.sh` がゲート）により、Rust/Java/Python の
  スキーマ成果物がずれた場合にビルドを停止します。
- **ケイデンス監視：** `scripts/check_fixture_cadence.py` が
  `artifacts/*_fixture_regen_state.json` を読み取り、火/金（Android/Python）と
  水（Swift）のウィンドウを強制し、ロードマップのゲートに監査可能な
  タイムスタンプを提供します。

## バインディング・マトリクス

| バインディング | エントリポイント | フィクスチャ/再生成コマンド | ドリフト防止 | 証跡 |
|----------------|------------------|-----------------------------|---------------|------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh`（必要に応じて `SWIFT_FIXTURE_ARCHIVE`）→ `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## バインディング詳細

### Android (Java)
Android SDK は `java/iroha_android/` にあり、`scripts/android_fixture_regen.sh`
が生成するカノニカルな Norito フィクスチャを使用します。この helper は Rust
ツールチェーンから新しい `.norito` をエクスポートし、
`artifacts/android_fixture_regen_state.json` を更新し、
`scripts/check_fixture_cadence.py` とガバナンスダッシュボードが利用する
ケイデンス・メタデータを記録します。`scripts/check_android_fixtures.py`
（`ci/check_android_fixtures.sh` にも接続）と `java/iroha_android/run_tests.sh`
がドリフトを検出し、JNI バインディング、WorkManager のキュー再生、StrongBox
フォールバックを検証します。ローテーション証跡、失敗メモ、再実行ログは
`artifacts/android/fixture_runs/` に保存されます。

### Swift (macOS/iOS)
`IrohaSwift/` は `scripts/swift_fixture_regen.sh` を通じて同じ Norito
ペイロードを反映します。スクリプトはローテーション所有者、ケイデンスラベル、
ソース（`live`/`archive`）を `artifacts/swift_fixture_regen_state.json` に記録し、
ケイデンスチェックへ渡します。`scripts/swift_fixture_archive.py` は Rust 生成
アーカイブの取り込みを可能にし、`scripts/check_swift_fixtures.py` と
`ci/check_swift_fixtures.sh` がバイトレベルのパリティと SLA の年齢制限を
強制します。`scripts/swift_fixture_regen.sh` は手動ローテーション用の
`SWIFT_FIXTURE_EVENT_TRIGGER` をサポートします。エスカレーションフロー、KPI、
ダッシュボードは `docs/source/swift_parity_triage.md` と
`docs/source/sdk/swift/` のケイデンスブリーフに記載されています。

### Python
Python クライアント（`python/iroha_python/`）は Android のフィクスチャを共有します。
`scripts/python_fixture_regen.sh` を実行すると最新の `.norito` ペイロードを取得し、
`python/iroha_python/tests/fixtures/` を更新し、最初のロードマップ後ローテーションで
`artifacts/python_fixture_regen_state.json` にケイデンス・メタデータを出力します。
`scripts/check_python_fixtures.py` と `python/iroha_python/scripts/run_checks.sh`
が pytest/mypy/ruff とフィクスチャのパリティをローカルおよび CI でゲートします。
エンドツーエンドのドキュメント（`docs/source/sdk/python/…`）と再生成プレイブックが、
Android オーナーとローテーションを調整する方法を説明します。

### JavaScript
`javascript/iroha_js/` はローカルの `.norito` に依存しませんが、WP1-E は GPU CI
レーンが完全な provenance を継承できるよう、リリース証跡を追跡します。各リリースは
`npm run release:provenance`（`javascript/iroha_js/scripts/record-release-provenance.mjs`
が基盤）で provenance を記録し、`scripts/js_sbom_provenance.sh` で SBOM バンドルを
生成・署名し、`scripts/js_signed_staging.sh` で署名付きステージングを実行し、
`javascript/iroha_js/scripts/verify-release-tarball.mjs` でレジストリアーティファクトを
検証します。生成されたメタデータは `artifacts/js-sdk-provenance/`,
`artifacts/js/npm_staging/`, `artifacts/js/sbom/`, `artifacts/js/verification/` に保存され、
ロードマップ JS5/JS6 および WP1-F の実行に向けた決定論的証跡となります。
`docs/source/sdk/js/` の公開プレイブックが自動化全体を結び付けます。
