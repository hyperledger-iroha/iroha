---
lang: ja
direction: ltr
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
Esta pagina espelha `docs/source/sns/governance_playbook.md` e agora は como a を提供します
コピアカノニカドポータル。 PR を永続的に保存してください。
:::

# Sora ネームサービスの統治戦略 (SN-6)

**ステータス:** Redigido 2026-03-24 - SN-1/SN-6 の参照  
**ロードマップに関するリンク:** SN-6「コンプライアンスと紛争解決」、SN-7「リゾルバーとゲートウェイ同期」、政治的政策 ADDR-1/ADDR-5  
**前提条件:** レジストリのエスケマ [`registry-schema.md`](./registry-schema.md)、レジストラの API コントラト [`registrar-api.md`](./registrar-api.md)、エンデレータの UX の設定[`address-display-guidelines.md`](./address-display-guidelines.md)、詳細は [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) で確認できます。

ソラ ネーム サービス (SNS) に関するエステ プレイブックの説明
ドメイン カルタス、承認登録、エスカラム紛争、プロヴァム クエスト エスタドス
リゾルバーとゲートウェイは永続的に管理されます。ロードマップを実行するための要件
CLI `sns governance ...`、マニフェスト Norito の聴覚芸術品
compartilhem uma unica Referencia voltada ao operador antes do N1 (ランカメント)
パブリコ）。

## 1. エスコポとパブリック

目的地を文書化してください:

- Conselho de Governanca que votam em cartas、politicas de sufixo e のメンバー
  論争の結果。
- 緊急事態宣言を発出する保護者らのメンバー
  リヴィザム・レベルソ。
- オペラ フィラス ド レジストラ、アプロヴァム レイロエス エ ゲレンシアムの審査員
  ディヴィサオ・デ・レセイタス。
- リゾルバー/ゲートウェイ応答の操作は、SoraDNS の宣伝を行います。
  GAR とテレメトリのガードレール。
- 適合性を備え、デモンストレーションを行うためのサポートを提供します
  acao de Governmenta deixou artefatos Norito 監査。

ベータフェチャダ (N0)、公開ランカメント (N1)、拡張 (N2) としてのエレ コブレ
`roadmap.md` のリスト、証拠としてのビンキュランド カダ フルクソ デ トラバリョ
必要なもの、ダッシュボード、エスカロナメントなど。

## 2. パペイスとマパ・デ・コンタート

|パペル |校長の責任 | Artefatos と telemetria principais |エスカラソン |
|----------|------------------------------|------------------------------------------|----------|
|コンセリョ・デ・ゴベルナンサ |報告書と報告書、政治的政治、紛争の正当性、および執政官の回転。 | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`、投票は `sns governance charter submit` 経由で conselho armazenados を行います。 | Presidente do conselho + rastreador de agenda de Governmentca。 |
|コンセリョ・デ・ガーディアン |ソフト/ハードのコンジェラメントを発声し、72 時間の緊急時の修正を行います。 |チケット ガーディアンは `sns governance freeze` によって発行され、マニフェストは `artifacts/sns/guardian/*` のレジストラドを上書きします。 |ロタカオ島の保護者がオンコール中 (ACK は 15 分以内)。 |
|スフィッソ審査員 | Operam filas do registrar、leiloes、niveis de preco e Communicacao com clientes;コンヘセム・コンミダデス。 | Politicas de Steward em `SuffixPolicyV1`、folhas de Referencia de preco、Acknowledgement de Steward armamazenados junto a memoregulatorios。 |ライダーはプログラムのスチュワード + PagerDuty のサフィックスを実行します。 |
|コブランカのレジストラのオペラ | Operam エンドポイント `/v1/sns/*`、パガメントの調整、テレメトリの発行、および CLI の管理スナップショット。 | API はレジストラ ([`registrar-api.md`](./registrar-api.md))、メトリクス `sns_registrar_status_total`、`artifacts/sns/payments/*` のデータ収集を行います。 |登録管理者とテソラリアとの連絡を担当する職務マネージャー。 |
|リゾルバとゲートウェイのオペランド | Mantem SoraDNS、GAR はレジストラのゲートウェイ アリンハドス イベントを実行します。透明性のある指標を送信します。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 | SRE リゾルバーのオンコール + ポンテ操作によるゲートウェイ。 |
|テソウラリアと金融 |受付 70/30 の申請、照会のカーブアウト、登録 fiscais/tesouraria および aatestacoes SLA。 |受け取り累計マニフェスト、ストライプ/テスラリアの輸出、KPI トリメストラの付録 `docs/source/sns/regulatory/`。 |管理者はfinanceiro + official de conformidade。 |
|適合性と規制に関する連絡 | Acompanha obrigacoes globais (EU DSA など)、atualiza 約款 KPI e registra divulgacoes。 | `docs/source/sns/regulatory/` に関する規制メモ、テーブルトップに関する参考資料、`ops/drill-log.md` に関するメモ。 |ライダーは適合プログラムを実行します。 |
|サポート / SRE オンコール | Lida com Incidentes (colisoes、drift de cobranca、quedas desolver)、coordena mensagens a clientes、および dono dos runbook。 |事件のテンプレート、`ops/drill-log.md`、実験室の証拠、Slack/作戦会議室のアーカイブ `incident/`。 | Rotacao オンコール SNS + gestao SRE。 |## 3. アルテファトス・カノニコスとフォント・デ・ダドス

|アルテファト |ローカライザソン |プロポジト |
|----------|-----------|----------|
|カルタ + 追加 KPI | `docs/source/sns/governance_addenda/` | CLI の投票を管理するためのカルタ、および規約 KPI と政府参照の決定。 |
|エスケマ ド レジストロ | [`registry-schema.md`](./registry-schema.md) | Estruturas Norito canonicas (`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`)。 |
| Contrato do レジストラ | [`registrar-api.md`](./registrar-api.md) |ペイロード REST/gRPC、メトリクス `sns_registrar_status_total`、ガバナンス フックの期待。 |
|ガイア UX デ エンデレコス | [`address-display-guidelines.md`](./address-display-guidelines.md) |レンダリザコエス カノニカ I105 (プレフェリド) とコンプリミダ (セグンダ メルホール オプカオ) ウォレット/エクスプローラーのリフレティダ。 |
|ドキュメント SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |ホストの決定性を決定し、透明性と警告を追跡します。 |
|メモ規制 | `docs/source/sns/regulatory/` |法的規制に関する注記 (例: EU DSA)、管理者の承認、テンプレートの付属書。 |
|ドリルログ | `ops/drill-log.md` | IR は事前に登録する必要があります。 |
|アルマゼナメント・デ・アルティファト | `artifacts/sns/` |プロバス、チケット ガーディアン、差分デゾルバー、KPI のエクスポート、`sns governance ...` による CLI の作成。 |

政府活動の参考人としての政策は、さまざまな分野での成果を表しています
24 時間以内に、視聴者が再解釈または決定を行ってください。

## 4. 戦略的プレイブック

### 4.1 文書管理委員会

|エタパ |返信 | CLI / 証拠 |メモ |
|------|-----------|------|------|
| Redigir の付録とデルタの KPI |リレーター・ド・コンセリョ + ライダー・スチュワード |テンプレート マークダウン armazenado em `docs/source/sns/governance_addenda/YY/` |契約 KPI の ID、テレメトリのフック、およびアティバケーションの管理が含まれます。 |
|エンヴィア プロポスト |プレジデンテ ド コンセーリョ | `sns governance charter submit --input SN-CH-YYYY-NN.md` (製品 `CharterMotionV1`) CLI はマニフェスト Norito サルボ em `artifacts/sns/governance/<id>/charter_motion.json` を発行します。 |
|承認ガーディアンに投票する |コンセリョ + 保護者 | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` |アネクサー・アタス・ハッシュ・デ・クォーラム。 |
|アセイタソンのスチュワード |スチュワードプログラム | `sns governance steward-ack --proposal <id> --signature <file>` |法的政治の義務を負う。レジストラエンベロープem `artifacts/sns/governance/<id>/steward_ack.json`。 |
|アティヴァカオ |運用レジストラ | Atualizar `SuffixPolicyV1`、atualizar キャッシュはレジストラ、パブリック ノート `status.md` を実行します。 |登録時のタイムスタンプは `sns_governance_activation_total`。 |
|ログ・デ・オーディトリア |コンフォルミダード | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` テーブルトップにドリルログを追加する必要はありません。 |参照情報、テレメトリのダッシュボード、政治の差分を含めます。 |

### 4.2 登録、レイラオ、プレコの登録

1. **プリフライト:** レジストラは `SuffixPolicyV1` を確認してください
   プレコ、テルモスディスポンイベスとジャネラスデグラサ/リデンカオ。マンテンハ・フォリャス・デ
   preco sincronizadas com a tabela de niveis 3/4/5/6-9/10+ (ニベルベース +
   coeficientes de sufixo) 文書化にはロードマップがありません。
2. **Leiloes sealed-bid:** パラ プール プレミアム、ciclo 72 時間のコミットを実行 /
   `sns governance auction commit` / `... reveal` 経由で 24 時間表示されます。パブリックa
   `artifacts/sns/auctions/<name>/commit.json` のコミット (アペナス ハッシュ) のリスト
   para que Auditores verifiquem aleatoriedade。
3. **認証情報:** レジストラは `PaymentProofV1` と反対です
   divisao de tesouraria (70% tesouraria / 30% スチュワード・コム・カーブアウト・デ・リファーラル <=10%)。
   Armazene または JSON Norito em `artifacts/sns/payments/<tx>.json` e vincule-o na
   resposta do レジストラ (`RevenueAccrualEventV1`)。
4. **統治フック:** Anexe `GovernanceHookV1` プレミアム/ガード付きコム
   提案書を参照し、コンセルホとスチュワードの活動を支援します。フック
   オーセンテスの結果は `sns_err_governance_missing` です。
5. **Ativacao + sync dosolver:** イベントを登録するために Torii を発行し、
   リゾルバーを確認するためのトランスパレンシアの処理と新しい設定
   GAR/ゾーンの宣伝 (veja 4.5)。
6. **Divulgacao ao cliente:** 元帳 voltado ao cliente (ウォレット/エクスプローラー) をアチュアライズします。
   OS フィクスチャ compartilhados 経由 [`address-display-guidelines.md`](./address-display-guidelines.md)、
   I105 は、オリエンタコス デ コピー/QR に対応するものを保証します。

### 4.3 Renovacoes、コブランカとテソリアの調整- **Fluxo de renovacao:** レジストラは 30 ディアス + ジャネラを申請します
  `SuffixPolicyV1` の 60 件の特定の情報を確認してください。アポス 60 ディアス、
  Sequencia de reabertura holandesa (7 dias、分類群 10x decaindo 15%/dia) e
  `sns governance reopen` 経由で自動的に実行されます。
- **受信部門:** 再開発と転送の保証
  `RevenueAccrualEventV1`。テスラリアの輸出 (CSV/Parquet) の開発調整
  エセス・イベントス・ディアリアメンテ。 anexe provas em `artifacts/sns/treasury/<date>.json`。
- **紹介のカーブアウト:** rastreados での紹介オプションの割合
  sufixo ao adicionar `referral_share` 政治家管理人。レジストラは、
  最終的なディヴィサオとアルマゼナムのマニフェスト・デ・リファーラル・オ・ラド・ダ・プロバ・デ・パガメント。
- **関係性評価:** Financas publica anexos KPI mensais (registros,
  renovacoes、ARPU、紛争/債券の使用) em `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。
  ダッシュボードは、さまざまなデータをエクスポートできるように開発されています。
  Grafana Batam com の証拠として台帳を作成します。
- **KPI メンサルの改訂:** 主要なチェックポイント、テルカフェイラ軍事政権のチェックポイント
  金融機関、工場長、PM がプログラムを実行します。アブラ o [SNS KPI ダッシュボード](./kpi-dashboard.md)
  (`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` のポータルを埋め込みます)、
  スループットのテーブルとしてエクスポート + レジストラの受信、登録デルタなし
  anexo e anexe os artefatos ao メモ。見直しを行うために必要な出来事
  SLA のケブラ (72 時間以上のフリーズ、レジストラのピコ、ARPU のドリフト)。

### 4.4 コンゲラメント、紛争、および紛争

|ファセ |返信 | Acao と証拠 | SLA |
|-----|----------|----------|-----|
|ペディド・デ・フリーズソフト |スチュワード / サポート | Abrir ticket `SNS-DF-<id>` com provas de pagamento、referencia do Bond de disputa e Seletor(es) afetados。 |滞在時間は 4 時間以内です。 |
|チケットガーディアン |コンセリョの守護者 | `sns governance freeze --selector <I105> --reason <text> --until <ts>` は `GuardianFreezeTicketV1` を製品化します。 Armazene または JSON チケット em `artifacts/sns/guardian/<id>.json`。 | <=30 分 ACK、<=2 時間実行。 |
|ラティフィカオ ド コンセーリョ |コンセリョ・デ・ガバナンカ |コンジェラメントに関する承認、ドキュメントの決定、com リンク、チケット ガーディアン、ダイジェスト、および論争の絆を確認してください。 | Proxima sessao do conselho ou voto assincrono. |
|裁定取引のパイネル |コンフォルミダード + スチュワード | `sns governance dispute ballot` 経由で 7 つのジュラドス (適合ロードマップ) を会議します。事件発生時のアノニミザドスの再発行。 | Veredito <=7 dias apos デポジットは保証金を保証します。 |
|アペラカオ |ガーディアン + コンセルホ |ジュラドスの絆と繰り返しのプロセスを支援します。レジストラマニフェスト Norito `DisputeAppealV1` 参照チケットのプリマリオ。 |直径10以下|
|デスコンゲラーと救済策 |レジストラ + リゾルバ操作 | Executar `sns governance unfreeze --selector <I105> --ticket <id>`、レジストラとプロパガーのステータスは GAR/リゾルバーと異なります。 |即時に確認します。 |

緊急事態 (congelamentos acionados por Guard <=72 時間) 緊急事態
フラクソ、レトロアティバの見直し、透明性に関する情報
`docs/source/sns/regulatory/`。

### 4.5 リゾルバーとゲートウェイの伝達

1. **イベントのフック:** イベントのストリームを実行するために登録されたイベントを発行します
   リゾルバ (`tools/soradns-resolver` SSE)。リゾルバーの操作に関する注意事項
   透明ファイルのテーラーを介したレジストラムの差分
   (`scripts/telemetry/run_soradns_transparency_tail.sh`)。
2. **テンプレート GAR の取得:** ゲートウェイは GAR テンプレートを開発します
   `canonical_gateway_suffix()` の再登録リストを参照
   `host_pattern`。 Armazene の差分 `artifacts/sns/gar/<date>.patch`。
3. **ゾーンファイルの公開:** ゾーンファイルの説明に `roadmap.md` のスケルトンを使用します。
   (名前、ttl、cid、証明) Torii/SoraFS のような羨ましい値。 JSON Norito em をアーカイブします
   `artifacts/sns/zonefiles/<name>/<version>.json`。
4. **透明性確認書:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml` を実行します。
   シガムベルデスの警告を保証します。本文の別紙
   Prometheus 透明な関係。
5. **ゲートウェイのオーディオ:** ヘッダーの登録 `Sora-*` (政治的情報)
   キャッシュ、CSP、ダイジェスト GAR) およびオペランド政府のログとしての別館
   ポッサム プロバー ケ、ゲートウェイ サービス、新しいガードレール エスペラード。

## 5. テレメトリと関係|シナル |フォンテ |説明 / アカオ |
|------|-------|------|
| `sns_registrar_status_total{result,suffix}` |ハンドラーはレジストラー Torii | Contador de sucesso/erro para registros、renovacoes、congelamentos、transferencias;警告 `result="error"` 拡張子を表示します。 |
| `torii_request_duration_seconds{route="/v1/sns/*"}` |メトリカス Torii | API ハンドラーの遅延に関する SLO。 alimenta ダッシュボード ベース em `torii_norito_rpc_observability.json`。 |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` |リゾルバーの透明化のテーラー | GAR のドリフトの時代遅れを検出します。ガードレールは `dashboards/alerts/soradns_transparency_rules.yml` と定義されています。 |
| `sns_governance_activation_total` | CLI の行政 |コンタドール増分契約書/追加条項アティバ;米国との和解決定は、コンセルホ対追加公開を行います。 |
| `guardian_freeze_active` ゲージ | CLI ガーディアン |アコンパニャ・ジャネラス・デ・フリーズソフト/ハードポートセレクター。ページ SRE セキュリティー フィカール `1` は SLA 宣言を行います。 |
| anexos KPI のダッシュボード |財務 / ドキュメント |規制に関する情報や公開情報をロールアップします。 o ポータル OS は、[SNS KPI ダッシュボード](./kpi-dashboard.md) を通じてスチュワードと規制当局にアクセスし、管理者 Grafana を介してエンビュートします。 |

## 6. 証拠と聴覚の要件

|アカオ |アーキバールの証拠 |アルマゼナメント |
|------|---------------------|---------------|
|カルタ / 政治 | ムダンカ デ カルタ / 政治マニフェスト Norito 暗殺、トランスクリプト CLI、KPI の差分、スチュワードの承認。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|レジストロ / レノバカオ |ペイロード `RegisterNameRequestV1`、`RevenueAccrualEventV1`、プロバ デ パガメント。 | `artifacts/sns/payments/<tx>.json`、API のログはレジストラに記録されます。 |
|レイラオ |マニフェストのコミット/公開、アレアトリエダーデの決定、ヴェンセドールの計算の計画。 | `artifacts/sns/auctions/<name>/`。 |
|コンゲラー / デスコンゲラー |チケット ガーディアン、コンセルのハッシュ、インシデントのログの URL、クライアントの通信テンプレート。 | `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|リゾルバーの宣伝 |ゾーンファイル/GAR の差分、トレチョ JSONL ドゥ テーラー、スナップショット Prometheus。 | `artifacts/sns/resolver/<date>/` + 透明性に関する関係。 |
|摂取量調整 |摂取のメモ、期限の追跡、スチュワードの承認、ムダンカス KPI の回復。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. 事前のチェックリスト

|ファセ |基準 |証拠のバンドル |
|------|----------------------|----------------------|
| N0 - ベータフェチャダ |レジストラ SN-1/SN-2 のエスケマ、レジストラの CLI マニュアル、ドリル ガーディアンの完全版。 |カルタ + ACK スチュワードのモーション、レジストラのドライラン ログ、リゾルバの透明性関係、`ops/drill-log.md` のエントリ。 |
| N1 - ランカメント パブリック | Leiloes + Tiers de preco fixo ativos para `.sora`/`.nexus`、レジストラーのセルフサービス、自動同期デゾルバー、コブランカのダッシュボード。 |事前の違い、登録結果の CI、登録/KPI の確認、透明性の確認、事故の記録。 |
| N2 - エクスパンサオ | `.dao`、リセラーの API、議論のポータル、スチュワードのスコアカード、分析のダッシュボード。 |キャプチャは、ポータル、紛争の SLA の指標、スチュワードのスコアカードの輸出、再販業者の政治情報の管理を行います。 |

言い表わされたように、卓上レジストラド (フラクソ フェリス デ レジストロ、
フリーズ、停止デゾルバー) com artefatos anexados em `ops/drill-log.md`。

## 8. 事件とエスカロナメントの報告

|ガティーリョ |セヴェリダーデ |すぐにアコス・オブリガトリアス |
|----------|-----------|--------------|---------------------|
|リゾルバー/GAR が廃止されたドリフト |セクション 1 | SRE リゾルバー + conselho ガーディアン |オンコールのリゾルバーをページングし、テーラーを捕捉し、30 分間の公開状況を確認します。 |
|レジストラのクエリ、コブランカのファルハ、エラー API 全般 |セクション 1 |登録管理者 | 業務管理者新たな情報、CLI マニュアルの変更、スチュワード/tesouraria への通知、別のログが Torii およびインシデントのドキュメントを参照してください。 |
|名前の争い、顧客の不一致、クライアントのエスカロナメント |セクション 2 |スチュワード + ライダー デ サポート |報告書を提出し、フリーズ ソフトの必要性を判断し、SLA の要請に応答し、紛争のトラッカーなしで結果を登録します。 |
|聴衆の適合性を確認 |セクション 2 |連絡窓口 |救済策の計画、`docs/source/sns/regulatory/` の保管メモ、コンセルホ デ アコンパンハメントの議題。 |
|ドリル・オ・エンサイオ |セクション 3 | PM はプログラムを実行します | `ops/drill-log.md` のシナリオを実行し、成果物を作成し、ロードマップのギャップをマークします。 |Todos OS のインシデント 開発者情報 `incident/YYYY-MM-DD-sns-<slug>.md` com テーブル
所有権、コマンドのログ、および証拠としての参照
デレステ攻略本。

## 9. 参考資料

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (セコエスSNS、DG、ADDR)

Mantenha este playbook atualizado semper que o texto das caras、地上権として
CLI とテレメトリア ムダレムの対照。 entradas がロードマップ クエリを行うように
Referenciam `docs/source/sns/governance_playbook.md` devem semper 通信者
アルティマ リヴィサオ。