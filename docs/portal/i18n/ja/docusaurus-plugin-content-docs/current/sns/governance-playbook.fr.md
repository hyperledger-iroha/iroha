---
lang: ja
direction: ltr
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
ページの更新 `docs/source/sns/governance_playbook.md` とメンテナンスの開始
コピー カノニーク デュ ポルタイユ。 Le fichier ソースは、PR の翻訳を継続します。
:::

# ソラネームサービスの統治戦略 (SN-6)

**ステータス:** Redige 2026-03-24 - 参照 vivante pour la 準備 SN-1/SN-6  
**ロードマップの先取特権:** SN-6「コンプライアンスと紛争解決」、SN-7「リゾルバーとゲートウェイ同期」、アドレス ADDR-1/ADDR-5  
**前提条件:** 登録スキーマ [`registry-schema.md`](./registry-schema.md)、登録 API の対照 [`registrar-api.md`](./registrar-api.md)、アドレス UX ガイド[`address-display-guidelines.md`](./address-display-guidelines.md)、およびコンパイルの構造に関する規則 [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md)。

Ce プレイブックの批評コメント lesorganes de gouvernance du Sora Name Service (SNS)
養子縁組、登録承認、訴訟のエスカレートなど
リゾルバーとゲートウェイの保持同期のテストを証明します。満足です
CLI `sns governance ...`、マニフェストのロードマップの必要性
Norito et les artefacts d'audit partagent une Reference unique cote Operator
アバントN1（ランセメントパブリック）。

## 1. ポーティーとパブリック

文書ファイル:

- チャート、政治の投票権を持つ行政管理委員会
  接尾辞と結果の結果。
- 緊急の保護者および検査官の監視員
  レ・リバース。
- レジストラのファイルの接尾辞を管理し、環境を承認する
  収益の一部。
- オペレーターはリゾルバー/ゲートウェイの伝播責任を負い、SoraDNS を必要とします。
  テレメトリの GAR などの情報。
- 適合、トレゾレリー、およびデモントレールのサポートを装備します
  action de governance a laisse des artefacts Norito 監査可能。

Il couvre les Phases de beta fermee (N0)、lancement public (N1) および拡張 (N2)
`roadmap.md` に依存する Chaque ワークフロー aux preuves が必要とする列挙、
ダッシュボードとエスカレードの声。

## 2. 役割と連絡先のカルテ|役割 |責任主体 |人工物と遠隔測定の主な方法 |エスカレード |
|-----|-----------------------------|------------------------------------------|----------|
|行政管理 |憲章の承認と接尾辞の政治、訴訟の評決とスチュワードの交替。 | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`、`sns governance charter submit` 経由の株式管理速報。 |大統領コンセイユ + 行政文書の作成。 |
|コンセイユ・デ・ガーディアン |ゲルのソフト/ハード、緊急の規範、およびレビュー 72 時間。 |チケット ガーディアン emis パー `sns governance freeze`、マニフェスト ドーオーバーライド 委託品 `artifacts/sns/guardian/*`。 |ローテーション ガーディアンがオンコール中 (ACK が 15 分未満)。 |
|スチュワード・ド・サフィックス |レジストラのファイル、アンシェール、プリニヴォーなどの通信クライアント。偵察兵。 | Politics de Steward dans `SuffixPolicyV1`、Fiches de Reference de prix、Acknowledgments de Steward は、cote des memo reglementaires をストックします。 |主任プログラム スチュワード + PagerDuty のサフィックス。 |
|運用レジストラとファクチュレーション |エンドポイント `/v2/sns/*` の操作、支払いの調整、テレメトリーおよびスナップショット CLI の保守の管理。 | API レジストラー ([`registrar-api.md`](./registrar-api.md))、メトリクス `sns_registrar_status_total`、preuves de paiement archivees sous `artifacts/sns/payments/*`。 |登録および連絡管理の責任者。 |
|オペレーター リゾルバーとゲートウェイ | SoraDNS、GAR などのゲートウェイをレジストラのイベントに合わせて保守します。拡散性の透明性。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 | SRE リゾルバー オンコール + ブリッジ運用ゲートウェイ。 |
|トレゾリーと金融 |再分割 70/30、照会のカーブアウト、保管庫/保管庫および証明書 SLA の申請。 |収益の蓄積マニフェスト、ストライプ/トレゾリの輸出、付録 KPI トリメストリエル スー `docs/source/sns/regulatory/`。 |財務管理者 + 責任ある適合者。 |
|連絡調整と規制 |グローバルな義務に準拠し (EU DSA など)、1 時間の契約 KPI と漏洩の証拠を満たしました。 | `docs/source/sns/regulatory/` のレグルメンテール、デッキのリファレンス、テーブル上のリハーサルのメインディッシュ `ops/drill-log.md` のメモ。 |プログラム不適合者をリードします。 |
|サポート / SRE オンコール |インシデントの取得 (衝突、事実関係の導出、リゾルバーのパンネス)、通信クライアントの調整、およびランブックの所有。 |事件のモデル、`ops/drill-log.md`、現場での実験室の準備、転写 Slack/作戦室アーカイブ `incident/`。 |ローテーションオンコールSNS＋マネジメントSRE。 |

## 3. 正規品と情報源の資料

|アーティファクト |定置 |目的 |
|----------|---------------|----------|
|チャート + 付録 KPI | `docs/source/sns/governance_addenda/` |チャートの署名者は、バージョン管理、規約 KPI および統治上の決定を参照して、投票 CLI を参照します。 |
|レジストリのスキーマ | [`registry-schema.md`](./registry-schema.md) |構造体 Norito canonique (`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`)。 |
|レジストラとの契約 | [`registrar-api.md`](./registrar-api.md) |ペイロード REST/gRPC、メトリック `sns_registrar_status_total` および管理上のフック。 |
|ガイド UX d'addresses | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canoniques I105 (優先) および圧縮 (2 番目の選択) は、ウォレット/エクスプローラーの再生産品です。 |
|ドキュメント SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |ホストの決定、透明性と規則に関するワークフローの導出。 |
|メモレグルメンテア | `docs/source/sns/regulatory/` |管轄権に関するメモ (例: EU DSA)、管理責任者への謝辞、モデルの附属書。 |
|ジャーナル・ド・ドリル | `ops/drill-log.md` |ジャーナル・デ・リハーサルの混乱とIRには、段階的な出撃が必要です。 |
|アーティファクトのストック | `artifacts/sns/` | Preuves de paiement、チケット ガーディアン、差分リゾルバー、エクスポート KPI および出撃 CLI 署名者製品パー `sns governance ...`。 |

成果物を管理するための行動を参照する
追跡調査を再構築するためのテーブルの作成
24時間以内に決定。

## 4. サイクル・デ・ヴィーのプレイブック

### 4.1 カルテおよびスチュワードの動議|エタップ |所有権 | CLI / プルーヴェ |メモ |
|------|-------------|--------------|------|
| KPI の追加とデルタの追加 | Redigerコンセイユ報告者 + 主任管理人 |テンプレート マークダウン ストック `docs/source/sns/governance_addenda/YY/` |契約 KPI の ID、テレメトリのフック、およびアクティブ化の条件が含まれます。 |
|スメトル・ラ・プロポジション |デュ・コンセイユ大統領 | `sns governance charter submit --input SN-CH-YYYY-NN.md` (製品 `CharterMotionV1`) LA CLI はマニフェスト Norito を取得し、`artifacts/sns/governance/<id>/charter_motion.json` を取得します。 |
|投票および承認ガーディアン |コンセイル + ガーディアン | `sns governance ballot cast --proposal <id>` と `sns governance guardian-ack --proposal <id>` |分のハッシュと定足数に参加します。 |
|受け入れ管理者 |プログラムスチュワード | `sns governance steward-ack --proposal <id> --signature <file>` |接尾辞の政治的変更は事前に必要です。 enregister l'enveloppe sous `artifacts/sns/governance/<id>/steward_ack.json`。 |
|アクティベーション |レジストラの操作 | Mettre a jour `SuffixPolicyV1`、rafraichir les caches du registrar、publier une note dans `status.md`。 |アクティベーションログのタイムスタンプは `sns_governance_activation_total` です。 |
|監査ジャーナル |適合 | Ajouter une entre a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et au ジャーナル ド ドリル SI 卓上効果。 |テレメトリーのダッシュボードと政治の差分の参考資料が含まれます。 |

### 4.2 登録、登録および優先の承認

1. **プリフライト:** 登録官の尋問 `SuffixPolicyV1` 確認者ル ニボー
   優先権、免責期間、猶予/償還のフェネトル。ガーダー・レ
   フィッシュ デ プリ シンクロニゼ アベック ル タブロー ド ニヴォー 3/4/5/6-9/10+ (ニヴォー
   基本 + 接尾辞の係数) ロードマップの文書化。
2. **Encheres sealed-bid:** プール プレミアム、実行者ファイル サイクル 72 時間のコミット /
   `sns governance auction commit` / `... reveal` 経由で 24 時間表示されます。出版社ラ・リスト
   des commit (ハッシュの一意性) so `artifacts/sns/auctions/<name>/commit.json`
   afin que les Auditeurs puissent verifier l'aleatoire。
3. **支払いの確認:** レジストラの有効性 `PaymentProofV1` パーラポール
   トレゾリの再分割 (70% トレゾレリ / 30% のスチュワード アベック カーブアウト)
   紹介 <=10%)。ストッカー ファイル JSON Norito ス `artifacts/sns/payments/<tx>.json`
   レジストラからの応答 (`RevenueAccrualEventV1`)。
4. **管理フック:** アタッチャー `GovernanceHookV1` プレミアム/ガード付き
   コンセイユの提案およびスチュワードの署名に関する参照。
   レ・フック・マンカンツ・デクレンセント `sns_err_governance_missing`。
5. **アクティベーション + 同期リゾルバー:** 登録された Torii のイベントが発生しました。
   デクレンチャー ル テイラー デ トランスペアレンス デュ リゾルバー フォア コンファマー キュー ル ヌーベル
   ETAT GAR/zone の最も重要なプロパティ (voir 4.5)。
6. **漏洩クライアント:** Mettre a jour le ledger oriente クライアント (ウォレット/エクスプローラー)
   フィクスチャ パルタージュ経由 [`address-display-guidelines.md`](./address-display-guidelines.md)、
   en s'assurant que les rendus I105 は、対応する補助ガイドのコピー/QR を圧縮します。

### 4.3 再建、分割および和解のトレゾレリー

- **再構築のワークフロー:** 登録申請者の猶予期間
  30 時間 + 60 時間の引き換えは、`SuffixPolicyV1` で指定されます。
  4 時間 60 時間、オランデーズ ルーベルチュール シーケンス (7 時間、フライ 10x)
  `sns governance reopen` 経由で自動デクロワッサン 15%/時間) をデクレンシュします。
- **収益の再分割:** 資金再編または資金移動
  `RevenueAccrualEventV1`。トレゾリ輸出品 (CSV/寄木細工) の取引
  会議の夕方の日常を調整します。ジョインドル レ プルーヴ
  `artifacts/sns/treasury/<date>.json`。
- **紹介のカーブアウト:** 紹介オプションの合計数
  par suffixe en ajoutant `referral_share` a la politique steward.レレジストラ
  最終的な分割とストックのマニフェスト・デ・リファーラルをコート・デ・ラに提出する
  プレヴ・ド・ペイメント。
- **レポートの頻度:** La Finance publie des annexes KPI mensuelles
  (登記、再建、ARPU、訴訟/債券の利用)
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。ダッシュボードの説明
  s'appuyer sur les memes テーブル輸出者は que les chiffres Grafana
  オー・プルーヴ・デュ・レジャー特派員。
- **KPI のレビュー:** プレミア マルディ アソシエのチェックポイント、リード ファイナンス、
  サービス管理者および PM プログラム。 Ouvrir le [SNS KPI ダッシュボード](./kpi-dashboard.md)
  (`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` のポータルを埋め込む)、
  輸出業者のスループット表 + 登録業者の収益、荷送人のデルタ
  別館、およびメモの成果物に参加します。デクレンチャー アン インシデント シラ
  SLA 違反のレビュー (72 時間以上のフリーズ、エラーの写真)
  レジストラ、ARPU を導き出します）。

### 4.4 ゲル、訴訟および訴訟|フェーズ |所有権 |アクションとプリューブ | SLA |
|------|--------------|------||-----|
|デマンド ドゥ ジェル ソフト |スチュワード / サポート |寄託者はチケット `SNS-DF-<id>` の支払い、保証金の参照、および影響の選択を行っています。 |メインディッシュの4時間前まで。 |
|チケットガーディアン |コンセイユの守護者 | `sns governance freeze --selector <I105> --reason <text> --until <ts>` 製品 `GuardianFreezeTicketV1`。 Stocker le JSON du ticket sous `artifacts/sns/guardian/<id>.json`。 | <=30 分の ACK、<=2 時間の実行。 |
|コンセイユの批准 |行政管理 |承認者はジェルを拒否し、文書を作成して決定を下し、チケットの保護者と保証金のダイジェストを記録します。 |プロチェーンセッションは非同期で投票します。 |
|パネル裁定取引 |適合 + スチュワード | 7 人の陪審員パネルの会議 (セロン ロードマップ) は、`sns governance dispute ballot` 経由で速報ハッシュを取得します。投票参加者は事件の報告を匿名化します。 |評決 <= 保釈金留置後 7 時間。 |
|アピール |ガーディアン+コンセイユ |二重の法廷での絆と反復的な陪審員の手続き。登録者ファイル マニフェスト Norito `DisputeAppealV1` および参照者ファイル チケット プライマリ。 | <=10 時間。 |
|デゲルと修復 |レジストラ + ops リゾルバー |実行者 `sns governance unfreeze --selector <I105> --ticket <id>`、レジストラの最新情報、および GAR/リゾルバーの差分の管理。 |判決後即時。 |

Les canons d'urgence (ガーディアンのゲルのデクレンシュ <= 72 時間) ミームの流れに沿ったもの
あなたのレビューは遡及的であり、透明性のあるものである必要があります
`docs/source/sns/regulatory/`。

### 4.5 伝播リゾルバーとゲートウェイ

1. **Hook d'evenement:** Chaque Evenement de registre emet と le flux d'evenements
   リゾルバ (`tools/soradns-resolver` SSE)。 Les Ops Resolver の概要
   ファイルの透明性を介して差分を登録
   (`scripts/telemetry/run_soradns_transparency_tail.sh`)。
2. **時間のテンプレート GAR:** 時間のテンプレートを確認するゲートウェイ
   GAR 参照パー `canonical_gateway_suffix()` および再署名者リスト
   `host_pattern`。ストッカー レ ディフ ダン `artifacts/sns/gar/<date>.patch`。
3. **ゾーンファイルの出版物:** ゾーンファイルの評価に関する使用者
   `roadmap.md` (名前、ttl、cid、証明) および Torii/SoraFS の情報。アーカイバ
   ファイル JSON Norito `artifacts/sns/zonefiles/<name>/<version>.json`。
4. **透明性の検証:** 実行者 `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   安全を保証し、警告を与えます。出撃テキストに参加する
   Prometheus 透明性のある信頼関係。
5. **監査ゲートウェイ:** Enregistrer des echantillons d'en-tetes `Sora-*` (ポリシー キャッシュ、
   CSP、ダイジェスト GAR) および政府機関のジャーナルに参加
   Operators puissent prouver que le Gateway a servi le nouveau nom avec les
   前衛的な前。

## 5. テレメトリとレポート

|信号 |出典 |説明/アクション |
|--------|--------|-----------|
| `sns_registrar_status_total{result,suffix}` |ゲストレジストラ Torii |登録、再構築、ジェル、転送の成功/失敗を計算します。接尾辞 `result="error"` を拡張します。 |
| `torii_request_duration_seconds{route="/v2/sns/*"}` |メトリクス Torii | SLO 遅延遅延ハンドラー API。 `torii_norito_rpc_observability.json` のダッシュボードの問題。 |
| `soradns_bundle_proof_age_seconds` & `soradns_bundle_cid_drift_total` |透明リゾルバーのテーラー | GAR を導出するペリメートの検出。 `dashboards/alerts/soradns_transparency_rules.yml` によるガードファウスの定義。 |
| `sns_governance_activation_total` | CLI ガバナンス | Compteur は、チャート/付録のアクティベーションを追加します。調整者による決定とコンセイユと追加の公開を利用します。 |
| `guardian_freeze_active` ゲージ | CLI ガーディアン |スーツ レ フェネトル ドゥ ジェル ソフト/ハード パー選択者。ページ SRE si la valeurreste `1` au-dela du SLA を宣言します。 |
|付録のダッシュボード KPI |財務 / ドキュメント |ロールアップは、定期的なメモを公開します。 [SNS KPI ダッシュボード](./kpi-dashboard.md) 経由で le portail les integre を作成し、スチュワードと規制者が Grafana に同意します。 |

## 6. 証拠と監査の要件|アクション |アーカイバーの証拠 |在庫 |
|----------|---------------------|----------|
|チャート/政治の変更 |マニフェスト Norito 署名、トランスクリプト CLI、差分 KPI、確認応答スチュワード。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|登記・改築 |ペイロード `RegisterNameRequestV1`、`RevenueAccrualEventV1`、preuve de paiement。 | `artifacts/sns/payments/<tx>.json`、レジストラーのログ API。 |
|エンシェール |マニフェストはコミット/公開、グレーン・ダレトワール、継続的な計算表を作成します。 | `artifacts/sns/auctions/<name>/`。 |
|ジェル/デジェル |チケット ガーディアン、投票のハッシュ、インシデントのログの URL、通信クライアントのモデル。 | `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|伝播リゾルバー |ゾーンファイル/GAR の差分、拡張子 JSONL デュ テイラー、スナップショット Prometheus。 | `artifacts/sns/resolver/<date>/` + 透明性のある関係。 |
|摂取基準 |記録のメモ、期限の追跡、承認管理者、KPI の変更履歴。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. 段階的チェックリスト

|フェーズ |出撃基準 |証拠のバンドル |
|------|-------|------|
| N0 - ベータフェルメ | SN-1/SN-2 レジストラ スキーマ、CLI レジストラ マニュアル、ドリル ガーディアンが完了しました。 |カルテ + ACK スチュワードのモーション、レジストラのドライラン ログ、透明性リゾルバーの関係、主要な `ops/drill-log.md`。 |
| N1 - ランセメント公開 | `.sora`/`.nexus` のエンシェールと段階的な修正プログラム、レジストラーのセルフサービス、自動同期リゾルバー、ダッシュボードの機能停止。 |賞の差、登録機関の結果、付録の支払い/KPI、透明性のある出撃、リハーサルインシデントのメモ。 |
| N2 - 拡張 | `.dao`、API リセラー、ポータル デリッジ、スコアカード スチュワード、ダッシュボード分析。 | ecran du portail、metriques SLA de litige、輸出スコアカードスチュワード、charte de gouvernance mise a jour avec politiques 再販業者を捕捉します。 |

訓練の緊急出動、卓上登録 (公園登録)
ハッピー パス、ゲル、パンネ リゾルバー) `ops/drill-log.md` で avec アーティファクトが添付されます。

## 8. 補助的な事件やエスカレードへの対応

|デクレンチャー |重度の |即時所有 |行動義務 |
|---------------|----------|---------------|----------------------|
|リゾルバー/GAR または Preuves perimees を導出する |セクション 1 | SRE リゾルバー + コンセイル ガーディアン |ポケベルはオンコールリゾルバー、キャプチャーは出撃のテイラー、決定者はサイレス名を変更し、30 分間の法定ポスターを宣伝します。 |
|パンネ レジストラ、技術的な事実確認、エラー API の一般化 |セクション 1 |レジストラの職務マネージャー | Arreter les nouvelles encheres、basculer sur CLI manuel、notifier Stewards/tresorerie、joindre les logs Torii au doc d'incident。 |
|命名上の問題、支払いの不一致、エスカレーション クライアント |セクション 2 |スチュワード + リード サポート |コレクターは支払いの準備をし、必要なソフトを決定し、SLA からの要求に応じ、荷送人は結果を追跡します。 |
|適合性の監査 |セクション 2 |連絡適合者 |修復計画を立てるのはレディガー、メモは `docs/source/sns/regulatory/` で保管者は、計画立案者は会議で調査を行います。 |
|ドリルとリハーサル |セクション 3 | PMプログラム | `ops/drill-log.md` で作成されたシナリオの実行者、アーティファクトのアーカイバ、ロードマップとのギャップの作成。 |

`incident/YYYY-MM-DD-sns-<slug>.md` avec des による事件の報告
固有のテーブル、コマンドのログ、予備のリファレンス
プロデュイツは、長い間プレイブックを宣伝しています。

## 9. 参考文献

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (セクション SNS、DG、ADDR)

Garder CE プレイブックを 1 時間かけて作成し、チャートのテキスト、CLI の表面を確認する
テレメトリーの変更に関する規制。ロードマップの主要部分を参照
`docs/source/sns/governance_playbook.md` 連絡先はアラにあります
デルニエール改訂版。