---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# パブリック プレビュー

## Цели программы

パブリック プレビューを公開、パブリック プレビューを公開、公開プレビューを公開
ワークフローが完了します。 DOCS-SORA を使用して、
гарантируя, что каждое приглазение уходит с проверяемыми артефактами, инструкциями
Собезопасности и ясным каналом обратной связи.

- **Аудитория:** курированный список членов сообщества, партнеров и мейнтейнеров, которые
  使用可能なプレビューを表示します。
- **Ограничения:** размер волны по умолчанию <= 25 ревьюеров, окно доступа 14 日, реакция на
  24 時間対応。

## Чеклист ゲート перед запуском

重要な情報:

1. プレビュー - CI (`docs-portal-preview`,
   チェックサム マニフェスト、記述子、SoraFS バンドル)。
2. `npm run --prefix docs/portal serve` (チェックサム ゲート) タグ。
3. Тикеты онбординга ревьюеров одобрены и связаны с волной приглазений.
4. セキュリティ、可観測性、およびインシデントの保護
   ([`security-hardening`](./security-hardening.md)、
   [`observability`](./observability.md)、
   [`incident-runbooks`](./incident-runbooks.md))。
5. フィードバックと問題のテンプレート (重大度、重要度、
   информация об окружении)。
6. ドキュメント/DevRel + ガバナンスを統合します。

## Пакет приглазения

メッセージ:

1. **Проверенные артефакты** — SoraFS マニフェスト/プランおよび GitHub アーティファクト、
   チェックサム マニフェストと記述子。 Явно укажите команду верификации, чтобы
   問題は、そのような問題を解決することです。
2. **サーブ** — チェックサム ゲートのプレビューを表示します:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Напоминания по безопасности** — Укажите, что токены истекают автоматически, ссылки нельзя
   делиться、а инциденты нужно сообщать немедленно。
4. **Канал обратной связи** — 問題テンプレート/форму и обозначьте ожидания по времени ответа。
5. **Даты программы** — Укажите даты начала/окончания、オフィスアワー、同期、更新。

Пример письма в
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
покрывает эти требования。プレースホルダ (даты、URL、контакты)
перед отправкой。

## Публикация プレビュー ホスト

プレビュー ホストを確認し、チケットを変更してください。
См. [露出プレビュー ホスト](./preview-host-exposure.md) エンドツーエンドの接続
ビルド/公開/検証。

1. **ビルドと упаковка:** リリース タグと артерминированные артефакты を作成します。

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

   Скрипт ピン записывает `portal.car`、`portal.manifest.*`、`portal.pin.proposal.json`、
   `portal.dns-cutover.json` × `artifacts/sorafs/`。 Приложите эти файлы к волне
   пригласений、чтобы каждый ревьюер мог проверить те же биты。

2. **Публикация プレビュー エイリアス:** Повторите команду без `--skip-submit`
   (укажите `TORII_URL`、`AUTHORITY`、`PRIVATE_KEY[_FILE]`、および доказательство エイリアス от ガバナンス)。
   マニフェスト к `docs-preview.sora` と выдаст
   `portal.manifest.submit.summary.json` は `portal.pin.report.json` 証拠バンドルです。

3. **Проба деплоя:** Убедитесь、что エイリアス разрезается および checksum соответствует タグ
   перед отправкой приглаприглазений。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) のフォールバック、
   プレビュー エッジを確認するだけで、プレビュー エッジを確認できます。

## Таймлайн коммуникаций

| День | Действие |オーナー |
| --- | --- | --- |
| D-3 |テスト、テスト、予行演習 |ドキュメント/開発リリース |
| D-2 |ガバナンスの承認 + チケットの変更 |ドキュメント/DevRel + ガバナンス |
| D-1 | Отправить приглазения по заблону, обновить tracker со списком получателей |ドキュメント/開発リリース |
| D |キックオフ コール / 営業時間、営業時間 телеметрии |ドキュメント/DevRel + オンコール |
| D+7 |フィードバック ダイジェスト、問題のトリアージ |ドキュメント/開発リリース |
| D+14 | `status.md` | Закрыть волну, отозвать временный доступ, опубликовать summary в `status.md` |ドキュメント/開発リリース |

## Трекинг доступа и телеметрия

1. フィードバック ロガーのプレビュー、タイムスタンプ、タイムスタンプの表示
   (см. [`preview-feedback-log`](./preview-feedback-log))、чтобы каждая волна разделяла
   証拠の痕跡:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```推奨事項: `invite-sent`、`acknowledged`、`feedback-submitted`、
   `issue-opened`、`access-revoked`。 Лог по умолчанию находится в
   `artifacts/docs_portal_preview/feedback_log.json`; приложите его к тикету волны
   Єормами согласия。ヘルパーの概要、説明、要約
   ロールアップの表示:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   概要 JSON の概要、フィードバック、フィードバックの概要
   タイムスタンプが表示されます。ヘルパー основан на
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)、
   CI のワークフローを確認します。ダイジェスト版をご覧ください
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   要約を要約します。
2. Тегируйте телеметрийные ダッシュボード `DOCS_RELEASE_TAG`、использованным для волны、чтобы пики
   グループは、グループのメンバーです。
3. `npm run probe:portal -- --expect-release=<tag>` をデプロイし、実行します。
   リリース メタデータのプレビュー。
4. ランブックとコホートを確認します。

## フィードバックとフィードバック

1. 問題掲示板からのフィードバック。 Маркируйте элементы `docs-preview/<wave>`,
   オーナーのロードマップが表示されます。
2. 概要とプレビュー ロガーの概要、詳細な情報を表示します。
   `status.md` (участники、ключевые находки、планируемые фиксы) および обновите `roadmap.md`、
   マイルストーン DOCS-SORA となります。
3. オフボーディングを解除する
   [`reviewer-onboarding`](./reviewer-onboarding.md): отзовите доступ, архивируйте заявки и
   そうです。
4. チェックサム ゲートのチェックサム ゲート
   Собновив саблон приглазения с новыми датами.

プレビューを監査可能かどうかを確認してください。
Docs/DevRel は、GA を参照してください。