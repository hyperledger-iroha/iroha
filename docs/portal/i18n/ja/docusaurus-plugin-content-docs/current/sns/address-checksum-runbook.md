---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 公式ソース
このページは `docs/source/sns/address_checksum_failure_runbook.md` を反映しています。まずソースファイルを更新し、その後このコピーを同期してください。
:::

チェックサムの失敗は `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) として Torii、SDK、wallet/explorer クライアントで発生する。ADDR-6/ADDR-7 のロードマップ項目により、チェックサムのアラートやサポートチケットが発火した場合はこのランブックに従うことが求められる。

## 実施する条件

- **アラート:** `AddressInvalidRatioSlo`（`dashboards/alerts/address_ingest_rules.yml` に定義）が発火し、注釈に `reason="ERR_CHECKSUM_MISMATCH"` が含まれる。
- **フィクスチャのドリフト:** Prometheus の textfile `account_address_fixture_status` または Grafana ダッシュボードが、いずれかの SDK コピーで checksum mismatch を報告する。
- **サポートのエスカレーション:** wallet/explorer/SDK チームが checksum エラー、IME の破損、またはクリップボードのスキャンがデコードできない事象を報告する。
- **手動観測:** Torii ログで `address_parse_error=checksum_mismatch` が本番エンドポイントに対して繰り返し出る。

Local-8/Local-12 の衝突に特化した事象の場合は、`AddressLocal8Resurgence` か `AddressLocal12Collision` の playbook を使用する。

## エビデンスチェックリスト

| 証跡 | コマンド / 場所 | メモ |
|------|-----------------|------|
| Grafana snapshot | `dashboards/grafana/address_ingest.json` | 無効理由の内訳と影響エンドポイントを取得する。 |
| アラート payload | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | コンテキストラベルとタイムスタンプを含める。 |
| fixture の健全性 | `artifacts/account_fixture/address_fixture.prom` + Grafana | SDK コピーが `fixtures/account/address_vectors.json` から逸脱したかを確認する。 |
| PromQL クエリ | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | 事故ドキュメント向けに CSV を出力する。 |
| ログ | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'`（またはログ集約） | 共有前に PII を削除する。 |
| fixture 検証 | `cargo xtask address-vectors --verify` | 正規ジェネレータとコミット済み JSON が一致することを確認する。 |
| SDK パリティチェック | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | アラート/チケットで報告された SDK ごとに実行する。 |
| クリップボード/IME 健全性 | `iroha address inspect <literal>` | 不可視文字や IME の書き換えを検出する。`address_display_guidelines.md` を参照。 |

## 即時対応

1. アラートを確認し、Grafana の snapshot と PromQL の出力をインシデントスレッドに共有し、影響を受けた Torii コンテキストを記録する。
2. アドレス解析に触れる manifest のプロモーションや SDK リリースを凍結する。
3. ダッシュボードの snapshot と生成した Prometheus textfile をインシデントフォルダ（`docs/source/sns/incidents/YYYY-MM/<ticket>/`）に保存する。
4. `checksum_mismatch` の payload を含むログサンプルを取得する。
5. SDK のオーナー（`#sdk-parity`）へサンプル payload を共有し triage を依頼する。

## 原因切り分け

### fixture またはジェネレータのドリフト

- `cargo xtask address-vectors --verify` を再実行し、失敗したら再生成する。
- `ci/account_fixture_metrics.sh`（または `scripts/account_fixture_helper.py check`）を SDK ごとに実行し、同梱 fixtures が正規 JSON と一致するか確認する。

### クライアントエンコーダ/IME の回帰

- `iroha address inspect` でユーザ提供の literal を調べ、幅ゼロ join、kana 変換、切り詰め payload を探す。
- `docs/source/sns/address_display_guidelines.md`（二重コピーの目標、警告、QR ヘルパー）に沿って wallet/explorer のフローを確認し、承認済み UX に従っているか検証する。

### manifest またはレジストリの問題

- `address_manifest_ops.md` に従って最新の manifest bundle を再検証し、Local-8 セレクタが再登場していないことを確認する。

### 悪意または不正なトラフィック

- Torii ログと `torii_http_requests_total` から不正な IPs/app IDs を分析する。
- Security/Governance 向けに少なくとも 24 時間のログを保存する。

## 緩和と復旧

| シナリオ | 対応 |
|----------|------|
| fixture のドリフト | `fixtures/account/address_vectors.json` を再生成し、`cargo xtask address-vectors --verify` を再実行し、SDK バンドルを更新して `address_fixture.prom` の snapshot をチケットに添付する。 |
| SDK/クライアント回帰 | 正規 fixture と `iroha address inspect` の出力を添えて issue を作成し、SDK パリティ CI（例: `ci/check_address_normalize.sh`）でリリースをゲートする。 |
| 悪意ある送信 | rate-limit またはブロックを適用し、セレクタの tombstone が必要なら Governance にエスカレーションする。 |

緩和後、PromQL のクエリを再実行し、`ERR_CHECKSUM_MISMATCH` が `/tests/*` を除いて 30 分以上ゼロで維持されることを確認してからインシデントを解除する。

## クローズ

1. Grafana の snapshot、PromQL の CSV、ログ抜粋、`address_fixture.prom` を保管する。
2. `status.md`（ADDR セクション）と roadmap の行を更新する（ツール/ドキュメントに変更がある場合）。
3. 新しい学びがあれば `docs/source/sns/incidents/` にポストモーテムを追加する。
4. SDK のリリースノートに checksum 修正が含まれることを確認する。
5. アラートが 24h 緑で維持され、fixture チェックも緑であることを確認して終了する。
