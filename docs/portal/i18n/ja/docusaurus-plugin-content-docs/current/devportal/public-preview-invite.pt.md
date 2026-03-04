---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プレイブックを招集し、プレビュー公開を行う

## オブジェクトはプログラムを実行します

エステ プレイブックの詳細な発表と実行者のプレビュー公開、およびそのプレビュー
オンボーディングおよび評価のワークフロー。ロードマップの詳細 DOCS-SORA 正直な青
保証人は、サイア com の認証を取得し、オリエンタソ デ セグランカを招待します
カミーニョ・クラロ・デ・フィードバック。

- **対象読者:** コミュニティの会員リスト、パートナーおよびメンテナーのリスト
  政治的政策はプレビューを行います。
- **制限:** タマンホ デ オンダ パドラオ <= 25 改訂、14 ディアスでのアクセス、レスポスタ
  事件は24時間発生します。

## ゲート・デ・ランカメントのチェックリスト

招待状を完成させるために必要な情報を記入してください:

1. CI プレビュー環境のアルティモス アート (`docs-portal-preview`、
   チェックサム、記述子、バンドル SoraFS) のマニフェスト。
2. `npm run --prefix docs/portal serve` (チェックサムのゲート) テストタグがありません。
3. オンボーディングの承認と招待の承認のチケット。
4. 安全性の文書、監視および事件の検証
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
5. フィードバックの定式化、問題の準備のためのテンプレート (重大な問題を含む)
   再現手順、スクリーンショット、環境情報など）。
6. Docs/DevRel + Governance の改訂をコピーします。

## パコート・デ・コンバイト

Cada convite deve include:

1. **Artefatos verificados** - マニフェスト/プラン SoraFS の Artefatos に関する Forneca リンク
   GitHub は、マニフェスト、チェックサム、記述子を作成します。コマンドの明示的な参照
   デ・ベリフィカオ・パラケ・オス・リバイザ・ポッサム・エクスキュータ・ロ・アンテス・デ・サブイル・サイト。
2. **サービスの命令** - チェックサムのプレビューコマンドが含まれます:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de seguranca** - トークンの有効期限を自動で通知し、開発者にリンクします
   事件と事故の即時報告を開発します。
4. **フィードバックの連絡方法** - テンプレート/形式をリンクし、応答のテンポを予測します。
5. **データはプログラムされます** - データの詳細、勤務時間、同期、近日のデータを通知します
   ジャネラ・デ・リフレッシュ。

例のメールを送信してください
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
コブレ・エセス・リクイシトス。 OS プレースホルダー (データ、URL、コンテナー) を自動化します。
アンビアル。

## ホストのプレビューをエクスポートする

それで、プロモーション、プレビューのホスト、オンボーディング エスティバーの完了、ムダンカ エスティバーのチケットの発行を完了しました
アプロバード。 Veja o [ホスト デ プレビューの説明](./preview-host-exposure.md) パラ オス パソス
エンドツーエンドで構築/公開/検証を行います。

1. **エンパコメントの構築:** 決定的な成果物を作成するためのタグをリリースします。

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   O スクリプト デ ピン グラバ `portal.car`、`portal.manifest.*`、`portal.pin.proposal.json`、
   e `portal.dns-cutover.json` em `artifacts/sorafs/`。 Anexe esses arquivos a onda de convites
   パラケ・カダ・リバイザー・ポッサ・ベリフィカル・オス・メスモスビット。

2. **パブリックまたはプレビューのエイリアス:** Rode o commando sem `--skip-submit`
   (forneca `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e a prova de alias Emida
   ペラ・ガバナンカ）。スクリプトを実行してください。`docs-preview.sora` を発行してください。
   `portal.manifest.submit.summary.json` は `portal.pin.report.json` に証拠のバンドルを提供します。

3. **デプロイメントのテスト:** クエリのエイリアス解決、チェックサムの対応、AO タグの確認
   アンティ・ド・アンヴィアは招待します。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) は、マオコモフォールバックパラメータです
   ポッサム・スビル・マ・コピア・ローカル・セキュリティー・エッジ・デ・プレビュー・ファルハールを改訂します。

## コミュニケーションのタイムライン

|ダイヤ |アカオ |オーナー |
| --- | --- | --- |
| D-3 |ファイナライザのコピーを依頼し、アートファイルを作成し、検証のドライランを実行します。ドキュメント/開発リリース |
| D-2 |政府の承認 + ムダンカのチケット |ドキュメント/DevRel + ガバナンス |
| D-1 | Enviar は、テンプレート、目的地リストの自動追跡トラッカーを招待します。ドキュメント/開発リリース |
| D |キックオフ コール / オフィス アワー、テレメトリのモニター ダッシュボード |ドキュメント/DevRel + オンコール |
| D+7 |フィードバックのダイジェストは、問題を解決し、問題をトリアージする必要はありません。ドキュメント/開発リリース |
| D+14 | Fechar a onda、revogar acesso Temparario、publicar resumo em `status.md` |ドキュメント/開発リリース |

## アクセスとテレメトリの追跡

1. 宛先、呼び出しデータのタイムスタンプを登録します。
   プレビュー フィードバック ロガー (veja
   [`preview-feedback-log`](./preview-feedback-log)) 必要な情報を確認してください
   証拠の記録:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```Sao イベント サポート `invite-sent`、`acknowledged`、
   `feedback-submitted`、`issue-opened`、e `access-revoked`。 O log fica em
   `artifacts/docs_portal_preview/feedback_log.json` por パドラオ;アネックスaoチケットだ
   同意を得るためには、事前に同意を得る必要があります。ヘルパーを使ってください
   製品のロールアップ監査に関する注意事項の概要:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   O 概要 JSON 列挙型は、一時的な目的地、異常な感染状況を示します。
   フィードバックとタイムスタンプは、最新のイベントを実行します。おおヘルパー、アポイアド・ポル
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)、
   CI をローカルに管理するためのワークフローのポータル。テンプレートのダイジェストを使用する
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ああ、広報、要約、ダ・オンダ。
2. `DOCS_RELEASE_TAG` を使用してテレメトリアのダッシュボードを表示する
   picos possam ser correlacionados com を coortes de convite としてご覧ください。
3. 確認のための `npm run probe:portal -- --expect-release=<tag>` のデプロイを実行します
   o リリースのメタデータ修正のプレビュー通知。
4. ランブックを実行するテンプレートを使用せずに、問題を登録し、コートを作成します。

## フィードバックとフェチャメント

1. ボードの問題に関するドキュメントのフィードバックに同意します。マルケ オス アイテンス コム
   `docs-preview/<wave>` パラケオスの所有者は、ロードマップを容易に実行します。
2. 事前にプレビュー ロガーを作成し、関連性を確認したり、デポジットを作成したりするためのサマリーを使用します。
   `status.md` (参加者、主要なアチャドス、修正済みのプレーンジャドス) e を参照してください。
   atualize `roadmap.md` セオマルコ DOCS-SORA mudou。
3. オフボーディングを完了する
   [`reviewer-onboarding`](./reviewer-onboarding.md): 取り消しアクセス、アーキブ要求メール
   アグラデカOS参加者。
4. 近似データを準備し、チェックサム ゲートを再実行します。
   com novas データを招待するテンプレートを用意しています。

一貫した形式のプレイブックを作成し、監査可能なプレビュー プログラムを作成する
Docs/DevRel を使用して、メディア クエリ ポータルを招待し、繰り返しエスカレートします。
GA に近づきます。