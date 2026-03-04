---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プレビューへの招待状

## プロポジト

ロードマップ **DOCS-SORA** の項目は、オンボーディングの改訂とプレビュー公開プログラムの招待を特定し、ポータル サルガのベータ版を公開します。このページでは、招待状の内容について説明し、試聴可能なデモを招待したり、招待状を作成したりできます。ウサラジュントコン:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) パラ エル マネホ ポート リバイザ。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) チェックサムのパラメータ。
- [`devportal/observability`](./observability.md) テレメトリのエクスポートとアラートのフック。

## デオラスを計画する

|オラ |オーディエンシア |参加基準 |サリダの基準 |メモ |
| --- | --- | --- | --- | --- |
| **W0 - メンテナ コア** |ドキュメント/SDK の管理者が内容を検証します。 | Equipo GitHub `docs-portal-preview` ポブラド、`npm run serve` のチェックサム ゲート、7 つの直径のアラート マネージャー サイレンシオソ。 | Todos los docs P0 の見直し、バックログのエチケット、犯罪事件の不正行為。 |安全性を確認してください。招待状にメ​​ールを送信する必要はありません。プレビューの詳細を確認して、ソロで作業する必要はありません。 |
| **W1 - パートナー** |オペラドール SoraFS、インテグレーター Torii、政府秘密保持契約の改訂。 | W0 セラード、法律上の終点、プロキシ Try-it ステージング。 |パートナーのサインオフ (会社の発行) 認識、テレメトリア ムエストラ <=10 の同時改訂、14 ディアスでの罪の回帰。 |植物の招待状 + チケットの請求。 |
| **W2 - コミュニティ** |投稿者はコミュニティのリストの選択を行います。 | W1 セラード、緊急事態の訓練、実際の状況に関するよくある質問。 |フィードバック ディジェリド、ロールバックとプレビューのパイプラインを介して文書化された環境を 2 つ以上リリースします。 |同時に招待できる制限 (<=25) と継続的な管理。 |

`status.md` のドキュメントは、安全な状況を確認するためのプレビュー用の追跡サービスです。

## プリフライトのチェックリスト

プログラムの招待状の**条件**を完了してください:

1. **CI 担当者向けの成果物**
   - 最大 `docs-portal-preview` + 記述子カルガド ポル `.github/workflows/docs-portal-preview.yml`。
   - `docs/portal/docs/devportal/deploy-guide.md` の SoraFS のピン (カットオーバーの記述子)。
2. **チェックサムの強制**
   - `npm run serve` 経由で `docs/portal/scripts/serve-verified-preview.mjs` を呼び出します。
   - macOS + Linux での `scripts/preview_verify.sh` の手順。
3. **テレメトリのベースライン**
   - `dashboards/grafana/docs_portal.json` 交通渋滞 `docs.preview.integrity` 最高の交通情報をお試しください。
   - `docs/portal/docs/devportal/observability.md` の実際の Grafana に関する付録。
4. **ゴベルナンザの工芸品**
   - 招待トラッカー リストを発行します (番号を発行しません)。
   - Plantilla de registro de revisores copiada (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - 問題に応じて、SRE の法的対応と補助的な要求。

フライト前に最終的な登録を行って、トラッカーを招待して、実際の環境を確認してください。

## パソス・デル・フルホ

1. **候補者の選択**
   - エクストラ・デ・ラ・ホハ・デ・エスペラ・オ・コーラ・デ・パートナー。
   - 完全なプランティージャの候補を見つけることができます。
2. **アプロバーアクセソ**
   - 招待トラッカーの問題を報告します。
   - 前提条件の検証 (CLA/コントラート、使用可能、簡単な説明)。
3. **羨ましいご招待**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`、`<request_ticket>`、コンタクト) のプレースホルダーが完全に失われています。
   - 補助的な記述子 + アーカイブのハッシュ、URL のステージング、試してみてください。
   - 問題に関する最終メール (Matrix/Slack のトランスクリプト) を保護します。
4. **Rastear のオンボーディング**
   - `invite_sent_at`、`expected_exit_at`、および確立 (`pending`、`active`、`complete`、`revoked`) のトラッカーを招待します。
   - 監査に関する監査の要求。
5. **監視テレメトリ**
   - `docs.preview.session_active` と `TryItProxyErrors` として警告します。
   - ベースラインとレジストラからの招待状の登録結果を基に、テレメトリーで発生した出来事を報告します。
6. **フィードバックとセラーを収集**
   - `expected_exit_at` のフィードバック ファイルを参照してください。
   - 再開されたコルト (ハラスゴス、事件、緊急事態) に関する問題を実際に解決します。

## 証拠とレポート|アーティファクト |ドンデ・ガーダー |実際のカデンシア |
| --- | --- | --- |
|招待状トラッカーの発行 |プロジェクト GitHub `docs-portal-preview` |招待状を実際に入手します。 |
|改訂名簿をエクスポート | `docs/portal/docs/devportal/reviewer-onboarding.md` で登録 |セマナル。 |
|テレメトリアのスナップショット | `docs/source/sdk/android/readiness/dashboards/<date>/` (テレメトリの再利用バンドル) |ポル・オラ+事件の起こり。 |
|フィードバックのダイジェスト | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (クリアカーペットポルオラ) | 5 つのディアス トラス サリル デ ラ オラのデントロ。 |
|ゴベルナンザの再会について | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | DOCS-SORA の同期が完了しました。 |

エジェクタ `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
マキナの読みやすいダイジェストを作成しないでください。補助的な JSON レンダリングは、問題を解決するために問題の見直しを確認し、招待状のコンテオスを確認し、ログを再現します。

`status.md` の証拠のリストは、ロードマップの実際の急速な進行状況を確認するために必要です。

## ロールバックと一時停止の基準

Pausa el flujo de invitaciones (y notifica a gobernanza) cuando ocurra cualquiera de estos casos:

- プロキシの未発生のロールバックを試してください (`npm run manage:tryit-proxy`)。
- アラートの詳細: エンドポイントごとに 3 つ以上のアラート ページがあり、7 つのディアでプレビューが表示されます。
- 報告書: 登録者からの登録を依頼するための招待状。
- 統合の問題: `scripts/preview_verify.sh` のチェックサム検出の不一致。

Reanuda は、48 時間にわたって、トラッカーを招待してテレメトリのダッシュボードを確認し、修復を記録します。