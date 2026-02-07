---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fluxo がプレビューを招待します

## オブジェティボ

ロードマップ **DOCS-SORA** の項目は、オンボーディングの改訂とプログラムの招待をプレビュー公開し、ポータル サイトのベータ版を公開します。ページを開くと、招待状を作成し、招待状を作成したり、監査を行ったりすることができます。ジュントコムを使用してください:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) マネホ ポート リバイザーのパラメタ。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) チェックサムのパラメータ。
- [`devportal/observability`](./observability.md) テレメトリのエクスポートとアラートのフック。

## プラノ デ オンダス

|恩田 |オーディエンシア |参加基準 |基準 |メモ |
| --- | --- | --- | --- | --- |
| **W0 - メンテナ コア** |メンテナはドキュメント/SDK を検証し、継続的に実行します。 |時間 GitHub `docs-portal-preview` 人口、ゲート デ チェックサム `npm run serve` バージョン、アラート マネージャー サイレンシオソ ポート 7 ディアス。 | Todos os docs P0 改訂、バックログ タグエアド、セキュリティ インシデント ブロックアドール。 | Usado para validar o fluxo;電子メールで招待し、プレビューの詳細情報を確認します。 |
| **W1 - パートナー** |オペレーター SoraFS、インテグレーター Torii、政府の NDA 改訂。 | W0 のライセンス、アプロバドス ライセンス、プロキシ Try-it em ステージング。 |サインオフ dos パートナー coletado (問題の式を発行)、telemetria mostra <=10 revisores concorrentes、sem regressoes de seguranca por 14 dias のサインオフ。 |招待用のテンプレート + 請求用のチケット。 |
| **W2 - コミュニティ** |投稿者はコミュニティのリストの選択を行います。 | W1 の監視、緊急事態の訓練、一般公開に関するよくある質問。 |フィードバック ディジリド、プレビュー ロールバックのパイプライン経由でドキュメントを 2 つ以上リリースします。 |リミターは、合意 (<=25) と管理を強化します。 |

`status.md` に関する文書は、政府が迅速に調査するためにプレビューを要求するトラッカーではありません。

## プリフライトのチェックリスト

馬上で議題を決定する **予定**:

1. **CI ディスポンティブの技術**
   - Ultimo `docs-portal-preview` + 記述子 enviado por `.github/workflows/docs-portal-preview.yml`。
   - SoraFS および `docs/portal/docs/devportal/deploy-guide.md` のピン (カットオーバーの記述子)。
2. **チェックサムの強制**
   - `npm run serve` 経由で `docs/portal/scripts/serve-verified-preview.mjs` を呼び出します。
   - macOS + Linux の `scripts/preview_verify.sh` テストの手順。
3. **テレメトリのベースライン**
   - `dashboards/grafana/docs_portal.json` mostra trafego 試してみてください。
   - `docs/portal/docs/devportal/observability.md` の究極の付録は、Grafana の com リンクです。
4. **統治技術**
   - トラッカー プロンタを招待する問題を発行します (uma issue por onda)。
   - コピアドの改訂登録テンプレート (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - SRE に関する法律および問題に関する要求。

事前にメールを送信する前に、招待トラッカーを招待せずに、プリフライトを行うための結論を登録してください。

## Etapas はフラクソを行います

1. **選択候補者**
   - パートナーとの共同作業。
   - 完全なテンプレートを確認して候補者を確認してください。
2. **承認申請**
   - 問題を承認してトラッカーを招待します。
   - Verificar prerequisitos (CLA/contrato、uso aceitavel、brief de seguranca)。
3. **エンビアが招待します**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`、`<request_ticket>`、続き) のプリエンチャー OS プレースホルダー。
   - ディスクリプタ + ハッシュのアーカイブ、URL のステージングを試してみて、サポートできます。
   - Guardar または最終メール (Matrix/Slack のトランスクリプト) の問題。
4. **Acompanhar のオンボーディング**
   - トラッカー コム `invite_sent_at`、`expected_exit_at`、ステータス (`pending`、`active`、`complete`、`revoked`) を招待します。
   - 監査の見直しを依頼するためのリンク。
5. **監視テレメトリア**
   - オブザーバー `docs.preview.session_active` は `TryItProxyErrors` として警告します。
   - テレメトリ デスビアからの事件が発生し、ベースラインとレジストラの結果が報告され、報告が行われます。
6. **コレタールフィードバックエンサーラー**
   - Encerrar は、`expected_exit_at` expirar のフィードバック チェガーを招待します。
   - 問題は、すぐに解決する前に、すぐに解決する必要があります。

## 証拠と報告|アルテファト |オンデ・アルマゼナール | Cadencia de atualizacao |
| --- | --- | --- |
|トラッカーを招待する問題 | Projeto GitHub `docs-portal-preview` | Atualizar apos cada convite。 |
|改訂名簿をエクスポート |登録 vinculado em `docs/portal/docs/devportal/reviewer-onboarding.md` |セマナル。 |
|テレメトリアのスナップショット | `docs/source/sdk/android/readiness/dashboards/<date>/` (テレメトリの再利用バンドル) |ポル・オンダ+アポス事件。 |
|フィードバックのダイジェスト | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (クリアパスタポルオンダ) | 5 ディアス アポス ア セイダ ダ オンダ。 |
|統治の記録 | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` |事前同期 DOCS-SORA。 |

`cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`を実行
マキナの合法的なダイジェスト製品はたくさんあります。付録の JSON レンダリングは、問題の再現に関する問題のログとして、政府の改訂を確認するために必要です。

`status.md` の証拠リストを添付し、ロードマップを迅速に確認できるターミナル パスを確認してください。

## ロールバックと一時停止の基準

会議を一時停止します (政府に通知する) 緊急に会議を開きます:

- インシデント デ プロキシ ロールバック後に試行してください (`npm run manage:tryit-proxy`)。
- アラートの表示: エンドポイントごとに 7 日間で 3 つ以上のアラート ページがプレビューされます。
- コンプライアンスのギャップ: セキュリティ管理者、セキュリティ レジストラ、テンプレートの要請を奨励します。
- 統合の危険性: `scripts/preview_verify.sh` のチェックサム検出の不一致。

48 時間分の記録を記録し、招待トラッカーを招待せずに、テレメトリーのダッシュボードを確認してください。