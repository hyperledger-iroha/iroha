---
lang: ja
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-04T11:42:43.489571+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android AND6 準拠チェックリスト

このチェックリストは、マイルストーンを決定するコンプライアンスの成果物を追跡します **AND6 -
CI とコンプライアンスの強化**。要求された規制上の成果物を統合します
`roadmap.md` でストレージ レイアウトを定義します。
`docs/source/compliance/android/` だからリリースエンジニアリング、サポート、法務
Android リリースを承認する前に、同じ証拠セットを参照できます。

## 範囲と所有者

|エリア |成果物 |主要所有者 |バックアップ/レビュー担当者 |
|------|--------------|---------------|---------------------|
| EU 規制バンドル | ETSI EN 319 401 セキュリティ目標、GDPR DPIA 概要、SBOM 認証、証拠ログ |コンプライアンスと法務 (ソフィア・マーティンズ) |リリースエンジニアリング (Alexei Morozov) |
|日本の規制バンドル | FISC セキュリティ管理チェックリスト、バイリンガル StrongBox 認証バンドル、証拠ログ |コンプライアンスと法務 (ダニエル・パーク) | Android プログラム リード |
|デバイス ラボの準備 |キャパシティ追跡、緊急事態トリガー、エスカレーション ログ |ハードウェア ラボ リーダー | Android の可観測性 TL |

## アーティファクト マトリックス|アーティファクト |説明 |ストレージパス |リフレッシュの頻度 |メモ |
|----------|---------------|--------------|---------------------|----------|
| ETSI EN 319 401 セキュリティ目標 | Android SDK バイナリのセキュリティ目標/前提条件を説明する説明。 | `docs/source/compliance/android/eu/security_target.md` |すべての GA + LTS リリースを再検証します。 |リリース トレインのビルド来歴ハッシュを引用する必要があります。 |
| GDPR DPIA の概要 |テレメトリ/ロギングを対象としたデータ保護の影響評価。 | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` |年次 + マテリアルテレメトリーが変更される前。 | `sdk/android/telemetry_redaction.md` の参照編集ポリシー。 |
| SBOM 認証 | Gradle/Maven アーティファクトの署名付き SBOM と SLSA の出所。 | `docs/source/compliance/android/eu/sbom_attestation.md` |すべての GA リリース。 | `scripts/android_sbom_provenance.sh <version>` を実行して、CycloneDX レポート、バンドルの署名、およびチェックサムを生成します。 |
| FISC セキュリティ管理チェックリスト | SDK コントロールを FISC 要件にマッピングするチェックリストが完了しました。 | `docs/source/compliance/android/jp/fisc_controls_checklist.md` |年次 + JP パートナーのパイロット前。 |見出しを二か国語 (EN/JP) で提供します。 |
| StrongBox 認証バンドル (JP) | JP 規制当局向けのデバイスごとの認証概要 + チェーン。 | `docs/source/compliance/android/jp/strongbox_attestation.md` |新しいハードウェアがプールに入るとき。 | `artifacts/android/attestation/<device>/` の下の生のアーティファクトを指します。 |
|法的承認メモ | ETSI/GDPR/FISC の範囲、プライバシーの姿勢、添付されたアーティファクトの保管過程をカバーする法律顧問の概要。 | `docs/source/compliance/android/eu/legal_signoff_memo.md` |アーティファクト バンドルが変更されるか、新しい管轄区域が追加されるたびに。 |メモは証拠ログのハッシュを参照し、デバイスとラボの緊急事態バンドルにリンクします。 |
|証拠ログ |ハッシュ/タイムスタンプのメタデータを含む、送信されたアーティファクトのインデックス。 | `docs/source/compliance/android/evidence_log.csv` |上記のエントリが変更されるたびに更新されます。 | Buildkite リンクとレビュー担当者のサインオフを追加します。 |
|デバイスラボ計測バンドル | `device_lab_instrumentation.md` で定義されたプロセスで記録されたスロット固有のテレメトリ、キュー、および証明の証拠。 | `artifacts/android/device_lab/<slot>/` (`docs/source/compliance/android/device_lab_instrumentation.md` を参照)すべての予約スロット + フェイルオーバー ドリル。 | SHA-256 マニフェストをキャプチャし、証拠ログとチェックリストでスロット ID を参照します。 |
|デバイスラボ予約ログ |フリーズ中に StrongBox プールを 80% 以上に保つために使用される予約ワークフロー、承認、キャパシティ スナップショット、およびエスカレーション ラダー。 | `docs/source/compliance/android/device_lab_reservation.md` |予約が作成/変更されるたびに更新します。 |手順に記載されている `_android-device-lab` チケット ID と週次カレンダーのエクスポートを参照してください。 |
| Device-lab フェールオーバー ランブックとドリル バンドル |フォールバック レーン、Firebase バースト キュー、外部 StrongBox リテイナーの準備状況を示す四半期ごとのリハーサル計画とアーティファクト マニフェスト。 | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` |四半期ごと (またはハードウェア名簿の変更後)。 |ドリル ID を証拠ログに記録し、Runbook に記載されているマニフェスト ハッシュ + PagerDuty エクスポートを添付します。 |

> **ヒント:** PDF または外部署名されたアーティファクトを添付する場合は、短いファイルを保存してください
> 不変アーティファクトにリンクするテーブルパス内のマークダウンラッパー
> ガバナンスシェア。これにより、リポジトリを軽量に保ちながら、
> 監査証跡。

## EU 規制パケット (ETSI/GDPR)EU のパケットには、上記の 3 つの成果物と法的メモがまとめられています。

- `security_target.md` をリリース識別子、Torii マニフェスト ハッシュで更新します。
  および SBOM ダイジェストを使用して、監査人がバイナリを宣言されたスコープと照合できるようにします。
- DPIA の概要を最新のテレメトリ編集ポリシーに合わせて維持し、
  `docs/source/sdk/android/telemetry_redaction.md` で参照されている Norito の差分の抜粋を添付します。
- SBOM 認証エントリには、CycloneDX JSON ハッシュ、出所を含める必要があります。
  バンドル ハッシュ、余署名ステートメント、およびそれらを生成した Buildkite ジョブ URL。
- `legal_signoff_memo.md` は顧問/日付をキャプチャし、すべてのアーティファクトをリストする必要があります +
  SHA-256、補償コントロールの概要、および証拠ログ行へのリンク
  加えて、承認を追跡したPagerDutyチケットID。

## 日本の規制パケット (FISC/StrongBox)

日本の規制当局は、次のようなバイリンガルのドキュメントを含む並行バンドルを期待しています。

- `fisc_controls_checklist.md` は公式スプレッドシートを反映しています。両方を埋める
  EN および JA 列および `sdk/android/security.md` の特定のセクションを参照
  または、各制御を満たす StrongBox 認証バンドル。
- `strongbox_attestation.md` は、の最新の実行を要約します。
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (デバイスごとの JSON + Norito エンベロープ)。不変のアーティファクトへのリンクを埋め込む
  `artifacts/android/attestation/<device>/` の下で、回転のリズムに注目してください。
- 提出物に同梱されるバイリンガルのカバーレターのテンプレートを記録します
  `docs/source/compliance/android/jp/README.md` なので、サポートが再利用できます。
- チェックリストを参照する単一行で証拠ログを更新します。
  構成証明バンドルのハッシュ、および配信に関連付けられた JP パートナー チケット ID。

## 提出ワークフロー

1. **ドラフト** - 所有者がアーティファクトを準備し、計画されたファイル名を記録します。
   上の表を参照し、更新された Markdown スタブと
   外部添付ファイルのチェックサム。
2. **レビュー** - リリース エンジニアリングにより、出所ハッシュが段階的なハッシュと一致することを確認します。
   バイナリ;コンプライアンスは規制文言を検証します。サポートにより SLA が保証され、
   テレメトリ ポリシーが正しく参照されています。
3. **サインオフ** - 承認者は自分の名前と日付を `Sign-off` テーブルに追加します
   以下。証拠ログは PR URL と Buildkite の実行によって更新されます。
4. **公開** - SRE ガバナンスの承認後、アーティファクトをリンクします。
   `status.md` を更新し、Android サポート プレイブックの参照を更新します。

### サインオフログ

|アーティファクト |レビュー者 |日付 | PR / 証拠 |
|----------|---------------|------|----------|
| *(保留中)* | - | - | - |

## デバイス ラボの予約と緊急時対応計画

ロードマップで指摘されている **デバイス ラボの可用性** リスクを軽減するには:- `docs/source/compliance/android/evidence_log.csv` で週次容量を追跡
  (列 `device_lab_capacity_pct`)。利用可能な場合はリリース エンジニアリングにアラートを送信
  2週連続で70％を下回る。
- 以下のストロングボックス/一般レーンを予約します
  `docs/source/compliance/android/device_lab_reservation.md` よりも先に
  凍結、リハーサル、またはコンプライアンススイープを実行するため、リクエスト、承認、およびアーティファクトが
  `_android-device-lab` キューにキャプチャされます。結果として得られたチケット ID をリンクします
  容量スナップショットを記録するときに証拠ログに記録されます。
- **フォールバック プール:** 最初に共有ピクセル プールにバーストします。まだ飽和している場合は、
  CI 検証のために Firebase Test Lab のスモーク実行をスケジュールします。
- **外部ラボ リテーナー:** StrongBox パートナーとリテーナーを保守します。
  これにより、フリーズ期間中にハードウェアを予約できるようになります (最低 7 日間のリード)。
- **エスカレーション:** 両方の場合、PagerDuty で `AND6-device-lab` インシデントが発生します。
  プライマリ プールとフォールバック プールの容量が 50 % を下回ります。ハードウェアラボのリーダー
  SRE と連携してデバイスの優先順位を再設定します。
- **フェイルオーバー証拠バンドル:** すべてのリハーサルを以下に保存します
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` 予約あり
  リクエスト、PagerDuty エクスポート、ハードウェア マニフェスト、およびリカバリ トランスクリプト。参考資料
  `device_lab_contingency.md` のバンドルを取得し、証拠ログに SHA-256 を追加します。
  したがって、法務部門は緊急事態対応ワークフローが実行されたことを証明できます。
- **四半期ごとの訓練:** でランブックを演習します。
  `docs/source/compliance/android/device_lab_failover_runbook.md`、添付してください
  結果として得られるバンドル パス + マニフェスト ハッシュから `_android-device-lab` チケット、および
  緊急時ログと証拠ログの両方にドリル ID をミラーリングします。

緊急時対応計画のすべての発動を文書化します。
`docs/source/compliance/android/device_lab_contingency.md` (日付を含む、
トリガー、アクション、フォローアップなど）。

## 静的解析プロトタイプ

- `make android-lint` は `ci/check_android_javac_lint.sh` をラップし、コンパイルします
  `java/iroha_android` および共有 `java/norito_java` ソース
  `javac --release 21 -Xlint:all -Werror` (フラグが設定されたカテゴリは次のとおりです)
- コンパイル後、スクリプトは AND6 依存関係ポリシーを適用します。
  `jdeps --summary`、承認された許可リスト外のモジュールがある場合は失敗します
  (`java.base`、`java.net.http`、`jdk.httpserver`) が表示されます。これにより、
  SDK カウンシルの「隠れた JDK 依存関係なし」に準拠した Android サーフェス
  StrongBox コンプライアンス レビューの前に必要となります。
- CI は次経由で同じゲートを実行します。
  `.github/workflows/android-lint.yml`、これを呼び出します
  Android にアクセスするすべてのプッシュ/PR で `ci/check_android_javac_lint.sh`
  共有 Norito Java ソースとアップロード `artifacts/android/lint/jdeps-summary.txt`
  そのため、コンプライアンス レビューでは、再実行せずに署名付きモジュール リストを参照できます。
  ローカルでスクリプトを作成します。
- 一時的なデータを保持する必要がある場合は、`ANDROID_LINT_KEEP_WORKDIR=1` を設定します。
  ワークスペース。スクリプトは、生成されたモジュールの概要を既にコピーしています。
  `artifacts/android/lint/jdeps-summary.txt`;セット
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (または同様の) 監査用に追加のバージョン管理されたアーティファクトが必要な場合。
  エンジニアは Android PR を送信する前にコマンドをローカルで実行する必要があります。
  Java ソースにアクセスし、記録された概要/ログをコンプライアンスに添付するもの
  レビュー。リリースノートから「Android javac lint + 依存関係」として参照してください。
  スキャン」。

## CI 証拠 (Lint、テスト、証明)- `.github/workflows/android-and6.yml` はすべての AND6 ゲートを実行するようになりました (javac lint +
  依存関係スキャン、Android テスト スイート、StrongBox 構成証明検証ツール、および
  デバイス ラボ スロット検証）は、Android サーフェスに触れるすべての PR/プッシュで行われます。
- `ci/run_android_tests.sh` は `ci/run_android_tests.sh` をラップして出力します
  `artifacts/android/tests/test-summary.json` での決定的な概要
  コンソール ログを `artifacts/android/tests/test.log` に永続化します。両方取り付けてください
  CI 実行を参照するときに、ファイルをコンプライアンス パケットに変換します。
- `scripts/android_strongbox_attestation_ci.sh --summary-out` は生成します
  `artifacts/android/attestation/ci-summary.json`、バンドルされたものを検証します
  StrongBox の `artifacts/android/attestation/**` の下の証明書チェーンと
  TEEプール。
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  CI で使用され、ポイントできるサンプル スロット (`slot-sample/`) を検証します。
  実際は `artifacts/android/device_lab/<slot-id>/` で実行されます。
  `--require-slot --json-out <dest>` は、インストルメンテーション バンドルが次のとおりであることを証明します。
  文書化されたレイアウト。 CI は検証の概要を次の場所に書き込みます。
  `artifacts/android/device_lab/summary.json`;サンプルスロットには以下が含まれます
  プレースホルダテレメトリ/アテステーション/キュー/ログ抽出と記録された
  再現可能なハッシュの場合は `sha256sum.txt`。

## デバイスラボ計測ワークフロー

すべての予約またはフェイルオーバーのリハーサルは、次の手順に従う必要があります。
`device_lab_instrumentation.md` ガイド、テレメトリ、キュー、および構成証明
アーティファクトは予約ログと一致します。

1. **シード スロット アーティファクト** の作成
   `artifacts/android/device_lab/<slot>/` を標準のサブフォルダーとともに実行します
   スロットが閉じた後は `shasum` (新しいドキュメントの「アーティファクト レイアウト」セクションを参照)
   ガイド）。
2. **インストルメンテーション コマンドを実行します。** テレメトリ/キュー キャプチャを実行します。
   ダイジェスト、StrongBox ハーネス、および lint/依存関係スキャンを正確にオーバーライドします。
   出力が CI を反映するように文書化されています。
3. **証拠を提出します。** アップデート
   `docs/source/compliance/android/evidence_log.csv`と予約チケット
   スロット ID、SHA-256 マニフェスト パス、および対応するダッシュボード/Buildkite を含む
   リンク。

アーティファクト フォルダーとハッシュ マニフェストを AND6 リリース パケットに添付します。
影響を受けるフリーズウィンドウ。ガバナンス審査担当者は、次のようなチェックリストを拒否します。
スロット識別子とインストルメンテーション ガイドは引用しないでください。

### 予約とフェイルオーバーの準備状況の証拠

ロードマップ項目「規制上のアーティファクトの承認と研究所の緊急事態」には、さらに多くのことが必要です
計装よりも。すべての AND6 パケットもプロアクティブ パケットを参照する必要があります。
予約ワークフローと四半期ごとのフェイルオーバー リハーサル:- **予約プレイブック (`device_lab_reservation.md`)。** 予約に従ってください
  テーブル (リード タイム、所有者、スロットの長さ)、共有カレンダーをエクスポートします。
  `scripts/android_device_lab_export.py`、およびレコード `_android-device-lab`
  `evidence_log.csv` の容量スナップショットとチケット ID。プレイブック
  エスカレーションラダーと緊急事態のトリガーを詳しく説明します。それらの詳細をコピーします
  予約が移動したとき、またはキャパシティが定員を下回ったときに、チェックリストのエントリに追加されます。
  ロードマップの目標は 80%。
- **フェールオーバー ドリル Runbook (`device_lab_failover_runbook.md`)。**
  四半期ごとのリハーサル (停止のシミュレーション → フォールバック レーンの促進 → 関与)
  Firebase バースト + 外部 StrongBox パートナー) にアーティファクトを保存します。
  `artifacts/android/device_lab_contingency/<drill-id>/`。各バンドルは、
  マニフェスト、PagerDuty エクスポート、Buildkite 実行リンク、Firebase バーストが含まれます
  レポートと、ランブックに記載されたリテイナーの承認。を参照してください。
  証拠ログと両方のドリル ID、SHA-256 マニフェスト、およびフォローアップ チケット
  このチェックリスト。

これらの文書を総合すると、デバイスの容量計画、停止のリハーサル、
そして、インスツルメンテーション バンドルは、システムが要求する同じ監査済み証跡を共有します。
ロードマップと法的審査担当者。

## レビューの頻度

- **四半期** - EU/JP のアーティファクトが最新であることを検証します。リフレッシュする
  証拠ログのハッシュ。来歴キャプチャをリハーサルします。
- **プレリリース** - GA/LTS カットオーバーのたびにこのチェックリストを実行し、アタッチします
  リリース RFC への完成したログ。
- **インシデント後** - 重大度 1/2 のインシデントがテレメトリ、署名、または
  証明、修復メモを使用して関連するアーティファクト スタブを更新し、
  証拠ログに参照をキャプチャします。