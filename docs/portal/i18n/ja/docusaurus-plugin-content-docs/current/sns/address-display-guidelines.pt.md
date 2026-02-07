---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard を '@site/src/components/ExplorerAddressCard' からインポートします。

:::note フォンテ カノニカ
Esta pagina espelha `docs/source/sns/address_display_guidelines.md` e agora サーブ
コピア・カノニカ・ド・ポータル。 PR の永続的な保管場所
トラドゥカオ。
:::

カルテ、SDK 開発のエクスプロラドールとサンプルをコンタ コモで開発
ペイロードはイムタベです。小売 Android em の例
`examples/android/retail-wallet` UX でのパドラオのデモンストレーション:

- **Dois alvos de copia.** Envie dois botoes de copia Explicitos: IH58
  (preferido) e a forma comprimida somente Sora (`sora...`, segunda melhor opcao)。 IH58 エ センペル セグロ パラ
  QR のペイロードに対する外部栄養との比較。変種コンプリミダ
  Deve には、aviso インライン ポーク機能が含まれているため、アプリの互換性が提供されます。
  ソラ。小売 Android リーグの主要な製品の例 材料と用途
  ツールチップem
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`、e a
  デモ iOS SwiftUI espelha o mesmo UX via `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **等幅、テキスト選択。** ambas を com uma fonte の文字列としてレンダリングします。
  等幅電子 `textIsSelectable="true"` パラケオスユーリオスポッサム検査官
  IME を呼び出す値を設定します。 Evite Campos editaveis: IME podem reescrever kana
  インジェーター ポントス デ コディゴ デ ラルグラ ゼロ。
- **ディカス デ ドミニオ パドラオ 暗黙的。** Quando o seletor aponta para o dominio
  暗黙の `default`、最も伝説的なレンブランドのオペラドール
  サフィックスは必要です。 Exploradores タンベム デベム デスタカー、ラベル デ ドミニオ
  canonico quando または seletor codificaum ダイジェスト。
- **QR IH58.** Codigos QR は文字列 IH58 をコード化します。 QR を実行してください
  ファルハール、最も正確な画像はブランコにあります。
- **Mensageria da area de transferencia.** コピー用紙の保管庫、
  エミッタ・ウム・トースト・オ・スナックバー・レンブランド・オス・ユーズアリオス・デ・ケ・エラ・ソメンテ・ソラ・エ
  IME の変更を許可します。

Unicode/IME を使用してガードレールをセキュリティ保護し、OS 基準を守る
カルテ/エクスプローラーの UX に関するロードマップ ADDR-6 を実行します。

## リファレンスのキャプチャ

地域保証に関する最新情報を参照として使用してください
ボットの詳細、ツールチップ、プラットフォームの有効性に関する情報:

- 参照 Android: `/img/sns/address_copy_android.svg`

  ![Android デ デュプラ コピアの参照](/img/sns/address_copy_android.svg)

- 参照 iOS: `/img/sns/address_copy_ios.svg`

  ![iOS デプラコピーの参照](/img/sns/address_copy_ios.svg)

## SDK のヘルパー

Cada SDK は、IH58 形式およびコンプリミダとしての便利なヘルパーを説明します
Camadas UI の一貫性として、junto com a string de aviso para que を実行します。

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript インスペクター: `inspectAccountId(...)` 文字列を出力します
  comprimida e a anexa a `warnings` quando Chamadores fornecem um 文字通り
  `sora...`、カルテ ダッシュボード/エクスプローラー ポッサム エクスビル オ アヴィソのパラケ ダッシュボード
  ソメンテ ソラ デュランテ フラクソス デ コラージュ/バリダカオ エム ヴェズ デ アペナス Quando geram
  固有の形式。
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- スイフト: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

NAS カメラ UI をエンコードするロジックを再実装するには、エステ ヘルパーを使用します。 ○
ヘルパー JavaScript タンベム expoe um ペイロード `selector` em `domainSummary` (`tag`,
`digest_hex`、`registry_id`、`label`) セレクターに固有の UI パラメータ
Local-12 は、ペイロード ブルートのレジストリ テストの解析を行っています。

## エクスプローラーを実行するためのデモ

<エクスプローラーアドレスカード />

Exploradores devem espelhar o trabalho de telemetria と accessibilidade da
カルテイラ:- アップリケ `data-copy-mode="ih58|compressed|qr"` aos botoes de copia para que
  フロントエンド ポッサム エミミール コンタドール デ ユーソ ジュント コム ア メトリカ Torii
  `torii_address_format_total`。 O コンポーネント デモ アシマ ディスパラ ウム イベント
  `iroha:address-copy` com `{mode,timestamp}`;パイプラインを接続します
  Analitica/telemetria (例、NORITO のコレクションによるセグメントの羨望)
  ダッシュボードのポッサム相関を使用してエンデレコのフォーマットを実行
  サーバーはクライアントのモードをコピーします。レプリケ タンベム オス コンタドレス デ
  dominio Torii (`torii_address_domain_total{domain_kind}`) メスモ フィード パラメータなし
  Local-12 possam exportar uma prova de 30 dias を修正してください
  `domain_kind="local12"` は、`address_ingest` は Grafana を実行します。
- `aria-label`/`aria-describedby` の互換性を持つコンピュータ制御
  que expliquem se um literal e segro para compartilhar (IH58) ou somente Sora
  （コンプリミド）。暗黙の説明を含む伝説を含む
  技術支援は最も重要なコンテキストを視覚的に表示します。
- Exponha uma regiao viva (例、`<output aria-live="polite">...</output>`)
  コピアとアヴィソスの結果を発表し、アリンハンドとコンポルタメントを行う
  VoiceOver/TalkBack は、Swift/Android と接続されています。

Esta 計測器満足度 ADDR-6b ao provar que operadores podem observar
タントを摂取する Torii Quanto os modos de copia do cliente antes que os
seletores 地元のセジャム デサティバドス。

## ローカル -> グローバル移行ツールキット

自動化パラメータとして [toolkit Local -> Global](local-to-global-toolkit.md) を使用します。
セレトレの見直しと会話 ローカルの代替手段。おおヘルパーはタントを発して、リラトリオを
JSON の聴覚量リスト変換 IH58/comprimida que operadores
チケットの準備、ランブックの関連付け、Vincula ダッシュボードの検査
Grafana アラート マネージャーのコントロール モードの切り替えを記録しています。

## 高速レイアウト バイナリオ (ADDR-1a) を参照

Quando SDK のエクスペリエンス ツール (インスペクター、ディカス ガイド)
validacao、ビルダー デ マニフェスト)、aponte desenvolvedores para o formato Wire
カノニコエム`docs/account_structure.md`。 O レイアウト設定
`header · selector · controller`、onde os ビットはヘッダー sao を実行します。

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (ビット 7 ～ 5) ホジェ;バロレス・ナオ・ゼロ・サオ・リザーバドス・エ・デベム
  レヴァンター`AccountAddressError::InvalidHeaderVersion`。
- `addr_class` は、シンプルなコントロール (`0`) とマルチシグ (`1`) を区別します。
- `norm_version = 1` セレクター Norm v1 をレグラとして記述します。規範フューチュラス
  2 ビットの再利用。
- `ext_flag` は `0` です。ペイロード nao のインディカム拡張ビット
  サポート。

すぐにセグエを選択するヘッダー:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI と SDK は、セレクターの公開および情報の開発を開始します。

- `0x00` = 暗黙のドミニオ・パドラオ (sem ペイロード)。
- `0x01` = ローカルダイジェスト (12 バイト `blake2s_mac("SORA-LOCAL-K:v1", label)`)。
- `0x02` = グローバル登録登録 (`registry_id:u32` ビッグエンディアン)。

サンプル 16 進数のカルテ フェラメント ポデム リンカーとエンブティル エム
 ドキュメント/テスト:

|セレクターのヒント |ヘックスカノニコ |
|---------------|---------------|
|暗黙のパドラオ | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|ローカルのダイジェスト (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
|レジストロ グローバル (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` は完全なテーブルです
seletor/estado e `docs/account_structure.md` バイトの完全な図。

## 形式をインポートします

IH58 canonico ou によるローカル代替機能の操作
ADDR-5 のワークフロー CLI ドキュメントに含まれる文字列:

1. `iroha tools address inspect` アゴラは、IH58 の JSON 関数を出力し、
   comprimido とペイロードの 16 進数のカノニコス。オブジェクトを含むすべてのタスクを実行してください
   `domain` com Campos `kind`/`warning` e ecoa qualquer dominio fornecido via o
   カンポ`input_domain`。 Quando `kind` e `local12`、CLI の主要な機能
   stderr e o resumo JSON ecoa a mesma orientacao para que パイプライン CI e SDK
   ポッサム・エクシビラ。 Passe `--append-domain` semper que quiser reproduzir a
   codificacao Convertida como `<ih58>@<domain>`。
2. SDK podem exibir o mesmo aviso/resumo via o helper JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  O helper preserva o prefixo IH58 detectado 文字通りの a menos que voce forneca
  明示的 `networkPrefix`、エントリ レコード パラ レデス ナオ パドラオ ナオ サオ
  プレフィックス パドラオを再レンダリングします。3. ペイロード canonico reutilizando os Campos `ih58.value` ou を変換します
   `compressed` は resumo を実行します (`--format` 経由で外部コードを要求します)。エッサス
   外部コンパルティリハメントの文字列を使用します。
4. マニフェスト、レジストリ、ドキュメント、ボルタドス、クライアント コムの形式を認証します。
   地元のセラオレジェイタドスと対照的なものとして正規化と通知
   結論のためのカットオーバー。
5. パラ・コンフントス・デ・ダドス・エム・マッサ、実行する
   `iroha tools address audit --input addresses.txt --network-prefix 753`。おおコマンドー
   le literais separados por nova linha (commentarios iniciados com `#` sao)
   ignorados、e `--input -` ou nenhum flag usa STDIN)、relatorio JSON を出力
   com resumos canonicos/IH58/comprimidos para cada entrada e conta erros de
   ローカルのアクセスを解析します。 `--allow-errors` ao 監査ダンプの代替を使用します。
   com linhas lixo、e trave a automacao com `--fail-on-warning` quando os
   operadores estiverem prontos para bloquear seletores ローカル CI なし。
6. Quando precisar de reescrita linha a linha、使用
  セレトレの救済策を講じる ローカル、使用
  CSV `input,status,format,...` の文書をエクスポートする
  canonicas、avisos および falhas de parse em uma unica passada。
   O helper ignora linhas nao Local por Padrao, Converte cada entrada restante
   法令上の要求 (IH58/comprimido/hex/JSON) を使用して管理を保存
   オリジナルのQuando `--append-domain`と定義。コンバインコム `--allow-errors`
   不正な文学に関する大量のデータを継続的に収集します。
7. automacao CI/lint pode executar `ci/check_address_normalize.sh`、追加リクエスト
   `fixtures/account/address_vectors.json` のローカルのセレトレ、変換経由
   `iroha tools address normalize`、再実行
   `iroha tools address audit --fail-on-warning` パラプロバークが nao のエミテムをリリースします
   ローカルをダイジェストします。

`torii_address_local8_total{endpoint}` ジュントコム
`torii_address_collision_total{endpoint,kind="local12_digest"}`、
`torii_address_collision_domain_total{endpoint,domain}`、e o painel Grafana
`dashboards/grafana/address_ingest.json` 強制執行: Quando
ほとんどの製品を製造する OS ダッシュボード ゼロ環境 ローカル合法性とゼロ環境
ローカル 12 ポート 30 ディア連続、Torii バイラル ゲート ローカル 8 パラ ハードフェイル
メインネット、ローカル 12 のセグイド グローバル ネットワークの管理
登録特派員。 CLI を使用して操作を行うことを検討してください。
esse congelamento - SDK のツールチップとアビソのメスマ文字列
オートマカオ パラマンター パリダード コム OS 基準は、ロードマップを実行します。 Torii アゴラ
クラスタ開発/テストの診断回帰。エスペルハンドを続ける
`torii_address_domain_total{domain_kind}` いいえ Grafana
(`dashboards/grafana/address_ingest.json`) ADDR-7 の証拠のパラグラフ
`domain_kind="local12"` を永久にゼロにし、30 番目の要求を証明する
 メインネットの廃止措置は代替手段を選択します。オパコートアラートマネージャー
(`dashboards/alerts/address_ingest_rules.yml`) アディシオナ トレス ガードレール:

- `AddressLocal8Resurgence` ページの内容をレポートし、増分します
  Local-8 ノボ。モードエストリトのロールアウトを削除し、表面的な SDK をローカライズします
  応答はダッシュボードにありません、必要があり、一時的なものを定義します
  パドラオ (`true`)。
- `AddressLocal12Collision` dispara quando dois ラベル Local-12 fazem hash para
  ○メスモダイジェスト。マニフェストのプロモーションを一時停止し、ツールキットを実行します。ローカル -> グローバル
  監査と地図のダイジェストと政府の調整 Nexus の条件
  ダウンストリームのロールアウトを登録し、再確認します。
- `AddressInvalidRatioSlo` 次回は無効になります
  (ローカル 8/厳密モードを除く) SLO を 0.1% 毎秒超えます。
  ローカライズまたはコンテキスト/razao 応答で `torii_address_invalid_total` を使用する
  SDK の独自性を備えたコーディネートが可能です。

### Trecho de nota de release (カルテイラと探検家)

カルテ/エクスプローラーのリリースに関するメモを含める
カットオーバー:

> **Enderecos:** Adicionado o helper `iroha tools address normalize --only-local --append-domain`
> CI (`ci/check_address_normalize.sh`) パイプラインの接続がありません
> carteira/explorador possam コンバーター seletores ローカル代替形式
> canonicas IH58/comprimidas antes de Local-8/Local-12 serem bloqueados na
>メインネット。ロッドやコマンドのカスタマイズによる、Quaisquer のエクスポートの実現
> リリースのリストと証拠のバンドルを正規化します。