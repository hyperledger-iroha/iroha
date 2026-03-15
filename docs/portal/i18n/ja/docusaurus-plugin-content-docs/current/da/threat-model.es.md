---
lang: ja
direction: ltr
source: docs/portal/docs/da/threat-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
リフレジャ`docs/source/da/threat_model.md`。 Mantenga ambas バージョン en
:::

# Sora Nexus のデータ可用性モデル

_Ultima 改訂版: 2026-01-19 -- Proxima 改訂プログラム: 2026-04-19_

Cadencia de mantenimiento: データ可用性ワーキング グループ (直径 90 未満)。カダ
`status.md` のリビジョンがチケットの軽減活動にリンクしています
シミュレーションの成果物。

## 提案と予定

データ可用性 (DA) の定期的な送信プログラム、BLOB
レーン Nexus y artefactos de gobernanza recuperables ante fallas bizantinas、
赤とオペラドール。エステ モデル デ アメナザス アンクラ エル トラバホ デ インジニアリア
DA-1 パラ (アーキテクチャとアメナザスのモデル) とタエリアのベースラインを提供
DA 事後 (DA-2 および DA-10)。

コンポーネントの詳細:
- 取り込み拡張子 DA en Torii とメタデータ記述子 Norito。
- Arboles de almacenamiento de blobs respaldados por SoraFS (層ホット/コールド) y
  レプリケーションの政治。
- ブロック Nexus の侵害 (ワイヤ形式、証明、クライアントの API)。
- ペイロード DA に特有の PDP/PoTR 施行のフック。
- オペラドールのフルホス (固定、エビクション、スラッシュ) とパイプライン
  観察可能です。
- オペラドールとコンテニド DA の追放を認めます。

最高の文書:
- 経済モデルの完全版 (ワークストリーム DA-7 のキャプチャ)。
- SoraFS のプロトコル ベースと、SoraFS のモデルの詳細。
- 顧客の SDK の人間性に関する管理の検討
  アメナザ。

## パノラマ建築物

1. **Envio:** Torii の API 取り込み DA を介したクライアントの環境ブロブ。エル
   ノード トロセア ブロブ、コードフィカ マニフェスト Norito (ブロブのヒント、レーン、エポック、
   コーデックのフラグ)、SoraFS の層ホットのアルマセナ チャンク。
2. **発表:** プロパガンダの複製のヒントを示す意図
   レジストリ (SoraFS マーケットプレイス) 経由でアルマセナミエントと政治のタグ
   ホット/コールドを保持するインドのオブジェクト。
3. **侵害:** Los secuenciadores Nexus には BLOB の侵害が含まれます (CID +
   ルーツ KZG opcionales) en el bloque canonico。クライアント リジェロス依存デル
   可用性を検証するために、メタデータを侵害するハッシュを作成します。
4. **レプリカ:** ノード デ アルマセナミエント デスカルガン シェア/チャンク アシニャド、
   PDP/PoTR を満たして、ホットとコールド セグンのデータを保護します。
   政治。
5. **回復:** SoraFS o ゲートウェイ DA 対応の Consumidores 回復データ、
   証拠を検証し、レバンタンドの賠償請求を確認してください。
   レプリカ。
6. **Gobernanza:** Parlamento y el comite de Supervision DA aprueban operadores、
   レンタルのスケジュールと執行のエスカレーション。ゴベルナンザの工芸品
   アルマセナン ポル ラ ミスマルタ DA パラ ガランティザール トランスパレンシア デル プロセソ。

## 活動担当者影響の拡大: **Critico** ロンペ セグリダード/ビバシダード デル レジャー。 **アルト**
bloquea バックフィル DA o クライアント。 **モデラード** デグラダ カリダ ペロ エス
回復可能; **バジョ** 効果の限界。

|アクティボ |説明 |インテジダード |ディスポニビリダード |機密情報 |責任者 |
| --- | --- | --- | --- | --- | --- |
| BLOB DA (チャンク + マニフェスト) | Blobs Taikai、レーンとゴベルナンザ アルマセナドス、SoraFS |クリティコ |クリティコ |モデラド | DA WG / ストレージ チーム |
|マニフェスト Norito DA | BLOB を説明するメタデータ情報 |クリティコ |アルト |モデラド |コアプロトコルWG |
|ブロックの妥協 | CID + ルート KZG デントロ デ ブロック Nexus |クリティコ |アルト |バジョ |コアプロトコルWG |
| PDP/PoTR のスケジュール | DA | レプリカの執行カデンシア |アルト |アルト |バジョ |ストレージチーム |
|オペラドール登録 |アルマセナミエントと政治のプロヴェドーレス |アルト |アルト |バジョ |ガバナンス評議会 |
|インセンティブレンタル登録 | DA | レンタルおよびペナリダデスに関する登録台帳アルト |モデラド |バジョ |財務WG |
|観察可能なダッシュボード | SLO DA、レプリケーションの深さ、アラート |モデラド |アルト |バジョ | SRE / 可観測性 |
|賠償の意図 | rehidratar チャンク ファルタンテスのソリチュード |モデラド |モデラド |バジョ |ストレージチーム |

## 敵対者と大容量

|俳優 |キャパシダーズ |動機 |メモ |
| --- | --- | --- | --- |
|クライアント マリシオソ | Enviar の不正な BLOB、古いマニフェストのリプレイ、意図的な DoS の摂取。 | Interrumpir は Taikai, inyectar datos validos を放送します。 |罪はクラベス特権です。 |
|ノド デ アルマセナミエント ビザンティーノ | Soltar レプリカの指示、forjar プルーフ PDP/PoTR、coludir con otros。 | Reducir retencion DA、evitarrenta、retener datos como rehens。 |オペレーターの資格を取得します。 |
|安全なコンプロメティド |侵害、あいまいなブロック、ブロブのメタデータの再配列を省略します。 | Ocultar envios DA、矛盾が生じます。 |マヨリア・デ・コンセンサスの制限。 |
|オペレーターインテルノ |行政へのアクセス、保持のための操作性の高い政治、フィルタラーの資格を取得します。 |ガナンシアエコノミカ、サボタヘ。 |インフラストラクチャのホット/コールドにアクセスします。 |
|アドベルサリオ・デ・レッド |パーティショナ ノード、デモラー レプリカシオン、インジェクター トラフィック MITM。 | Reducir の可用性、SLO の低下。 | puede romper TLS pero puede soltar/ralentizar リンクはありません。 |
|観察可能なアタカンテ |操作されたダッシュボード/アラート、至上主義的なインシデント。 | Ocultar caidas DA. |テレメトリのパイプラインへのアクセスが必要です。 |

## フロンテラス・デ・コンフィアンサ- **Frontera de ingreso:** Torii の拡張子 DA をクライアントにします。認証が必要です
  リクエスト、レート制限、ペイロードの検証。
- **複製の最前線:** カンビアン間チャンクのアルマセナミエントのノード
  証拠。形式的な規則を遵守し、相互に協力する必要があります
  ビザンティーナ。
- **フロンテーラ・デル・レジャー:** ダトス・デ・ブロック・コンプロメティドス vs アルマセナミエント
  オフチェーン。エル・コンセンサス・プロテゲ・インテリダード、個人の利用可能性が必要
  オフチェーンでの執行。
- **Frontera de gobernanza:** 4 月議会/議会の決定
  オペラドール、プレスプエストス、斬り込み。直接影響を受ける火事
  デスリーグDA.
- **観測可能範囲:** メトリクス/ログのエクスポートの収集
  ダッシュボード/アラート ツール。 La manipulacion oculta の停止または ataques。

## 管理と管理のシナリオ

### 摂取量の確認

**シナリオ:** ペイロード Norito 不正な BLOB を介した悪意のあるクライアント
禁制品のメタデータが無効な場合は、再帰的な問題が発生します。

**コントロール**
- スキーマ Norito のバージョン制限の検証。レチャザール
  デスコノシドスにフラグを立てます。
- エンドポイントの取り込み時のレート制限 Torii。
- チャンク サイズとエンコーディングの制限は、チャンカー SoraFS で決定されます。
- チェックサムの一致したマニフェストを許可するパイプラインを保持します
  インテリダード。
- キャッシュ デ リプレイ決定 (`ReplayCache`) rastrea ventanas `(レーン、エポック、
  シーケンス)`、ディスコで最高水準点を維持し、重複/リプレイを繰り返す
  オブレトス。プロピエダードのハーネスとファズキュブレンの指紋が発散する
  エンヴィオス・フエラ・デ・オルデン。 [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**ブレカス残差**
- Torii カーソルを許可してリプレイ キャッシュを取り込みます
  連続して移動します。
- ロス スキーマ Norito DA アホラ ティネン アン ファズ ハーネス デディカド
  (`fuzz/da_ingest_schema.rs`) エンコード/デコードのパラメータ不変式。ロス
  コベルトゥーラ デベン アラート ターゲット レポートのダッシュボード。

### 源泉徴収と複製の保持

**シナリオ:** アルマセナミエント ビザンティノス アセプタン アサインナシオネス オペラドール
ピン・ペロ・スエルタン・チャンク、パサンド・デサフィオス PDP/PoTR via respuestas forjadas o
共謀。

**コントロール**
- ペイロード DA コンバーチュラ ポートを拡張するための PDP/PoTR スケジュールの設定
  時代。
- レプリケーションのマルチソースではクォーラムが制限されます。エルフェッチオーケストレータ検出
  シャードファルタンテスと格差回復。
- デ・ゴベルナンサ・ビンクラドを斬りつけることで、ファリダスとレプリカ・ファルタンテスが証明される。
- 自動調整ジョブ (`cargo xtask da-commitment-reconcile`)
  侵害された DA と取り込み済みの領収書を比較 (SignedBlockWire、`.norito` o
  JSON)、Gobernanza の証拠となる JSON をバンドルして発行し、事前チケットを発行します
  Alertmanager ページの欠落/改ざんは偶然ではありません。**ブレカス残差**
- シミュレーション用ハーネス `integration_tests/src/da/pdp_potr.rs` (キュービエルト)
  por `integration_tests/tests/da/pdp_potr_simulation.rs`) アホラ・エジェルシタ
  共謀とパーティのシナリオ、スケジュールの検証 PDP/PoTR 検出
  comportamiento bizantino de forma determinista.シガ・エクステンディエンドロ・ジュント・ア
  DA-5 パラキュブリルヌエバス地上権証明。
- 監査証跡会社に対する冷酷な立ち退き要求の法廷政治
  プレベニールはエンクビエルトスをドロップします。

### 不正操作

**シナリオ:** 公共ブロックの安全性を確保し、代替策を講じる
DA の妥協、クライアントの不正なフェッチの矛盾を引き起こします。

**コントロール**
- エル・コンセンサス・クルーザ・プロプエスタ・デ・ブロック・コン・コーラス・デ・エンヴィオDA;ピア・レチャザン
  危険を冒す罪を犯します。
- クライアントは、エクスポナーがフェッチを処理する前に、インクルージョン証明を検証します。
- ブロック上の侵害に関する監査証跡のコンパランドレシート。
- 自動調整ジョブ (`cargo xtask da-commitment-reconcile`)
  侵害された DA と取り込み済みの領収書を比較 (SignedBlockWire、`.norito` o
  JSON)、Gobernanza の証拠となる JSON をバンドルして発行し、事前チケットを発行します
  Alertmanager ページの欠落/改ざんは偶然ではありません。

**ブレカス残差**
- 調整の仕事とアラートマネージャーのフックを作成します。ロス・パケテス・デ
  Gobernanza アホラ インジェレン エル バンドル JSON の証拠と欠陥。

### 赤と検閲の分割

**シナリオ:** レプリケーションでの敵対行為、危険な行為
オブテンガンは、PDP/PoTR の指定に応答するチャンクを取得します。

**コントロール**
- マルチリージョン ガランティザン パス デ レッド ダイバーソの証明の必要条件。
- ベンタナス デ サフィオには、ジッターとフォールバック、損害賠償のカナルが含まれます
  デ・バンダ。
- ダッシュボードを観察し、複製を深く監視し、終了します
  安全性と潜在性をフェッチし、警告を参照します。

**ブレカス残差**
- 太海の生体内イベントにおけるファルタンのシミュレーション。必要です
  浸漬テスト。
- 賠償責任を負う政治政策。

### アブソ インテルノ

**シナリオ:** レジストリ管理の政治管理を担当するオペレーター、
ホワイトリストは、マリシオーソスまたは最高の警告を証明します。

**コントロール**
- 複数の当事者による登録会社の要求 Norito
  公証人。
- 政治活動はイベントを監視し、アーカイブを記録します。
- アプリケーションのログを監視するパイプライン Norito 追加専用のハッシュ チェーン。
- トリメストラルに基づく改訂の自動化
  (`cargo xtask da-privilege-audit`) マニフェスト/リプレイのディレクトリを記録する
  (mas paths provistos por operadores)、マルカ エントラダス ファルタンテス/ディレクトリなし/
  世界中で書き込み可能で、ダッシュボードの JSON ファームウェアをバンドルして出力します。

**ブレカス残差**
- ダッシュボードの改ざんを証拠するには、スナップショットの企業が必要です。

## レジストロ・デ・リースゴス残余|リースゴ |確率 |インパクト |責任者 |緩和策を計画する |
| --- | --- | --- | --- | --- |
|マニフェスト DA を順番に再生し、シーケンス DA-2 をキャッシュします。可能 |モデラド |コアプロトコルWG | DA-2 のシーケンス キャッシュ + ノンス検証を実装します。 agregar は回帰をテストします。 |
|共謀 PDP/PoTR は >f ノードを妥協する |ありえない |アルト |ストレージチーム |クロスプロバイダーによる新しいスケジュールの管理。ハーネス・デ・シミュレーションを介して検証します。 |
|寒さによる立ち退きの聴衆 |可能 |アルト | SRE / ストレージ チーム | Adjuntar は、チェーン上のパラ立ち退きに関する企業の領収書を記録します。ダッシュボード経由で監視します。 |
|セキュリティの検出と省略の遅延 |可能 |アルト |コアプロトコルWG | `cargo xtask da-commitment-reconcile` 夜の領収書と侵害の比較 (SignedBlockWire/`.norito`/JSON) y ページは、ゴベルナンザのアンティ チケット ファルタンテスまたは偶然ではありません。 |
| Taikai で生体内ストリームを再生するためのレジリエンシア |可能 |クリティコ |ネットワーキングTL |エジェクターは参加訓練を行う。リザーバー・アンチョ・デ・バンダ・デ・レパラシオン。フェイルオーバーに関するドキュメント SOP。 |
|ゴベルナンザ特権のデリバ |ありえない |アルト |ガバナンス評議会 | `cargo xtask da-privilege-audit` トリメストラル (ディレクトリ マニフェスト/リプレイ + 追加パス) JSON ファームド + ダッシュボードのゲート。オンチェーンの聴覚アーティファクトのアンカー。 |

## フォローアップが必要です

1. パブリックスキーマ Norito デインジェスタ DA y ベクトルデジェムプロ (DA-2 を販売)。
2. Torii のカーソルを保持するための DA の取り込みキャッシュの再生
   ノードの移動をシーケンスします。
3. **Completado (2026-02-05):** PDP/PoTR によるシミュレーションのハーネス
   共謀とバックログ QoS のモデル化のシナリオ。バージョン
   `integration_tests/src/da/pdp_potr.rs` (テストとテスト
   `integration_tests/tests/da/pdp_potr_simulation.rs`) 実装に関連
   ロス・レジュメネス・デターミニスタ・キャプチャー・アバホ。
4. **完全版 (2026-05-29):** `cargo xtask da-commitment-reconcile` の比較
   不正行為に反対する領収書 DA (SignedBlockWire/`.norito`/JSON)、
   `artifacts/da/commitment_reconciliation.json` を発行して、接続してください
   アラートマネージャー/不作為/改ざんに関するアラートのパケット
   (`xtask/src/da.rs`)。
5. **完了 (2026-05-29):** `cargo xtask da-privilege-audit` スプールを修正
   デ・マニフェスト/リプレイ (マス・パス・プロヴィストス・ポル・オペラドーレス)、マルカ・エントラダス
   faltantes/no directory/world-writable、y バンドル解除 JSON ファームード パラを生成します
   ダッシュボード/ゴベルナンザのリビジョン (`artifacts/da/privilege_audit.json`)、
   自動化されたアクセス機能を備えています。

**ドンデ ミラー デス:**- DA-2 でカーソルを永続化するリプレイ キャッシュ。ヴァーラ
  `crates/iroha_core/src/da/replay_cache.rs` の実装 (キャッシュのロジック)
  統合 Torii と `crates/iroha_torii/src/da/ingest.rs`、エンヘブラ ラス
  `/v2/da/ingest` の指紋との比較。
- エル ハーネス プルーフ ストリームを介したストリーミング PDP/PoTR のシミュレーション
  en `crates/sorafs_car/tests/sorafs_cli.rs`、相談内容の確認
  PoR/PDP/PoTR および動物モデルのシナリオ。
- 損失の結果、容量を修復し、生きたままにします
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`、マトリス・デ・ミエントラ
  Sumeragi マスアンプリアセラストレア `docs/source/sumeragi_soak_matrix.md` を浸します
  (ローカルの変種)。エストス アーティファクトス キャプチャラン ロス ドリル デ ラルガ
  残りの登録期間を参照します。
- 調整の自動化 + 存続する特権監査
  `docs/automation/da/README.md` ロス ヌエボス コマンド
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`;使う
  ラス・サリダス・ポル・ディフェクト・バホ `artifacts/da/` al adjuntar evidencia a paquetes
  デ・ゴベルナンザ。

## シミュレーションとモデルの QoS の証拠 (2026-02)

DA-1 #3 のフォローアップ、PDP/PoTR シミュレーションのコード化

決定的なバホ `integration_tests/src/da/pdp_potr.rs` (キュービエルト ポル)
`integration_tests/tests/da/pdp_potr_simulation.rs`)。 El ハーネス アシニャ ノードス

地域、感染拡大/共謀の可能性
ロードマップ、ラストレア タルダンサ PoTR、未処理の賠償モデルの支給
QUE refleja el presupuesto de reparacion del tier hot。エジェクター・エル・エスシナリオ
欠陥のある (12 エポック、18 デサフィオス PDP + 2 ベンタナ PoTR またはエポック) 生産
ラス・シグエンテスの指標:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
|メトリカ |勇気 |メモ |
| --- | --- | --- |
|ファジャス PDP 検出 | 48 / 49 (98.0%) |不審な検出を行うためのパーティション。ウナ・ソラ・ファラは、正直なところを検出できません。 |
| PDP のメディア検出遅延 | 0.0 エポック |起源となる時代が到来する。 |
| Fallas PoTR 検出 | 28 / 77 (36.4%) | La deteccion se activa cuando un nodo pierde >=2 ventanas PoTR、dejando laMayoria de events en el registro deriesgos Residence。 |
| PoTR メディア検出の遅延 | 2.0 エポック |アーカイブのエスカレーションを組み込んだ時代の計画が一致しました。 |
|ピコ・デ・コーラ・デ・レパラシオン | 38 マニフェスト |時代ごとに未解決の処理が行われ、迅速な対応が行われます。 |
|ラテンシア デ レスペスタ p95 | 30,068ミリ秒 | 30 秒間のジッターで +/-75 ms の QoS アプリケーションを実現します。 |
<!-- END_DA_SIM_TABLE -->

ダッシュボードのプロトティポスの結果、満足度を確認する
「シミュレーション ハーネス + QoS モデリング」の承認基準

ロードマップ。

自動化されたアホラ・ライブ・デトラス
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`、
ラマ・アル・ハーネス・コンパルティド・イ・エミット Norito JSON a
`artifacts/da/threat_model_report.json` の欠陥。夜行性のジョブが消費されました
エステ・アーカイブ・パラ・リフレスカー・ラス・マトリクス・エン・エステ・ドキュメントとアラート・デリバ

QoS の検出、修復、および管理。ドキュメントを参照してタブラデアリーバを参照し、`make docs-da-threat-model` を出力し、
クエリ呼び出し `cargo xtask da-threat-model-report`、再生成
`docs/source/da/_generated/threat_model_report.json`、y は esta セクションを再記述します
`scripts/docs/render_da_threat_model_tables.py`経由。エルエスペホ `docs/portal`
(`docs/portal/docs/da/threat-model.md`) 現実と誤解が現実になります

アンバス・コピア・セ・マンテンガン・アン・シンク。