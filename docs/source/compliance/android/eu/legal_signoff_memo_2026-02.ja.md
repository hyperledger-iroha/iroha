---
lang: ja
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EU 法的承認メモ — 2026.1 GA (Android SDK)

## 概要

- **リリース / トレイン:** 2026.1 GA (Android SDK)
- **レビュー日:** 2026-04-15
- **顧問/査読者:** Sofia Martins — コンプライアンスと法務
- **範囲:** ETSI EN 319 401 セキュリティ目標、GDPR DPIA 概要、SBOM 証明書、AND6 デバイスラボの緊急事態証拠
- **関連チケット:** `_android-device-lab` / AND6-DR-202602、AND6 ガバナンス トラッカー (`GOV-AND6-2026Q1`)

## アーティファクトチェックリスト

|アーティファクト | SHA-256 |場所/リンク |メモ |
|----------|-----------|------|------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA リリース識別子と脅威モデル デルタ (Torii NRPC 追加) に一致します。 |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | AND7 テレメトリ ポリシー (`docs/source/sdk/android/telemetry_redaction.md`) を参照します。 |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore バンドル (`android-sdk-release#4821`)。 | CycloneDX + 来歴を確認。 Buildkite ジョブ `android-sdk-release#4821` と一致します。 |
|証拠ログ | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (行 `android-device-lab-failover-20260220`) |ログにキャプチャされたバンドル ハッシュ + 容量スナップショット + メモ エントリを確認します。 |
|デバイスラボの緊急事態バンドル | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json` から取得したハッシュ。チケット AND6-DR-202602 には、法務/コンプライアンス部門への引き継ぎが記録されています。 |

## 調査結果と例外

- 障害となる問題は確認されていません。アーティファクトは ETSI/GDPR 要件に準拠しています。 AND7 テレメトリの同等性は DPIA の概要に記載されており、追加の緩和策は必要ありません。
- 推奨事項: スケジュールされた DR-2026-05-Q2 ドリル (チケット AND6-DR-202605) を監視し、次のガバナンス チェックポイントの前に結果のバンドルを証拠ログに追加します。

## 承認

- **決定:** 承認
- **署名/タイムスタンプ:** _Sofia Martins (ガバナンスポータル経由でデジタル署名、2026-04-15 14:32 UTC)_
- **フォローアップ所有者:** Device Lab Ops (DR-2026-05-Q2 証拠バンドルを 2026 年 5 月 31 日より前に提供)