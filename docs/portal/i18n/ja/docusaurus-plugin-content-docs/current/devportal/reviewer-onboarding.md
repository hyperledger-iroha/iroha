---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# プレビューレビュアーのオンボーディング

## 概要

DOCS-SORA は開発者ポータルの段階的なローンチを追跡しています。チェックサムでゲートされたビルド
(`npm run serve`) と強化された Try it フローにより、次のマイルストーンである
公開プレビューを広く開く前のレビュアーのオンボーディングが可能になります。このガイドでは、
リクエストの収集、適格性の確認、アクセスの付与、参加者の安全なオフボーディング方法を説明します。
コホート計画、招待のケイデンス、テレメトリー出力については
[preview invite flow](./preview-invite-flow.md) を参照してください。以下の手順は、レビュアーが選定された後に行う
アクションに焦点を当てています。

- **対象範囲:** GA 前に docs のプレビュー (`docs-preview.sora`、GitHub Pages のビルド、または SoraFS バンドル) へのアクセスが必要なレビュアー。
- **対象外:** Torii または SoraFS のオペレーター (それぞれのオンボーディングキットで対応) と、本番ポータルのデプロイ (参照 [`devportal/deploy-guide`](./deploy-guide.md))。

## 役割と前提条件

| 役割 | 典型的な目的 | 必要なアーティファクト | 備考 |
| --- | --- | --- | --- |
| コアメンテナ | 新しいガイドの検証、スモークテストの実施。 | GitHub ハンドル、Matrix 連絡先、署名済み CLA。 | 通常は `docs-preview` GitHub チームに既に参加しているが、アクセスを監査可能にするため依頼は提出する。 |
| パートナーレビュアー | 公開前の SDK スニペットまたはガバナンス内容の検証。 | 企業メール、法務 POC、署名済みプレビュー条項。 | テレメトリーとデータ取扱い要件の承認が必要。 |
| コミュニティボランティア | ガイドのユーザビリティに関するフィードバック。 | GitHub ハンドル、連絡先、タイムゾーン、CoC 受諾。 | コホートは小さく保つ。貢献者同意書に署名済みのレビュアーを優先する。 |

すべてのレビュアーは以下を満たす必要があります:

1. プレビューアーティファクトの許容利用ポリシーに同意する。
2. セキュリティ/オブザーバビリティの付録を読む
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. ローカルでスナップショットを提供する前に `docs/portal/scripts/preview_verify.sh` を実行することに同意する。

## 受付フロー

1. 依頼者に
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   フォームを記入してもらう (または issue にコピー/ペースト)。最低限、身元、連絡方法、GitHub ハンドル、
   予定レビュー期間、セキュリティドキュメントを読んだ確認を記録する。
2. `docs-preview` トラッカー (GitHub issue またはガバナンスチケット) に記録し、承認者を割り当てる。
3. 前提条件を確認する:
   - CLA / 貢献者同意書の記録 (またはパートナー契約の参照)。
   - 依頼内に許容利用の承認が含まれていること。
   - リスク評価が完了していること (例: パートナーレビュアーが法務で承認済み)。
4. 承認者が依頼にサインオフし、変更管理エントリにトラッキング issue をリンクする (例: `DOCS-SORA-Preview-####`)。

## プロビジョニングとツール

1. **アーティファクト共有** — CI ワークフローまたは SoraFS の pin から最新のプレビュー descriptor + アーカイブ
   (`docs-portal-preview` アーティファクト) を提供する。レビュアーに以下の実行を促す:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **チェックサム強制で提供** — チェックサムゲート付きのコマンドを案内する:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   これにより `scripts/serve-verified-preview.mjs` を再利用し、未検証のビルドが誤って起動されるのを防ぐ。

3. **GitHub アクセス付与 (任意)** — 非公開ブランチが必要な場合、レビュー期間中だけ
   `docs-preview` GitHub チームに追加し、メンバー変更を依頼に記録する。

4. **サポートチャネルの共有** — オンコール連絡先 (Matrix/Slack) と
   [`incident-runbooks`](./incident-runbooks.md) のインシデント手順を共有する。

5. **テレメトリー + フィードバック** — 匿名化された分析が収集されることを知らせる
   (参照 [`observability`](./observability.md))。招待に記載されたフィードバックフォームまたは issue テンプレートを渡し、
   [`preview-feedback-log`](./preview-feedback-log) ヘルパーでイベントを記録してウェーブのサマリを最新に保つ。

## レビュアーチェックリスト

プレビューにアクセスする前に、レビュアーは以下を完了する必要があります:

1. ダウンロードしたアーティファクトを検証する (`preview_verify.sh`)。
2. `npm run serve` (または `serve:verified`) でポータルを起動し、チェックサムガードが有効であることを確認する。
3. 上記リンクのセキュリティ/オブザーバビリティノートを読む。
4. OAuth/Try it コンソールを device-code ログインでテストし (該当する場合)、本番トークンの再利用を避ける。
5. 合意済みのトラッカー (issue、共有ドキュメント、またはフォーム) に所見を記録し、プレビューのリリースタグでタグ付けする。

## メンテナの責任とオフボーディング

| フェーズ | アクション |
| --- | --- |
| Kickoff | 受付チェックリストが依頼に添付されていることを確認し、アーティファクト + 手順を共有し、[`preview-feedback-log`](./preview-feedback-log) を使って `invite-sent` の記録を追加し、レビューが1週間を超える場合は中間同期を予定する。 |
| Monitoring | プレビューのテレメトリーを監視し (Try it の異常トラフィックや probe 失敗を確認)、疑わしい場合はインシデントランブックに従う。所見が届いたら `feedback-submitted`/`issue-opened` を記録してウェーブ指標を正確に保つ。 |
| Offboarding | 一時的な GitHub または SoraFS のアクセスを解除し、`access-revoked` を記録し、依頼をアーカイブする (フィードバック要約 + 未解決アクションを含む)。レビュアーにローカルビルドの削除を依頼し、[`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) から生成した digest を添付する。 |

レビュアーをウェーブ間でローテーションする場合も同じプロセスを使用する。リポジトリ内の証跡
(issue + テンプレート) を維持することで DOCS-SORA の監査性が保たれ、ガバナンスが
プレビューアクセスが文書化された管理策に従っていることを確認できる。

## 招待テンプレートと追跡

- すべてのアウトリーチは
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  から開始する。最小限の法的文言、プレビューのチェックサム手順、レビュアーが許容利用ポリシーに
  同意することを記載する。
- テンプレートを編集する際は `<preview_tag>`、`<request_ticket>`、連絡チャネルの
  プレースホルダーを置き換える。最終メッセージのコピーを intake チケットに保存し、
  レビュアー、承認者、監査担当が送信内容を参照できるようにする。
- 招待送信後、追跡スプレッドシートまたは issue を `invite_sent_at` タイムスタンプと
  予定終了日で更新し、[preview invite flow](./preview-invite-flow.md) レポートが
  コホートを自動取得できるようにする。
