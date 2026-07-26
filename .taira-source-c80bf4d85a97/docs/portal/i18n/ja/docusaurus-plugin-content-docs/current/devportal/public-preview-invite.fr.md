---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プレイブックの招待プレビュー公開

## プログラムの目的

Ce プレイブックの明示的なコメント アナウンサーとフェア ツアーナーのプレビュー公開 une fois que le
レビュー担当者のオンボーディングのワークフローが実行されます。 Il garde la roadmap DOCS-SORA 正直なところ
保証付きの招待状の部分は、検証可能な成果物、安全な委託品を保証します
et un chemin clair pour le フィードバック。

- **対象者:** コミュニティのメンバー、パートナー、メンテナーの質問をリストします。
  プレビュー期間中は許容される使用法に関する署名。
- **Plafonds:** デフォルトの漠然とした内容 <= 25 人の査読者、14 時間のフェネトレ、
  24 時間体制で事故に対応します。

## ゲート・デ・ランスメントのチェックリスト

Terminez ces taches avant d'envoyer une Invitation:

1. CI でのプレビュー料金の詳細なアーティファクト (`docs-portal-preview`、
   チェックサム、記述子、バンドル SoraFS) のマニフェスト。
2. `npm run --prefix docs/portal serve` (ゲート パー チェックサム) ミーム タグのテスト。
3. チケットはレビュー担当者が承認し、曖昧な招待状を承認します。
4. ドキュメントの安全性、監視性、およびインシデントの有効性
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
5. フィードバックの定式化と問題のテンプレートの準備 (厳しい要求を含む)
   複製、スクリーンショットと環境情報のテープ）。
6. Docs/DevRel + Governance に従って改訂をコピーします。

## 招待状のパケット

Chaque の招待状には以下が含まれます:

1. **アーティファクトの検証** - マニフェスト/プラン SoraFS ou les artifacts
   GitHub、およびチェックサムおよび記述子のマニフェスト。コマンドによる参照者の明示
   検証は、レビュー担当者がサイトを実行する前に行う必要があります。
2. **提供手順** - チェックサムごとにコマンド プレビュー ゲートを含めます:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **安全な懸垂下降** - 期限切れの自動トークン、先取特権の無効化
   ne doivent past etre Partages, et que les incivent doivent etre は直ちに通知します。
4. **フィードバックの方法** - テンプレート/公式および応答の注意事項を明確にします。
5. **プログラムの日付** - デビュー/フィンの日付、オフィスアワー、同期、その他プロチェーンの日付
   フェネトレ・ド・リフレッシュ。

例をメールで送信
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
クーブル・セ・エクシジェンス。 Mettre a jour les プレースホルダー (日付、URL、連絡先)
アバンレンヴォイ。

## Exposer l'hote プレビュー

オンボーディングの終了と変更のチケットの承認については、ホテルのプレビューを確認してください。
Voir le [ホテルの展示ガイド](./preview-host-exposure.md) エンドツーエンドのエテープを注ぐ
ビルド/公開/検証はセクションに従って使用されます。

1. **ビルドとパッケージング:** リリース タグと成果物の製造方法が決定されます。

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

   Le script de pin ecrit `portal.car`、`portal.manifest.*`、`portal.pin.proposal.json`、
   et `portal.dns-cutover.json`、つまり `artifacts/sorafs/`。曖昧な表現で参加する
   招待状は、私のレビュー担当者、検証者、ミームのビットを参照してください。

2. **パブリッシャー別名プレビュー:** Relancer la commande sans `--skip-submit`
   (fournir `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, et la preuve d'alias emise
   政府による）。スクリプトは `docs-preview.sora` などのマニフェストに嘘をつきます
   `portal.manifest.submit.summary.json` プラス `portal.pin.report.json` プル バンドル ド プルーブ。

3. **プロバール展開:** au タグに対応するエイリアス リソースとチェックサムの確認者
   前衛的な招待状。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) スー・ラ・メイン・コム・フォールバック・プール
   クエリレビュー担当者は、ランサーを使用してロケールをコピーし、エッジプレビューフランシュを実行します。

## コミュニケーションのタイムライン

|ジュール |アクション |オーナー |
| --- | --- | --- |
| D-3 |招待状のコピー、アーティファクトの作成、検証の予行演習のファイナライザー |ドキュメント/開発リリース |
| D-2 |承認ガバナンス + チケット変更 |ドキュメント/DevRel + ガバナンス |
| D-1 |招待状はテンプレートを介して招待状を送信し、宛先のリストを追跡して、定期的な追跡を行うことができます。ドキュメント/開発リリース |
| D |キックオフ コール / オフィス アワー、テレメトリーのダッシュボードの監視 |ドキュメント/DevRel + オンコール |
| D+7 |曖昧なフィードバックのダイジェスト、ブロカンテの問題のトリアージ |ドキュメント/開発リリース |
| D+14 | `status.md` | 曖昧な内容を削除し、一時的なアクセスを取り消し、履歴書を発行します。ドキュメント/開発リリース |

## アクセスとテレメトリ1. 登録者の登録先、招待のタイムスタンプ、および失効の平均日付
   プレビュー フィードバック ロガー (voir
   [`preview-feedback-log`](./preview-feedback-log)) アフィン ケ チャク 曖昧なパートジュ ラ ミーム
   トレース・ド・プリューブ:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   安全なイベント `invite-sent`、`acknowledged`、
   `feedback-submitted`、`issue-opened`、および `access-revoked`。ル・ログ・セット・トルーベ
   `artifacts/docs_portal_preview/feedback_log.json` デフォルト。ジョワネス・ル・オー・チケット
   曖昧な招待状の形式的な同意。概要のヘルパーを活用する
   監査可能な事前準備のロールアップと製造作業の注釈:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   概要 JSON 列挙の招待状、漠然とした目的地、結果
   フィードバックとタイムスタンプを夕方に計算します。ヘルパーの休息を
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)、
   ミーム ワークフローは、CI のロケールを監視します。ダイジェストファイルテンプレートの利用者
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   曖昧な要約の出版物。
2. テレメトリのダッシュボードをタグ付けして `DOCS_RELEASE_TAG` を使用し、漠然とした情報を提供する
   招待状と一緒に写真を撮りましょう。
3. デプロイ後の実行者 `npm run probe:portal -- --expect-release=<tag>` 確認者キュー
   環境プレビューでは、ボンヌ メタデータのリリースをお知らせします。
4. 荷送人は、ランブックのテンプレートやコホートに関するインシデントを宣伝します。

## フィードバックとクロージャ

1. 問題を解決するために、ドキュメントの一部としてフィードバックを収集します。アイテムの平均タグ付け
   `docs-preview/<wave>` は、所有者がロードマップを再構築するための機能を提供します。
2. プレビュー ロガーの概要を確認して、曖昧な関係を確認するために使用します。
   履歴書ラ コホルテ ダン `status.md` (参加者、主要な統計、以前の修正) など
   1 週間の `roadmap.md` si le jalon DOCS-SORA 変更。
3. Suivre les etapes d'offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md): アクセスを取り消し、要求をアーカイブする
   ルメルシエの参加者。
4. アーティファクト、チェックサムに関する曖昧なプロチェーンの作成者
   その他、新しい日付の招待状のテンプレートを表示します。

プログラムのプレビューを監査可能にし、一貫性のあるアプリケーターのプレイブック
Docs/DevRel では、GA へのアプローチを測定するための、壮大なイベントへの繰り返しの招待状を用意しています。