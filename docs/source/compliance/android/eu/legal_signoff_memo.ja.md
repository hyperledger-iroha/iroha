---
lang: ja
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EU 法的承認メモのテンプレート

このメモは、ロードマップ項目 **AND6** によって要求される法的レビューを記録します。
EU (ETSI/GDPR) アーティファクト パケットが規制当局に提出されます。弁護士はクローンを作成する必要があります
リリースごとにこのテンプレートを作成し、以下のフィールドに値を入力し、署名されたコピーを保存します
メモで参照されている不変のアーティファクトと一緒に。

## 概要

- **リリース/トレイン:** `<e.g., 2026.1 GA>`
- **レビュー日:** `<YYYY-MM-DD>`
- **顧問/査読者:** `<name + organisation>`
- **範囲:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **関連チケット:** `<governance or legal issue IDs>`

## アーティファクトチェックリスト

|アーティファクト | SHA-256 |場所/リンク |メモ |
|----------|-----------|------|------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + ガバナンス アーカイブ |リリース識別子と脅威モデルの調整を確認します。 |
| `gdpr_dpia_summary.md` | `<hash>` |同じディレクトリ/ローカリゼーションミラー |リダクションポリシー参照が `sdk/android/telemetry_redaction.md` と一致することを確認します。 |
| `sbom_attestation.md` | `<hash>` |証拠バケット内の同じディレクトリ + 署名バンドル | CycloneDX + 来歴署名を検証します。 |
|証拠ログ行 | `<hash>` | `docs/source/compliance/android/evidence_log.csv` |行番号 `<n>` |
|デバイスラボの緊急事態バンドル | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` |このリリースに関連付けられたフェイルオーバーのリハーサルを確認します。 |

> パケットにさらに多くのファイル（プライバシーなど）が含まれる場合は、追加の行を添付します。
> 付録または DPIA 翻訳)。すべてのアーティファクトはその不変を参照する必要があります
> ターゲットとそれを生成した Buildkite ジョブをアップロードします。

## 調査結果と例外

- `None.` *(残留リスクをカバーし、補償する箇条書きリストに置き換えます)
  コントロール、または必要なフォローアップアクション。)*

## 承認

- **決定:** `<Approved / Approved with conditions / Blocked>`
- **署名/タイムスタンプ:** `<digital signature or email reference>`
- **フォローアップ所有者:** `<team + due date for any conditions>`

最終メモをガバナンス証拠バケットにアップロードし、SHA-256 を
`docs/source/compliance/android/evidence_log.csv` にアップロード パスをリンクします。
`status.md`。決定が「ブロック」の場合は、AND6 ステアリングにエスカレーションします。
ロードマップのホットリストと
デバイスラボの緊急事態ログ。