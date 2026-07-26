---
lang: ja
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox 認証証拠 — 日本展開

|フィールド |値 |
|------|------|
|評価ウィンドウ | 2026-02-10 – 2026-02-12 |
|アーティファクトの場所 | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` によるバンドル形式) |
|キャプチャツール | `scripts/android_keystore_attestation.sh`、`scripts/android_strongbox_attestation_ci.sh`、`scripts/android_strongbox_attestation_report.py` |
|査読者 |ハードウェア ラボ リード、コンプライアンスおよび法務 (日本) |

## 1. キャプチャ手順

1. StrongBox マトリックスにリストされている各デバイスで、チャレンジを生成し、証明書バンドルをキャプチャします。
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. バンドル メタデータ (`result.json`、`chain.pem`、`challenge.hex`、`alias.txt`) を証拠ツリーにコミットします。
3. CI ヘルパーを実行して、すべてのバンドルをオフラインで再検証します。
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. デバイスの概要 (2026-02-12)

|デバイスタグ |モデル / StrongBox |バンドルパス |結果 |メモ |
|-----------|---------------------|---------------|----------|----------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ 合格 (ハードウェアサポート) |課題はバウンド、OS パッチ 2025-03-05。 |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ 合格 |主要な CI レーン候補。仕様内の温度|
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ 合格 (再試験) | USB-Cハブを交換しました。 Buildkite `android-strongbox-attestation#221` は通過するバンドルをキャプチャしました。 |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ 合格 | Knox 認証プロファイルは 2026 年 2 月 9 日にインポートされました。 |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ 合格 | Knox 認証プロファイルがインポートされました。 CI レーンが緑色になりました。 |

デバイスタグは `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` にマップされます。

## 3. 査読者のチェックリスト

- [x] `result.json` が `strongbox_attestation: true` を示し、信頼されたルートへの証明書チェーンが表示されていることを確認します。
- [x] チャレンジ バイトが一致することを確認します。Buildkite は `android-strongbox-attestation#219` (初期スイープ) および `#221` (Pixel 8 Pro 再テスト + S24 キャプチャ) を実行します。
- [x] ハードウェア修正後に Pixel 8 Pro キャプチャを再実行します（所有者: ハードウェア ラボ リーダー、2026 年 2 月 13 日に完了）。
- [x] Knox プロファイルの承認が到着したら、Galaxy S24 のキャプチャを完了します (所有者: Device Lab Ops、2026 年 2 月 13 日に完了)。

## 4. 配布

- この概要と最新のレポート テキスト ファイルをパートナーのコンプライアンス パケットに添付します (FISC チェックリスト §データ常駐)。
- 規制当局の監査に対応する際の参照バンドル パス。未加工の証明書を暗号化されたチャネルの外に送信しないでください。

## 5. 変更ログ

|日付 |変更 |著者 |
|------|--------|----------|
| 2026-02-12 |初期JPバンドル攻略＋レポート。 |デバイス ラボ運用 |