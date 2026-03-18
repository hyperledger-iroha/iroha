---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: プレビュー-フィードバック-w1-plan
タイトル: План プリフライト для партнеров W1
サイドバーラベル: План W1
説明: Задачи、владельцы и чек-лист доказательств для партнерской プレビュー - когорты.
---

| Пункт |ださい |
| --- | --- |
| Волна | W1 - と интеграторы Torii |
| Целевое окно | 2025 年第 2 四半期 3 |
| Тег артефакта (план) | `preview-2025-04-12` |
| Трекер | `DOCS-SORA-Preview-W1` |

## Цели

1. ガバナンスとプレビューを確認します。
2. プロキシを試してみてください。
3. チェックサム - プレビュー - プローブを確認します。
4. 最高のパフォーマンスを発揮します。

## Разбивка задач

| ID | Задача | Владелец | Срок | Статус | Примечания |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Получить юридическое одобрение на дополнение условий プレビュー |ドキュメント/DevRel リード -> 法務 | 2025-04-05 | ✅ Заверзено | Юридический тикет `DOCS-SORA-Preview-W1-Legal` одобрен 2025-04-05; PDF が表示されます。 |
| W1-P2 |ステージング-окно Try it プロキシ (2025-04-10) と проверить прокси | Зафиксировать staging-окноドキュメント/DevRel + オペレーション | 2025-04-06 | ✅ Заверзено | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` выполнено 2025-04-06; CLI および `.env.tryit-proxy.bak` です。 |
| W1-P3 |プレビュー - 説明 (`preview-2025-04-12`)、`scripts/preview_verify.sh` + `npm run probe:portal`、記述子/チェックサム |ポータルTL | 2025-04-08 | ✅ Заверзено | Артефакт и логи проверки сохранены в `artifacts/docs_preview/W1/preview-2025-04-12/`;プローブ приложен к трекеру。 |
| W1-P4 |摂取量を確認する (`DOCS-SORA-Preview-REQ-P01...P08`)、NDA を取得する |ガバナンス連絡窓口 | 2025-04-07 | ✅ Заверзено | Все восемь запросов одобрены (последние два 2025-04-11); Ссылки на одобрения в трекере. |
| W1-P5 | Подготовить текст приглазения (на базе `docs/examples/docs_preview_invite_template.md`), задать `<preview_tag>` и `<request_ticket>` для каждого партнера |ドキュメント/DevRel リード | 2025-04-08 | ✅ Заверзено | 2025 年 4 月 12 日 15:00 UTC から、今日までに ссылками на артефакт が表示されます。 |

## プリフライト чек-лист

> Совет: запустите `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`、чтобы автоматически выполнить заги 1-5 (ビルド、チェックサム、ポータル プローブ、リンク チェッカー、および Try it プロキシ)。 Скрипт записывает JSON-лог、который можно приложить к issue трекера.

1. `npm run build` (с `DOCS_RELEASE_TAG=preview-2025-04-12`) と `build/checksums.sha256` および `build/release.json`。
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` または `build/link-report.json` 記述子。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (つまり、указать нужный ターゲット через `--tryit-target`); `.env.tryit-proxy` と `.bak` がロールバックされました。
6. W1 の問題を解決します (チェックサム記述子、プローブ、Try it プロキシ、Grafana スナップショット)。

## Чек-лист доказательств

- [x] Подписанное юридическое одобрение (PDF или ссылка на тикет) приложено к `DOCS-SORA-Preview-W1`.
- [x] Grafana は、`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` です。
- [x] 記述子とチェックサム - `preview-2025-04-12` と `artifacts/docs_preview/W1/`。
- [x] Таблица 名簿 с заполненными `invite_sent_at` (см. W1 ログ в трекере)。
- [x] Артефакты обратной связи отражены в [`preview-feedback/w1/log.md`](./log.md) с одной строкой на партнера (обновлено) 2025-04-26 日名簿/テレメトリア/問題)。

Обновляйте этот план по мере продвижения;監査可能性のロードマップを確認します。

## Процесс обратной связи

1. Для каждого レビュアー дублировать заблон
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)、
   заполнить метаданные и сохранить готовую копию в
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. チェックポイントと問題を解決する
   [`preview-feedback/w1/log.md`](./log.md)、ガバナンス審査員による評価
   полностью、не покидая репозиторий。
3. 知識チェック или опросов, прикреплять их по пути артефакта, указанному в логе,
   и связывать с трекера を発行します。