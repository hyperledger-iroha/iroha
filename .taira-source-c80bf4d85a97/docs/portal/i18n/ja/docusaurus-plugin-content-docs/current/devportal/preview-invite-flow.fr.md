---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Flux d'invitation プレビュー

## 目的

ロードマップ **DOCS-SORA** の要素は、オンボーディングの選考者および招待プレビューのプログラムを引用し、ベータ版の前にパブリック ブロックを公開します。 Cette ページの批評コメントは、曖昧な招待状、招待状、および監査可能なコメントを証明するために必要なアーティファクトを確認します。アベックの利用:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) 選択者からの質問を注ぎます。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) チェックサムの保証。
- [`devportal/observability`](./observability.md) テレメトリーのエクスポートとアラートのフック。

## 漠然とした計画

|あいまい |観客 |エントリー基準 |出撃基準 |メモ |
| --- | --- | --- | --- | --- |
| **W0 - メンテナ コア** |メンテナのドキュメント/SDK の継続的な検証。 | GitHub `docs-portal-preview` を実行し、ゲート チェックサム `npm run serve` を en vert、Alertmanager は 7 時間沈黙します。 | Tous les docs P0 relus、バックログタグ、aucun Incident bloquant。 |より有効なファイルフラックスをサートします。招待状の電子メール、芸術品のプレビューの詳細。 |
| **W1 - パートナー** |オペレータ SoraFS、インテグレータ Torii、選任者が NDA を管理します。 | W0 の終了、司法の承認、プロキシ Try-it のステージング。 |承認パートナーは収集 (式の署名を発行)、テレメトリ モントレ <=10 の同時選出者、パス ド リグレッションのセキュリティ ペンダント 14 時間。 |招待状 + チケット デデマンドのインポーザー テンプレート。 |
| **W2 - コミュノート** |寄稿者の選択は、通信リストのリストです。 | W1 は終了し、訓練は繰り返され、FAQ は 1 時間で公開されます。 |フィードバックが異なります。2 件以上のドキュメントは、ロールバックなしでパイプライン プレビュー経由で迅速にリリースされます。 |リミッターは同時参加者 (<=25) とバッチャー チャケ セメインを招待します。 |

アクティブな `status.md` およびトラッカーの要求プレビューに関する文書は、政府の法規に関する情報を提供します。

## チェックリストのプリフライト

曖昧な招待状を注ぐ計画立案者の行動 **前衛**:

1. **アーティファクト CI の責任**
   - デルニエ `docs-portal-preview` + 記述子料金パー `.github/workflows/docs-portal-preview.yml`。
   - ピン SoraFS と `docs/portal/docs/devportal/deploy-guide.md` に注意してください (カットオーバーの記述子が存在します)。
2. **チェックサムの適用**
   - `docs/portal/scripts/serve-verified-preview.mjs` は `npm run serve` 経由で呼び出します。
   - 手順 `scripts/preview_verify.sh` の受験者は macOS + Linux を使用します。
3. **ベースラインテレメトリ**
   - `dashboards/grafana/docs_portal.json` 交通状況を確認してください。 `docs.preview.integrity` est au vert.
   - `docs/portal/docs/devportal/observability.md` の別館 Grafana を参照してください。
4. **遺物の管理**
   - 招待トラッカーの問題 (曖昧な問題)。
   - 登録テンプレートのコピー (voir [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - 承認法および SRE は添付職員に発行を要求します。

フライト前に招待トラッカーの電子メールを登録します。

## エテープ・デュ・フラックス

1. **候補者の選択者**
   - ファイル パートナーの情報リストを作成します。
   - 要求を満たしたテンプレートを保証する候補者を決定します。
2. **承認者へのアクセス**
   - 招待トラッカーの問題の承認者。
   - 前提条件の検証 (CLA/契約、使用可能、簡単なセキュリティ)。
3. **招待状の使者**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`、`<request_ticket>`、連絡先) のプレースホルダーのコンプリーター。
   - 結合ファイル記述子 + アーカイブのハッシュ、ステージングの URL などのサポートをお試しください。
   - 最終的な電子メール (マトリックス/スラックの記録) を発行します。
4. **新人研修**
   - 1 時間の招待トラッカーの平均 `invite_sent_at`、`expected_exit_at`、およびステータス (`pending`、`active`、`complete`、`revoked`) を測定します。
   - 審査員の要求事項を監査します。
5. **遠隔測定の監視者**
   - 監視者 `docs.preview.session_active` および警告 `TryItProxyErrors`。
   - ベースラインのテレメトリー装置と登録者が招待状を提出する際に、事件が発生したときにその結果を記録します。
6. **フィードバックを収集して分類する**
   - 招待状のフィードバックが `expected_exit_at` で到着します。
   - 法廷での曖昧な履歴書 (統計、事件、一連の行動) を前もって調べてください。

## 証拠と報告|アーティファクト |ああストッカー | 1 時間のリズム |
| --- | --- | --- |
|招待トラッカーの問題 |プロジェクト GitHub `docs-portal-preview` |チャックへの招待状を 1 時間かけて飲みます。 |
|名簿選出者の輸出 | `docs/portal/docs/devportal/reviewer-onboarding.md` | に登録してください。ヘブドマデール。 |
|スナップショットテレメトリー | `docs/source/sdk/android/readiness/dashboards/<date>/` (再利用ファイル バンドル テレメトリ) |曖昧な + アフターインシデント。 |
|ダイジェストフィードバック | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (曖昧な文書の作成) |漠然とした出撃の5時間。 |
|同窓会ガバナンスのノート | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | remplir アバンチャク同期ガバナンス DOCS-SORA。 |

ランセズ `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
マシンごとに消化可能な後、バッチで生産物を注ぎます。 Joignez le JSON は、ログを参照せずに、曖昧な問題を解決するために、選出者による統治の確認を行います。

Joignez la liste de preuves `status.md` は、漠然としたメイン ロードマップを 1 時間のペースで確認します。

## ロールバックと一時停止の基準

招待状の流動性を一時停止し、生存者に通知する必要があります:

- インシデント プロキシ ロールバックが必要な場合は試してください (`npm run manage:tryit-proxy`)。
- アラートの疲労: 7 時間にわたってエンドポイントのプレビューのみで 3 つ以上のアラート ページが表示されます。
- Ecart de conformite: 期限付き署名なしでの招待使節、要求のテンプレートなしでの登録なし。
- 危険な危険性: `scripts/preview_verify.sh` によるチェックサム検出の不一致。

Reprenez は、48 時間以内に安定したダッシュボードのテレメトリーを確認して、トラッカーを招待し、修復に関する文書を確認します。