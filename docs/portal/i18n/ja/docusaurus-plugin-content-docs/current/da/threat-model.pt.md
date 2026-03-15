---
lang: ja
direction: ltr
source: docs/portal/docs/da/threat-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
エスペラ `docs/source/da/threat_model.md`。デュアス・ヴェルソエス・エムとしてのマンテンハ
:::

# Sora Nexus のデータ可用性モデル

_Ultima 改訂: 2026-01-19 -- Proxima 改訂プログラム: 2026-04-19_

Cadencia de manutencao: データ可用性ワーキング グループ (直径 90 未満)。改訂版

Deve aparecer em `status.md` com リンク チケット パラレル チケット
シミュレーションの芸術品。

## プロポジトとエスコポ

データ可用性 (DA) 管理プログラム Taikai、ブロブ デ レーン
Nexus 政府の芸術品を回復して、すすり泣くファルハスビザンティナスを回復してください
オペラドール。エステ モデル デ アメアカス アンコラ オ トラバルホ デ エンゲンハリア パラ DA-1
(arquitetura emodelo de ameacas) タレファ DA と同様にベースラインを提供します
事後 (DA-2 と DA-10)。

エスコポなしのコンポーネント:
- DA の拡張機能は Torii、メタデータ Norito のライターです。
- SoraFS の BLOB サポートのアルボレス (ホット/コールド層)
  レプリカオの政治。
- ブロック Nexus のコミットメント (ワイヤ フォーマット、プルーフ、クライアント レベルの API)。
- ペイロード DA に特有の PDP/PoTR 施行のフック。
- オペランドのワークフロー (ピン留め、エビクション、スラッシュ) とパイプライン
  観察可能です。
- 政府の承認を得て、オペラドールとコンテウド DA を削除します。

文書の作成:
- Modelagem Economya completa (Capturada ワークストリーム DA-7 なし)。
- プロトコル ベース SoraFS、コベルトス ペロ モデル、アメカス SoraFS。
- SDK のエルゴノミアはクライアントの安全性を考慮します。

## ビザオ建築

1. **送信:** クライアントは、DA を取得する API を介して、Torii を実行して BLOB をサブメタします。 ○
   ノード分割 BLOB、codifica マニフェスト Norito (BLOB、レーン、エポック、フラグのヒント)
   デ コーデック）、アルマゼナ チャンクの層ホット ド SoraFS はありません。
2. **発表:** 証明されたプロパガンダの複製の意図とヒントをピン留めします
   レジストリ経由のストレージ (SoraFS マーケットプレイス) 政治的指標の com タグ
   メタス デ リテンソン ホット/コールド。
3. **コミットメント:** Sequenciadores Nexus には BLOB のコミットメントが含まれます (CID + ルート)
   KZG opcionais) ブロコ カノニコはありません。クライアントはハッシュに依存します
   可用性を検証するためのメタデータのコミットメント。
4. **Replicacao:** ノード デ アルマゼナメント プクサム シェア/チャンク アトリビュード、アテンデム
   PDP/PoTR を廃止し、政治の熱意と冷酷さを維持しながら政治を推進します。
5. **フェッチ:** SoraFS ou ゲートウェイ DA 対応のコンスミドール バスカム ダドス、
   検証証拠とレパロクアンドレプリカの放出を確認します。
6. **Governanca:** Parlamento e o comite de supervisao DA aprovam operadores、
   賃貸料と執行のエスカレーションのスケジュール。 Artefatos de Governmenta sao
   アルマゼナドス ペラ メスマ ロタ DA パラ ガランティル トランスパレンシア ド プロセス。

## 活動と応答の保存

影響の拡大: **Critico** quebra seguranca/vivacidade do ledger; **アルト**bloqueia バックフィル DA ou クライアント。 **モデラード** 永久的な劣化
回復する。 **バイショ** エフェイト リミタード。

|アティボ |説明 |インテジダーデ |ディスポニビリダーデ |秘密情報 |返信 |
| --- | --- | --- | --- | --- | --- |
| BLOB DA (チャンク + マニフェスト) |ブロブ大海、レーンとアルマゼナドス em SoraFS |クリティコ |クリティコ |モデラド | DA WG / ストレージ チーム |
|マニフェスト Norito DA |メタデータに関する情報の説明 BLOB |クリティコ |アルト |モデラド |コアプロトコルWG |
|ブロコの取り組み | CID + ルート KZG デントロ デ ブロコス Nexus |クリティコ |アルト |バイショ |コアプロトコルWG |
| PDP/PoTR のスケジュール | DA | レプリカの執行カデンシア |アルト |アルト |バイショ |ストレージチーム |
|オペラドール登録 |ストレージと政治のプロヴェドー |アルト |アルト |バイショ |ガバナンス評議会 |
|賃貸インセンティブ登録 | DA とペナリダデスの登録台帳パラレント |アルト |モデラド |バイショ |財務WG |
|観察可能なダッシュボード | SLO DA、複製の深さ、警告 |モデラド |アルト |バイショ | SRE / 可観測性 |
|修復の意図 |オーセンテス レイドラタル チャンクのペディドス |モデラド |モデラド |バイショ |ストレージチーム |

## 敵対的な大規模な衝突

|アトール |キャパシダーズ |モティバコーズ |メモ |
| --- | --- | --- | --- |
|クライアント マリシオソ |サブメーター BLOB の不正行為、リプレイ デ マニフェスト アンティゴ、Tentar DoS の取り込みなし。 |インターロンパーはタイカイ、インジェター・ダドス・インバリドスを放送する。 | Sem は特権を持っています。 |
|ビザンティーノのノード |レプリカ アトリビューダ、forjar プルーフ PDP/PoTR、coludir をドロップします。 |レドゥジル・リテンカオDA、エヴィター・レント、レテル・ダドス。 |オペレータの有効性に関する資格を取得します。 |
|コンプロメティドのシーケンス |コミットメントの省略、あいまいなブロック、BLOB の再配列メタデータ。 | Ocultar submissao DA、criar inconsistencia。 |コンセンサスに関する制限。 |
|オペレーターインテルノ |統治権限の行使、保持政治の操作、国家資格。 |ガンホー経済、妨害行為。 |ホット/コールドのインフラストラクチャにアクセスします。 |
|アドベルサリオ デ レデ |パーティショナノード、アトラサールレプリカオ、インジェタートラフェゴMITM。 | Reduzir の可用性、SLO の低下。 |ナオ ケブラ TLS マス ポデ ドロッパー/アトラサール リンク。 |
|観察可能なアタカンテ |操作されたダッシュボード/アラート、至上主義的なインシデント。 | Ocultar の停止 DA。 |テレメトリのパイプラインへのアクセスを要求します。 |

## フロンテイラス・デ・コンフィアンカ- **Fronteira de ingress:** Cliente para extensao DA do Torii。認証を要求する
  リクエスト、レート制限、ペイロードの検証。
- **フロンテイラ デ レプリカ:** ストレージ トロカム チャンクと証明のノード。 OSノード
  ビザンティナの形式に合わせて相互監視を行います。
- **フロンテイラ・ド・レジャー:** オフチェーンストレージとの連携管理。
  合意は完全に保証されており、大量の可用性にはオフチェーンでの強制が必要です。
- **Fronteira de Governmenta:** Decisoes Council/Parliament aprovando operadores、
  オルカメントスと斬撃。 DA の展開に影響を与える可能性があります。
- **観測最前線:** メトリクス/ログのエクスポートに関する情報
  ダッシュボード/アラート ツール。改ざんにより、攻撃が停止されます。

## アメアカとコントロールのセナリオス

### アタケスのカミーニョ デ インジェスタオ

**シナリオ:** Cliente malicioso submete payloads Norito malformados ou blobs
超次元の再帰的なメタデータの無効化。

**コントロール**
- スキーマ Norito com negociacao estrita de versao の検証。レジェイタールの旗
  デスコンヘシドス。
- Torii を取り込む際のレート制限、エンドポイントなしの認証。
- チャンク サイズとエンコーディングの決定的な制限は、チャンカー SoraFS に対して行われます。
- マニフェストのチェックサムを統合して永続化するためのパイプラインの承認
  偶然。
- リプレイ キャッシュ決定 (`ReplayCache`) rastreia janelas `(レーン、エポック、
  シーケンス)`、ディスコ、重複/リプレイなどの最高水準点を維持します
  オブレトス。ハーネスの所有物とファズコブレムの指紋が分岐する
  秩序を羨む。 [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**ラクナス残余**
- Torii キャッシュを取り込み、カーソルを保持してリプレイ キャッシュを取得します
  シーケンス・デュランテ・レイニシオ。
- スキーマ Norito DA アゴラ ポースエム ウム ファズ ハーネス デディカド
  (`fuzz/da_ingest_schema.rs`) エンコード/デコードのパラメータ不変式。 OS
  ダッシュボードのコベルトゥーラ開発アラート、ターゲットの登録。

### 複製による源泉徴収の保持

**シナリオ:** ストレージ ビザンティノスの操作は、マス ドロパム チャンクをピン留めし、
passando desafios PDP/PoTR via respostas forjadas ou Colusao。

**コントロール**
- ペイロード DA combertura por を設定する PDP/PoTR スケジュール
  時代。
- Replicacao マルチソース COM しきい値のクォーラム。 o オーケストレーターがシャードを検出する
  ファルタンテスとディスパラ・レパロ。
- ガバナンカ・ヴィンキュラドを斬りつけることで、ファルハスとファルタンテスのレプリカが証明される。
- 自動調整ジョブ (`cargo xtask da-commitment-reconcile`) クエリ
  COM コミットメントの受信確認を比較 (SignedBlockWire、`.norito` ou)
  JSON)、政府の証拠となるバンドル JSON を発行、ファルハ チケット
  アラートマネージャーのページに省略/改ざんがあるかどうかを確認します。**ラクナス残余**
- ハーネス デ シミュレーション `integration_tests/src/da/pdp_potr.rs` (コベルト ポート)
  `integration_tests/tests/da/pdp_potr_simulation.rs`) 演習を行う
  PDP/PoTR のスケジュールを確認し、ビザンティーノのコンポルタメントを検出します
  形式的決定論。続行 estendendo junto com DA-5 para cobrir novas
  表面的な証明。
- 政治的立ち退きのコールド層要求監査証跡アサシナド・パラ・エビター・ドロップ
  エンコベルトス。

### 約束の操作

**シナリオ:** 公共のブロックの同時実行と代替の実行
DA、顧客の矛盾をフェッチするための約束。

**コントロール**
- DA の提出に関する同意の決定;ピアレジェイタム
  プロポスト sem コミットメント要求。
- クライアントは、エクスポート処理およびフェッチの前に、包含証明を検証します。
- 監査証跡は、ブロコで提出されたレシートとコミットメントを比較します。
- 自動調整ジョブ (`cargo xtask da-commitment-reconcile`) クエリ
  COM コミットメントの受信確認を比較 (SignedBlockWire、`.norito` ou)
  JSON)、政府の証拠となるバンドル JSON を発行、ファルハ チケット
  アラートマネージャーのページに省略/改ざんがあるかどうかを確認します。

**ラクナス残余**
- Coberto pelo job de reconciliacao + フック アラートマネージャー;パコテス・デ・ガバナンカ
  デフォルトの証拠となる JSON をバンドルする前に。

### 検閲の当事者

**シナリオ:** レプリカでの敵対行為、ノードの妨害
応答側の PDP/PoTR のチャンクを取得します。

**コントロール**
- プロバイダーはマルチリージョンの多様な保証を必要とします。
- ジャネラス デ デサフィオには、修復のためのジッターとフォールバックが含まれます
  バンダ。
- ダッシュボードの監視、複製の詳細な監視、継続
  アラートのフェッチ COM しきい値の遅延と遅延。

**ラクナス残余**
- タイカイライブアインダファルタムのイベントに関するシミュレーション。サオ・ネセサリオス
  浸漬テスト。
- 政治的保護策、再検討のための政策。

### アブソ インテルノ

**シナリオ:** オペレータ コム アセソ アオ レジストリ操作による政治管理、
悪意のあるプロバイダーのホワイトリスト、最高のアラート。

**コントロール**
- 多党電子レジストリに対する統治要請 Norito
  公証人。
- 政治に関するムダンカスは、監視に関するイベントの記録を記録します。
- アプリケーションのログを監視するパイプライン Norito 追加専用の COM ハッシュ チェーン。
- トリメストラル自動改訂 (`cargo xtask da-privilege-audit`) が実行されます
  マニフェスト/リプレイのディレトリオス (オペラドールのパスを作成)、マルカ
  entradas faltantes/nao diretorio/world-writable、電子放出バンドル JSON assinado
  政府のダッシュボード。

**ラクナス残余**
- ダッシュボードの改ざんの証拠には、スナップショットが必要です。

## 残存登録簿|リスコ |確率 |インパクト |オーナー |プラノ デ ミティガカオ |
| --- | --- | --- | --- | --- |
|マニフェスト DA を再生してからシーケンス キャッシュを実行 DA-2 |可能性 |モデラド |コアプロトコルWG | DA-2 のシーケンス キャッシュ + ノンス検証を実装します。退行性精巣の追加。 |
| Colusao PDP/PoTR Quando >f ノードの互換性 |改善 |アルト |ストレージチーム |デサフィオス コム サンプリング クロスプロバイダーの新しいスケジュールをデリバリングします。ハーネスデシミュレーションを介して検証します。 |
|オーディオと立ち退きのコールド層のギャップ |可能性 |アルト | SRE / ストレージ チーム | Anexar は、チェーン上のパラ立ち退きに関する暴動や領収書を記録します。ダッシュボード経由で監視します。 |
|順番を忘れてからの遅延 |可能性 |アルト |コアプロトコルWG | `cargo xtask da-commitment-reconcile` は、レシートとコミットメントを比較しません (SignedBlockWire/`.norito`/JSON) チケットを管理し、分岐するページを作成します。 |
| Taikai ライブ配信における Resiliencia a particao |可能性 |クリティコ |ネットワーキングTL |執行者は参加訓練を行う。リザーバーバンダデレパロ。フェイルオーバーに関するドキュメント SOP。 |
|統治特権のデリバ |改善 |アルト |ガバナンス評議会 | `cargo xtask da-privilege-audit` トリメストラル (ディレクトリ マニフェスト/リプレイ + パス エクストラ) com JSON アッシナド + ダッシュボードのゲート;オンチェーンのアンカー・アルテファト・デ・オーディトリア。 |

## フォローアップが必要です

1. 公開スキーマ Norito DA の取り込み例 (カレガド エム
   DA-2)。
2. Encadear はリプレイ キャッシュを DA に取り込み、Torii でカーソルを永続化します。
   ノードの継続的なシーケンス。
3. **結論 (2026-02-05):** PDP/PoTR 演習のシミュレーションを活用する
   バックログ QoS のモデル化とパーティショニングを追加します。ヴェジャ
   `integration_tests/src/da/pdp_potr.rs` (com テスト
   `integration_tests/tests/da/pdp_potr_simulation.rs`) 実装パラメタ
   レスモス・デターミニスタ・キャプチャー・アバイショ。
4. **結論 (2026-05-29):** `cargo xtask da-commitment-reconcile` 比較
   com コミットメント DA (SignedBlockWire/`.norito`/JSON) を取り込んだレシート、
   `artifacts/da/commitment_reconciliation.json` を発して、エスタ リガド ア
   アラートマネージャー/不作為/改ざんに関するアラートの管理
   (`xtask/src/da.rs`)。
5. **結論 (2026-05-29):** `cargo xtask da-privilege-audit` percorre o spool
   マニフェスト/リプレイ (オペラドールのパスを作成)、マルカ エントラダス
   faltantes/nao diretorio/world-writable、電子製品バンドル JSON assinado para
   ダッシュボード/政府改訂版 (`artifacts/da/privilege_audit.json`)、
   自動的にアクセスできるスペースがありません。

**あなたは次のように行動します:**- DA-2 でキャッシュを再生し、カーソルを永続化します。ヴェジャ
  実装acao em `crates/iroha_core/src/da/replay_cache.rs` (ロジックキャッシュ)
  Torii と `crates/iroha_torii/src/da/ingest.rs` を統合し、エンカディア チェックを実行します。
  `/v1/da/ingest` 経由の指紋。
- ストリーミング PDP/PoTR のシミュレーションとして、プルーフ ストリームを利用して実行
  em `crates/sorafs_car/tests/sorafs_cli.rs`、コブリンド・フラクソス・デ・レクイシカオ
  PoR/PDP/PoTR は、ファルハ動物のシナリオではなく、アメリカのモデルでもありません。
- 結果の容量と修復の結果
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`、マトリックスを浸すだけ
  Sumeragi は、`docs/source/sumeragi_soak_matrix.md` をサポートします
  (variantes localizadas incluidas)。 Esses artefatos キャプチャー オス ドリル デ ロンガ
  デュラソー島の参照は残りの登録簿がありません。
- 自動調整 + 特権監査が存続します
  `docs/automation/da/README.md` のコマンド
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`;使う
  言ったように、パドラオは泣き叫んでいます `artifacts/da/` は、パコートの証拠を示しています
  ガバナンカ。

## シミュレーションとモデル化 QoS の証拠 (2026-02)

DA-1 #3 のフォローアップ、PDP/PoTR シミュレーションのコード化
決定的なすすり泣き `integration_tests/src/da/pdp_potr.rs` (コベルト ポル
`integration_tests/tests/da/pdp_potr_simulation.rs`)。アロカノードを活用してください
ロードマップの可能性に従って、地域、インジェタ・パルティコエス/コルサオが順守されます。
遅刻 PoTR、未払いの給与モデル、再パロクの再検討
オルカメント デ レパロ ド ティア ホット。 Rodar または cenario のデフォルト (12 エポック、18 デサフィオ)
PDP + 2 janelas PoTR por epoch) セギンテス メトリクスとして生成:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
|メトリカ |勇気 |メモ |
| --- | --- | --- |
| Falhas PDP 検出 | 48 / 49 (98.0%) |議論は差別を防止するものです。ウマ・ファルハ・ナオ・ディテクタダ・ヴェム・デ・ジッター・正直。 |
| PDP のメディア検出の遅延 | 0.0 エポック |ファルハス・サージェム・デントロ・ド・エポック・デ・オリジェム。 |
| Falhas PoTR 検出 | 28 / 77 (36.4%) | 2 日以上の PoTR に関する問題を検出し、残りのレジストリがないマイオリア ドス イベントを検出します。 |
| PoTR メディアの検出 | 2.0 エポック |あらゆる時代の出来事に対応し、議論を深めます。 |
|ピコ・ダ・フィラ・デ・レパロ | 38 マニフェスト |バックログは、時代ごとに迅速な対応が必要な状況にあります。 |
|レスポスタの潜在能力 p95 | 30,068ミリ秒 |サンプリング QoS なしで +/-75 ミリ秒の 30 秒通信ジッターを反映します。 |
<!-- END_DA_SIM_TABLE -->

Esses は、ダッシュボード DA の満足度を確認するためのサンプルを出力します。
「シミュレーション ハーネス + QoS モデリング」参照基準
ロードマップはありません。

自動アゴラ・ライブ・ポート・トラス・デ
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`、
ハーネスを比較して Norito JSON パラメータを発行します
`artifacts/da/threat_model_report.json` はデフォルトです。ジョブズ・ノトゥルノス・コンソメム・エステ
重要な文書と警告を取得するための基本的な派生情報

QoS のサンプルを検出し、修理します。ドキュメントを参照して、`make docs-da-threat-model` を実行してください。
クエリ呼び出し `cargo xtask da-threat-model-report`、再生成
`docs/source/da/_generated/threat_model_report.json`、e reescreve esta secao via
`scripts/docs/render_da_threat_model_tables.py`。オエスペリョ `docs/portal`
(`docs/portal/docs/da/threat-model.md`) e atualizado no mesmo passo para que
デュアスコピーフィケムエムシンクとして。