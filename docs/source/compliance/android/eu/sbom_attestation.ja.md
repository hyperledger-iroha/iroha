---
lang: ja
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM と出所証明書 — Android SDK

|フィールド |値 |
|------|------|
|範囲 | Android SDK (`java/iroha_android`) + サンプル アプリ (`examples/android/*`) |
|ワークフロー所有者 |リリースエンジニアリング (Alexei Morozov) |
|最終検証日 | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. 生成ワークフロー

ヘルパー スクリプト (AND6 自動化用に追加) を実行します。

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

スクリプトは次のことを実行します。

1. `ci/run_android_tests.sh` および `scripts/check_android_samples.sh` を実行します。
2. `examples/android/` の下で Gradle ラッパーを呼び出して、CycloneDX SBOM を構築します。
   付属の `:android-sdk`、`:operator-console`、および `:retail-wallet`
   `-PversionName`。
3. 各 SBOM を正規名で `artifacts/android/sbom/<sdk-version>/` にコピーします
   (`iroha-android.cyclonedx.json` など)。

## 2. 出所と署名

同じスクリプトがすべての SBOM に `cosign sign-blob --bundle <file>.sigstore --yes` で署名します
そして、宛先ディレクトリに `checksums.txt` (SHA-256) を発行します。 `COSIGN`を設定します
バイナリが `$PATH` の外にある場合は環境変数。スクリプトが終了したら、
バンドル/チェックサム パスと Buildkite 実行 ID を記録します。
`docs/source/compliance/android/evidence_log.csv`。

## 3. 検証

公開された SBOM を確認するには:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

出力 SHA を `checksums.txt` にリストされている値と比較します。レビュー担当者はまた、依存関係の差分が意図的であることを確認するために、SBOM を以前のリリースと比較します。

## 4. 証拠のスナップショット (2026-02-11)

|コンポーネント | SBOM | SHA-256 | Sigstore バンドル |
|----------|------|----------|------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` バンドルは SBOM の横に保存されています |
|オペレーターコンソールのサンプル | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
|リテールウォレットのサンプル | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite 実行 `android-sdk-release#4821` からキャプチャされたハッシュ。上記の検証コマンドで再現されます。)*

## 5. 優れた作品

- GA の前に、リリース パイプライン内の SBOM + 署名ステップを自動化します。
- AND6 がチェックリストに完了のマークを付けたら、SBOM をパブリック アーティファクト バケットにミラーリングします。
- ドキュメントと連携して、パートナー向けリリース ノートから SBOM のダウンロード場所をリンクします。