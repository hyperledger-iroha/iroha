---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プレビューのオンボーディング

## ヴュー・ダンサンブル

DOCS-SORA スーツ アン ランスメント パー エテープ デュ ポルテイル 開発。チェックサムの平均ゲートを構築します
(`npm run serve`) et les flux 試してみてください:
選出者のオンボーディングは、事前のプレビュー公開の規模を検証します。 Ceガイド
批判的なコメント収集者、要件、検証者、資格、プロビジョニング者、アクセス権、オフボーダー
参加者は安全です。報告書
[プレビュー招待フロー](./preview-invite-flow.md) 全体的な計画、リズムを注ぐ
テレメトリの招待とエクスポート。 les etapes ci-dessous se concentrent sur les action
あなたの選択は、あなたが選択するものです。

- **周囲:** 審査員はプレビュー ドキュメントに参加する必要があります (`docs-preview.sora`,
  GitHub ページを構築し、バンドル SoraFS) を前もって GA にします。
- **馬の周囲:** オペレーター Torii または SoraFS (オンボーディングに必要なキットの準備)
  ポルタイユの展開と生産 (ヴォワール)
  [`devportal/deploy-guide`](./deploy-guide.md))。

## 役割と前提条件

|役割 |オブジェクトの種類 |アーティファクトが必要 |メモ |
| --- | --- | --- | --- |
|コアメンテナー |ヌーヴォーの検証者、スモークテストの実行者。 | GitHub を処理し、Matrix に連絡し、CLA の書類に署名します。 |装備を整える GitHub `docs-preview`;追放者は、監査可能なように、要求を満たしている必要があります。 |
|パートナーレビューアー | SDK のスニペットの内容が事前に公開されています。 |企業、POC 法務、契約プ​​レビューの署名を電子メールで送信します。 |遠隔測定の偵察と監視の訓練を行います。 |
|地域ボランティア |実用的なガイドのフィードバックを提供します。 | GitHub を処理し、prefere に連絡し、Fuseau horaire、CoC の受け入れを処理します。 | Garder les cohortes petites;優先的に選出者が貢献に同意する必要があります。 |

レビュー担当者が行うタスク:

1. 成果物をプレビューするために許容される使用法を政治的に偵察します。
2. 付録のセキュリティ/監視機能
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
3. 執行者に関与する `docs/portal/scripts/preview_verify.sh` 前衛的なサービス
   スナップショットのロケール。

## ワークフローの取り入れ方

1. デマンダー・オー・デマンダー・デ・レムプリル
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   Formulaire (コピー機/コレクターの問題)。最小限のキャプチャー: 同一性、連絡先、
   GitHub を処理し、レビューの日付と安全性の確認を行います。
2. トラッカーの要求を登録する者 `docs-preview` (GitHub ou ticket de gouvernance を発行する)
   譲渡者と承認者。
3. 検証者の前提条件:
   - CLA / 文書への貢献の合意 (契約パートナーの参照)。
   - 要求に応じて使用法を告発します。
   - 危険な評価終了 (例: レビュー担当者のパートナーが法務部門を承認)。
4. 変更管理の主要な問題に対する要求と嘘の承認
   (例: `DOCS-SORA-Preview-####`)。

## プロビジョニングと費用

1. **Partager les artefacts** - Fournir le dernier 記述子 + プレビュー アーカイブ
   ファイル ワークフロー CI ファイル ピン SoraFS (アーティファクト `docs-portal-preview`)。ラペラー補助レビュアー
   実行者:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **チェックサムの平均的な強制執行** - レビュアーとコマンドゲートの方向性:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Cela は `scripts/serve-verified-preview.mjs` を再利用し、検証されていないビルドを実行します
   これは事故によるものです。

3. **GitHub にアクセスする (オプション)** - 発行者以外のブランチにいるレビュー担当者、
   GitHub `docs-preview` のレビューと委託者の変更を要求する必要があります。
   要求に応じてメンバーシップを確立します。

4. **サポートに関するコミュニケ** - オンコール連絡先 (Matrix/Slack) など
   [`incident-runbooks`](./incident-runbooks.md) の事件の手順。

5. **テレメトリ + フィードバック** - Rappeler 補助レビュー担当者、分析対象者、匿名者、および収集者
   (ヴォワール [`observability`](./observability.md))。フィードバックの定式化とテンプレートの作成
   問題について言及し、招待とジャーナライザーとイベントを開催し、協力者を呼び出す
   [`preview-feedback-log`](./preview-feedback-log) 曖昧な履歴書を一時間かけてください。

## レビュー担当者のチェックリスト

事前にプレビューを行って、レビュー担当者が完了者を指示します:1. アーティファクトのテレチャージの検証 (`preview_verify.sh`)。
2. Lancer le portail via `npm run serve` (ou `serve:verified`) は、セキュリティ チェックサムの推定を実行します。
3. セキュリティと監視のメモは、ci-dessus を参照します。
4. コンソールで OAuth をテストし、デバイス コード ログイン (該当する場合) を介してトークンを再利用して本番環境でテストします。
5. 保管者は、統計情報と追跡者との協議（問題、文書の一部または公式）およびタグ付け者との協議を行う
   リリース プレビューの平均タグ。

## メンテナーとオフボーディングの責任

|フェーズ |アクション |
| --- | --- |
|キックオフ |確認者は、要求に応じて摂取するチェックリストを確認し、成果物 + 説明書を作成し、[`preview-feedback-log`](./preview-feedback-log) 経由でエントリ 18NI00000043X を確認し、レビュー期間中にパルクールを計画し、レビュー期間を追加します。 |
|モニタリング |監視員はテレメトリのプレビュー (実際に交通事故を試して調査する) と事件の実行手順書を作成し、容疑者を選択しました。 Journaliser les Evenements `feedback-submitted`/`issue-opened` 毛皮などの情報を正確に把握し、正確な情報を収集します。 |
|オフボーディング | GitHub へのアクセス権を取り消す人は SoraFS、荷送人は `access-revoked`、要求をアーカイブする人 (フィードバックの履歴書と出席者のアクションを含む)、レビュー担当者の 1 時間の登録に応じます。要求者は、[`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) の部分を作成し、結合したダイジェストを作成するレビュー担当者を要求します。 |

曖昧なレビュアーの回転によるミームプロセスを利用します。ガーダー・ラ・トレース・ダン・ル・レポ
(問題 + テンプレート) 補助者 DOCS-SORA 監査可能であり、確認者による管理を許可します。
スイビ ファイル コントロールのドキュメントをプレビューします。

## 招待状と招待状のテンプレート

- コマーシャル チャク アウトリーチ アベック ル
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  フィシエ。法的最低限の言語、チェックサムのプレビューおよび注意事項の説明を取得します。
  que les reviewers reconnaissent la politique d'uses は許容されます。
- テンプレートの編集、プレースホルダーの置き換え `<preview_tag>`、`<request_ticket>`
  連絡先を教えてください。レビュー担当者がチケットを受け取り、メッセージの最終コピーを取得し、
  承認者と監査人は、テキストの正確な使者を直接参照します。
- 招待状の発行後、1 時間ごとにスプレッドシートを管理し、タイムスタンプを発行します
  `invite_sent_at` 親密な関係を築くための日付
  [プレビュー招待フロー](./preview-invite-flow.md) 自動キャプチャー機能。