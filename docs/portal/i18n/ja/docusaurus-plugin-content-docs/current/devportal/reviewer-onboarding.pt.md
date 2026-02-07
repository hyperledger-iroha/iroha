---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# オンボーディングとレビュー担当者がプレビューを行う

## ヴィサオ・ゲラル

DOCS-SORA は、ポータル デ センボルベドアのランカメント管理をサポートします。チェックサムのコムゲートを構築します
(`npm run serve`) e fluxos reforcados destravam o proximo marco を試してみてください:
オンボーディングは改訂の有効性を確認し、プレビュー公開を行って公開します。エステギア
coletar solicitacoes、verificar elegibilidade、暫定アクセス権および Fazer オフボーディングを解除します
参加者コムセグランカ。相談してください
[プレビュー招待フロー](./preview-invite-flow.md) para o planjamento de coortes、a
テレメトリの招待と輸出の記録。 os passos abaixo focam nas acoes
トマール・クアンド・ウム・リバイザー・ジャ・フォイ・セレシオナド。

- **Escopo:** ドキュメントのプレビューの正確性を改訂します (`docs-preview.sora`,
  ビルドは、GitHub Pages または SoraFS のバンドルを GA 前に実行します。
- **Fora do escopo:** operadores de Torii ou SoraFS (オンボーディングの専用キットの操作)
  インプラントはポータルを生成します (ver
  [`devportal/deploy-guide`](./deploy-guide.md))。

## 書類と前提条件

|パペル |オブジェクトのティピコス |アルテファトス・レケリドス | 写真メモ |
| --- | --- | --- | --- |
|コアメンテナー | Verificar novos guias、executar スモークテスト。 | GitHub ハンドル、マトリックス、CLA に関する情報。 | Geralmente ja esta no time GitHub `docs-preview`;アシム登録は、監査を承認するための申請を行います。 |
|パートナーレビューアー | SDK の有効なスニペットやガバナンスの詳細は、公開前に公開されています。 |企業、POC 法務、プレビューに関する条件をすべて電子メールで送信します。 | Deve reconhecer requisitos de telemetria + tratamento dedados。 |
|地域ボランティア | Fornecer フィードバック デ ユーザービリダーデ ソブレ ギア。 | GitHub ハンドル、contato prioritydo、fuso horario、aceitacao do CoC。 |マンテンハ・コルテス・ペケナス。貢献度の高い修正を優先します。 |

開発のヒント:

1. プレビューの最新情報をもとに政治を再会議します。
2. セキュリティ/観察の付録の確認
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
3. 執行者との合意 `docs/portal/scripts/preview_verify.sh` 安全な準備
   ローカルのスナップショット。

## フラクソデ摂取

1. ペディル・アオ・ソリシタンテ・ケ・プリエンチャ・オ
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   Formulario (コピー/カラー EM UM 発行)。キャプチャー・アオ・メノス: 同一性、接触方法、
   GitHub ハンドル、改訂前のデータ、およびセキュリティ フォームのドキュメントの確認。
2. トラッカー要請なしのレジストラ `docs-preview` (GitHub ou ticket de Governmentを発行)
   私はアプロバドールを取得します。
3. 有効な前提条件:
   - CLA / acordo de contribuicao em arquivo (参照パートナー)。
   - 私たちの協力を得て、協力を求めます。
   - Avaliacao de risco completa (例として、パートナーの法的承認を改訂します)。
4. 取引の追跡に関する問題の承認を承認します。
   変更管理 (例: `DOCS-SORA-Preview-####`)。

## プロビジョナメントとフェラメント

1. **最新の技術情報** - ワークフローの記述子とプレビューの基礎
   CI はピン SoraFS (artefato `docs-portal-preview`) を実行します。実行者の Lembrar OS 改訂版:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **チェックサムの強制サーバー** - チェックサムのコマンドを実行するための Apontar OS の改訂:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Isso reutiliza `scripts/serve-verified-preview.mjs` para que nenhum build nao verificado
   セイジャ・イニシアド・ポル・アシデンテ。

3. **GitHub へのアクセス許可 (オプション)** - 公開ブランチの改訂を承認、
   adiciona-los ao time GitHub `docs-preview` レジストラとメンバーシップの見直し期間
   ナ・ソリタカオ。

4. **サポートに関する連絡** - オンコール連絡 (Matrix/Slack) と手順の比較
   [`incident-runbooks`](./incident-runbooks.md) の事件。

5. **テレメトリア + フィードバック** - Lembrar os は分析を修正し、これを修正します
   (ver [`observability`](./observability.md))。フィードバックや問題のテンプレートを作成するためのフォーネサー
   Citado はレジストラやイベント コム、ヘルパーを招待しません
   [`preview-feedback-log`](./preview-feedback-log) 問題を解決するためのパラメータ。

## 見直しを行うチェックリスト

プレビューへのアクセス前、完成した開発のリバイザー:1. ベリフィカル オス アルテファト バイシャドス (`preview_verify.sh`)。
2. `npm run serve` (`serve:verified`) 経由でポータルを開始し、チェックサムの保護を保証します。
3. 安全性と観察性を記録します。
4. コンソール OAuth をテストし、デバイス コード ログイン (アプリケーション レベル) を使用して再利用トークンを生成します。
5. レジストラ achados no tracker acordado (問題、ドキュメント compartilhado ou Formulario) e taguea-los com
   o タグでリリースしてプレビューします。

## メンテナーとオフボーディングの責任

|ファセ |アコス |
| --- | --- |
|キックオフ | [`preview-feedback-log`](./preview-feedback-log) 経由で、摂取チェックリストを確認し、最新の情報と説明書を確認し、定期的なスケジュールを同期してスケジュールを確認してください。セマナ。 |
|モニタリング |テレメトリのプレビューを監視 (通信での通信の試行、調査のファルハス) を実行し、インシデントの監視を監視します。レジストラ イベント `feedback-submitted`/`issue-opened` は、正確なメトリクスとして、正確なOS を確認します。 |
|オフボーディング | GitHub の SoraFS、レジストラ `access-revoked`、リクエストの検索 (フィードバックの再開 + 保留中のコメントを含む)、レジストリの更新に関する Revogar の一時的なアクセス。弁護士によるリバイザー クエリの削除は、[`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) のロケールとアネクサーまたはダイジェスト ジェラドをビルドします。 |

内部プロセスや回転改訂を使用します。 Manter o rastro のリポジトリなし (問題 + テンプレート)
DOCS-SORA は、政府機関のプレビューに対する永続的な監査と許可を確認します。
seguiu os はドキュメンタドを制御します。

## 招待および追跡用のテンプレート

- Inicie todo outreach com o arquivo
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)。
  法的言語のキャプチャやプレビュー、期待値のチェックサムの説明など
  政治的政策を見直します。
- テンプレートやエディター、`<preview_tag>`、`<request_ticket>` などのプレースホルダーを置き換えることができます。
  決勝戦のチケットはなく、改訂、承認、監査が行われます
  テキストを参照してください。
- 証拠を提出し、タイムスタンプ `invite_sent_at` データを追跡し、問題を追跡する計画を作成します
  デ・エンセラメント・エスペラダ・パラ・ケ・オ・リレーション
  [プレビュー招待フロー](./preview-invite-flow.md) 自動的にコートを識別します。