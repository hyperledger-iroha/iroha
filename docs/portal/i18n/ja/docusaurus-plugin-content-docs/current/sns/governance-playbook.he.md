---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 212c7a8f5f6e24ed81fde84f2480e596602c253dfdf86fd686c6a894a46b53e2
source_last_modified: "2025-11-14T04:43:21.335593+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正規ソース
このページは `docs/source/sns/governance_playbook.md` を反映しており、
現在はポータルの正規コピーとして扱われます。翻訳PRのためにソース
ファイルは維持されます。
:::

# Sora Name Service ガバナンス・プレイブック (SN-6)

**状態:** 2026-03-24 起草 - SN-1/SN-6 準備の生きた参照  
**ロードマップリンク:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", ADDR-1/ADDR-5 アドレスポリシー  
**前提条件:** レジストリスキーマ [`registry-schema.md`](./registry-schema.md)、registrar API 契約 [`registrar-api.md`](./registrar-api.md)、アドレス UX ガイダンス [`address-display-guidelines.md`](./address-display-guidelines.md)、およびアカウント構造ルール [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md)。

このプレイブックは、Sora Name Service (SNS) のガバナンス組織がチャーター
を採択し、登録を承認し、紛争をエスカレーションし、resolver と gateway の
状態が同期していることを証明する方法を説明します。`sns governance ...`
CLI、Norito マニフェスト、監査アーティファクトが N1 (一般公開) 前に単一の
運用参照を共有するというロードマップ要件を満たします。

## 1. 範囲と対象

本ドキュメントの対象:

- チャーター、サフィックスポリシー、紛争結果に投票するガバナンス評議会
  のメンバー。
- 緊急フリーズを発行し、解除をレビューする guardian ボードのメンバー。
- registrar キュー、オークション、収益分配を運用するサフィックス steward。
- SoraDNS 伝播、GAR 更新、テレメトリガードレールを担当する resolver/gateway
  オペレーター。
- すべてのガバナンス行為が監査可能な Norito アーティファクトを残したことを
  示す必要があるコンプライアンス、財務、サポートの各チーム。

`roadmap.md` に列挙されたクローズドベータ (N0)、一般公開 (N1)、拡張 (N2)
フェーズを対象とし、各ワークフローを必要な証跡、ダッシュボード、
エスカレーション経路に紐づけます。

## 2. 役割と連絡マップ

| 役割 | 主な責務 | 主なアーティファクトとテレメトリ | エスカレーション |
|------|----------|-----------------------------------|------------------|
| ガバナンス評議会 | チャーター、サフィックスポリシー、紛争評決、steward ローテーションを策定・批准。 | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, `sns governance charter submit` で保存される評議会投票。 | 議長 + ガバナンス議事トラッカー。 |
| guardian ボード | soft/hard フリーズ、緊急カノン、72 h レビューを発行。 | `sns governance freeze` が発行する guardian チケット、`artifacts/sns/guardian/*` に記録される override マニフェスト。 | guardian on-call ローテーション (<=15 min ACK)。 |
| サフィックス steward | registrar キュー、オークション、価格階層、顧客連絡を運用; コンプライアンスを確認。 | `SuffixPolicyV1` の steward ポリシー、価格参照シート、規制メモと併置される steward acknowledgement。 | steward プログラムリード + サフィックス別 PagerDuty。 |
| registrar/請求オペレーション | `/v1/sns/*` エンドポイントの運用、支払調整、テレメトリ出力、CLI スナップショット維持。 | registrar API ([`registrar-api.md`](./registrar-api.md)), `sns_registrar_status_total` メトリクス, `artifacts/sns/payments/*` に保存する支払証跡。 | registrar duty manager と財務リエゾン。 |
| resolver/gateway オペレーター | SoraDNS、GAR、gateway 状態を registrar イベントに整合; 透明性メトリクスをストリーム。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | resolver SRE on-call + gateway ops ブリッジ。 |
| 財務/会計 | 70/30 収益分配、紹介 carve-out、税務/財務申告、SLA 認証を適用。 | 収益累計マニフェスト、Stripe/財務エクスポート、`docs/source/sns/regulatory/` の四半期 KPI 付録。 | 財務コントローラ + コンプライアンス責任者。 |
| コンプライアンス/規制リエゾン | EU DSA などの義務追跡、KPI covenant 更新、開示提出。 | `docs/source/sns/regulatory/` の規制メモ、参照デッキ、`ops/drill-log.md` の tabletop リハーサル記録。 | コンプライアンスプログラムリード。 |
| サポート / SRE オンコール | 事故対応 (衝突、請求ドリフト、resolver 障害)、顧客連絡、ランブックのオーナー。 | 事故テンプレート、`ops/drill-log.md`, 検証ラボ証跡, `incident/` に保存する Slack/war-room 記録。 | SNS on-call ローテーション + SRE マネジメント。 |

## 3. 正規アーティファクトとデータソース

| アーティファクト | 位置 | 目的 |
|------------------|------|------|
| チャーター + KPI 付録 | `docs/source/sns/governance_addenda/` | バージョン管理された署名済みチャーター、KPI covenant、CLI 投票で参照されるガバナンス決定。 |
| レジストリスキーマ | [`registry-schema.md`](./registry-schema.md) | 正規 Norito 構造 (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| registrar 契約 | [`registrar-api.md`](./registrar-api.md) | REST/gRPC ペイロード、`sns_registrar_status_total` メトリクス、ガバナンス hook 期待値。 |
| アドレス UX ガイド | [`address-display-guidelines.md`](./address-display-guidelines.md) | ウォレット/エクスプローラが再現する IH58（推奨）/圧縮（次善）表現の正規版。 |
| SoraDNS / GAR ドキュメント | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | ホストの決定的導出、透明性テーラーのワークフロー、アラートルール。 |
| 規制メモ | `docs/source/sns/regulatory/` | 司法管轄別の受入メモ (例: EU DSA)、steward acknowledgement、テンプレート付録。 |
| drill log | `ops/drill-log.md` | フェーズ退出前に必要なカオス/IR リハーサル記録。 |
| アーティファクト保管 | `artifacts/sns/` | 支払証跡、guardian チケット、resolver 差分、KPI エクスポート、`sns governance ...` が生成する署名済み CLI 出力。 |

すべてのガバナンス行為は、上記テーブルの少なくとも 1 つの
アーティファクトを参照し、監査者が 24 時間以内に意思決定の経路を
再構築できるようにします。

## 4. ライフサイクル・プレイブック

### 4.1 チャーターと steward のモーション

| ステップ | オーナー | CLI / 証拠 | メモ |
|---------|---------|------------|------|
| 付録ドラフトと KPI 差分 | 評議会ラポーター + steward リード | `docs/source/sns/governance_addenda/YY/` の Markdown テンプレート | KPI covenant ID、テレメトリ hook、起動条件を含める。 |
| 提案提出 | 評議会議長 | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` を生成) | CLI が `artifacts/sns/governance/<id>/charter_motion.json` に Norito マニフェストを出力。 |
| 投票と guardian acknowledgement | 評議会 + guardians | `sns governance ballot cast --proposal <id>` と `sns governance guardian-ack --proposal <id>` | ハッシュ化された議事録とクオーラム証跡を添付。 |
| steward 受諾 | steward プログラム | `sns governance steward-ack --proposal <id> --signature <file>` | サフィックスポリシー変更前に必須; `artifacts/sns/governance/<id>/steward_ack.json` に記録。 |
| 有効化 | registrar ops | `SuffixPolicyV1` を更新し、registrar キャッシュを更新、`status.md` に告知を掲載。 | 有効化タイムスタンプを `sns_governance_activation_total` に記録。 |
| 監査ログ | コンプライアンス | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` に追記し、tabletop 実施時は drill log も更新。 | テレメトリダッシュボードとポリシー差分への参照を含める。 |

### 4.2 登録・オークション・価格承認

1. **Preflight:** registrar は `SuffixPolicyV1` を参照し、価格階層、利用可能
   期間、猶予/償還ウィンドウを確認します。ロードマップで示す 3/4/5/6-9/10+
   階層 (ベース階層 + サフィックス係数) の価格表と同期させます。
2. **sealed-bid オークション:** premium プールでは 72 h commit / 24 h reveal
   サイクルを `sns governance auction commit` / `... reveal` で実行します。
   監査者が乱数を検証できるよう、commit リスト (ハッシュのみ) を
   `artifacts/sns/auctions/<name>/commit.json` に公開します。
3. **支払検証:** registrar は `PaymentProofV1` を財務分配 (70% treasury / 30%
   steward、referral carve-out <=10%) と照合します。Norito JSON を
   `artifacts/sns/payments/<tx>.json` に保存し、registrar レスポンス
   (`RevenueAccrualEventV1`) に紐づけます。
4. **ガバナンス hook:** premium/guarded 名には `GovernanceHookV1` を付与し、
   評議会提案 ID と steward 署名を参照します。欠落すると
   `sns_err_governance_missing` になります。
5. **有効化 + resolver 同期:** Torii がレジストリエベントを発行したら、
   resolver の透明性 tailer を起動し、新しい GAR/zone 状態の伝播を確認します
   (4.5 を参照)。
6. **顧客開示:** [`address-display-guidelines.md`](./address-display-guidelines.md)
   の共有フィクスチャを使って顧客向けレジャー (wallet/explorer) を更新し、
   IH58 と圧縮レンダリングが copy/QR ガイダンスと一致することを確認します。

### 4.3 更新、請求、財務照合

- **更新フロー:** registrar は `SuffixPolicyV1` の 30 日グレース + 60 日償還
  ウィンドウを適用します。60 日経過後は、オランダ式再開シーケンス
  (7 日、10x 手数料が 15%/日で減衰) が `sns governance reopen` により自動起動します。
- **収益分配:** 更新や移転のたびに `RevenueAccrualEventV1` を作成します。
  財務エクスポート (CSV/Parquet) は毎日これらのイベントと照合し、
  `artifacts/sns/treasury/<date>.json` に証跡を添付します。
- **Referral carve-out:** 任意の referral 割合はサフィックスごとに
  `referral_share` を steward ポリシーへ追加して追跡します。registrar は
  最終分配を出力し、支払証跡の横に referral マニフェストを保存します。
- **レポート頻度:** 財務は月次 KPI 付録 (登録数、更新数、ARPU、紛争/bond 利用)
  を `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` に公開します。ダッシュボード
  は同じエクスポート表を参照し、Grafana の数値がレジャー証跡と一致するようにします。
- **月次 KPI レビュー:** 毎月第 1 火曜のチェックポイントで財務リード、当番 steward、
  プログラム PM が参加します。[SNS KPI dashboard](./kpi-dashboard.md)
  (ポータル埋め込み `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`) を開き、
  registrar の throughput + revenue 表をエクスポートし、差分を付録に記録して
  アーティファクトをメモに添付します。SLA 違反 (freeze ウィンドウ >72 h、
  registrar エラースパイク、ARPU ドリフト) が見つかった場合はインシデントを起票します。

### 4.4 フリーズ、紛争、控訴

| フェーズ | オーナー | アクションと証拠 | SLA |
|---------|---------|------------------|-----|
| soft freeze 申請 | steward / サポート | 支払証跡、紛争 bond 参照、影響セレクタを含むチケット `SNS-DF-<id>` を提出。 | <=4 h 以内に受付。 |
| guardian チケット | guardian ボード | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` が `GuardianFreezeTicketV1` を生成。チケット JSON を `artifacts/sns/guardian/<id>.json` に保存。 | <=30 min ACK, <=2 h 実行。 |
| 評議会承認 | ガバナンス評議会 | フリーズ承認/却下、guardian チケットと紛争 bond digest へのリンク付きで決定を記録。 | 次回評議会または非同期投票。 |
| 仲裁パネル | コンプライアンス + steward | 7 名の陪審パネル (ロードマップ準拠) を招集し、`sns governance dispute ballot` でハッシュ化された投票を提出。匿名投票の受領をインシデントパケットに添付。 | verdict <=7 日 (bond 入金後)。 |
| 控訴 | guardian + 評議会 | 控訴は bond を倍増し陪審プロセスを再実施; Norito マニフェスト `DisputeAppealV1` を記録し一次チケットを参照。 | <=10 日。 |
| 解凍と是正 | registrar + resolver ops | `sns governance unfreeze --selector <IH58> --ticket <id>` を実行し、registrar 状態を更新、GAR/resolver 差分を伝播。 | 判定後すぐ。 |

緊急カノン (guardian 起動のフリーズ <=72 h) も同じ流れですが、事後の評議会レビューと
`docs/source/sns/regulatory/` に透明性メモが必要です。

### 4.5 Resolver と Gateway の伝播

1. **イベント hook:** すべてのレジストリエベントは resolver イベントストリーム
   (`tools/soradns-resolver` SSE) に送られます。resolver ops が購読し、透明性 tailer
   (`scripts/telemetry/run_soradns_transparency_tail.sh`) で差分を記録します。
2. **GAR テンプレート更新:** gateway は `canonical_gateway_suffix()` が参照する GAR
   テンプレートを更新し、`host_pattern` リストを再署名します。差分は
   `artifacts/sns/gar/<date>.patch` に保存します。
3. **ゾーンファイル公開:** `roadmap.md` のゾーンファイル骨格 (name, ttl, cid, proof)
   を使用し、Torii/SoraFS に公開します。Norito JSON を
   `artifacts/sns/zonefiles/<name>/<version>.json` に保存します。
4. **透明性チェック:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   を実行し、アラートがグリーンであることを確認します。Prometheus のテキスト出力を
   週次透明性レポートに添付します。
5. **gateway 監査:** `Sora-*` ヘッダ (cache policy, CSP, GAR digest) のサンプルを記録し、
   ガバナンスログに添付して、意図したガードレールで新しい名前が提供されたことを証明します。

## 5. テレメトリとレポート

| シグナル | ソース | 説明 / アクション |
|----------|--------|-------------------|
| `sns_registrar_status_total{result,suffix}` | Torii registrar ハンドラ | 登録、更新、フリーズ、移転の成功/失敗カウンタ。`result="error"` の急増をサフィックス別にアラート。 |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii メトリクス | API ハンドラの遅延 SLO; `torii_norito_rpc_observability.json` を元にしたダッシュボードへ供給。 |
| `soradns_bundle_proof_age_seconds` と `soradns_bundle_cid_drift_total` | resolver 透明性 tailer | 期限切れの証明や GAR ドリフトを検出; ガードレールは `dashboards/alerts/soradns_transparency_rules.yml` に定義。 |
| `sns_governance_activation_total` | ガバナンス CLI | チャーター/付録の有効化ごとに増加するカウンタ; 評議会決定と公開付録の整合に使用。 |
| `guardian_freeze_active` gauge | guardian CLI | セレクタごとの soft/hard フリーズウィンドウを追跡; `1` が SLA を超える場合は SRE をページ。 |
| KPI 付録ダッシュボード | 財務 / Docs | 規制メモに合わせて月次ロールアップを公開; ポータルが [SNS KPI dashboard](./kpi-dashboard.md) 経由で埋め込み、steward と規制担当が同じ Grafana ビューにアクセス可能。 |

## 6. 証跡と監査要件

| アクション | 監査用に保存する証跡 | 保管先 |
|-----------|------------------------|--------|
| チャーター / ポリシー変更 | 署名済み Norito マニフェスト、CLI 文字列、KPI 差分、steward acknowledgement。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| 登録 / 更新 | `RegisterNameRequestV1` payload, `RevenueAccrualEventV1`, 支払証跡。 | `artifacts/sns/payments/<tx>.json`, registrar API ログ。 |
| オークション | commit/reveal マニフェスト、乱数シード、勝者計算スプレッドシート。 | `artifacts/sns/auctions/<name>/`. |
| フリーズ / 解凍 | guardian チケット、評議会投票ハッシュ、インシデントログ URL、顧客連絡テンプレート。 | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| resolver 伝播 | zonefile/GAR 差分、tailer JSONL 抜粋、Prometheus スナップショット。 | `artifacts/sns/resolver/<date>/` + 透明性レポート。 |
| 規制インテーク | 受入メモ、期限トラッカー、steward acknowledgement、KPI 変更サマリー。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. フェーズゲートのチェックリスト

| フェーズ | 退出条件 | 証跡バンドル |
|---------|---------|--------------|
| N0 - クローズドベータ | SN-1/SN-2 レジストリスキーマ、手動 registrar CLI、guardian ドリル完了。 | チャーターモーション + steward ACK、registrar ドライランログ、resolver 透明性レポート、`ops/drill-log.md` の記録。 |
| N1 - 一般公開 | `.sora`/`.nexus` のオークション + 固定価格階層、セルフサービス registrar、resolver 自動同期、請求ダッシュボード。 | 価格表差分、registrar CI 結果、支払/KPI 付録、透明性 tailer 出力、インシデントリハーサルノート。 |
| N2 - 拡張 | `.dao`、リセラー API、紛争ポータル、steward スコアカード、分析ダッシュボード。 | ポータルのスクリーンショット、紛争 SLA メトリクス、steward スコアカードのエクスポート、リセラー方針を参照した更新版チャーター。 |

フェーズ退出には、tabletop ドリル (登録ハッピーパス、フリーズ、resolver 障害) の
記録と `ops/drill-log.md` へのアーティファクト添付が必要です。

## 8. インシデント対応とエスカレーション

| トリガー | 重大度 | 直ちに対応する担当 | 必須アクション |
|---------|--------|---------------------|----------------|
| resolver/GAR のドリフトまたは証跡の古さ | Sev 1 | resolver SRE + guardian ボード | resolver on-call をページし、tailer 出力を保存、影響名をフリーズするか判断し、30 min ごとにステータス更新。 |
| registrar 障害、請求失敗、広範な API エラー | Sev 1 | registrar duty manager | 新規オークション停止、CLI 手動運用へ切替、steward/財務へ通知、Torii ログをインシデント文書に添付。 |
| 単一名の紛争、支払不一致、顧客エスカレーション | Sev 2 | steward + サポートリード | 支払証跡の収集、soft freeze の要否判断、SLA 以内で回答、結果を紛争トラッカーに記録。 |
| コンプライアンス監査指摘 | Sev 2 | コンプライアンスリエゾン | 是正計画を作成し `docs/source/sns/regulatory/` にメモを保存、フォローアップ評議会を予定。 |
| drill またはリハーサル | Sev 3 | プログラム PM | `ops/drill-log.md` のシナリオを実行し、アーティファクトを保存、ギャップをロードマップタスクとして記録。 |

すべてのインシデントは `incident/YYYY-MM-DD-sns-<slug>.md` を作成し、
オーナー表、コマンドログ、プレイブック全体で生成された証拠への参照を含めます。

## 9. 参照

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR セクション)

チャーター文面、CLI サーフェス、テレメトリ契約が変わるたびにこの
プレイブックを更新し、`docs/source/sns/governance_playbook.md` を参照する
ロードマップ項目が常に最新の改訂と一致するようにしてください。
