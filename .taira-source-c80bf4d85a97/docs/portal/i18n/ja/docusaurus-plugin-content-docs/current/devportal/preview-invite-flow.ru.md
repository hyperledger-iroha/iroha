---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Поток приглазений プレビュー

## Назначение

Пункт дорожной карты **DOCS-SORA** указывает onboarding ревьюеров и программу приглазений パブリック プレビュー как последние блокерыベータ版です。 Эта страница описывает, как открывать каждую волну приглазений, какие артефакты должны быть отправлены перед рассылкой и как доказать、что поток аудируем。必要な情報:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) が表示されます。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) チェックサム。
- [`devportal/observability`](./observability.md) はフックを提供します。

## План волн

| Волна | Аудитория | Критерии входа | Критерии выхода | Примечания |
| --- | --- | --- | --- | --- |
| **W0 - コアメンテナー** |メンテナ ドキュメント/SDK、今日の更新。 | GitHub `docs-portal-preview` 、ゲート チェックサム、`npm run serve` 、Alertmanager の日付が 7 日です。 | Все P0 доки просмотрены、バックログ промаркирован、блокирующих инцидентов нет。 | Используется для проверки потока;メールでプレビューを送信してください。 |
| **W1 - パートナー** | SoraFS、Torii、ガバナンス、NDA を要求します。 | W0 を使用して、プロキシとステージングを試してみてください。 |サインオフ партнеров (問題 или подписанная форма)、телеметрия показывает <=10 одновременных ревьюеров, нет регрессий 14日。 |チケットをリクエストしてください。 |
| **W2 - コミュニティ** |コミュニティの待機リストを参照してください。 | W1 については、FAQ をご覧ください。 |パイプラインのプレビューとロールバックが 2 つ以上発生しています。 | Ограничить одновременные приглазения (<=25) и батчить еженедельно. |

`status.md` とプレビュー リクエスト トラッカー、ガバナンスを監視します。

## プリフライト чеклист

***** からのメッセージ:

1. **Артефакты CI доступны**
   - Последний `docs-portal-preview` + 記述子 загружен `.github/workflows/docs-portal-preview.yml`。
   - SoraFS ピンと `docs/portal/docs/devportal/deploy-guide.md` (記述子カットオーバー присутствует)。
2. **Принудительный チェックサム**
   - `docs/portal/scripts/serve-verified-preview.mjs` は、`npm run serve` を表示します。
   - macOS + Linux の `scripts/preview_verify.sh` です。
3. **Базовая телеметрия**
   - `dashboards/grafana/docs_portal.json` показывает здоровый трафик 試してみてください。`docs.preview.integrity` зеленый。
   - Последнее приложение в `docs/portal/docs/devportal/observability.md` обновлено ссылками Grafana.
4. **ガバナンス**
   - 問題の招待トラッカー готов (одна 問題 на волну)。
   - Шаблон реестра ревьюеров скопирован (см. [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - SRE 承認の問題。

プリフライトとトラッカーの招待を確認できます。

## Шаги потока

1. **Выбор кандидатов**
   - 待機リストのスプレッドシートとパートナーのキューを表示します。
   - Убедиться、что у каждого кандидата заполнен リクエスト テンプレート。
2. **Одобрение доступа**
   - 承認者 - 問題の招待トラッカーを作成します。
   - Проверить требования (CLA/контракт、許容される使用法、セキュリティ概要)。
3. **Отправка приглазений**
   - [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, контакты) を確認します。
   - 記述子 + ハッシュ архива、ステージング URL と каналы поддержки を試してください。
   - Сохранить финальное письмо (Matrix/Slack トランスクリプト) の問題。
4. **オンボーディングの開始**
   - トラッカーを招待するには、`invite_sent_at`、`expected_exit_at`、および статусом (`pending`、`active`、`complete`、 `revoked`)。
   - 摂取リクエストを受け取る ревьюера для аудитабельности。
5. **Мониторинг телеметрии**
   - `docs.preview.session_active` と `TryItProxyErrors` を確認してください。
   - ベースラインとベースラインを確認し、その結果を確認します。
6. **Сбор фидбека и выход**
   - `expected_exit_at` を確認してください。
   - 問題が発生しました (находки、инциденты、следующие bolаги) перед переходом к следующей когорте。

## 証拠と証拠| Артефакт | Где хранить | Частота обновления |
| --- | --- | --- |
|招待トラッカーの問題 | GitHub は `docs-portal-preview` | Обновлять после каждого приглазения。 |
| Экспорт 名簿 ревьюеров | Реестр、связанный в `docs/portal/docs/devportal/reviewer-onboarding.md` | Еженедельно。 |
| Снимки телеметрии | `docs/source/sdk/android/readiness/dashboards/<date>/` (テレメトリ バンドル) | На каждую волну + после инцидентов. |
|フィードバックダイジェスト | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (標準) | 5 日以内に到着します。 |
|ガバナンス会議メモ | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | DOCS-SORA ガバナンス同期を実行します。 |

Запускайте `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
ビデオのダイジェストをご覧ください。 JSON の発行問題、ガバナンス、問題の解決、問題の解決Ссего лога。

Прикрепляйте список 証拠 к `status.md` при завергении каждой волны, чтобы дорожную карту можно было быстро обновить。

## ロールバックを実行する

ガバナンス (ガバナンス) の概要:

- プロキシを試行し、ロールバックしてください (`npm run manage:tryit-proxy`)。
- 通知: > 3 アラート ページ、プレビュー専用エンドポイント、7 日。
- リクエスト テンプレートの例: リクエスト テンプレート。
- 問題: チェックサムの不一致、обнаруженный `scripts/preview_verify.sh`。

トラッカーを招待して修復を行い、トラッカーを招待して、48 時間以内に連絡します。