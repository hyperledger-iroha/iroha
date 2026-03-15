---
lang: ja
direction: ltr
source: docs/portal/docs/sns/governance-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
エスタ ページ リフレジャ `docs/source/sns/governance_playbook.md` とアホラ シルベ コモ
ラ コピア カノニカ デル ポータル。 PR の取引に関するアーカイブを保存します。
:::

# ソラの奉仕活動のプレイブック (SN-6)

**エスタード:** ボラドール 2026-03-24 - 準備中の生き生きとした参照 SN-1/SN-6  
**ロードマップの内容:** SN-6「コンプライアンスと紛争解決」、SN-7「リゾルバーとゲートウェイ同期」、ADDR-1/ADDR-5 の方針  
**前提条件:** 登録のエスケマ [`registry-schema.md`](./registry-schema.md)、登録 API の制御 [`registrar-api.md`](./registrar-api.md)、UX の指示[`address-display-guidelines.md`](./address-display-guidelines.md)、[`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) の構造を規制します。

Este プレイブックでは、Sora ネーム サービス (SNS) について説明しています。
採用カルタ、プルエバン登録、エスカラン紛争、プルエバン クエスト ロス スタドス
リゾルバーとゲートウェイの永続的な同期。必要なロードマップの作成
CLI `sns governance ...`、マニフェスト Norito および成果物を失う
Auditoria compartan una unica Referencia operativa antes de N1 (ランサミエント)
パブリコ）。

## 1. アルカンスと観客

El documento esta dirigido a:

- ゴベルナンサの政策、政治活動のミエンブロス・デル・デ・ゴベルナンサ
  論争の結果。
- 緊急時の保護者会議
  リバイザン・リバーテス。
- Stewards de sufijos que operan colas de registrator、aprueban subastas y
  ゲストのレパートス・デ・イングレソス。
- SoraDNS の宣伝を担当するリゾルバー/ゲートウェイのオペレーター、
  テレメトリの実際の GAR とガードレール。
- Equipos decumplimiento、tesoreria y soporte que deben Demonstrar que toda
  アクシオン デ ゴベルナンザ デホ アーティファクト Norito 監査可能。

ベータセラダのキューブ (N0)、公共のランザミエント (N1) および拡張 (N2)
listadas en `roadmap.md`, vinculando cada flujo de trabajo con la evidencia,
 ダッシュボードとエスカラミエントの要求。

## 2. 役割と連絡先のマップ|ロール |プリンシパルの責任 |アーティファクトとテレメトリアの原理 |エスカラシオン |
|------|------------------------------|------------------------------------------|-----------|
|コンセホ・デ・ゴベルナンサ |批准文書の編集、スーフィホの政治、係争中の評決、管理委員会の回転の裁定。 | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`、`sns governance charter submit` 経由の投票。 |コンセホ大統領 + ゴベルナンサの議題追跡。 |
|ガーディアネスフンタ |ソフト/ハードのコンゲラミエント、緊急時および 72 時間の改訂を発行します。 | `sns governance freeze` によるガーディアンのチケット、`artifacts/sns/guardian/*` のオーバーライドのマニフェスト。 | Guardia オンコール (<=15 分の ACK)。 |
|スフィホ審査員 |オペラン・コーラ・デル・レジストラドール、サブバス、ニベレス・デ・プレシオ・コンクライアント・コミュニケーション。レコノセン・クムプリミエントス。 | `SuffixPolicyV1` のスチュワードの政治、優先事項の参照、メモ規制に関するスチュワードの告発。 |スチュワード + PagerDuty のプログラムのライダー。 |
|登録と機能の操作 |オペランドポイント `/v1/sns/*`、調整パゴス、CLI でのテレメトリと複数のスナップショットの発行。 | API 登録者 ([`registrar-api.md`](./registrar-api.md))、メトリック `sns_registrar_status_total`、`artifacts/sns/payments/*` の登録。 |レジストラドールとテソレリアの管理責任者。 |
|リゾルバーとゲートウェイのオペランド | Mantienen SoraDNS、GAR およびレジストラドールのイベントに関するゲートウェイの設定。透明性のある指標を送信します。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 | SRE デル リゾルバー オンコール + ゲートウェイの操作。 |
|テソレリアとフィナンザス |アプリケーション レポート 70/30、カーブアウト デ レファリド、宣言会計/テゾレリアおよびアテスタシオン SLA。 | `docs/source/sns/regulatory/` の KPI トリメストラレスの付録、ストライプ/テソレリアのエクスポート、蓄積のマニフェスト。 |財務管理者 + 最高責任者。 |
| Enlace de Cumplimiento y Regulacion | Rastrea obligaciones globales (EU DSA など)、実際の契約 KPI および presenta divulgaciones。 | `docs/source/sns/regulatory/` の規制に関するメモ、参照用のデッキ、テーブルトップに関するエントリ `ops/drill-log.md`。 |最高のプログラムのライダー。 |
| Soporte / SRE オンコール |マネジャ インシデント (衝突、事実上のドリフト、解決策の解決)、クライアントとランブックの調整。 | Plantillas de Incidente、`ops/drill-log.md`、実験室の証拠、Slack/war-room en `incident/` の転写。 | Rotacion オンコール SNS + ジェスション SRE。 |

## 3. アーティファクト・カノニコスとフエンテス・デ・ダトス

|アーティファクト |ユビカシオン |プロポジト |
|----------|----------|----------|
|カルタ + anexos KPI | `docs/source/sns/governance_addenda/` |バージョン管理、規約、KPI および CLI 投票に関する決定に関する決定書。 |
|エスケマ デ レジストロ | [`registry-schema.md`](./registry-schema.md) | Estructuras Norito canonicas (`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`)。 |
|登録者との契約 | [`registrar-api.md`](./registrar-api.md) |ペイロード REST/gRPC、メトリクス `sns_registrar_status_total` およびガバナンス フックの期待。 |
|ガイア UX の指示 | [`address-display-guidelines.md`](./address-display-guidelines.md) | canonicos I105 (preferido) と comprimidos (segunda mejor opcion) のウォレット/エクスプローラーのリフレハドをレンダリングします。 |
|ドキュメント SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |ホストの決定性の導出、透明性の追跡および警告の監視。 |
|メモ規制 | `docs/source/sns/regulatory/` | Notas de ingreso jurisdiccional (p. ej.、EU DSA)、acuses de Steward、anexos plantilla。 |
|ドリルログ | `ops/drill-log.md` |登録は、事前に IR を要求します。 |
|アルマセンの工芸品 | `artifacts/sns/` |プルバス、ガーディアンのチケット、リゾルバーの差分、`sns governance ...` の KPI およびサリダ企業のエクスポート。 |

Todas las acciones de gobernanza deben リファレンス al menos un artefacto en la
 tabla anterior para que los audiores reconstruyan el rastro de Decision en 24
 ほら。

## 4. 戦略的プレイブック

### 4.1 カルタおよびスチュワードの行動|パソ |ドゥエノ | CLI / 証拠 |メモ |
|------|------|--|------|
| Redactar の付録とデルタの KPI |コンセホ報告者 + 管理責任者 | Plantilla マークダウン en `docs/source/sns/governance_addenda/YY/` |契約 KPI の ID、テレメトリのフック、アクティベーションの条件が含まれます。 |
|プレゼンター・プロプエスタ |プレジデンシア デル コンセホ | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` を生成) | La CLI は、Norito と `artifacts/sns/governance/<id>/charter_motion.json` を出力します。 |
|保護者への投票とレコノシミエント |コンセホ + 守護者 | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` |定足数に対する追加議事録。 |
|アセプタシオン・デ・スチュワード |スチュワードプログラム | `sns governance steward-ack --proposal <id> --signature <file>` |治安維持政治に対する要求。 Guardar ソブレ en `artifacts/sns/governance/<id>/steward_ack.json`。 |
|アクティバシオン |登録業務 |実際の `SuffixPolicyV1`、レジストラ キャッシュの参照、`status.md` の公開メモ。 | `sns_governance_activation_total` のアクティブ化のタイムスタンプ。 |
|監査ログ |クンプリミエント | Agregar entrada a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` y ドリルログ si Hubo テーブルトップ。 |参照情報、テレメトリのダッシュボード、政治の差分を含めます。 |

### 4.2 レジストロス、サブバス、およびプレシオの業務

1. **プリフライト:** 登録者は `SuffixPolicyV1` を確認してください
   プレシオ、テルミノス・ディスポニブルズ、ベンタナス・デ・グラシア/リデンシオン。マンテナー・ホハス・デ
   ニベル 3/4/5/6-9/10+ (ニベル ベース +
   係数) ロードマップに関する文書。
2. **Subastas sealed-bid:** Para pools premium、ejecutar el ciclo 72 h commit /
   `sns governance auction commit` / `... reveal` 経由で 24 時間表示されます。パブリックラ
   コミットのリスト (ソロ ハッシュ) `artifacts/sns/auctions/<name>/commit.json`
   パラ・ケ・ロス・オーディトレス・ベリフィケン・ラ・レアトリエダ。
3. **認証の確認:** 有効な `PaymentProofV1` コントラドレスの登録
   レパルト デ テソレリア (テソレリア 70% / スチュワード コン カーブアウト デ レフェリド 30% <=10%)。
   JSON Norito en `artifacts/sns/payments/<tx>.json` および enlazarlo を保護します
   登録者登録 (`RevenueAccrualEventV1`)。
4. **フック デ ゴベルナンザ:** アジャンター `GovernanceHookV1` パラ番号プレミアム/ガード付き
   管理者と管理会社の情報を参照してください。フック
   ファルタンテスの結果 en `sns_err_governance_missing`。
5. **アクティベーション + 同期デゾルバー:** Torii がイベントを発行する
   登録、確認の際の透明性の確認
   新旧の GAR/ゾーン SE プロパゴ (ver 4.5)。
6. **顧客の漏洩:** 顧客の元帳の記録
   （ウォレット/エクスプローラー）Los fixtures compartidos en 経由
   [`address-display-guidelines.md`](./address-display-guidelines.md)、アセグランド
   I105 とコピア/QR の同時実行。

### 4.3 改修、事実確認とテソリアの和解

- **改修期間:** 登録申請者による登録の完了
  `SuffixPolicyV1` で 30 ディアス + 60 ディアス固有の記述。デプス・ド・60
  ディアス、ラ セクエンシア デ レアペルトゥラ ホランデサ (7 ディアス、タリファ 10x ディケイエンド 15%/ディア)
  `sns governance reopen` 経由で自動アクティベートしてください。
- **Reparto de ingresos:** 改修と移転の作成
  `RevenueAccrualEventV1`。 Las exportaciones de tesoreria (CSV/パーケット) デベン
  和解会議の開催。アジャンタル プルエバス エン
  `artifacts/sns/treasury/<date>.json`。
- **レファリドのカーブアウト:** レファリドのオプションの記録
  スフィホ・アグレガンド `referral_share` ア・ラ・ポリティカ・ド・スチュワード。ロスレジストラドレス
  エミテンエルスプリットファイナルとガーダンマニフィストスデレリドジュントアラプルエバデパゴ。
- **レポート報告書:** 金融関連の KPI 管理表 (登録機関、
  renovaciones、ARPU、紛争/債券の使用) en
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。 Los ダッシュボード デベン・トマール
  Grafana 偶然の一致で、ミスマス タブラスをエクスポートします。
  台帳の証拠。
- **KPI 改訂版:** 主要なチェックポイントの概要
  フィナンザス、スチュワード・ド・トゥルノ、PM デル・プログラム。アブリル エル [SNS KPI ダッシュボード](./kpi-dashboard.md)
  (`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` のポータルを埋め込みます)、
  レジストラのスループットと収益をエクスポートする、レジストラのデルタを表示する
  追加の付属品のメモ。改訂版との相違点
  SLA の停止 (72 時間以上のフリーズ、登録エラーの発生、
  ドリフト de ARPU)。

### 4.4 コンゲラミエントス、紛争およびアペラシオン|ファセ |ドゥエノ |アクシオンと証拠 | SLA |
|------|------|--------|-----|
|フリーズソフトのご相談 |スチュワード / 出張 |プレゼンター チケット `SNS-DF-<id>` は、パゴ、債券、紛争セレクターの参照を含む。 | <= 4 時間以内です。 |
|チケットdeガーディアン |保護者フンタ | `sns governance freeze --selector <I105> --reason <text> --until <ts>` は `GuardianFreezeTicketV1` を生成します。 JSON デル チケット en `artifacts/sns/guardian/<id>.json` を保護します。 | <=30 分の ACK、<=2 時間の排出。 |
|コンセホの承認 |コンセホ・デ・ゴベルナンサ |会議の予定、保護者のチケットと紛争のダイジェストを記録するドキュメンタリー決定。 |プロキシマ セッション デル コンセホ オ ヴォート アシンクロノ。 |
|裁定パネル |クンプリミエント + スチュワード | `sns governance dispute ballot` 経由で、7 つのジュラドス (セガン ロードマップ) に関するコンボカー パネルが報告されています。事件発生時の追加の通知。 |保証金の保証金 <=7 が保証されています。 |
|アペラシオン |ガーディアンズ + コンセホ |二重の絆と繰り返しの手続きを忘れないでください。レジストラは、Norito `DisputeAppealV1` y 参照チケット primario を明示します。 |直径10以下|
|デスコンゲラーと救済策 |レジストラ + リゾルバ操作 | Ejecutar `sns governance unfreeze --selector <I105> --ticket <id>`、実際の登録者とプロパガーの差分 GAR/リゾルバー。 |インメディアト・デプス・デル・ヴェディクト。 |

Los canones de emencia (congelamientos activados por Guardes <=72 時間) シグエン
エル ミスモ フルホ ペロ 必要なリビジョン レトロアクティバ デル コンセホとウナ ノタ デ
トランスパレンシア en `docs/source/sns/regulatory/`。

### 4.5 リゾルバーとゲートウェイの伝播

1. **イベントのフック:** イベントのストリームを登録するためのイベントの送信
   リゾルバ (`tools/soradns-resolver` SSE)。リゾルバーのサブスクリプション操作
   el tailer de transparencia 経由の登録差分
   (`scripts/telemetry/run_soradns_transparency_tail.sh`)。
2. **plantilla GAR の実現:** ゲートウェイの plantilla GAR の実現
   `canonical_gateway_suffix()` を参照し、リストを再確認してください
   `host_pattern`。 Guardar の差分は `artifacts/sns/gar/<date>.patch` です。
3. **ゾーンファイルの公開:** ゾーンファイルのスケルトン説明を使用する
   `roadmap.md` (名前、ttl、cid、証明) と Torii/SoraFS を入力してください。アーカイブエル
   JSON Norito と `artifacts/sns/zonefiles/<name>/<version>.json`。
4. **Chequeo de transparencia:** Ejecutar `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   永続的な状況を監視します。テキストの追加
   Prometheus 透明性レポート。
5. **ゲートウェイの監査:** ヘッダーの登録機関 `Sora-*` (政治的情報)
   キャッシュ、CSP、GAR のダイジェスト）および政府のログに関する補助的な機能
   オペラドール プエダン プロバル ケ エル ゲートウェイ シルビオ エル ヌエボ ノンブル コン ロス
   ガードレールプレビスト。

## 5. テレメトリアのレポート

|セナル |フエンテ |説明 / アクシオン |
|--------|--------|-----------|
| `sns_registrar_status_total{result,suffix}` | Torii レジストラー ハンドラー | Contador de exito/error para registers、renovaciones、congelamientos、transferencias;アラート クアンド `result="error"` スベ ポル スフィジョ。 |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii のメトリクス |ハンドラー API のレイテンシアの SLO。 alimenta ダッシュボード basados en `torii_norito_rpc_observability.json`。 |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` |リゾルバーの透明化のテーラー | GAR のドリフトの古い情報を検出します。ガードレールは `dashboards/alerts/soradns_transparency_rules.yml` で定義されています。 |
| `sns_governance_activation_total` |ガバナンス CLI | Contador は、認可/追加の活動を開始します。米国パラ和解決定デルコンセホ対追加公開。 |
| `guardian_freeze_active` ゲージ |ガーディアンCLI | Rastrea ventanas de フリーズソフト/ハードポートセレクター; SRE SI EL VALOR QUEDA en `1` mas alla del SLA declarado を参照してください。 |
| KPI 別館ダッシュボード |フィナンザ / ドキュメント |ロールアップは、定期刊行物と規制に関するメモを公開します。 [SNS KPI ダッシュボード](./kpi-dashboard.md) のポータルで、スチュワードと規制当局が Grafana の素晴らしい景色を確認できます。 |

## 6. 証拠と聴覚の要件|アクシオン |アーカイブの証拠 |アルマセン |
|----------|----------------------|----------|
|カンビオ・デ・カルタ / 政治 | Norito ファーム、トランスクリプト CLI、差分 KPI、アキューズ ド スチュワードをマニフェストします。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|レジストロ / リノベーション |ペイロード `RegisterNameRequestV1`、`RevenueAccrualEventV1`、プルエバ デ パゴ。 | `artifacts/sns/payments/<tx>.json`、API 登録者のログ。 |
|スバスタ |コミット/公開、アレアトリエダのセミリャ、ガナドールの計算スプレッドシートをマニフェストします。 | `artifacts/sns/auctions/<name>/`。 |
|コンゲラー / デスコンゲラー |保護者のチケット、コンセホのハッシュ、インシデントのログの URL、顧客とのコミュニケーションのプラント。 | `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|リゾルバの伝播 |ゾーンファイル/GAR の差分、JSONL デル テーラーの抽出、スナップショット Prometheus。 | `artifacts/sns/resolver/<date>/` + 透明レポート。 |
|イングレソ規制 |記録のメモ、期限の追跡、スチュワードの告発、カンビオの KPI の再開。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. 事前チェックリスト

|ファセ |サリダの基準 |証拠のバンドル |
|------|---------|----------------------|
| N0 - ベータセラーダ |レジストリ SN-1/SN-2、CLI レジストラ マニュアル、ガーディアンのドリル。 |カルタのモーション + スチュワードの ACK、レジストラのドライラン ログ、リゾルバの透明性レポート、`ops/drill-log.md` のエントリ。 |
| N1 - ランサミエント公共 | `.sora`/`.nexus` のサブバスと優先順位のアクティビティ、レジストラのセルフサービス、自動同期デゾルバー、ダッシュボードの事実確認。 |精度の差分、CI の登録結果、パゴス/KPI の確認、透明性の確認、事故の記録。 |
| N2 - 拡張 | `.dao`、リセラーの API、議論のポータル、スチュワードのスコアカード、分析のダッシュボード。 |ポータルのキャプチャ、紛争の SLA の測定、スチュワードのスコアカードの輸出、再販業者の実際の政治に関する憲章。 |

必要な要件を満たしているかどうかは、卓上レジストラド (レジストロ ハッピー パス、
フリーズ、停止デゾルバ) の付属品 `ops/drill-log.md`。

## 8. 事件とエスカラミエントの解決

|トリガー |セベリダッド |ドゥエノ インメディアト |アクシオネス・オブリガトリアス |
|----------|----------|----------------|----------|
|リゾルバー/GAR のドリフトが廃止されました。セクション 1 |リゾルバー SRE + 保護軍事政権 |ページのオンコール デル リゾルバー、キャプチャー サリダ デル テイラー、デシディル SI コンジェラル ネームレス、公開時間 30 分。 |
|登録者、事実上のエラー API 全般 |セクション 1 |登録管理者 |サブバスの決定、CLI マニュアル、通知スチュワード/テソレリア、Torii およびインシデントの付属ログ。 |
|ユニコの名前に関する紛争、クライアントとのエスカラミエントの不一致 |セクション 2 |スチュワード + ライダー デ ソポルテ |プルエバスの再コピー、ファルタ フリーズ ソフトの決定、SLA の要請に対する応答者、紛争のトラッカーの登録結果。 |
|講堂のホールズゴ |セクション 2 |アンレース・デ・クンプリミエント |修復計画の編集、`docs/source/sns/regulatory/` のアーカイブメモ、安全保障会議の議題。 |
|ドリルをえんさよ |セクション 3 | PM デル プログラム | Ejecutar escenario guionado desde `ops/drill-log.md`、アーカイブ アーティファクト、ロードマップに含まれるマルカー ギャップ。 |

Todos los Incidentes deben crear `incident/YYYY-MM-DD-sns-<slug>.md` con tablas
所有権、コマンドおよび参照の記録、証拠の作成
プレイブック。

## 9. 参考資料

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS、DG、ADDR セクション)

Mantener エステ プレイブックの実際のクアンド カンビエン エル テキスト デ カルタス、ラス
CLI またはテレメトリーの管理権限。ロードマップのラス・エントラーダス
参照者 `docs/source/sns/governance_playbook.md` デベン コインシディル シエンプレ コン
ラリビジョンマスレジェンテ。