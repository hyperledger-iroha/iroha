---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard を '@site/src/components/ExplorerAddressCard' からインポートします。

:::ノート フエンテ カノニカ
Esta pagina refleja `docs/source/sns/address_display_guidelines.md` y ahora sirve
コモ ラ コピア カノニカ デル ポータル。マンティエンの PR をアーカイブする
取引。
:::

SDK の開発、管理、開発を行ってください。
cuenta como ペイロードは不変です。 Android の小売用ツール
`examples/android/retail-wallet` UX に対する要求:

- **コピーのオブジェクト。**明示的なコピーのボットのエンビア: IH58
  (preferido) y la forma comprimida Solo Sora (`sora...`, segunda mejor opción).
  IH58 は、QR の外部および栄養物との比較における安全性を保証します。ラ・ヴァリアンテ
  comprimida debe incluir una advertencia en linea porque Solo funciona dentro
  ソラのアプリケーションをサポートします。 Android の小売用ツール
  conecta ambos botones マテリアル y sus ツールチップ en
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`、yla
  iOS SwiftUI のデモ `AddressPreviewCard` によるミスモ UX のデモ
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **等幅、テキスト選択可能。** レンダリザ アンバス カデナス コン ウナ フェンテ
  モノスペース y `textIsSelectable="true"` パラ ケ ロス アメリカリオス プエダン インスペクション
  IME の呼び出しを評価します。 Evita の編集可能ファイル: los IME pueden reescribir
  カナ・オ・インイェクター・プントス・デ・コディゴ・デ・アンチョ・セロ。
- **暗黙的に支配権を持っています。** 選択可能な機能を選択してください。
  暗黙のドミニオ `default`、字幕記録とオペラドールの記録
  ケ・ノー・セ・リキエレ・スフィホ。ロス・エクスプロラドーレス・タンビエン・デベン・レサルタル・ラ・エチケット
  デ・ドミニオ・カノニカ・クアンド・エル・セレクター・コーディフィカ・アン・ダイジェスト。
- **QR IH58.** ロス コディゴス QR デベン コディフィカル ラ カデナ IH58。シ・ラ・ジェネラシオン
  QR を削除し、画像上の画像を明示的にエラー表示します。
- **Mensajeria del portapapeles.** Despues de copiar la forma comprimida, Emite
  トースト、スナックバーの記録、ロス・ユーズアリオス・ケ・エス・ソロ、ソラ・イ・プロペンサ・ア・ラ
  IMEによる歪み。

Unicode/IME の不正行為を防止し、基準を満たす必要があります
ビルテラス/エクスプローラーの UX に関するロードマップ ADDR-6 を受け入れます。

## パンタラ デ レファレンシアのキャプチャ

米国の最新情報を参照し、地域の最新情報を永続的に改訂します。
ボットンのエチケット、ツールチップ、広告をマンテンガン ラインで見ることができます
プラットフォームの入口:

- 参照 Android: `/img/sns/address_copy_android.svg`

  ![Android デダブルコピアのリファレンス](/img/sns/address_copy_android.svg)

- 参照 iOS: `/img/sns/address_copy_ios.svg`

  ![iOS デダブルコピア参照](/img/sns/address_copy_ios.svg)

## SDK のヘルパー

Cada SDK は IH58 y の便利な機能を提供します
UI をマンテンガンに設定して広告を表示する必要はありません
一貫しています:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript インスペクター: `inspectAccountId(...)` 広告のデブエルブ
  comprimida y la agrega a `warnings` cuando los llamadores proporcionan un
  リテラル `sora...`、拡張エクスプローラー/ダッシュボードの請求書を作成します
  Mostrar el aviso Solo Sora durante los flujos de pegado/validacion en lugar de
  ハセルロ ソロ クアンド ジェネラン ラ フォーマ コンプリミダ ポル ス クエンタ。
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- スイフト: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

米国の estos ヘルパーは、論理的な再実装、エンコード、ラス キャップス UI をサポートします。
JavaScript のヘルパーとペイロードの説明 `selector` と `domainSummary`
(`tag`、`digest_hex`、`registry_id`、`label`) パラ ケ ラス UI プエダン インディカー シ
セレクタを解除して、ローカル 12 またはレジストリ スキャン ボルバーの解析とペイロードのレスパルダを実行します
アンブルート。

## エクスプローラーのデモ

<エクスプローラーアドレスカード />

ロス・エクスプロラドーレス・デベン・リフレハル・エル・トラバホ・デ・テレメトリアとアクセス
ビルテラ:- Aplica `data-copy-mode="ih58|compressed|qr"` a los botones de copia para que
  ロス フロント エンド プエダン エミミル コンタドレス デ ユーソ ジュント コン ラ メトリカ Torii
  `torii_address_format_total`。イベント前のコンポーネントのデモ
  `iroha:address-copy` con `{mode,timestamp}`: パイプラインへの接続
  Analitica/telemetria (por ejemplo, envia a Segment o a uncollector respaldado)
  por NORITO) ダッシュボードとフォーマットの相関関係を確認
  クライアントからの情報提供のための指示。タンビエン・リフレハ・ロス
  Torii (`torii_address_domain_total{domain_kind}`) ドミニオの管理者
  Local-12 プエダン輸出業者のレヴィジョンに関するミスモ フィード
  30 ディアス `domain_kind="local12"` の指示に従ってください
  `address_ingest` と Grafana。
- `aria-label`/`aria-describedby` の距離を制御する必要があります
  expliquen si un literal es segro para compartir (IH58) o ソロ ソラ
  （コンプリミド）。暗黙的な説明のキャプションを含めます
  視覚的な視覚的な技術の進歩。
- 生きた領域を公開します (例、`<output aria-live="polite">...</output>`)
  コピアと広告の結果を発表し、イグアランド・エル・コンポルタミエント
  VoiceOver/TalkBack と Swift/Android の接続。

ADDR-6b のデモストラル クエリ ロス オペラドールを満足させるための機器
オブザーバー タント ラ 摂取 Torii コモ ロス モドス デ コピア デル クライアント アンテス デ
ローカルのセレクターを選択できます。

## ローカル -> グローバル移行ツールキット

米国エル [ツールキット ローカル -> グローバル](local-to-global-toolkit.md) 自動化ツール
リビジョン y 変換のセレクターのローカル ヘレダド。エルヘルパーエミテタントエル
Reporte de Auditoria JSON como la lista Convertida IH58/comprimida que los
オペラドールの準備のためのチケットの準備、ミエントラの実行手順書
Grafana のダッシュボードとアラート マネージャーの規則に準拠
モード制限によるカットオーバーの制御。

## レイアウト バイナリオの参照 (ADDR-1a)

Cuando los SDK の指示ツール (検査員、検査員)
検証、マニフェストのコンストラクター)、ディリジャン、ロス デサロラドールの形式
ワイヤー canonico キャプチャ `docs/account_structure.md`。エルレイアウトシエンプレス
`header · selector · controller`、ヘッダー息子のビットが失われています:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (ビット 7 ～ 5) ほら。バロレス ノー セロ エスタン リザーバドス y デベン
  ランザール `AccountAddressError::InvalidHeaderVersion`。
- `addr_class` は、シンプル (`0`) とマルチシグ (`1`) を区別します。
- `norm_version = 1` セレクター標準 v1 のコード。規範フューチュラス
  再利用可能なエルミスモカンポデ2ビット。
- `ext_flag` は `0` を指します。ペイロードのビット アクティビティ インディカン拡張子なし
  ソポルタダ。

セレクターの中間ヘッダー:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

セレクターに関する最新の UI と SDK のスター リスト:

- `0x00` = 暗黙的な欠陥 (罪ペイロード)。
- `0x01` = ローカルダイジェスト (12 バイト `blake2s_mac("SORA-LOCAL-K:v1", label)`)。
- `0x02` = グローバル登録登録 (`registry_id:u32` ビッグエンディアン)。

Ejemplos hex canonicos que las herramientas de billetera pueden enlazar o
ドキュメント/テストの挿入:

|ティポデセレクター |ヘックスカノニコ |
|---------------|---------------|
|暗黙的に欠陥がある | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|ローカルのダイジェスト (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
|レジストロ グローバル (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

`docs/source/references/address_norm_v1.md` タブラ完全版を参照してください
selector/estado y `docs/account_structure.md` バイトのパラレル図が完成しました。

## Forzar formas canonicas

Los operadores que convierten codificaciones ローカル ヘレダダ IH58 canonico
ADDR-5 の CLI ドキュメントに含まれるカデナ:

1. `iroha tools address inspect` アホラは、IH58 を使用して JSON 構造を再開します。
   comprimido y ペイロード 16 進数の正規版。オブジェクトを含む再開
   `domain` コン カンポス `kind`/`warning` y refleja cualquier dominio proporcionado
   エル・カンポ`input_domain`経由。 Cuando `kind` es `local12`、EL CLI インプライム
   JSON を参照して標準エラーと履歴書を表示する
   CI および SDK のパイプラインは最も重要です。パサ `legacy  suffix` クアンド
   `<ih58>@<domain>` を参照してください。
2. Los SDKs pueden mostrar la missma advertencia/resumen via el helper de
   JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  エル ヘルパー コンセルバ エル プレフィホ IH58 検出器リテラル メノス ケ
  明示的な割合 `networkPrefix`、履歴書を参照してください
  デフォルトはありません。再レンダリングは行われません。事前に設定が必要です。
  欠陥品。

3. Convierte el payload canonico reutilizando los Campos `ih58.value` o
   `compressed` は再開します (`--format` を介して文書の提出を求めます)。エスタス
   カデナスやソンセグラスパラコンパルティルエクステルナメンテ。
4. クライアントの実際のマニフェスト、レジストリ、ドキュメント
   正規の形式と通知を対照的にローカルのセレクターに表示します
   完全なカットオーバーを再現します。
5. パラコンフントスデダトスマシボス、エジェクタ
   `iroha tools address audit --input addresses.txt --network-prefix 753`。エルコマンド
   リー リテラル セパラドス ポル ヌエバ リネア (コメント クエリ エンピエザン コン `#` se)
   ignoran、y `--input -` o ningun flag usa STDIN)、JSON con を報告します
   履歴書 canonicos/IH58/comprimidos para cada entrada, y cuenta errores de
   ローカルの広告を解析します。米国 `--allow-errors` アル監査ダンプ
   コンティネン フィラス バスラ、自動化されたブロックへのアクセス
   `strict CI post-check` cuando los operadores esten listos para bloquear
   CI でローカルを選択します。
6.クアンド・ネセシテ・ウナ・レスクリトゥーラ・リネア・ア・リネア、アメリカ
  選択の修正を計算するパラパラ、米国ローカル
  CSV `input,status,format,...` の文書をエクスポートする
  canonicas、advertencias、fallos de parse en una sola pasada。
   ヘルパーはフィラスを省略し、ローカルの欠陥を確認し、再スタンテを確認します
   コードの承認 (IH58/comprimido/hex/JSON)、および管理権の保持
   オリジナルクアンドSE米国`legacy  suffix`。 Combinalo コン `--allow-errors` パラ
   Seguir escaneando incluso cuando un dump contiee literales mal formados を選択してください。
7. CI/lint puede エジェクターの自動化 `ci/check_address_normalize.sh`、
   ローカルの `fixtures/account/address_vectors.json` の余分なセレクター、
   los convierte via `iroha tools address normalize`、y vuelve a ejecutar
   `iroha tools address audit` デモストラル リリースはありません
   エミテンダイジェストローカル。

`torii_address_local8_total{endpoint}` ジュントコン
`torii_address_collision_total{endpoint,kind="local12_digest"}`、
`torii_address_collision_domain_total{endpoint,domain}`、y el tablero Grafana
`dashboards/grafana/address_ingest.json` 絶頂に比例:
cuando los ダッシュボードを製造するムエストランの cero envios ローカル合法性と cero
衝突 Local-12 デュランテ 30 回連続、Torii カンビアラ エル ゲート Local-8
メインネットの継続的な降下、Local-12 cuando los dominios globales の安全性
登録特派員の登録。 CLI のサリダを検討してください
コモ・エル・アビソ・パラ・オペラドール・デ・エステ・コンジェラミエント: ラ・ミスマ・カデナ・デ
SDK および自動化機能の管理ツールチップを米国に広告表示します
サリダのロードマップの基準。 Torii アホラ米国の欠陥
クアンド診断回帰。シグ・レフレジャンド
`torii_address_domain_total{domain_kind}` と Grafana
(`dashboards/grafana/address_ingest.json`) 証拠のパラレル
ADDR-7 要求 `domain_kind="local12"` 永久保証
ベンタナ リクエスト 30 ディアス アンテス デ ケ メインネット デシャビリテ ロス セレクター
(`dashboards/alerts/address_ingest_rules.yml`) アグレガ トレス ガードレール:

- `AddressLocal8Resurgence` コンテキストレポートとインクリメントページの作成
  Local-8のフレスコ画。 SDK の責任者として、モード制限のロールアウトを決定する
  ダッシュボードの表示、必要な設定、一時的な設定
  el デフォルト (`true`)。
- `AddressLocal12Collision` ローカル 12 ヘイセン ハッシュを参照してください
  アルミスモダイジェスト。マニフェストのプロモシオネス、ツールキットの取り出しの停止
  ローカル -> グローバル パラ監査エルマペオのダイジェストと政府との調整
  Nexus 登録または再アクティブ化ロールアウトを再実行する前に
  アバホ。
- `AddressInvalidRatioSlo` 無効な金額の割合を確認してください
  flota (ローカル 8/ストリクト モードを除く) SLO を 0.1% の期間超過
  ディエス・ミヌート。米国 `torii_address_invalid_total` 識別番号
  contexto/razon が責任を持ってコーディネートを調整し、SDK が所有する準備を整えます。
  再活性化エルモード制限。

### ランサミエントのフラグメント (ビルテラと探検家)

ランサミエントのメモ/エクスプローラードールの弾丸を含む
公開されたカットオーバー:> **指示:** ヘルパー `iroha tools address normalize` を選択してください
> CI (`ci/check_address_normalize.sh`) パイプラインとの接続を確立
> billetera/explorador puedan Convertir selectores 形式的なローカル遺伝情報
> canonicas IH58/comprimidas antes de que Local-8/Local-12 se bloqueen en mainnet。
> 緊急にコマンドを実行して個人情報をエクスポートする
> 証拠のリリースを正常にリストするための追加機能。