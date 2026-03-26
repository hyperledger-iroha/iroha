---
lang: ja
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ExplorerAddressCard を '@site/src/components/ExplorerAddressCard' からインポートします。

:::note ソースカノニク
Cette ページを参照 `docs/source/sns/address_display_guidelines.md` など
メンテナンスデコピーカノニークデュポルテイル。 Le fichier ソース レスト プール レ PR
デ・トラダクション。
:::

SDK のポートフォイユ、探索者、およびアドレスの例
不変のペイロードを計算します。 L'exemple de portefeuille 小売 Android
`examples/android/retail-wallet` montre メンテナンス ファイル パターン UX には次のものが必要です:

- **コピーの二重ファイル。** Fournissez のコピーの二重明示: I105
  (優先) および形式の圧縮者 Sora のみ (`sora...`、2 番目の選択)。 I105 エスト トゥージュール
  QR のペイロードの外部および管理を確実に行います。ラ・ヴァリアント・コンプレッシー
  回避策をインラインで実行する必要があります。
  アプリはSoraの料金に応じて課金されます。 Android のボタンのマテリアルとサンプルの例
  leurs ツールチップ dans
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`など
  デモ iOS SwiftUI reflete le meme UX via `AddressPreviewCard` dans
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **等幅、テキスト選択可能** Affichez les deux chaines avec une Police
  モノスペースと `textIsSelectable="true"` の実用的な機能
  IME を呼び出す必要のない検査官。 Evitez レ シャンの編集可能ファイル: レ
  IME は、大きいゼロをコード化するインジェクターのポイントを再確認します。
- **デフォルトの暗黙的なドメインの表示。** Quand le selecteur pointe sur
  ル・ドメーヌ・インプリシテ `default`、アフィシェ・ユヌ・レジェンド・ラペラント・オー・オペレーターズ
  qu'aucun 接尾辞は必須ではありません。前衛的なオーストラリアの探検家たち
  ドメーヌ カノニクのラベル ラベルを選択し、ダイジェストをエンコードします。
- **QR I105.** QR コードはチェーン I105 のエンコーダーに対応しています。シ・ラ・ジェネレーション・ドゥ
  QR エコー、添付ファイルは、画像ビデオを明示的に表示します。
- **印刷用紙にメッセージを送信してください。** 形式的な圧縮のコピーを作成し、印刷してください。
  トースト ou スナックバー ラペラント aux utilisateurs qu'elle est Sora-only et sujette
  歪みのあるIME。

Unicode/IME などの破損を徹底的に排除し、基準を満たします
ロードマップ ADDR-6 のポルトフォイユ/探検家を受け入れます。

## 参照のキャプチャ

保証のためにローカリゼーションのレビューを参照することを利用します。
ブートンのラベル、ツールチップなどの回避策は、中央の位置に合わせて維持されます
プレートフォーム:

- 参考 Android: `/img/sns/address_copy_android.svg`

  ![参考 Android 二重コピー](/img/sns/address_copy_android.svg)

- 参照 iOS: `/img/sns/address_copy_ios.svg`

  ![参考 iOS 二重コピー](/img/sns/address_copy_ios.svg)

## ヘルパー SDK

Chaque SDK は、I105 形式のレポートを作成するのに役立つ機能を公開します
ソファの UI を圧縮して安全を確保します
コヒーレンテス:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript インスペクター: `inspectAccountId(...)` retourne la chaine
  d'avertissement compressee et l'ajoute a `warnings` quand les apelants
  fournissent un literal `sora...`、探検家/板絵を注ぐ
  portefeuille puissent afficher l'avertissement ソラ専用ペンダント les flux de
  コラージュ/検証 plutot que seulement lorsqu'ilsgenerent eux-memes la forme
  圧縮する人。
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- スイフト: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

エンコードのロジックを再実装するためのヘルパーを利用します。
ソファUI。 Le helper JavaScript がペイロード `selector` のオーストラリアを公開します
`domainSummary` (`tag`、`digest_hex`、`registry_id`、`label`) は UI を作成します
ローカル 12 を選択して登録を拒否する必要があります
リパーサー ル ペイロード ブリュット。

## 実験のデモ

<エクスプローラーアドレスカード />

遠隔測定とアクセスの再現を行う探索者
フェイト・プール・ル・ポルトフォイユ:- Appliquez `data-copy-mode="i105|qr"` aux boutons de copy afin que
  フロントエンドのコンピュータの使用法を並行して管理する
  メトリック Torii `torii_address_format_total`。 Le composant デモ ci-dessus
  特使 `iroha:address-copy` avec `{mode,timestamp}` - reliez cela
  分析/遠隔測定の投票パイプライン (例: セグメントおよび国連の使者)
  Collecteur Base sur NORITO) pour que les ダッシュボード puissent correler l'usage
  コートサーバーのアドレスをフォーマットし、コピーコートクライアントのモードをコピーします。リフレテズ
  ドメイン コンピューティング Torii (`torii_address_domain_total{domain_kind}`)
  Local-12 の puissent 輸出者は、レビュー ド リトレイトを注ぐミーム フラックスを流します。
  preuve de 30 jours `domain_kind="local12"` directement depuis le tableau
  `address_ingest` と Grafana。
- アソシエ チャック コントロールの適応症 `aria-label`/`aria-describedby`
  distines qui expliquent si un literal est sur a partager (I105) ou Sora-only
  (圧縮)。説明を含む暗黙のドメーヌの伝説を含む
  テクノロジーは、ミームのコンテキストを反映した支援を提供します。
- 地域ライブを公開 (例: `<output aria-live="polite">...</output>`) qui
  結果をコピーして回避し、整合性のあるファイルを通知します
  Swift/Android の例となる VoiceOver/TalkBack デジャ ケーブル。

Cette の計測器は、運用者が確認した ADDR-6b を満足しています
オブザーバーは取り込み Torii およびコピー クライアントのモードを事前に確認しています
selecteurs ローカルの soient 無効化。

## 移行ツールキット ローカル -> グローバル

[ツールキット ローカル -> グローバル](local-to-global-toolkit.md) を利用します
自動監査と選択ツールの変換 地元の遺伝。ル・ヘルパー
JSON および変換リスト I105/compressee que を取得します。
準備を整える補助チケットを操作するための準備、ランブック関連の手続きを行う
ダッシュボード Grafana と Alertmanager の規則が表示されます
カットオーバーモードは厳密です。

## 高速レイアウト バイネアのリファレンス (ADDR-1a)

SDK が事前に公開される (検査官、指示)
検証、マニフェストの構築)、開発とフォーマットのポイント
ワイヤー カノニク キャプチャ ダン `docs/account_structure.md`。ル レイアウト エスト トゥージュール
`header · selector · controller`、ヘッダー ゾーンのビット:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (ビット 7 ～ 5) オージュールフイ;レ・ヴァルール・ノン・ゼロ・ソン・リザーブ
  et doivent レバー `AccountAddressError::InvalidHeaderVersion`。
- `addr_class` は、シンプルなコントローラー (`0`) とマルチシグ (`1`) を区別します。
- `norm_version = 1` は、選択規則 v1 をエンコードします。レ・ノルム先物
  ミームチャンピオン 2 ビットを再利用します。
- `ext_flag` ヴォー・トゥージュール `0`;拡張機能に関連する行為の詳細
  ペイロードは有料です。

選択したスーツの即時ヘッダー:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI と SDK の種類の選択方法は次のとおりです。

- `0x00` = デフォルトの暗黙的なドメイン (ペイロードなし)。
- `0x01` = ローカルダイジェスト (12 バイト `blake2s_mac("SORA-LOCAL-K:v1", label)`)。
- `0x02` = グローバル登録エントリ (`registry_id:u32` ビッグエンディアン)。

例 16 進法規則のポルトフォイユ プヴァン リエ ou インテグレーター
補助ドキュメント/テスト:

|タイプ選択者 |ヘックスカノニク |
|---------------|---------------|
|暗黙的なデフォルト | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|ローカルのダイジェスト (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
|グローバル登録 (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` テーブル完成
selecteur/etat et `docs/account_structure.md` 完成図
バイト。

## 正典の形式をインポーザー

I105 正規の地域遺産を操作する方法
ADDR-5 を使用して、ワークフロー CLI ドキュメントを圧縮する必要があります。

1. `iroha tools address inspect` メンテナンスを再開して JSON 構造 avec I105、
   圧縮とペイロードの 16 進正規化。オーストラリアのオブジェクトを含む履歴書
   `domain` avec les Champs `kind`/`warning` et reflete tout ドメーヌ フルニ 経由
   ル・チャンプ `input_domain`。 Quand `kind` vaut `local12`、CLI インプリミアン
   標準エラーの防止と再開 JSON を参照してミームを送信する
   パイプライン CI と SDK の添付ファイル。パセズ `legacy  suffix`
   lorsque vous voulez rejouer l'encodage Converti sous la form `<i105>@<domain>`。
2. ファイルヘルパー経由で SDK の添付ファイルのミーム回避/再開
   JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  Le helper le prefixe I105 detecte depuis le literal sauf si vous を保持する
  fournissez 明示的 `networkPrefix`、ドンク・レ・レジューム・プール・デ・レゾー
  デフォルトではなく、デフォルトのプレフィックスを再実行することはできません。

3. Convertissez le payload canonique en reutilisant les Champs `i105.value` ou
   `i105` 再開します (`--format` 経由で再開を要求します)。セス
   鎖は、外部との対話を確実にします。
4. 時効のマニフェスト、登録文書、クライアントの安全性を確認する
   canonique et notifiez les contreparties que les selecteurs ローカル セロントを形成する
   カットオーバー終了を拒否します。
5. 一斉に注ぐ、実行する
   `iroha tools address audit --input addresses.txt --network-prefix 753`。ラ・コマンド
   lit des literaux separes par nouvelle ligne (les commentaires comencant par)
   `#` は無視します、et `--input -` ou aucun フラグは STDIN を使用します)、信頼関係は確立されません
   JSON avec desresumes canoniques/I105/compresse pour Chaque entree, et compte
   ドメイン ローカルの解析に関するエラー。利用する
   `--allow-errors` 遺産内容の監査のダンプ
   寄生虫、およびブロックによる自動化 (`strict CI post-check` lorsque les 経由)
   オペレーターは、ローカルと CI のブロックを選択します。
6. Quand vous avez besoin d'une reecriture ligne a ligne、utilisez
  選択ツールの修復計算に使用するフィーユを注ぎます。ローカルで使用します。
  CSV `input,status,format,...` を使用してエクスポータを前もって実行する
  canoniques、avertissements、および echecs de parse en une seule passe。
   ヘルパーはローカル パー デフォルトを無視し、メインディッシュを変換します
   エンコード要求 (I105/compresse/hex/JSON) を保存し、ファイルを保存します
   ドメーヌ オリジナル クアンド `legacy  suffix` EST アクティブ。アソシエ・ル・ア
   `--allow-errors` 引き続きミームを分析し、コンテンツをダンプします
   literaux の奇形。
7.自動化 CI/lint 実行プログラム `ci/check_address_normalize.sh`、qui
   ローカルの `fixtures/account/address_vectors.json`、ファイルの追加
   `iroha tools address normalize` 経由で変換し、その後、
   `iroha tools address audit` プルーバー ケ レ リリースを注ぐ
   ローカルの項目とダイジェストを追加します。

`torii_address_local8_total{endpoint}`プラス
`torii_address_collision_total{endpoint,kind="local12_digest"}`、
`torii_address_collision_domain_total{endpoint,domain}`、および表 Grafana
`dashboards/grafana/address_ingest.json` 4 つの信号のアプリケーション:
生産モントレントのダッシュボードはローカルのミッションゼロです
合法的かつ衝突ゼロ Local-12 ペンダント 30 ジュール連続、Torii
basculera la porte Local-8 en echec strict sur mainnet、Suvi par Local-12 une
世界中のメインディッシュの登録特派員です。
CLI comme l'avis オペレーターがジェルを注ぐことを検討します - la memechaine
 SDK とツールチップの自動化を最大限に活用する
ロードマップの基準を維持します。 Torii を利用する
クラスターの開発/テストを行って、回帰の診断を行います。続けてください
ミロイター `torii_address_domain_total{domain_kind}` と Grafana
(`dashboards/grafana/address_ingest.json`) ADDR-7 を注ぐ
ピュイセ モントレ ケ `domain_kind="local12"` エスト レスト ア ゼロ ペンダント ラ フェネトル
アラートマネージャー (`dashboards/alerts/address_ingest_rules.yml`) アジュート トロワ
障壁:

- `AddressLocal8Resurgence` ページは、新しい情報を通知します。
  インクリメントローカル-8。厳密なモードでのロールアウトを停止し、ラをローカル化する
  ダッシュボードなどのサーフェス SDK の機能、一時的な定義
  デフォルト (`true`)。
- `AddressLocal12Collision` se declenche lorsque deux ラベル Local-12 ハッシュ
  ミームダイジェストとの対比。マニフェストのプロモーションを一時停止し、実行します
  ツールキット ローカル -> グローバル 監査ファイル マッピング ダイジェストなどの調整
  avec la gouvernance Nexus avant de reemettre l'entre de registre ou de
  ロールアウトを再強化します。
- `AddressInvalidRatioSlo` 不正比率を回避し、レシェルを無効にします
  flotte (ローカル 8/厳密モードを除く) SLO のデパス 0.1%
  ペンダントディックス分。 `torii_address_invalid_total` の識別子ファイルを利用します
  コンテキスト/レゾン責任と調整を可能にし、SDK の所有権を前もって確保する
  厳格なモードを再強化します。

### Extrait de note de release (ポルトフォイユと探検家)

ポルトフォイユ/エクスプローラーのリリースノートを含める
カットオーバーの失敗:> **アドレス:** Ajoute le helper `iroha tools address normalize`
> CI (`ci/check_address_normalize.sh`) パイプラインの分岐
> portefeuille/explorateur puissent Convertir les selecteurs 地元の遺産対
> Local-8/Local-12 ソエントの標準 I105/compressees の形式
> メインネットのブロック。メッテズ 1 時間の輸出は、実行者に個人情報を提供します
> リリースのバンドルを正常にリストするコマンドと参加者。