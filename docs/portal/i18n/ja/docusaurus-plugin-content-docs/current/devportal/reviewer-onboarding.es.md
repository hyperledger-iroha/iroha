---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# オンボーディングとレビューとプレビュー

## 履歴書

DOCS-SORA は、ポータル デ サルロラドーレスのエスカロナードに影響を与えています。ロスビルドチェックサムコンゲート
(`npm run serve`) 問題を解決してみてください:
オンボーディングは、正式なプレビュー公開の有効性を検証します。エスタギア
共同の要請、検証のエレギビリダ、暫定的な同意事項、ダル デ バハについて説明します。
参加者デ・フォルマ・セグラ。相談してください
[プレビュー招待フロー](./preview-invite-flow.md) グループの計画に沿って、ラ
招待状とテレメトリの輸出に関する記録。ロス・パソス・デ・アバホ・セ・エンフォカン・エン・ラス・アクシオネス
トマール・ウナ・ベス・ケ・アン・リバイザー・ハ・シド・セレシオナド。

- **Alcance:** ドキュメントのプレビューに必要な情報を改訂します (`docs-preview.sora`、
  GitHub ページのビルドまたは SoraFS のバンドル) が GA 前に作成されます。
- **Fuera de alcance:** Torii または SoraFS の操作 (オンボーディングのプロピオス キットの準備)
  制作ポータルの設定 (ver
  [`devportal/deploy-guide`](./deploy-guide.md))。

## 役割と前提条件

|ロール |オブジェクトのティピコス | Artefactos requeridos | 写真メモ |
| --- | --- | --- | --- |
|コアメンテナー | Verificar nuevas guias、エジェクター煙テスト。 | GitHub ハンドル、連絡先 Matrix、CLA 企業アーカイブ。 |通常は GitHub `docs-preview`;監査可能な登録手続きを行う必要があります。 |
|パートナーレビューアー | SDK の有効なスニペットやコンテンツの内容は、リリース前に公開されます。 |企業、POC 法務、プレビュー会社の期限を電子メールで送信します。 |デベ・レコノサー・ロス・レクエリミエントス・デ・テレメトリア+マネホ・デ・ダトス。 |
|地域ボランティア | Aportar フィードバック デ ユーサビリダド ソブレ ギア。 | GitHub ハンドル、contacto prioritydo、huso horario、acceptacion del CoC。 |マンテナー コホーテ ペケナス。優先順位は、投稿の内容を修正します。 |

改訂情報の重要性:

1. プレビューで許容される成果物の政治的調査。
2. セグリダード/観察の追加情報を確認する
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
3. Aceptar ejecutar `docs/portal/scripts/preview_verify.sh` 安全なサービス
   ローカルのスナップショット。

## フルホデ摂取

1. 完全なメールの勧誘
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   Formulario (コピー/ペガー・アン・ナ発行)。キャプチャー・アル・メノス: 身元確認、連絡方法、
   GitHub ハンドル、リビジョンの事前確認とセキュリティ ドキュメントの確認。
2. トラッカーのレジストラ `docs-preview` (GitHub または政府のチケットの発行)
   y は aprobador を割り当てます。
3. 有効な前提条件:
   - CLA / acuerdo de contribucion en archive (o Referencia de contrato パートナー)。
   - 承認されたアルマセナドの要請を承認します。
   - 完全な評価 (評価、法務に関するパートナーの見直し)。
4. 追跡に関する問題に関する要求と解決策を提示する
   変更管理のエントリ (例: `DOCS-SORA-Preview-####`)。

## Aprovisionamiento y herramientas

1. **アーティファクトの比較** - 記述子と保存されたプレビューのアーカイブ
   CI のワークフローと SoraFS (アーティファクト `docs-portal-preview`) のピン。ロスレビューを記録する
   イジェクター:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **チェックサムの強制サーバー** - チェックサムの改訂コマンドを示します:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Esto reutiliza `scripts/serve-verified-preview.mjs` para que no selance un build sin verificar
   偶然です。

3. **GitHub にアクセスします (オプション)** - 必要な情報を改訂し、公開する必要はありません。
   GitHub `docs-preview` はリビジョンとレジストラのメンバーとしての要求を継続します。

4. **交通機関の連絡方法** - オンコール連絡先 (Matrix/Slack) と手順の比較
   [`incident-runbooks`](./incident-runbooks.md) の事件。

5. **テレメトリア + フィードバック** - 分析分析を記録した記録
   (ver [`observability`](./observability.md))。プラントからのフィードバックに比例した計算式
   招待状やレジストラ、イベント、ヘルパーの発行に関する参照
   [`preview-feedback-log`](./preview-feedback-log) 定期的に再開します。

## 改訂チェックリスト

プレビューに同意する前に、情報を完全に改訂してください:1. 検証ロス・アルティファクト・デスカルガドス (`preview_verify.sh`)。
2. `npm run serve` (または `serve:verified`) 経由の Lanzar ポータルで、チェックサムの保護を確認します。
3. 安全な情報を確認し、監視します。
4. OAuth/Try it を使用してログインし、デバイス コード (アプリケーション) と再利用トークンの生成を確認します。
5. レジストラ・ホールズゴス・エン・エル・トラッカー・アコルダード（発行、文書比較、公式）および倫理
   タグとリリースとプレビューを管理します。

## メンテナーとオフボーディングの責任

|ファセ |アクシオネス |
| --- | --- |
|キックオフ | [`preview-feedback-log`](./preview-feedback-log) 経由で `invite-sent` を介して、要請に応じて摂取するためのチェックリストを確認し、成果物と指示を比較し、改訂期間中のスケジュールを同期します。セマナ。 |
|モニターレオ |テレメトリのプレビューを監視 (バスカー トラフィック、通常どおり試してみて、調査の結果を表示) し、発生するアルゴ ソスペショソのインシデントとランブックを監視します。レジストラ イベント `feedback-submitted`/`issue-opened` は、満天の星を祝うために必要なイベントに準拠しています。 |
|オフボーディング | GitHub の一時的なリボカー SoraFS、レジストラー `access-revoked`、申請用のアーカイブ (フィードバックの再開 + 保留中のアクションを含む)、レジストリの改訂の実際。改訂版の削除により、ロケールと補助的なダイジェスト ジェネレータが作成されます [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)。 |

米国は、ローターの見直し手続きを進めています。 Mantener el rastro en el repo (問題 + plantillas) アユダ
DOCS-SORA は監査可能であり、プレビューの確認を許可します。
los はドキュメンタドを制御します。

## Plantillas の招待と追跡

- Inicia todo アウトリーチ コンエル
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  アーカイブ。法的ミニモのキャプチャ、チェックサムのプレビューと期待値の説明
  政治の見直しは受け入れられます。
- `<preview_tag>`、`<request_ticket>` およびカナレスのプランティーリャ、レムプラザ ロス プレースホルダーの編集
  デ・コンタクト。決勝戦のチケットの見直し、アプロバドレスの確認
  テキストを正確に参照してください。
- 招待状、実際の追跡、タイムスタンプ `invite_sent_at` の問題の解決
  あなたの報告書を精査してください
  [プレビュー招待フロー](./preview-invite-flow.md) 自動検出器を検出します。