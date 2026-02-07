---
lang: ja
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android リリース チェックリスト (AND6)

このチェックリストは、**AND6 — CI およびコンプライアンスの強化** ゲートをキャプチャします。
`roadmap.md` (§優先度 5)。 Android SDK リリースを Rust と連携させます。
CI ジョブ、コンプライアンス成果物、
デバイスラボの証拠、および GA の前に添付する必要がある出所バンドル、
LTS (ホットフィックス トレイン) が前進します。

このドキュメントは以下と組み合わせて使用してください。

- `docs/source/android_support_playbook.md` — リリース カレンダー、SLA、および
  エスカレーションツリー。
- `docs/source/android_runbook.md` — 日常の運用手順書。
- `docs/source/compliance/android/and6_compliance_checklist.md` — レギュレーター
  アーティファクトの在庫。
- `docs/source/release_dual_track_runbook.md` — デュアルトラックリリースガバナンス。

## 1. ステージゲートの概要

|ステージ |必要なゲート |証拠 |
|------|----------------|----------|
| **T−7 日 (凍結前)** | 14 日間毎晩 `ci/run_android_tests.sh` グリーン。 `ci/check_android_fixtures.sh`、`ci/check_android_samples.sh`、および `ci/check_android_docs_i18n.sh` は合格します。 lint/依存関係スキャンがキューに入れられました。 | Buildkite ダッシュボード、フィクスチャ差分レポート、サンプル スクリーンショット スナップショット。 |
| **T−3 日 (RC プロモーション)** |デバイスラボの予約が確認されました。 StrongBox 構成証明 CI 実行 (`scripts/android_strongbox_attestation_ci.sh`); Robolectric/計装スイートは、スケジュールされたハードウェア上で実行されます。 `./gradlew lintRelease ktlintCheck detekt dependencyGuard` きれいです。 |デバイス マトリックス CSV、構成証明バンドル マニフェスト、Gradle レポートは `artifacts/android/lint/<version>/` の下にアーカイブされます。 |
| **T−1 日 (行く/行かない)** |テレメトリ編集ステータス バンドルが更新されました (`scripts/telemetry/check_redaction_status.py --write-cache`)。 `and6_compliance_checklist.md` に従って更新されたコンプライアンス アーティファクト。来歴リハーサルが完了しました (`scripts/android_sbom_provenance.sh --dry-run`)。 | `docs/source/compliance/android/evidence_log.csv`、テレメトリ ステータス JSON、来歴ドライラン ログ。 |
| **T0 (GA/LTS カットオーバー)** | `scripts/publish_android_sdk.sh --dry-run` が完了しました。来歴 + SBOM 署名済み。リリースチェックリストはエクスポートされ、ゴー/ノーゴー議事録に添付されます。 `ci/sdk_sorafs_orchestrator.sh` スモークジョブグリーン。 | RFC 添付ファイル、Sigstore バンドル、`artifacts/android/` の下の採用アーティファクトをリリースします。 |
| **T+1 日 (カットオーバー後)** |ホットフィックスの準備が完了していることを確認しました (`scripts/publish_android_sdk.sh --validate-bundle`)。ダッシュボードの差分を確認しました (`ci/check_android_dashboard_parity.sh`)。証拠パケットが `status.md` にアップロードされました。 |ダッシュボードの差分エクスポート、`status.md` エントリへのリンク、アーカイブされたリリース パケット。 |

## 2. CI および品質ゲート マトリックス|ゲート |コマンド / スクリプト |メモ |
|------|----------------------|------|
|単体テスト + 統合テスト | `ci/run_android_tests.sh` (`ci/run_android_tests.sh` をラップ) | `artifacts/android/tests/test-summary.json` + テスト ログを出力します。 Norito コーデック、キュー、StrongBox フォールバック、および Torii クライアント ハーネス テストが含まれます。毎晩、タグ付け前に必要です。 |
|フィクスチャのパリティ | `ci/check_android_fixtures.sh` (`scripts/check_android_fixtures.py` をラップ) |再生成された Norito フィクスチャが Rust の正規セットと一致することを確認します。ゲートが失敗したときに JSON の差分を添付します。 |
|サンプルアプリ | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` をビルドし、`scripts/android_sample_localization.py` 経由でローカライズされたスクリーンショットを検証します。 |
|ドキュメント/I18N | `ci/check_android_docs_i18n.sh` | Guards README + ローカライズされたクイックスタート。ドキュメントの編集がリリース ブランチに到達したら、再度実行します。 |
|ダッシュボードの同等性 | `ci/check_android_dashboard_parity.sh` | CI/エクスポートされたメトリクスが Rust のメトリクスと一致していることを確認します。 T+1 検証時に必要です。 |
| SDK採用スモーク | `ci/sdk_sorafs_orchestrator.sh` |現在の SDK を使用してマルチソースの Sorafs オーケストレーター バインディングを実行します。ステージングされたアーティファクトをアップロードする前に必要です。 |
|証明書の検証 | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | StrongBox/TEE 構成証明バンドルを `artifacts/android/attestation/**` の下に集約します。概要を GA パケットに添付します。 |
|デバイスラボのスロット検証 | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` |リリースパケットに証拠を添付する前にインスツルメンテーションバンドルを検証します。 CI は、`fixtures/android/device_lab/slot-sample` (テレメトリ/アテステーション/キュー/ログ + `sha256sum.txt`) のサンプル スロットに対して実行されます。 |

> **ヒント:** これらのジョブを `android-release` Buildkite パイプラインに追加すると、
> フリーズ週間は、リリース ブランチ チップを使用してすべてのゲートを自動的に再実行します。

統合された `.github/workflows/android-and6.yml` ジョブは lint を実行します。
すべての PR/プッシュでのテストスイート、アテステーションサマリー、およびデバイスラボのスロットチェック
Android ソースにアクセスし、`artifacts/android/{lint,tests,attestation,device_lab}/` で証拠をアップロードします。

## 3. lint と依存関係のスキャン

リポジトリのルートから `scripts/android_lint_checks.sh --version <semver>` を実行します。の
スクリプトが実行されます:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- レポートと依存関係ガードの出力は以下にアーカイブされます。
  リリース用の `artifacts/android/lint/<label>/` および `latest/` シンボリックリンク
  パイプライン。
- lint の検出結果に失敗した場合は、修復するか、リリースへのエントリが必要です
  受け入れられたリスクを文書化した RFC (リリース エンジニアリング + プログラムによって承認)
  リード）。
- `dependencyGuardBaseline` は依存関係ロックを再生成します。デフを取り付けます
  ゴー/ノーゴーパケットに。

## 4. デバイス ラボと StrongBox の範囲

1. で参照されている容量トラッカーを使用して Pixel + Galaxy デバイスを予約します。
   `docs/source/compliance/android/device_lab_contingency.md`。リリースをブロックする
   可用性が 70% 未満の場合。
2. `scripts/android_strongbox_attestation_ci.sh --report \ を実行します。
   artifacts/android/attestation/` を使用して、構成証明レポートを更新します。
3. インストルメンテーション マトリックスを実行します (デバイス内のスイート/ABI リストを文書化します)
   トラッカー）。再試行が成功した場合でも、失敗をインシデント ログに記録します。
4. Firebase Test Lab へのフォールバックが必要な場合はチケットを提出します。チケットをリンクする
   以下のチェックリストに記載されています。

## 5. コンプライアンスとテレメトリのアーティファクト- EU の場合は `docs/source/compliance/android/and6_compliance_checklist.md` に従ってください
  そしてJPの投稿。アップデート `docs/source/compliance/android/evidence_log.csv`
  ハッシュ + Buildkite ジョブ URL を使用します。
- テレメトリ編集証拠を更新します
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`。
  結果の JSON を以下に保存します
  `artifacts/android/telemetry/<version>/status.json`。
- からのスキーマ diff 出力を記録します。
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Rust エクスポーターと同等であることを証明するため。

## 6. 来歴、SBOM、および出版

1. パブリッシュ パイプラインをドライランします。

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore 来歴を生成します。

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` を添付し、署名します
   `checksums.sha256` をリリース RFC に追加します。
4. 実際の Maven リポジトリに昇格するときに、再実行します。
   `scripts/publish_android_sdk.sh` (`--dry-run` なし)、コンソールをキャプチャします
   ログを作成し、結果のアーティファクトを `artifacts/android/maven/<semver>` にアップロードします。

## 7. 送信パケットのテンプレート

すべての GA/LTS/ホットフィックス リリースには次のものが含まれている必要があります。

1. **完成したチェックリスト** — このファイルの表をコピーし、各項目にチェックを入れてリンクします
   サポートするアーティファクト (Buildkite の実行、ログ、ドキュメントの差分) まで。
2. **デバイス ラボの証拠** — 認証レポートの概要、予約ログ、および
   不測の事態によるアクティベーション。
3. **テレメトリ パケット** — 編集ステータス JSON、スキーマ差分、へのリンク
   `docs/source/sdk/android/telemetry_redaction.md` 更新 (存在する場合)。
4. **コンプライアンス アーティファクト** — コンプライアンス フォルダに追加/更新されたエントリ
   加えて、更新された証拠ログ CSV も含まれます。
5. **来歴バンドル** — SBOM、Sigstore 署名、および `checksums.sha256`。
6. **リリース概要** — `status.md` に添付されている 1 ページの概要をまとめたもの
   上記（日付、バージョン、免除されたゲートのハイライト）。

パケットを `artifacts/android/releases/<version>/` に格納し、参照します
`status.md` およびリリース RFC に記載されています。

- `scripts/run_release_pipeline.py --publish-android-sdk ...` 自動的に
  最新の lint アーカイブ (`artifacts/android/lint/latest`) と
  コンプライアンスの証拠は `artifacts/android/releases/<version>/` にログインするため、
  送信パケットには常に正規の場所があります。

---

**リマインダー:** 新しい CI ジョブ、コンプライアンス成果物、
またはテレメトリ要件が追加されます。ロードマップ項目 AND6 は、
チェックリストと関連する自動化は、2 回連続のリリースでも安定していることが証明されています
電車。