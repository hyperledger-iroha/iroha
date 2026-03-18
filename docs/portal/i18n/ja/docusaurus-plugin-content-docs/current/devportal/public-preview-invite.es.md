---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プレビュー公開の招待状のプレイブック

## プログラムの目的

エステ プレイブックの詳細な発表とプレビューの公開
オンボーディングとエステ活動の見直しのワークフロー。 Mantiene 正直なロードマップ DOCS-SORA al
確実に成果物を羨望の的に見ることができ、安全性を保証することができます
カミノクラロデフィードバック。

- **対象読者:** 共同体メンバーのリスト、パートナーおよびメンテナーのリスト
  政府の政治政策のプレビューが受け入れられます。
- **制限:** タマノ デ オラ ポル ディフェクト <= 25 修正、ベンタナ デ アクセソ デ 14 ディアス、レスプエスタ
  24時間いつでも事件が起きる。

## ランサミエント門のチェックリスト

より良い招待状を記入してください:

1. CI のプレビュー カルガドの究極の成果物 (`docs-portal-preview`、
   チェックサム、記述子、バンドル SoraFS) のマニフェスト。
2. `npm run --prefix docs/portal serve` (ゲートアド ポート チェックサム) エラー タグを禁止します。
3. 招待状のオンボーディングと改訂のチケット。
4. セキュリティ文書、事件の検証、監視
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
5. フィードバックや問題の準備の定式化 (重大な問題を含む)
   再現手順、スクリーンショット、およびエンターノ情報)。
6. Docs/DevRel + Governance の改訂テキスト。

## 招待状を渡す

招待状の内容:

1. **Artefactos verificados** - Proporciona enlaces al manifyto/plan de SoraFS o a los
   GitHub のマニフェストとチェックサムと記述子の成果物。コマンドの参照
   レバンタルの安全性を確認するための明示的な見直し
   エル・サイトオ。
2. **サービスの指示** - チェックサムのプレビューコマンドが含まれています:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Recordatorios de seguridad** - インディカ ケ ロス トークンの有効期限が自動的に切れ、リンクが失われる
   事件や事件に関する報道は行われません。
4. **フィードバックの連絡方法** - プラント/フォーミュラリオとアクララの期待値を確認します。
5. **プログラムのスケジュール** - 開始/終了のプロポーシオナ、オフィス時間、同期、近くの時間
   ベンタナ・デ・リフレッシュ。

メールでのメール
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
クブレ・エストス・リクイシトス。 Actualiza los プレースホルダー (フェチャ、URL、連絡先)
アンビアル。

## ホストのプレビューの説明者

ソロプロモーションとプレビュー、オンボーディング、カンビオチケットの完了
エステ・アプロバド。 [ホストのプレビューに関する説明](./preview-host-exposure.md) を参照してください。
エンドツーエンドで構築/公開/検証を行うことができます。

1. **エンパケタドを構築する:** マルカ エル リリース タグは、決定的な成果物を生み出します。

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

   `portal.car`、`portal.manifest.*`、`portal.pin.proposal.json`、を記述したスクリプト
   y `portal.dns-cutover.json` バジョ `artifacts/sorafs/`。ラ・オラ・デの補助的なアーカイブ
   招待状は、ミスモスのビットを検証するプエダ・パラ・ケ・カダ・リバイザーです。

2. **プレビューの公開エイリアス:** コマンドを繰り返して `--skip-submit`
   (proporciona `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y la prueba de alias Emida
   ポル・ゴベルナンザ）。 El スクリプト enlazara el マニフェスト `docs-preview.sora` y Emiira
   `portal.manifest.submit.summary.json` mas `portal.pin.report.json` は証拠のバンドルです。

3. **プロバーエルデスリーグ:** エイリアスの結果とチェックサムがタグと一致することを確認します
   羨望の招待状。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantener `npm run serve` (`scripts/serve-verified-preview.mjs`) マノ コモ フォールバック パラ
   プエダン レヴァンタル ウナ コピア ローカル サイト エッジ デ プレビュー フォールアを確認してください。

## コミュニケーションのタイムライン

|ダイヤ |アクシオン |オーナー |
| --- | --- | --- |
| D-3 |招待状の最終コピー、再作成アーティファクト、検証のドライラン |ドキュメント/開発リリース |
| D-2 |サインオフ・デ・ゴベルナンサ + チケット・デ・カンビオ |ドキュメント/DevRel + ガバナンス |
| D-1 |プランティラの招待状、目的地リストの実際のトラッカーを参照 |ドキュメント/開発リリース |
| D |キックオフ コール / オフィス アワー、テレメトリのダッシュボードを監視 |ドキュメント/DevRel + オンコール |
| D+7 |オラのフィードバックのダイジェスト、ブロカントの問題のトリアージ |ドキュメント/開発リリース |
| D+14 | Cerrar ola、一時的な削除、公開再開 en `status.md` |ドキュメント/開発リリース |

## テレメトリーへのアクセスを確保1.登録先の登録、招待のタイムスタンプ、および取り消しの連絡先
   プレビュー フィードバック ロガー (ver
   [`preview-feedback-log`](./preview-feedback-log)) 問題は解決します
   証拠の記録:

   ```bash
   # Agrega un nuevo evento de invitacion a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   ロス イベントトス ソポルタドス ソン `invite-sent`、`acknowledged`、
   `feedback-submitted`、`issue-opened`、y `access-revoked`。エル・ログ・ヴィベ・エン
   `artifacts/docs_portal_preview/feedback_log.json` 欠陥あり。追加チケット
   ラ・オラ・デ・インビタシオネス・ジュント・コン・ロス・デ・コンセントイミエント。アメリカ エル ヘルパー デ
   プロデューサーと再開の監査可能な手順の概要:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   概要 JSON 列挙型の招待状、目的地、コンテオス デ
   イベントのタイムスタンプをフィードバックしてください。エル ヘルパー エスタ レスパルダード ポル
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)、
   ミスモ ワークフローは CI のローカル管理で発生します。アメリカの植物のダイジェスト版
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ラオラの要約を公開します。
2. `DOCS_RELEASE_TAG` に関するテレメトリア ダッシュボードのルール
   ロス ピコ セ プエダン コルレラシオナル コン ラス コホーテス デ 招待。
3. Ejecuta `npm run probe:portal -- --expect-release=<tag>` デプロイパラ確認
   リリースのメタデータ修正をプレビュー通知します。
4. ランブックおよびコホートのプラントに関する事件の登録。

## フィードバックとシエール

1. 問題を文書で比較し、フィードバックを集約します。エチケットアイテムコン
   `docs-preview/<wave>` は、オーナーのロードマップに関する相談を容易にします。
2. アメリカのサリダの概要とプレビュー ロガーのポブラーとレポート、ルエゴの履歴書
   la cohorte en `status.md` (参加者、ホールズゴス プリンシパル、修正プラナド) y
   実際の `roadmap.md` シエルヒト DOCS-SORA カンビオ。
3. オフボーディングを楽しむ
   [`reviewer-onboarding`](./reviewer-onboarding.md): 同意の取り消し、アーカイブの要求 y
   勝ち負けの参加者。
4. チェックサムの再取得、再参照の情報を準備します。
   実際のプランティーリャ デ 招待状コンヌエバス フェチャス。

監査可能なプレビュー形式の一貫したプログラムのプレイブックを作成する
Docs/DevRel を、ポータル サイトからの繰り返しのエスカレーション招待状に追加します。
アセルカ、GA。