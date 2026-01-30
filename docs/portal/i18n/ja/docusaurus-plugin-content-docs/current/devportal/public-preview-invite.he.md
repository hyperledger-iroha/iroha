---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/public-preview-invite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bb3bcbcfd932a031518034de56cb89de642770f53f4fda98e0f1820aca0040d
source_last_modified: "2025-11-14T04:43:20.113656+00:00"
translation_last_reviewed: 2026-01-30
---

# 公開プレビュー招待プレイブック

## プログラムの目的

このプレイブックは、レビュアーのオンボーディングフローが稼働した後に公開プレビューを告知し、
運用する方法を説明します。各招待に検証可能なアーティファクト、セキュリティガイダンス、
明確なフィードバック経路が含まれることを保証し、DOCS-SORA のロードマップを正直に保ちます。

- **対象:** プレビューの acceptable-use policy に署名したコミュニティ、パートナー、メンテナの
  キュレーション済みリスト。
- **上限:** 既定のウェーブサイズは <= 25 レビュアー、アクセス期間は 14 日、インシデント対応は 24 時間以内。

## ローンチのゲートチェックリスト

招待を送る前に次を完了してください:

1. 最新のプレビューアーティファクトを CI にアップロード済み (`docs-portal-preview`,
   checksum manifest, descriptor, SoraFS bundle)。
2. `npm run --prefix docs/portal serve` (checksum ゲート) を同じ tag で検証済み。
3. レビュアーオンボーディングのチケットが承認され、招待ウェーブにリンク済み。
4. セキュリティ、observability、incident のドキュメントが検証済み
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. フィードバックフォームまたは issue テンプレートを準備 (severity、再現手順、スクリーンショット、
   環境情報の項目を含める)。
6. アナウンス文面を Docs/DevRel + Governance がレビュー済み。

## 招待パッケージ

各招待に含めるもの:

1. **検証済みアーティファクト** — SoraFS の manifest/plan または GitHub artefact へのリンクに加え、
   checksum manifest と descriptor を提供します。検証コマンドを明示し、レビュアーが
   サイトを起動する前に実行できるようにします。
2. **Serve 手順** — checksum ゲート付きのプレビューコマンドを記載:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **セキュリティ注意** — トークンは自動的に失効すること、リンクを共有しないこと、
   インシデントは直ちに報告することを明記します。
4. **フィードバック経路** — issue テンプレート/フォームへのリンクと、応答時間の期待値を明記します。
5. **プログラム日程** — 開始/終了日、office hours または sync ミーティング、
   次の refresh ウィンドウを記載します。

サンプルメールは
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
にあり、これらの要件を満たします。送信前にプレースホルダー (日付、URLs、連絡先)
を更新してください。

## プレビューホストの公開

オンボーディングが完了し、変更チケットが承認されるまではプレビューホストを宣伝しないでください。
このセクションで使用する build/publish/verify の end-to-end 手順は
[プレビューホスト公開ガイド](./preview-host-exposure.md) を参照してください。

1. **ビルドとパッケージ:** release tag を刻印し、決定的なアーティファクトを生成します。

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

   pin スクリプトは `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   `portal.dns-cutover.json` を `artifacts/sorafs/` に書き出します。これらのファイルを
   招待ウェーブに添付し、各レビュアーが同じビットを検証できるようにします。

2. **プレビューエイリアスの公開:** `--skip-submit` なしでコマンドを再実行します
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, およびガバナンス発行のエイリアス証明を指定)。
   スクリプトは manifest を `docs-preview.sora` にバインドし、
   `portal.manifest.submit.summary.json` と `portal.pin.report.json` を evidence bundle 用に出力します。

3. **デプロイのプローブ:** 招待を送る前に、エイリアスが解決され、checksum が tag と一致することを確認します。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) を手元の fallback として保持し、
   preview edge が不調な場合にレビュアーがローカルコピーを起動できるようにします。

## コミュニケーションタイムライン

| 日 | アクション | Owner |
| --- | --- | --- |
| D-3 | 招待文面の最終化、アーティファクト更新、検証の dry-run | Docs/DevRel |
| D-2 | ガバナンスの sign-off + 変更チケット | Docs/DevRel + Governance |
| D-1 | テンプレートで招待送信、受信者リストで tracker 更新 | Docs/DevRel |
| D | キックオフ呼び出し / office hours、テレメトリーダッシュボード監視 | Docs/DevRel + On-call |
| D+7 | 中間フィードバック digest、ブロッキング issue のトリアージ | Docs/DevRel |
| D+14 | ウェーブ終了、一時アクセス取り消し、`status.md` に要約を公開 | Docs/DevRel |

## アクセス追跡とテレメトリー

1. 各受信者、招待タイムスタンプ、取り消し日を preview feedback logger
   (参照: [`preview-feedback-log`](./preview-feedback-log)) に記録し、
   すべてのウェーブが同じエビデンスの足跡を共有できるようにします:

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json に新しい招待イベントを追加
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   対応イベントは `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, `access-revoked` です。ログは既定で
   `artifacts/docs_portal_preview/feedback_log.json` にあり、同意書と一緒に招待ウェーブ
   チケットへ添付します。クローズアウト前に summary helper を使って監査可能な
   ロールアップを生成してください:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   summary JSON はウェーブ別の招待数、未対応の受信者、フィードバック数、直近イベントの
   タイムスタンプを列挙します。helper は
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs) に基づいており、
   同じ workflow をローカルまたは CI で実行できます。ウェーブの recap を公開する際は
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   の digest テンプレートを使用してください。
2. ウェーブで使用した `DOCS_RELEASE_TAG` をテレメトリーダッシュボードに付与し、
   スパイクを招待コホートと相関できるようにします。
3. デプロイ後に `npm run probe:portal -- --expect-release=<tag>` を実行して、
   プレビュー環境が正しい release metadata を告知していることを確認します。
4. インシデントは runbook テンプレートに記録し、コホートにリンクします。

## フィードバックとクローズアウト

1. フィードバックを共有ドキュメントまたは issue ボードに集約し、
   `docs-preview/<wave>` のラベルで整理して roadmap owner が容易に検索できるようにします。
2. preview logger の summary 出力でウェーブレポートを作成し、
   `status.md` にコホート概要 (参加者、主要な発見、計画済み修正) を記載し、
   DOCS-SORA のマイルストーンが変わった場合は `roadmap.md` を更新します。
3. [`reviewer-onboarding`](./reviewer-onboarding.md) の offboarding 手順に従い、
   アクセスを取り消し、リクエストをアーカイブし、参加者に感謝を伝えます。
4. 次のウェーブに向けてアーティファクトを更新し、checksum のゲートを再実行し、
   招待テンプレートの日付を更新します。

このプレイブックを一貫して適用することで、プレビュー運用の監査性が保たれ、
Docs/DevRel はポータルが GA に近づくにつれて招待をスケールさせる再現可能な方法を持てます。
