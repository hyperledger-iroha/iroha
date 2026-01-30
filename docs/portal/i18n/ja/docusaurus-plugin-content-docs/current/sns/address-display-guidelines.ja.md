---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8216f51c60ae61528c95c43ccfcd609e7dc79ad921a2ce08a81076a519441db2
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: address-display-guidelines
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note 正規ソース
このページは `docs/source/sns/address_display_guidelines.md` を反映しており、
現在はポータルの正規コピーとして扱われます。翻訳PRのためにソース
ファイルは維持されます。
:::

ウォレット、エクスプローラ、SDKサンプルはアカウントアドレスを不変
ペイロードとして扱う必要があります。`examples/android/retail-wallet` に
あるAndroidの小売ウォレットサンプルは、必要なUXパターンを示します:

- **二つのコピー先。** 明示的なコピー按钮を二つ用意します: IH58
  (推奨) とSora専用の圧縮形式(`sora...`、次善)。IH58は常に外部共有に安全で、
  QRのペイロードになります。圧縮形式はSora対応アプリ内でしか動かない
  ため、インライン警告を必ず付けます。AndroidサンプルはMaterialの
  両ボタンとツールチップを
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml` に
  接続し、iOS SwiftUIのデモも `examples/ios/NoritoDemo/Sources/ContentView.swift`
  内の `AddressPreviewCard` で同じUXを再現しています。
- **モノスペースと選択可能テキスト。** 両方の文字列をモノスペースで
  描画し、`textIsSelectable="true"` を付けてIMEを呼び出さずに値を検査
  できるようにします。編集可能フィールドは避けてください: IMEは
  kanaを書き換えたりゼロ幅コードポイントを注入する可能性があります。
- **暗黙デフォルトドメインのヒント。** セレクタが暗黙の `default`
  ドメインを指す場合、サフィックス不要であることを示すキャプション
  を表示します。エクスプローラは、セレクタがdigestをエンコードして
  いるときに正規ドメインラベルも強調してください。
- **IH58 QRペイロード。** QRコードはIH58文字列をエンコードする必要が
  あります。QR生成が失敗した場合は、空白画像ではなく明示的なエラーを
  表示します。
- **クリップボード通知。** 圧縮形式をコピーした後、Sora専用でIMEに
  よる破損に弱いことを伝えるtoastまたはsnackbarを表示します。

これらのガードレールに従うことでUnicode/IMEの破損を防ぎ、ウォレット/
エクスプローラUXに対するADDR-6の受け入れ基準を満たします。

## スクリーンショット参照

ローカライズレビューで、ボタンラベル、ツールチップ、警告が各
プラットフォームで揃っていることを確認するために次の参照を
使用してください:

- Android参照: `/img/sns/address_copy_android.svg`

  ![Android二重コピー参照](/img/sns/address_copy_android.svg)

- iOS参照: `/img/sns/address_copy_ios.svg`

  ![iOS二重コピー参照](/img/sns/address_copy_ios.svg)

## SDKヘルパー

各SDKはIH58と圧縮形式に加え、警告文字列を返す便利ヘルパーを公開して
おり、UI層で一貫性を保てます:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` は圧縮警告文字列を返し、
  呼び出し側が `sora...` リテラルを渡した場合に `warnings` に追加します。
  これにより、エクスプローラ/ウォレットダッシュボードは貼り付けや
  検証フローでSora専用の注意を表示でき、圧縮形式を自前生成する時だけ
  の警告にしません。
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

UI層でエンコードロジックを再実装せず、これらのヘルパーを使って
ください。JavaScriptヘルパーは `domainSummary` の `selector` ペイロード
(`tag`, `digest_hex`, `registry_id`, `label`) も公開し、UIsが生のペイロード
を再解析せずに、セレクタがLocal-12かレジストリ由来かを示せます。

## エクスプローラ計測デモ

<ExplorerAddressCard />

エクスプローラはウォレットのテレメトリとアクセシビリティ実装を
反映すべきです:

- コピー按钮に `data-copy-mode="ih58|compressed|qr"` を付け、フロントエンド
  がToriiの `torii_address_format_total` と並行して使用カウンタを出せる
  ようにします。上のデモコンポーネントは `{mode,timestamp}` 付きの
  `iroha:address-copy` イベントを発火します。これを分析/テレメトリ
  パイプライン(例: SegmentまたはNORITOベースの収集器)に接続して、
  サーバ側のアドレス形式利用とクライアントのコピー方式を相関させます。
  Toriiのドメインカウンタ(`torii_address_domain_total{domain_kind}`)も
  同じフィードに反映し、Local-12廃止レビューが `address_ingest` Grafana
  ボードから `domain_kind="local12"` の30日証跡を出力できるようにします。
- 各コントロールに個別の `aria-label`/`aria-describedby` を付け、共有に
  安全か(IH58) Sora専用か(圧縮)を説明します。暗黙ドメインのキャプションを
  説明に含め、支援技術が視覚と同じ文脈を提示するようにします。
- `<output aria-live="polite">...</output>` のようなライブリージョンを
  用意し、コピー結果と警告を通知します。Swift/Androidサンプルに実装済み
  のVoiceOver/TalkBack挙動と揃えます。

この計測は、Localセレクタが無効化される前にToriiの受信とクライアントの
コピー方式の両方を観測できることを示し、ADDR-6bを満たします。

## Local -> Global 移行ツールキット

[Local -> Global toolkit](local-to-global-toolkit.md) を使って、レガシーな
Localセレクタの監査と変換を自動化します。ヘルパーはJSON監査レポートと
変換済みIH58/圧縮リストの両方を出力し、オペレータはそれをreadiness
チケットに添付します。付属のrunbookは、strictモードのcutoverをゲート
するGrafanaダッシュボードとAlertmanagerルールにリンクします。

## バイナリレイアウト簡易リファレンス (ADDR-1a)

SDKが高度なアドレスツール(インスペクタ、検証ヒント、manifestビルダー)
を公開する場合、`docs/account_structure.md` にある正規ワイヤ形式を
参照するよう案内してください。レイアウトは常に
`header · selector · controller` で、headerのビットは以下です:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) が現行で、非ゼロ値は予約され
  `AccountAddressError::InvalidHeaderVersion` を返さなければなりません。
- `addr_class` は単一(`0`)とマルチシグ(`1`)を区別します。
- `norm_version = 1` はNorm v1のセレクタルールをエンコードします。将来の
  Normも同じ2ビットフィールドを再利用します。
- `ext_flag` は常に `0`。立ったビットは未対応のペイロード拡張を意味します。

セレクタはheaderの直後に続きます:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIとSDKはセレクタ種別の表示に備えるべきです:

- `0x00` = 暗黙のデフォルトドメイン(ペイロードなし)。
- `0x01` = ローカルdigest (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = グローバルレジストリエントリ(`registry_id:u32` big-endian)。

ウォレットツールがdocs/testsにリンクや埋め込みできる正規hex例:

| セレクタ種別 | 正規hex |
|---------------|---------------|
| 暗黙のデフォルト | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| ローカルdigest (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| グローバルレジストリ (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

完全なセレクタ/状態表は `docs/source/references/address_norm_v1.md`、
完全なバイト図は `docs/account_structure.md` を参照してください。

## 正規形式の強制

レガシーなLocalエンコードを正規IH58または圧縮文字列に変換する
オペレータは、ADDR-5で文書化されたCLIフローに従ってください:

1. `iroha tools address inspect` はIH58、圧縮、正規hexペイロードを含む構造化
   JSONサマリを出力します。サマリには `kind`/`warning` を持つ `domain`
   オブジェクトが含まれ、`input_domain` を通じて提供されたドメインも
   反映されます。`kind` が `local12` の場合、CLIはstderrに警告を出し、
   JSONサマリも同じ注意を反映してCIパイプラインやSDKが表示できるよう
   にします。変換されたエンコードを `<ih58>@<domain>` として再現したい
   場合は `--append-domain` を付けます。
2. SDKはJavaScriptヘルパーで同じ警告/サマリを表示できます:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  ヘルパーはリテラルから検出したIH58プレフィックスを保持し、
  `networkPrefix` を明示的に与えない限り、非デフォルトネットワークの
  サマリを既定プレフィックスで再描画しません。

3. 正規ペイロードはサマリの `ih58.value` または `compressed` を再利用
   して変換します(または `--format` で別エンコードを要求)。これらの
   文字列は外部共有に安全です。
4. マニフェスト、レジストリ、顧客向け文書を正規形式で更新し、
   cutover完了後にLocalセレクタが拒否されることを関係者へ通知します。
5. 大量データセットでは
   `iroha tools address audit --input addresses.txt --network-prefix 753` を実行します。
   コマンドは改行区切りリテラル(先頭が `#` のコメントは無視、`--input -`
   またはフラグ無しでSTDINを使用)を読み、各エントリの正規/IH58/圧縮
   サマリを含むJSONレポートを生成し、パースエラーとLocalドメイン警告を
   カウントします。ゴミ行を含むレガシーdumpを監査する場合は
   `--allow-errors` を使い、CIでLocalセレクタをブロックできる段階になったら
   `--fail-on-warning` で自動化をゲートします。
6. 行単位の書き換えが必要な場合は
  を使用します。Localセレクタの是正スプレッドシートには
  を使い、`input,status,format,...` CSVを出力して正規エンコード、警告、
  パース失敗を一度に可視化します。ヘルパーはデフォルトで非Local行を
  スキップし、残りのエントリを要求エンコード(IH58/圧縮/hex/JSON)へ
  変換し、`--append-domain` 指定時は元のドメインを保持します。
  `--allow-errors` と併用して、不正リテラルを含むdumpでもスキャンを
  続行してください。
7. CI/lint自動化は `ci/check_address_normalize.sh` を実行できます。これは
   `fixtures/account/address_vectors.json` からLocalセレクタを抽出し、
   `iroha tools address normalize` で変換し、
   `iroha tools address audit --fail-on-warning` を再実行して、リリースがLocal
   digestを出さなくなったことを証明します。

`torii_address_local8_total{endpoint}` に加えて
`torii_address_collision_total{endpoint,kind="local12_digest"}`、
`torii_address_collision_domain_total{endpoint,domain}`、およびGrafanaの
`dashboards/grafana/address_ingest.json` は強制の信号となります。プロダクション
ダッシュボードが正当なLocal送信ゼロとLocal-12衝突ゼロを30日連続で示したら、
ToriiはLocal-8ゲートをmainnetでhard-failに切り替え、その後、グローバル
ドメインに対応するレジストリエントリが揃った段階でLocal-12も切り替えます。
この凍結に関する運用向け通知はCLI出力と見なしてください。同じ警告文言が
SDKツールチップと自動化に使われ、ロードマップの退出基準と整合します。
dev/testクラスタで回帰を診断する場合のみ `false` に上書きしてください。
`torii_address_domain_total{domain_kind}` をGrafana
(`dashboards/grafana/address_ingest.json`) に反映し続け、ADDR-7の証跡パックが
`domain_kind="local12"` が30日間ゼロだったことを証明できるようにします。
Alertmanagerパック(`dashboards/alerts/address_ingest_rules.yml`)は3つの
ガードレールを追加します:

- `AddressLocal8Resurgence` は、コンテキストが新しいLocal-8増分を報告すると
  ページします。strictモードのロールアウトを止め、ダッシュボードで該当SDK
  信号がゼロに戻るまで待ち、デフォルト(`true`)を復元します。
- `AddressLocal12Collision` は、2つのLocal-12ラベルが同じdigestにハッシュ
  されたときに発火します。manifestプロモーションを停止し、Local -> Global
  toolkitでdigestマッピングを監査し、Nexusガバナンスと調整してから
  レジストリエントリを再発行するか、下流のロールアウトを再有効化します。
- `AddressInvalidRatioSlo` は、フリート全体の無効率(Local-8/strict-mode拒否を
  除外)が10分間0.1% SLOを超えたときに警告します。
  `torii_address_invalid_total` を使って原因コンテキスト/理由を特定し、
  所有SDKチームと調整してからstrictモードを再有効化します。

### リリースノート断片 (ウォレット & エクスプローラ)

cutover時のウォレット/エクスプローラリリースノートに次の箇条書きを
含めてください:

> **Addresses:** `iroha tools address normalize --only-local --append-domain` ヘルパーを
> 追加し、CI (`ci/check_address_normalize.sh`) に接続しました。これにより、
> ウォレット/エクスプローラのパイプラインがLocal-8/Local-12がmainnetで
> ブロックされる前に、レガシーLocalセレクタを正規IH58/圧縮形式へ
> 変換できます。任意のカスタムエクスポートはこのコマンドを実行し、
> 正規化リストをリリース証跡バンドルに添付してください。
