---
lang: ja
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
یہ صفحہ `docs/source/sns/governance_playbook.md` کی عکاسی کرتا ہے اور اب پورٹل
٩ی کینونیکل کاپی ہے۔広報活動 広報活動 広報活動 広報活動 広報活動 広報活動 広報活動
:::

# ソラネームサービス گورننس پلی بک (SN-6)

**حالت:** 2026-03-24 مسودہ - SN-1/SN-6 تیاری کے لئے زندہ حوالہ  
** 説明:** SN-6「コンプライアンスと紛争解決」、SN-7「リゾルバーとゲートウェイ同期」、ADDR-1/ADDR-5  
**پیشگی شرائط:** رجسٹری اسکیمہ [`registry-schema.md`](./registry-schema.md) میں، رجسٹرار API معاہدہ [`registrar-api.md`](./registrar-api.md) میں، ایڈریس UX رہنمائی [`address-display-guidelines.md`](./address-display-guidelines.md) میں، اور اکاؤنٹ سٹرکچر قواعد [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) میں۔

یہ پلی بک بتاتی ہے کہ Sora Name Service (SNS) کی گورننس باڈیز کیسے چارٹر اپناتی
ہیں، رجسٹریشن منظور کرتی ہیں، تنازعات کو エスカレーション کرتی ہیں، اور ثابت کرتی ہیں کہ
リゾルバー ゲートウェイ کی حالتیں ہم آہنگ رہتی ہیں۔ یہ روڈمیپ کی اس ضرورت کو پورا
`sns governance ...` CLI، Norito は監査アーティファクト N1 をマニフェストします
(عوامی لانچ) سے پہلے ایک ہی آپریٹر ریفرنس شیئر کریں۔

## 1. いいえ

یہ دستاویز ان کے لئے ہے:

- ガバナンス評議会 کے اراکین جو چارٹرز، 接尾語 پالیسیوں اور تنازعہ نتائج پر ووٹ دیتے ہیں۔
- ガーディアンボードの凍結防止、反転防止、反転防止の防止
- サフィックス スチュワード レジストラ کی قطاریں چلاتے ہیں، オークション منظور کرتے ہیں، اور 収益分割 سنبھالتے ہیں۔
- リゾルバー/ゲートウェイ、SoraDNS、GAR、テレメトリ ガードレール、およびテレメトリ ガードレール
- コンプライアンス財務サポート、監査、監査、Noritoアーティファクト

یہ `roadmap.md` میں درج بند-بیٹا (N0)، عوامی لانچ (N1) اور توسیع (N2) مراحل کو
ダッシュボード エスカレーション ダッシュボード ダッシュボード エスカレーション ダッシュボード
جوڑتی ہے۔

## 2. いいえ

| | بنیادی ذمہ داریاں |アーティファクトとテレメトリー |エスカレーション |
|------|---------------|-----------------------------|-----------|
|ガバナンス評議会 | چارٹرز، 接尾辞 پالیسیوں، تنازعہ فیصلوں اور スチュワードローテーション کی تدوین و توثیق۔ | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`、議会投票用紙 جو `sns governance charter submit` سے محفوظ ہوتے ہیں۔ |評議会議長 + ガバナンス ドケット トラッカー|
|ガーディアンボード |ソフト/ハード フリーズ数 キャノン数 72 時間のレビュー数|ガーディアン チケット جو `sns governance freeze` سے نکلتے ہیں، オーバーライド マニフェスト جو `artifacts/sns/guardian/*` میں لاگ ہوتے ہیں۔ |ガーディアンのオンコール ローテーション (<=15 分の ACK)۔ |
|サフィックススチュワード |レジストラのキュー、オークション、価格設定、顧客とのコミュニケーション、コンプライアンスの承認 دیتے ہیں۔ |スチュワードポリシー `SuffixPolicyV1` 価格設定リファレンスシート スチュワードの謝辞 規制メモ ساتھ محفوظ ہوتے ہیں۔ |スチュワード プログラムのリード + サフィックス مخصوص PagerDuty۔ |
|レジストラと請求業務 | `/v1/sns/*` エンドポイントの支払いの調整、テレメトリの発行、CLI スナップショットの作成、および CLI スナップショットの作成|レジストラー API ([`registrar-api.md`](./registrar-api.md))、`sns_registrar_status_total` メトリクス、支払い証明、`artifacts/sns/payments/*` میں محفوظ ہیں۔ |レジストラ義務マネージャー、財務連絡担当者|
|リゾルバーおよびゲートウェイ オペレーター | SoraDNS GAR ゲートウェイ ステート レジストラ イベント 調整済みの状態透明性メトリクス ストリーム| [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 |リゾルバー SRE オンコール + ゲートウェイ運用ブリッジ|
|財務および財務 | 70/30 の収益分割、紹介のカーブアウト、税金/財務省申告、SLA 証明書の作成|収益見越マニフェスト、ストライプ/国庫輸​​出、KPI 付録 `docs/source/sns/regulatory/` میں۔ |財務管理者 + コンプライアンス責任者۔ |
|コンプライアンスおよび規制担当窓口 |認証 (EU DSA وغیرہ) 認証 KPI 規約 認証 開示情報|規制メモ `docs/source/sns/regulatory/` リファレンス デッキ、テーブルトップ リハーサル、`ops/drill-log.md` エントリ|コンプライアンス プログラム リード|
|サポート / SRE オンコール |インシデント (衝突、請求のドリフト、リゾルバーの停止) の監視、顧客メッセージング、監視、ランブックの監視|インシデントテンプレート、`ops/drill-log.md`、段階的に作成されたラボ証拠、Slack/作戦室記録、`incident/` میں محفوظ ہیں۔ | SNS オンコールローテーション + SRE 管理۔ |

## 3. アーティファクトの作成|アーティファクト |大事 |すごい |
|----------|------|------|
|憲章 + KPI 追加条項 | `docs/source/sns/governance_addenda/` | KPI 規約、CLI 投票数、および CLI 投票数やあ|
|レジストリスキーマ | [`registry-schema.md`](./registry-schema.md) | Norito 認証 (`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`) |
|レジストラ契約 | [`registrar-api.md`](./registrar-api.md) | REST/gRPC ペイロード、`sns_registrar_status_total` メトリクス、ガバナンス フックの管理|
|アドレス UX ガイド | [`address-display-guidelines.md`](./address-display-guidelines.md) | i105 (評価) / 圧縮 (`sora`) (`sora`、2 番目に優れた) レンダリング、ウォレット/エクスプローラーのレンダリング|
| SoraDNS / GAR ドキュメント | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |決定論的なホスト導出、透明性テーラー ワークフロー、アラート ルール|
|規制に関するメモ | `docs/source/sns/regulatory/` |管轄区域の摂取メモ (EU DSA)、スチュワードの謝辞、テンプレートの付属書|
|ドリルログ | `ops/drill-log.md` |カオス IR リハーサル、フェーズ終了、終了|
|アーティファクト保管 | `artifacts/sns/` |支払い証明、ガーディアンチケット、リゾルバー差分、KPI エクスポート、`sns governance ...` および署名付き CLI 出力|

特別な成果を得るために、アーティファクトを研究する必要があります。
監査人 24 人 意思決定トレイル دوبارہ بنا سکیں۔

## 4. और देखें

### 4.1 憲章およびスチュワード動議

| |意味 | CLI / 証拠 |にゅう |
|------|------|-----|------|
|追加草案 KPI デルタ |評議会報告者 + スチュワードリード |マークダウン テンプレート `docs/source/sns/governance_addenda/YY/` میں محفوظ | KPI コベナント ID、テレメトリ フック、アクティベーション条件など|
|提案の提出 |評議会議長 | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` 年) | CLI Norito マニフェスト `artifacts/sns/governance/<id>/charter_motion.json` میں محفوظ کرتی ہے۔ |
|保護者の承認に投票する |評議会 + 保護者 | `sns governance ballot cast --proposal <id>` は、`sns governance guardian-ack --proposal <id>` |ハッシュ化された議事録と定足数の証明|
|スチュワードの承認 |スチュワードプログラム | `sns governance steward-ack --proposal <id> --signature <file>` |サフィックスポリシー بدیل کرنے سے پہلے لازم؛ `artifacts/sns/governance/<id>/steward_ack.json` میں 封筒 ریکارڈ کریں۔ |
|アクティベーション |レジストラ業務 | `SuffixPolicyV1` اپڈیٹ کریں، レジストラ キャッシュ ریفریش کریں، `status.md` میں نوٹ شائع کریں۔ |アクティベーション タイムスタンプ `sns_governance_activation_total` میں لاگ ہوتا ہے۔ |
|監査ログ |コンプライアンス | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` ドリルログ میں اندراج کریں اگر 卓上 ہوا ہو۔ |テレメトリ ダッシュボード、ポリシーの相違点、およびポリシーの相違点|

### 4.2 登録、オークションおよび価格設定の承認

1. **プリフライト:** レジストラー `SuffixPolicyV1` 価格帯の条件
   猶予/償還ウィンドウ کی تصدیق کرتا ہے۔ロードマップのロードマップ
   3/4/5/6-9/10+ tier ٹیبل (基本 tier + サフィックス係数) کے مطابق synced رکھیں۔
2. **密閉入札オークション:** プレミアム プール 72 時間のコミット / 24 時間の公開サイクル
   `sns governance auction commit` / `... reveal` ذریعے چلائیں۔コミットします
   (ハッシュ) `artifacts/sns/auctions/<name>/commit.json` میں شائع کریں تاکہ
   監査人がランダム性を検証する
3. **支払いの確認:** レジストラ `PaymentProofV1` 財務分割
   (70% 財務 / 30% スチュワード、紹介カーブアウト <=10%) を検証します。 Norito JSON
   `artifacts/sns/payments/<tx>.json` レジストラの応答
   (`RevenueAccrualEventV1`) すごい
4. **ガバナンスフック:** プレミアム/ガード付き `GovernanceHookV1` جس میں
   評議会提案 ID とスチュワードの署名 ہوں۔フックがありません。
   `sns_err_governance_missing` آتا ہے۔
5. **アクティブ化 + リゾルバーの同期:** Torii レジストリ イベント リゾルバーの透明性
   テールラー چلائیں تاکہ نیا GAR/ゾーン状態 پھیلنے کی تصدیق ہو (4.5 دیکھیں)۔
6. **顧客開示:** 顧客向け台帳 (ウォレット/エクスプローラー)
   [`address-display-guidelines.md`](./address-display-guidelines.md) 共有備品
   i105 圧縮 (`sora`) レンダリング コピー/QR
   ガイダンス سے میل کھاتے ہیں۔

### 4.3 更新、請求、財務調整- **更新ワークフロー:** レジストラ `SuffixPolicyV1` میں دی گئی 30 € 猶予 + 60 دن
  引き換えウィンドウ نافذ کرتے ہیں۔ 60 ユーロのオランダの再開シーケンス (7 ユーロの 10 倍の料金)
  15%/دن کم ہوتی ہے) خودکار طور پر `sns governance reopen` سے چلتی ہے۔
- **収益分割:** 更新、移管 `RevenueAccrualEventV1` بناتا ہے۔国庫輸出
  (CSV/Parquet) イベント、調整、調整、調整証明
  `artifacts/sns/treasury/<date>.json` میں منسلک کریں۔
- **紹介のカーブアウト:** اختیاری 紹介 فیصد 接尾辞 کے حساب سے `referral_share` کو
  スチュワードポリシー میں شامل کر کے ٹریک کیے جاتے ہیں۔レジストラの最終的な分割は、次のような問題を引き起こします
  紹介マニフェスト 支払い証明 ساتھ رکھتے ہیں۔
- **レポートの頻度:** 財務関連の KPI 付録 (登録、更新、ARPU、紛争/保証金)
  使用率) `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` میں پوسٹ کرتا ہے۔ダッシュボード
  テーブルをエクスポートする Grafana 台帳証拠をエクスポートする
- **月次 KPI レビュー:** チェックポイント ファイナンス リード、義務スチュワード、プログラム PM など。
  [SNS KPI ダッシュボード](./kpi-dashboard.md) کھولیں (ポータル埋め込み `sns-kpis` /
  `dashboards/grafana/sns_suffix_analytics.json`)Ì レジストラのスループット + 収益の表
  輸出 کریں، デルタ別館 میں لاگ کریں، اور アーティファクト メモ کے ساتھ لگائیں۔レビュー
  SLA 違反 (ウィンドウの 72 時間以上のフリーズ、レジストラ エラーの急増、ARPU ドリフト) のインシデント
  トリガー

### 4.4 凍結、紛争および控訴

| |意味 |行動と証拠 | SLA |
|------|------|-----------|-----|
|ソフトフリーズリクエスト |スチュワード / サポート |チケット `SNS-DF-<id>` 支払い証明、紛争保証金参照、影響を受けるセレクター| <=4 時間の摂取量|
|ガーディアンチケット |ガーディアンボード | `sns governance freeze --selector <i105> --reason <text> --until <ts>` `GuardianFreezeTicketV1` いいえチケット JSON `artifacts/sns/guardian/<id>.json` میں رکھیں۔ | <=30 分 ACK、<=2 時間実行|
|理事会の批准 |ガバナンス評議会 |承認/拒否の凍結 ガーディアン チケット 争議債券ダイジェスト 承認 承認 承認 承認拒否 ガーディアン チケット 紛争保証金ダイジェスト 承認 承認拒否|議会セッション、非同期投票、 |
|仲裁委員会 |コンプライアンス + スチュワード | 7 陪審員パネル (ロードマップ بلائیں، ハッシュ化投票 `sns governance dispute ballot` کے ذریعے جمع کریں۔)匿名投票受領事件パケット میں لگائیں۔ |評決 <=7 日、保証金の預け入れ|
|アピール |保護者 + 評議会 |控訴保証金 ہیں 陪審員プロセス دہراتی ہیں؛ Norito マニフェスト `DisputeAppealV1` ریکارڈ کریں اور プライマリ チケット ریفرنس کریں۔ | <=10 日|
|凍結解除と修復 |レジストラー + リゾルバー操作 | `sns governance unfreeze --selector <i105> --ticket <id>` レジストラのステータス اپڈیٹ کریں، اور GAR/リゾルバーの差分が伝播する|評決 فوراً بعد۔ |

緊急大砲（ガーディアン発動によるフリーズ <=72 時間）
遡及審議会の見直し `docs/source/sns/regulatory/` 透明性
メモ درکار ہے۔

### 4.5 リゾルバーとゲートウェイの伝播

1. **イベント フック:** レジストリ イベント リゾルバー イベント ストリーム (`tools/soradns-resolver` SSE)
   ہوتا ہے۔を発するリゾルバー操作のサブスクライブ、透明性テーラー
   (`scripts/telemetry/run_soradns_transparency_tail.sh`) ٩ے ذریعے diffs ریکارڈ کرتے ہیں۔
2. **GAR テンプレートの更新:** ゲートウェイ `canonical_gateway_suffix()` は、GAR テンプレートを参照してください。
   اپڈیٹ کرنے ہوں گے اور `host_pattern` リスト کو دوبارہ 記号 کرنا ہوگا۔差分
   `artifacts/sns/gar/<date>.patch` میں رکھیں۔
3. **ゾーンファイルの発行:** `roadmap.md` میں بیان کردہ ゾーンファイルのスケルトン (名前、ttl、cid、証明)
   Torii/SoraFS پر プッシュ کریں۔ Norito JSON
   `artifacts/sns/zonefiles/<name>/<version>.json` アーカイブ کریں۔
4. **透明性チェック:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   警告緑色のアラートPrometheus テキスト出力 週間透明性レポート セキュリティ レポート
5. **ゲートウェイ監査:** `Sora-*` ヘッダー サンプル (キャッシュ ポリシー、CSP、GAR ダイジェスト)
   ガバナンス ログを添付する کریں تاکہ آپریٹرز ثابت کر سکیں کہ ゲートウェイ نے نیا نام
   ガードレール、サービス、サービス

## 5. テレメトリーレポート|信号 |出典 |説明/アクション |
|--------|--------|-----------|
| `sns_registrar_status_total{result,suffix}` | Torii レジストラー ハンドラー |更新、凍結、転送、成功/エラーカウンターجب `result="error"` サフィックス کے حساب سے بڑھ جائے تو アラート۔ |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii メトリクス | API ハンドラーとレイテンシ SLO `torii_norito_rpc_observability.json` ダッシュボードとフィードの管理|
| `soradns_bundle_proof_age_seconds` または `soradns_bundle_cid_drift_total` |リゾルバー透明度テーラー |証明、GAR ドリフト、検出、検出ガードレール `dashboards/alerts/soradns_transparency_rules.yml` میں 定義 ہیں۔ |
| `sns_governance_activation_total` |ガバナンス CLI |チャーター/付録の有効化 پر بڑھنے والا カウンター評議会の決定、追加の公開、和解、和解、追加|
| `guardian_freeze_active` ゲージ |ガーディアンCLI |セレクタ ソフト/ハード フリーズ Windows トラック トラックاگر `1` SLA سے زیادہ رہے تو SRE کو ページ کریں۔ |
| KPI 別館ダッシュボード |財務 / ドキュメント |ロールアップ 規制メモ ٩ے ساتھ شائع ہوتے ہیں؛ポータル انہیں [SNS KPI ダッシュボード](./kpi-dashboard.md) ذریعے embed کرتا ہے تاکہ スチュワード اور 規制当局 ایک ہی Grafana view تک پہنچیں۔ |

## 6. 証拠と監査の要件

|アクション |アーカイブする証拠 |ストレージ |
|--------|---------|----------|
|憲章・方針変更 |署名済み Norito マニフェスト、CLI トランスクリプト、KPI 差分、スチュワード承認| `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|登録・更新 | `RegisterNameRequestV1` ペイロード `RevenueAccrualEventV1` 支払い証明| `artifacts/sns/payments/<tx>.json`、レジストラ API ログ|
|オークション |マニフェストのコミット/公開、ランダム性シード、勝者計算スプレッドシート| `artifacts/sns/auctions/<name>/`。 |
|凍結/凍結解除 |ガーディアン チケット、議会投票ハッシュ、インシデント ログ URL、顧客コミュニケーション テンプレート| `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|リゾルバーの伝播 |ゾーンファイル/GAR 差分テーラー JSONL 抜粋 Prometheus スナップショット| `artifacts/sns/resolver/<date>/` + 透明性レポート|
|規制摂取量 |摂取メモ、期限トラッカー、スチュワード承認、KPI 変更概要| `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. 位相ゲートのチェックリスト

|フェーズ |終了基準 |証拠の束 |
|------|------|------|
| N0 - クローズドベータ | SN-1/SN-2 レジストリ スキーマ、マニュアル レジストラ CLI、ガーディアン ドリル|チャーターモーション + スチュワード ACK、レジストラのドライランログ、リゾルバー透明性レポート、`ops/drill-log.md` エントリ|
| N1 - 一般公開 | `.sora`/`.nexus`、セルフサービス レジストラー、リゾルバー自動同期、請求ダッシュボードのオークション + 固定価格レベルが有効になります。 |価格設定シートの差分、レジストラの CI 結果、支払い/KPI の付録、透明性、テーラーの出力、インシデントのリハーサル メモなど|
| N2 - 拡張 | `.dao`、リセラー API、紛争ポータル、スチュワード スコアカード、分析ダッシュボード|ポータルのスクリーンショット、SLA メトリクスの異議申し立て、スチュワード スコアカードのエクスポート、リセラー ポリシーを参照する更新されたガバナンス憲章|

段階終了 テーブルトップ ドリル (登録ハッピー パス、フリーズ、リゾルバー停止) 終了
アーティファクト `ops/drill-log.md` میں منسلک ہوں۔

## 8. インシデント対応とエスカレーション

|トリガー |重大度 |直接の所有者 |必須のアクション |
|----------|----------|---------------------|----------|
|リゾルバー/GAR ドリフト、古い証拠 |セクション 1 |リゾルバー SRE + ガーディアンボード |リゾルバ オンコール ページの表示 テーラーの出力キャプチャ 保存の凍結 保存 30 分のステータス更新 保存|
|レジストラーの停止、請求の失敗、広範な API エラー |セクション 1 |レジストラ業務マネージャー |オークション、マニュアル、CLI、スチュワード/財務省、Torii ログ、インシデント ドキュメント、添付ファイル。 |
|単一名義の紛争、支払いの不一致、顧客エスカレーション |セクション 2 |スチュワード + サポートリード |支払い証明 セキュリティ ソフト フリーズ セキュリティ セキュリティ SLA リクエスタ 紛争追跡 セキュリティ トラブルシューティング|
|コンプライアンス監査の結果 |セクション 2 |コンプライアンス連絡窓口 |修復計画 تیار کریں، `docs/source/sns/regulatory/` میں メモ فائل کریں، フォローアップ評議会セッション شیڈول کریں۔ |
|ドリルリハーサル |セクション 3 |プログラムPM | `ops/drill-log.md` スクリプト シナリオ アーティファクト アーカイブ ギャップ ロードマップ タスク スクリプト シナリオ|

重要なインシデント `incident/YYYY-MM-DD-sns-<slug>.md` の所有権テーブル
コマンドログ ،اور اس پلی بک میں تیار ہونے والے 証拠 کے 参照 ہوں۔

## 9. いいえ- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS、DG、ADDR セクション)

憲章の文言 CLI サーフェス テレメトリー契約の説明
और देखेंロードマップ エントリ جو `docs/source/sns/governance_playbook.md` کو
ریفرنس کرتی ہیں انہیں ہمیشہ تازہ ترین ورژن سے میل کھانا چاہیے۔