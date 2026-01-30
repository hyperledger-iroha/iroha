---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-invite-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77c8bb984de9d4a9222f2d4eca5d5de5d47f8a7131fe59ec8b5a369ba9add8
source_last_modified: "2025-11-14T04:43:20.043603+00:00"
translation_last_reviewed: 2026-01-30
---

# プレビュー招待フロー

## 目的

ロードマップ項目 **DOCS-SORA** は、レビュアーのオンボーディングと公開プレビュー招待プログラムを、ポータルがベータを抜ける前の最終ブロッカーとして挙げています。このページでは、各招待ウェーブの開き方、招待送信前に出荷すべきアーティファクト、そしてフローの監査性を示す方法を説明します。以下と併せて使用してください:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) (レビュアー別の取り扱い)
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) (チェックサム保証)
- [`devportal/observability`](./observability.md) (テレメトリー出力とアラートフック)

## ウェーブ計画

| ウェーブ | 対象 | 入口条件 | 退出条件 | 注記 |
| --- | --- | --- | --- | --- |
| **W0 - コアメンテナー** | 初期コンテンツを検証する Docs/SDK メンテナー。 | GitHub チーム `docs-portal-preview` が登録済み、`npm run serve` のチェックサムゲートが緑、Alertmanager が7日間静穏。 | すべての P0 ドキュメントをレビュー済み、バックログにタグ付け、ブロッカーなし。 | フロー検証用。招待メールは送らず、プレビュー成果物のみ共有。 |
| **W1 - パートナー** | SoraFS オペレーター、Torii インテグレーター、NDA 下のガバナンスレビュアー。 | W0 完了、法務条件承認、Try-it プロキシが staging。 | パートナーのサインオフ (issue または署名済みフォーム) を取得、テレメトリーで同時レビュアー <=10、14 日間セキュリティ回帰なし。 | 招待テンプレート + 申請チケットを必須化。 |
| **W2 - コミュニティ** | コミュニティ待機リストから選抜された貢献者。 | W1 完了、インシデントドリル済み、公開 FAQ 更新済み。 | フィードバック消化、プレビュー パイプライン経由のドキュメントリリースが >=2 回、rollback なし。 | 同時招待を <=25 に制限し、週次でバッチ処理。 |

どのウェーブがアクティブかを `status.md` とプレビューリクエストトラッカーに記録し、ガバナンスが一目で把握できるようにします。

## プレフライトチェックリスト

ウェーブの招待を予定する **前** に次を完了してください:

1. **CI 成果物の準備**
   - `.github/workflows/docs-portal-preview.yml` により最新の `docs-portal-preview` + descriptor がアップロード済み。
   - SoraFS pin が `docs/portal/docs/devportal/deploy-guide.md` に記載済み (cutover descriptor が存在)。
2. **チェックサム強制**
   - `docs/portal/scripts/serve-verified-preview.mjs` が `npm run serve` 経由で起動される。
   - `scripts/preview_verify.sh` の手順が macOS + Linux でテスト済み。
3. **テレメトリーのベースライン**
   - `dashboards/grafana/docs_portal.json` が Try it の健全なトラフィックを示し、`docs.preview.integrity` アラートがグリーン。
   - `docs/portal/docs/devportal/observability.md` の最新付録が Grafana リンクで更新済み。
4. **ガバナンス成果物**
   - 招待トラッカー issue が準備済み (ウェーブごとに 1 issue)。
   - レビュアー登録テンプレートがコピー済み (参照: [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - 法務および SRE の必須承認が issue に添付済み。

招待メール送信前に、プレフライト完了を invite tracker に記録してください。

## フロー手順

1. **候補の選定**
   - 待機リストスプレッドシートまたはパートナーキューから抽出。
   - 各候補が申請テンプレートを完了済みであることを確認。
2. **アクセス承認**
   - invite tracker issue に承認者を割り当てる。
   - 前提条件 (CLA/契約、許容利用、セキュリティブリーフ) を確認する。
3. **招待送信**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) のプレースホルダ (`<preview_tag>`, `<request_ticket>`, 連絡先) を埋める。
   - descriptor + archive hash、Try it の staging URL、サポートチャネルを添付する。
   - 最終メール (または Matrix/Slack のトランスクリプト) を issue に保存する。
4. **オンボーディング追跡**
   - invite tracker に `invite_sent_at`、`expected_exit_at`、ステータス (`pending`, `active`, `complete`, `revoked`) を更新する。
   - 監査性のため、レビュアーの申請リンクを紐付ける。
5. **テレメトリー監視**
   - `docs.preview.session_active` と `TryItProxyErrors` アラートを監視する。
   - テレメトリーがベースラインから逸脱したらインシデントを起こし、結果を招待エントリの横に記録する。
6. **フィードバック収集と終了**
   - フィードバックが到着するか `expected_exit_at` を過ぎたら招待を閉じる。
   - 次のコホートに移る前に、ウェーブ issue を短いサマリ (所見、インシデント、次アクション) で更新する。

## 証拠とレポート

| アーティファクト | 保存場所 | 更新頻度 |
| --- | --- | --- |
| invite tracker issue | GitHub プロジェクト `docs-portal-preview` | 招待ごとに更新。 |
| レビュアーロスターのエクスポート | `docs/portal/docs/devportal/reviewer-onboarding.md` にリンクされたレジストリ | 週次。 |
| テレメトリー スナップショット | `docs/source/sdk/android/readiness/dashboards/<date>/` (テレメトリーバンドルを再利用) | 各ウェーブ + インシデント後。 |
| フィードバック ダイジェスト | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (ウェーブごとにフォルダ作成) | ウェーブ終了後 5 日以内。 |
| ガバナンス会議メモ | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | 各 DOCS-SORA ガバナンス同期の前に作成。 |

各バッチ後に `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
を実行して機械可読なダイジェストを作成します。レンダリングした JSON をウェーブ issue に添付し、ガバナンスレビュー担当が全ログを再生せずに招待数を確認できるようにします。

ウェーブが終了するたびに証拠リストを `status.md` に添付し、ロードマップ項目を素早く更新できるようにしてください。

## ロールバックと一時停止の基準

次のいずれかが発生した場合、招待フローを停止 (およびガバナンスに通知) します:

- rollback が必要な Try it プロキシのインシデント (`npm run manage:tryit-proxy`).
- アラート疲労: 7 日以内にプレビュー専用エンドポイントのアラートページが >3。
- コンプライアンスギャップ: 署名済み条件がない、または申請テンプレートの記録がないまま招待を送信。
- 整合性リスク: `scripts/preview_verify.sh` によって checksum mismatch が検出された。

invite tracker に remediation を記録し、テレメトリーダッシュボードが少なくとも 48 時間安定していることを確認してから再開してください。
