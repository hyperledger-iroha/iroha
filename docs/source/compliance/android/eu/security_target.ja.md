---
lang: ja
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK セキュリティ ターゲット — ETSI EN 319 401 アラインメント

|フィールド |値 |
|------|------|
|ドキュメントのバージョン | 0.1 (2026-02-12) |
|範囲 | Android SDK (`java/iroha_android/` のクライアント ライブラリとサポート スクリプト/ドキュメント) |
|オーナー |コンプライアンスと法務 (ソフィア・マーティンズ) |
|査読者 | Android プログラム リード、リリース エンジニアリング、SRE ガバナンス |

## 1. TOE の説明

評価対象 (TOE) は、Android SDK ライブラリ コード (`java/iroha_android/src/main/java`)、その構成サーフェス (`ClientConfig` + Norito インジェスト)、およびマイルストーン AND2/AND6/AND7 の `roadmap.md` で参照される運用ツールで構成されます。

主なコンポーネント:

1. **構成の取り込み** — `ClientConfig` は、生成された `iroha_config` マニフェストからの Torii エンドポイント、TLS ポリシー、再試行、およびテレメトリ フックをスレッドし、初期化後の不変性を強制します (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`)。
2. **キー管理 / StrongBox** — ハードウェアによる署名は、`SystemAndroidKeystoreBackend` および `AttestationVerifier` を介して実装され、ポリシーは `docs/source/sdk/android/key_management.md` に文書化されています。証明書のキャプチャ/検証では、`scripts/android_keystore_attestation.sh` および CI ヘルパー `scripts/android_strongbox_attestation_ci.sh` を使用します。
3. **テレメトリと編集** — `docs/source/sdk/android/telemetry_redaction.md` で説明されている共有スキーマを介したインストルメンテーション ファネルで、ハッシュ化された権限、バケット化されたデバイス プロファイル、サポート プレイブックによって強制される監査フックのオーバーライドをエクスポートします。
4. **運用ランブック** — `docs/source/android_runbook.md` (オペレーターの応答) および `docs/source/android_support_playbook.md` (SLA + エスカレーション) は、決定論的なオーバーライド、カオス ドリル、および証拠の取得により TOE の運用フットプリントを強化します。
5. **リリースの出所** — Gradle ベースのビルドでは、CycloneDX プラグインに加えて、`docs/source/sdk/android/developer_experience_plan.md` および AND6 準拠チェックリストでキャプチャされた再現可能なビルド フラグが使用されます。リリース アーティファクトは、`docs/source/release/provenance/android/` で署名され、相互参照されています。

## 2. 資産と前提条件

|資産 |説明 |セキュリティ目標 |
|----------|---------------|----------|
|構成マニフェスト |アプリとともに配布される Norito から派生した `ClientConfig` スナップショット。 |信頼性、完全性、保存時の機密性。 |
|鍵の署名 | StrongBox/TEE プロバイダーを通じて生成またはインポートされたキー。 | StrongBox の設定、認証ログ、キーのエクスポートなし。 |
|テレメトリーストリーム | SDK インストルメンテーションからエクスポートされた OTLP トレース/ログ/メトリクス。 |仮名化（機関のハッシュ化）、PII の最小化、監査の無効化。 |
|台帳の相互作用 | Norito ペイロード、アドミッション メタデータ、Torii ネットワーク トラフィック。 |相互認証、リプレイ耐性のあるリクエスト、確定的な再試行。 |

仮定:

- モバイル OS は標準のサンドボックス + SELinux を提供します。 StrongBox デバイスは、Google のキーマスター インターフェイスを実装しています。
- オペレーターは、市議会の信頼された CA によって署名された TLS 証明書を使用して Torii エンドポイントをプロビジョニングします。
- インフラストラクチャのビルドでは、Maven に公開する前に再現可能なビルド要件が尊重されます。

## 3. 脅威と制御|脅威 |コントロール |証拠 |
|----------|----------|----------|
|改ざんされた構成マニフェスト | `ClientConfig` は適用前にマニフェスト (ハッシュ + スキーマ) を検証し、拒否されたリロードを `android.telemetry.config.reload` 経由で記録します。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2。 |
|署名キーの侵害 | StrongBox に必要なポリシー、認証ハーネス、およびデバイス マトリックス監査によりドリフトが特定されます。オーバーライドはインシデントごとに文書化されます。 | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`。 |
|テレメトリにおける PII 漏洩 | Blake2b でハッシュされた認証機関、バケット化されたデバイス プロファイル、キャリアの省略、ログのオーバーライド。 | `docs/source/sdk/android/telemetry_redaction.md`;サポート ハンドブック §8。 |
| Torii RPC での再生またはダウングレード | `/v1/pipeline` リクエスト ビルダーは、ハッシュ化された権限コンテキストを使用して、TLS ピニング、ノイズ チャネル ポリシー、および再試行バジェットを強制します。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md`（予定）。 |
|署名のないリリースまたは複製不可能なリリース | CycloneDX SBOM + Sigstore 認証は AND6 チェックリストによってゲートされます。リリース RFC には `docs/source/release/provenance/android/` の証拠が必要です。 | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`。 |
|不完全なインシデント処理 |ランブック + プレイブックは、オーバーライド、カオス ドリル、エスカレーション ツリーを定義します。テレメトリのオーバーライドには、署名された Norito リクエストが必要です。 | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`。 |

## 4. 評価活動

1. **設計レビュー** — コンプライアンス + SRE は、構成、キー管理、テレメトリ、およびリリース制御が ETSI セキュリティ目標にマッピングされていることを検証します。
2. **実装チェック** — 自動テスト:
   - `scripts/android_strongbox_attestation_ci.sh` は、マトリックスにリストされているすべての StrongBox デバイスについてキャプチャされたバンドルを検証します。
   - `scripts/check_android_samples.sh` および管理対象デバイス CI は、サンプル アプリが `ClientConfig`/テレメトリ契約を遵守していることを保証します。
3. **運用検証** — `docs/source/sdk/android/telemetry_chaos_checklist.md` に基づく四半期ごとのカオス ドリル (編集 + オーバーライド演習)。
4. **証拠保持** — `docs/source/compliance/android/` (このフォルダー) に保存され、`status.md` から参照されるアーティファクト。

## 5. ETSI EN 319 401 マッピング| EN 319 401 条項 | SDK コントロール |
|---------------------|---------------|
| 7.1 セキュリティポリシー |このセキュリティ目標 + サポート ハンドブックに文書化されています。 |
| 7.2 組織のセキュリティ |サポート ハンドブック §2 の RACI + オンコール所有権。 |
| 7.3 資産管理 |上記の §2 で定義された構成、キー、およびテレメトリの資産目標。 |
| 7.4 アクセス制御 | StrongBox ポリシー + 署名された Norito アーティファクトを必要とするオーバーライド ワークフロー。 |
| 7.5 暗号化コントロール | AND2 からのキーの生成、保管、および証明の要件 (キー管理ガイド)。 |
| 7.6 運用上のセキュリティ |テレメトリ ハッシュ、カオス リハーサル、インシデント対応、リリース証拠ゲート。 |
| 7.7 通信セキュリティ | `/v1/pipeline` TLS ポリシー + ハッシュされた権限 (テレメトリ編集ドキュメント)。 |
| 7.8 システムの取得/開発 | AND5/AND6 プランの再現可能な Gradle ビルド、SBOM、来歴ゲート。 |
| 7.9 サプライヤーとの関係 | Buildkite + Sigstore 証明書は、サードパーティの依存関係 SBOM とともに記録されます。 |
| 7.10 インシデント管理 |ランブック/プレイブックのエスカレーション、ログのオーバーライド、テレメトリの失敗カウンター。 |

##6. メンテナンス

- SDK に新しい暗号化アルゴリズム、テレメトリ カテゴリ、またはリリース自動化の変更が導入されるたびに、このドキュメントを更新します。
- `docs/source/compliance/android/evidence_log.csv` の署名済みコピーを SHA-256 ダイジェストおよびレビュー担当者のサインオフとリンクします。