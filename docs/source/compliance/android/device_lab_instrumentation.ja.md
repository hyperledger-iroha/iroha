---
lang: ja
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab 計測フック (AND6)

このリファレンスは、ロードマップ アクション「残りのデバイス ラボのステージング」を終了します。
AND6 キックオフに先立って計装がフックされます。」各予約がどのように行われるかを説明します
device-lab スロットは、テレメトリ、キュー、および認証アーティファクトをキャプチャする必要があります。
AND6 コンプライアンス チェックリスト、証拠ログ、ガバナンス パケットは同じものを共有します
決定的なワークフロー。このメモを予約手順と組み合わせてください
(`device_lab_reservation.md`) およびリハーサルを計画する際のフェールオーバー Runbook。

## 目標と範囲

- **決定的な証拠** – すべての計測出力は以下に存在します。
  SHA-256 を使用した `artifacts/android/device_lab/<slot-id>/` マニフェストがあるため、監査人
  プローブを再実行せずにバンドルの差分を確認できます。
- **スクリプトファーストのワークフロー** – 既存のヘルパーを再利用します
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`、`scripts/android_override_tool.sh`)
  カスタムの adb コマンドの代わりに。
- **チェックリストは同期を保ちます** – すべての実行で、このドキュメントが参照されます。
  AND6 準拠チェックリストにアーティファクトを追加します。
  `docs/source/compliance/android/evidence_log.csv`。

## アーティファクトのレイアウト

1. 予約チケットに一致する一意のスロット ID を選択します。
   `2026-05-12-slot-a`。
2. 標準ディレクトリをシードします。

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. すべてのコマンド ログを一致するフォルダー内に保存します (例:
   `telemetry/status.ndjson`、`attestation/pixel8pro.log`)。
4. スロットが閉じたら、SHA-256 マニフェストをキャプチャします。

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## インストルメンテーション マトリックス

|フロー |コマンド |出力場所 |メモ |
|------|-----------|------|------|
|テレメトリ編集 + ステータス バンドル | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`、`telemetry/status.log` |スロットの開始時と終了時に実行します。 CLI stdout を `status.log` に接続します。 |
|保留中のキュー + カオスの準備 | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`、`queue/*.json`、`queue/*.sha256` | `readiness/labs/telemetry_lab_01.md` のミラーシナリオ D;スロット内のすべてのデバイスの環境変数を拡張します。 |
|元帳ダイジェストを上書きする | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` |アクティブなオーバーライドがない場合でも必須です。ゼロ状態を証明します。 |
| StrongBox / TEE 認証 | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` |予約されたデバイスごとに繰り返します (`android_strongbox_device_matrix.md` の名前と一致します)。 |
| CI ハーネス構成証明の回帰 | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | CI がアップロードするのと同じ証拠をキャプチャします。対称性を保つために手動実行に含めます。 |
| lint / 依存関係のベースライン | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`、`logs/lint.log` |フリーズウィンドウごとに 1 回実行します。コンプライアンスパケットの概要を引用します。 |

## 標準スロット手順1. **フライト前 (T-24h)** – 予約航空券がこれを参照していることを確認します。
   ドキュメントを編集し、デバイス マトリックス エントリを更新し、アーティファクト ルートをシードします。
2. **スロット中**
   - 最初にテレメトリ バンドル + キュー エクスポート コマンドを実行します。パス
     `--note <ticket>` から `ci/run_android_telemetry_chaos_prep.sh` なので、ログ
     インシデント ID を参照します。
   - デバイスごとに構成証明スクリプトをトリガーします。ハーネスに異常が発生すると、
     `.zip`、それをアーティファクトルートにコピーし、出力された Git SHA を記録します。
     スクリプトの終わり。
   - CI の場合でも、オーバーライドされたサマリー パスを使用して `make android-lint` を実行します。
     すでに実行されました。監査人はスロットごとのログを期待します。
3. **実行後**
   - スロット内に `sha256sum.txt` および `README.md` (自由形式のメモ) を生成します
     実行したコマンドをまとめたフォルダー。
   - `docs/source/compliance/android/evidence_log.csv` に行を追加します。
     スロット ID、ハッシュ マニフェスト パス、Buildkite 参照 (存在する場合)、および最新
     予約カレンダーのエクスポートからのデバイス ラボのキャパシティの割合。
   - `_android-device-lab` チケット、AND6 のスロット フォルダーをリンクします。
     チェックリスト、および `docs/source/android_support_playbook.md` リリース レポート。

## 障害の処理とエスカレーション

- コマンドが失敗した場合は、`logs/` で stderr 出力をキャプチャし、次の手順に従います。
  `device_lab_reservation.md` §6 のエスカレーション ラダー。
- キューまたはテレメトリの不足がある場合は、オーバーライド ステータスをすぐに記録する必要があります。
  `docs/source/sdk/android/telemetry_override_log.md` とスロット ID を参照します
  そうすることで、ガバナンスが訓練を追跡できるようになります。
- 証明の回帰は次の場所に記録する必要があります。
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  失敗したデバイスのシリアルと上記で記録したバンドル パスを使用します。

## レポートチェックリスト

スロットを完了としてマークする前に、次の参照が更新されていることを確認してください。

- `docs/source/compliance/android/and6_compliance_checklist.md` — マークを付けます
  インストルメンテーション行が完了し、スロット ID をメモします。
- `docs/source/compliance/android/evidence_log.csv` — 次のエントリを追加/更新します
  スロットのハッシュと容量の読み取り値。
- `_android-device-lab` チケット — アーティファクト リンクと Buildkite ジョブ ID を添付します。
- `status.md` — 次回の Android 準備ダイジェストに簡単なメモを含めます。
  ロードマップの読者は、どのスロットが最新の証拠を生み出したかを知っています。

このプロセスに従うと、AND6 の「デバイス ラボ + 計測フック」が維持されます。
マイルストーンは監査可能であり、予約、実行、
そして報告すること。