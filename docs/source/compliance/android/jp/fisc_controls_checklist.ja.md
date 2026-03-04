---
lang: ja
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC セキュリティ管理チェックリスト — Android SDK

|フィールド |値 |
|------|------|
|バージョン | 0.1 (2026-02-12) |
|範囲 |日本の金融導入で使用される Android SDK + オペレーター ツール |
|オーナー |コンプライアンスおよび法務 (Daniel Park)、Android プログラム リード |

## 制御マトリックス

| FISC制御 |実装の詳細 |証拠/参考資料 |ステータス |
|--------------|----------------------|----------------------|----------|
| **システム構成の整合性** | `ClientConfig` は、マニフェストのハッシュ、スキーマ検証、および読み取り専用のランタイム アクセスを強制します。構成のリロードが失敗すると、Runbook に記載されている `android.telemetry.config.reload` イベントが生成されます。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2。 | ✅ 実装済み |
| **アクセス制御と認証** | SDK は Torii TLS ポリシーと `/v1/pipeline` 署名付きリクエストを尊重します。オペレーターのワークフローは、エスカレーションおよび署名された Norito アーティファクトによるゲートのオーバーライドについてサポート プレイブック §4–5 を参照します。 | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (ワークフローをオーバーライド)。 | ✅ 実装済み |
| **暗号キー管理** | StrongBox が推奨するプロバイダー、構成証明検証、およびデバイス マトリックス カバレッジにより、KMS への準拠が保証されます。アテステーション ハーネスの出力は `artifacts/android/attestation/` の下にアーカイブされ、準備マトリックスで追跡されます。 | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`。 | ✅ 実装済み |
| **ログ、監視、保存** |テレメトリ編集ポリシーは、機密データをハッシュし、デバイス属性をバケット化し、保持 (7/30/90/365 日の期間) を強制します。サポート プレイブック §8 では、ダッシュボードのしきい値について説明しています。 `telemetry_override_log.md` に記録されたオーバーライド。 | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`。 | ✅ 実装済み |
| **運用と変更管理** | GA カットオーバー手順 (サポート プレイブック §7.2) と `status.md` により、トラック リリースの準備状況が更新されます。 `docs/source/compliance/android/eu/sbom_attestation.md` 経由でリンクされたリリース証拠 (SBOM、Sigstore バンドル)。 | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`。 | ✅ 実装済み |
| **インシデント対応とレポート** | Playbook は、重大度マトリックス、SLA 応答ウィンドウ、およびコンプライアンス通知ステップを定義します。テレメトリのオーバーライド + カオス リハーサルにより、パイロット前の再現性が保証されます。 | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`。 | ✅ 実装済み |
| **データの常駐/ローカリゼーション** | JP 導入用のテレメトリ コレクターは、承認された東京リージョンで実行されます。 StrongBox 証明書バンドルはリージョン内に保存され、パートナー チケットから参照されます。ローカリゼーション プランにより、ベータ版 (AND5) より前にドキュメントを日本語で利用できるようになります。 | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`。 | 🈺 In Progress (localisation ongoing) |

## 査読者のメモ

- 規制対象パートナーのオンボーディング前に、Galaxy S23/S24 のデバイス マトリックス エントリを確認します (準備状況ドキュメントの行 `s23-strongbox-a`、`s24-strongbox-a` を参照)。
- JP 展開のテレメトリ コレクターが、DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) で定義されているのと同じ保持/上書きロジックを強制するようにします。
- 銀行パートナーがこのチェックリストを確認したら、外部監査人からの確認を取得します。