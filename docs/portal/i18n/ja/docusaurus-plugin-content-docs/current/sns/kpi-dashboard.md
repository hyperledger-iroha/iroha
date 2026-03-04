---
id: kpi-dashboard
lang: ja
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
# Sora Name Service KPI ダッシュボード

KPIダッシュボードは、スチュワード、ガーディアン、規制担当者が月次アネックスのサイクル(SN-8a)に入る前に、採用、エラー、収益のシグナルを確認できる単一の場所を提供します。Grafana定義はリポジトリの `dashboards/grafana/sns_suffix_analytics.json` に含まれており、ポータルは埋め込みiframeで同じパネルを表示するため、内部Grafanaインスタンスと同じ体験になります。

## フィルターとデータソース

- **サフィックスフィルター** – `sns_registrar_status_total{suffix}` クエリを駆動し、`.sora`、`.nexus`、`.dao` を個別に確認できるようにします。
- **一括リリースフィルター** – `sns_bulk_release_payment_*` メトリクスの範囲を限定し、財務が特定のレジストラマニフェストを照合できるようにします。
- **メトリクス** – Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`)、guardian CLI (`guardian_freeze_active`)、`sns_governance_activation_total`、および一括オンボーディングヘルパーのメトリクスから取得します。

## パネル

1. **登録(過去24h)** – 選択したサフィックスに対する成功したレジストライベントの数。
2. **ガバナンスの有効化(30d)** – CLIが記録したチャーター/追補のモーション。
3. **レジストラのスループット** – サフィックスごとの成功したレジストラアクションのレート。
4. **レジストラのエラーモード** – エラーラベル付き `sns_registrar_status_total` カウンタの5分レート。
5. **ガーディアンのフリーズウィンドウ** – `guardian_freeze_active` がオープンなフリーズチケットを報告しているライブセレクタ。
6. **資産別ネット支払い単位** – `sns_bulk_release_payment_net_units` が資産別に報告する合計。
7. **サフィックス別一括リクエスト** – サフィックスIDごとのマニフェスト量。
8. **リクエストあたりのネット単位** – リリースメトリクスから算出したARPU型の計算。

## 月次KPIレビューチェックリスト

財務リードは毎月第1火曜日に定例レビューを実施します:

1. ポータルの **Analytics → SNS KPI** ページ(またはGrafanaダッシュボード `sns-kpis`)を開く。
2. レジストラのスループットと収益テーブルのPDF/CSVエクスポートを取得する。
3. SLA違反を確認するためサフィックスを比較する(エラー率スパイク、凍結セレクタ >72 h、ARPU差分 >10%)。
4. `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` 配下の該当アネックスに要約とアクション項目を記録する。
5. エクスポートしたダッシュボード成果物をアネックスのコミットに添付し、評議会アジェンダにリンクする。

レビューでSLA違反が判明した場合、該当オーナー(registrar duty manager, guardian on-call, または steward program lead)に対してPagerDutyインシデントを起票し、アネックスログで是正を追跡します。
