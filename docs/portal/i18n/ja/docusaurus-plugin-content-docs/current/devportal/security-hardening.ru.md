---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/security-hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ea461bcea26a5071ffd3c0ae95c9bb8e72fe1a1870ab8fd10e4d4cf13ef2d27b
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# セキュリティ強化とペンテストチェックリスト

## 概要

ロードマップ項目 **DOCS-1b** は、OAuth device-code login、強力なコンテンツセキュリティポリシー、
および再現可能なペネトレーションテストを、プレビューポータルがラボ外ネットワークで稼働する前に
必須としています。この付録では、脅威モデル、リポジトリに実装された制御、そしてゲートレビューが
実行すべき go-live チェックリストを説明します。

- **対象範囲:** Try it プロキシ、埋め込み Swagger/RapiDoc パネル、`docs/portal/src/components/TryItConsole.jsx`
  が描画するカスタム Try it コンソール。
- **対象外:** Torii 自体 (Torii readiness reviews で対応) と SoraFS の公開 (DOCS-3/7 で対応)。

## 脅威モデル

| 資産 | リスク | 緩和策 |
| --- | --- | --- |
| Torii bearer トークン | docs sandbox 外での盗難または再利用 | device-code login (`DOCS_OAUTH_*`) が短命トークンを発行し、プロキシがヘッダーをマスクし、コンソールがキャッシュ資格情報を自動失効する。 |
| Try it プロキシ | オープンリレーとしての悪用、または Torii の rate limit の回避 | `scripts/tryit-proxy*.mjs` が origin allowlists、rate limiting、health probes、明示的な `X-TryIt-Auth` 転送を強制する。資格情報は保存しない。 |
| ポータル runtime | クロスサイトスクリプティングまたは悪意ある埋め込み | `docusaurus.config.js` が Content-Security-Policy、Trusted Types、Permissions-Policy ヘッダーを注入。inline script は Docusaurus runtime に制限。 |
| Observability データ | テレメトリー欠落または改ざん | `docs/portal/docs/devportal/observability.md` が probes/dashboards を文書化。`scripts/portal-probe.mjs` が公開前に CI で実行。 |

敵対者には、公開プレビューを閲覧する好奇心のあるユーザー、盗まれたリンクを試す悪意ある行為者、
保存された資格情報の抽出を試みる侵害ブラウザが含まれます。すべての制御は信頼ネットワークのない
一般的なブラウザで動作する必要があります。

## 必須コントロール

1. **OAuth device-code login**
   - build 環境で `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` などの knobs を設定します。
   - Try it カードは sign-in ウィジェット (`OAuthDeviceLogin.jsx`) を描画し、
     device code を取得し、token endpoint をポーリングし、期限切れ時にトークンを自動クリアします。
     緊急時の fallback として手動の Bearer override も利用可能です。
   - OAuth 設定が欠落している場合、または fallback TTL が DOCS-1b で規定された 300-900 s の範囲外の場合、
     build は失敗します。`DOCS_OAUTH_ALLOW_INSECURE=1` は使い捨てのローカル preview のみに設定してください。
2. **Proxy guardrails**
   - `scripts/tryit-proxy.mjs` は allowed origins、rate limits、request size caps、upstream timeouts を適用し、
     トラフィックを `X-TryIt-Client` でタグ付けし、ログからトークンをマスクします。
   - `scripts/tryit-proxy-probe.mjs` と `docs/portal/docs/devportal/observability.md` が
     liveness probe と dashboard ルールを定義します。各 rollout 前に実行してください。
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` は決定的な security headers をエクスポートします:
     `Content-Security-Policy` (default-src self、厳格な connect/img/script リスト、Trusted Types 要件)、
     `Permissions-Policy`, `Referrer-Policy: no-referrer`。
   - CSP の connect リストは OAuth device-code と token endpoints を whitelist します
     (HTTPS のみ、`DOCS_SECURITY_ALLOW_INSECURE=1` の場合を除く)。これにより他の origin の sandbox を
     緩めずに device login が動作します。
   - headers は生成された HTML に直接埋め込まれるため、静的ホストの追加設定は不要です。
     inline script は Docusaurus の bootstrap に限定してください。
4. **Runbooks, observability, rollback**
   - `docs/portal/docs/devportal/observability.md` は login failures、proxy response codes、request budgets を監視する
     probes と dashboards を説明します。
   - `docs/portal/docs/devportal/incident-runbooks.md` は sandbox が悪用された場合のエスカレーション経路を示します。
     `scripts/tryit-proxy-rollback.mjs` と組み合わせて endpoints を安全に切り替えます。

## ペンテスト & リリースチェックリスト

各 preview promotion ごとに次の一覧を完了してください (結果を release チケットに添付):

1. **OAuth wiring を確認**
   - `npm run start` を本番 `DOCS_OAUTH_*` exports でローカル実行します。
   - クリーンなブラウザプロファイルから Try it コンソールを開き、device-code フローがトークンを発行し、
     寿命カウントダウンを行い、期限切れまたは sign-out 後にフィールドをクリアすることを確認します。
2. **Proxy を probe**
   - staging Torii に対して `npm run tryit-proxy` を実行し、続けて
     `npm run probe:tryit-proxy` を設定済み sample path で実行します。
   - ログで `authSource=override` エントリを確認し、rate limiting がウィンドウ超過時に
     カウンタを増加させることを確かめます。
3. **CSP/Trusted Types を確認**
   - `npm run build` を実行して `build/index.html` を開きます。`<meta
     http-equiv="Content-Security-Policy">` タグが想定のディレクティブと一致し、preview ロード時に
     DevTools が CSP violations を表示しないことを確認します。
   - `npm run probe:portal` (または curl) でデプロイ済み HTML を取得します。probe は
     `Content-Security-Policy`, `Permissions-Policy`, `Referrer-Policy` の meta tags が欠落しているか
     `docusaurus.config.js` の値と異なる場合に失敗するため、ガバナンスレビューは curl 出力を目視せずに
     exit code を信頼できます。
4. **Observability を確認**
   - Try it proxy ダッシュボードが green (rate limits, error ratios, health probe metrics) であることを確認します。
   - ホストが変更された場合 (新しい Netlify/SoraFS デプロイ) は
     `docs/portal/docs/devportal/incident-runbooks.md` の incident drill を実行します。
5. **結果を記録**
   - スクリーンショット/ログを release チケットに添付します。
   - 各 finding を remediation report テンプレートに記録します
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))。
     これにより owners, SLAs, retest evidence を後で監査しやすくします。
   - DOCS-1b の roadmap 項目が監査可能な状態を保つため、このチェックリストへのリンクを付けます。

いずれかの手順が失敗した場合はプロモーションを停止し、blocking issue を起票し、
`status.md` に remediation plan を記録してください。
