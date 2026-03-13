---
lang: ja
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 決済-iso-マッピング
title: 決済 ↔ ISO 20022 フィールドマッピング
サイドバーラベル: 和解 ↔ ISO 20022
説明: Iroha 決済フローと ISO 20022 ブリッジ間の正規マッピング。
---

:::note 正規ソース
:::

## 決済 ↔ ISO 20022 フィールドマッピング

このメモは、Iroha 決済命令間の正規マッピングをキャプチャします。
（`DvpIsi`、`PvpIsi`、リポ担保フロー）および実行される ISO 20022 メッセージ
橋のそばで。で実装されたメッセージ スキャフォールディングを反映しています。
`crates/ivm/src/iso20022.rs` は、作成または作成する際の参照として機能します。
Norito ペイロードを検証しています。

### 参照データ ポリシー (識別子と検証)

このポリシーには、識別子の設定、検証ルール、および参照データがパッケージ化されています。
Norito ↔ ISO 20022 ブリッジがメッセージを送信する前に強制しなければならない義務。

**ISO メッセージ内のアンカー ポイント:**
- **機器識別子** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (または同等の機器フィールド)。
- **当事者/代理人** → `DlvrgSttlmPties/Pty` および `RcvgSttlmPties/Pty` (`sese.*` の場合)、
  または、`pacs.009` のエージェント構造。
- **口座** → 保管/現金口座用の `…/Acct` 要素。台帳上のミラーリング
  `SupplementaryData`の`AccountId`。
- **独自の識別子** → `…/OthrId` と `Tp/Prtry` がミラーリングされます
  `SupplementaryData`。規制されている識別子を独自の識別子に置き換えないでください。

#### メッセージ ファミリごとの識別子の優先設定

##### `sese.023` / `.024` / `.025` (有価証券決済)

- **機器 (`FinInstrmId`)**
  - 推奨: `…/ISIN` の下の **ISIN**。これは、CSD/T2S の正規の識別子です。[^anna]
  - フォールバック:
    - ISO 外部から設定された `Tp/Cd` を使用した `…/OthrId/Id` の下の **CUSIP** またはその他の NSIN
      コードリスト (例: `CUSP`);必須の場合は、発行者を `Issr` に含めます。[^iso_mdr]
    - 独自の **Norito アセット ID**: `…/OthrId/Id`、`Tp/Prtry="NORITO_ASSET_ID"`、および
      `SupplementaryData` に同じ値を記録します。
  - オプションの記述子: **CFI** (`ClssfctnTp`) および **FISN** (サポートされている場合)
    和解。[^iso_cfi][^iso_fisn]
- **当事者 (`DlvrgSttlmPties`、`RcvgSttlmPties`)**
  - 推奨: **BIC** (`AnyBIC/BICFI`、ISO 9362)。[^swift_bic]
  - フォールバック: **LEI**。メッセージのバージョンによって専用の LEI フィールドが公開されます。もし
    存在せず、明確な `Prtry` ラベルが付いた独自の ID が保持され、メタデータに BIC が含まれます。[^iso_cr]
- **決済場所/会場** → 会場の場合は **MIC**、CSD の場合は **BIC**。[^iso_mic]

##### `colr.010` / `.011` / `.012` および `colr.007` (担保管理)

- `sese.*` と同じ機器規則に従います (ISIN を推奨)。
- 当事者はデフォルトで **BIC** を使用します。 **LEI** は、スキーマが公開している場合には受け入れられます。[^swift_bic]
- 現金金額には、正しい補助単位を備えた **ISO 4217** 通貨コードを使用する必要があります。[^iso_4217]

##### `pacs.009` / `camt.054` (PvP 資金調達と明細書)

- **エージェント (`InstgAgt`、`InstdAgt`、債務者/債権者エージェント)** → **BIC** (オプション)
  許可されている場合は LEI。[^swift_bic]
- **アカウント**
  - 銀行間: **BIC** および内部口座参照によって識別します。
  - 顧客向けステートメント (`camt.054`): **IBAN** が存在する場合はそれを含めて検証します
    (長さ、国の規則、mod-97 チェックサム).[^swift_iban]
- **通貨** → **ISO 4217** 3 文字コード、小単位の四捨五入を考慮します。[^iso_4217]
- **Torii 取り込み** → `POST /v2/iso20022/pacs009` 経由で PvP 資金調達レッグを送信します。橋
  には `Purp=SECU` が必要で、参照データが設定されているときに BIC クロスウォークが強制されるようになりました。

#### 検証ルール (出力前に適用)|識別子 |検証ルール |メモ |
|-----------|------|------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` および ISO 6166 Annex C に基づく Luhn (mod-10) チェック ディジット |ブリッジ放射の前に拒否します。上流の強化を好みます。[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` および 2 つの重み付けを備えたモジュラス 10 (文字が数字にマップされる) | ISIN が利用できない場合のみ。入手したら、ANNA/CUSIP 横断歩道経由で地図を作成します。[^cusip] |
| **レイ** | Regex `^[A-Z0-9]{18}[0-9]{2}$` および mod-97 チェック ディジット (ISO 17442) |承認される前に、GLEIF の日次デルタ ファイルに対して検証します。[^gleif] |
| **ビック** |正規表現 `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` |オプションの分岐コード (最後の 3 文字)。 RA ファイルでアクティブ ステータスを確認します。[^swift_bic] |
| **マイク** | ISO 10383 RA ファイルから維持します。会場がアクティブであることを確認します (`!` 終了フラグなし)。放出前に廃止された MIC にフラグを立てます。[^iso_mic] |
| **IBAN** |国固有の長さ、大文字の英数字、mod-97 = 1 | SWIFT が管理するレジストリを使用します。構造的に無効な IBAN を拒否します。[^swift_iban] |
| **独自のアカウント/パーティ ID** | `Max35Text` (UTF-8、≤35 文字) 空白をトリミング | `GenericAccountIdentification1.Id` および `PartyIdentification135.Othr/Id` フィールドに適用されます。ブリッジ ペイロードが ISO スキーマに準拠するように、35 文字を超えるエントリを拒否します。 |
| **プロキシ アカウント識別子** | `…/Prxy/Id` の下の空でない `Max2048Text`、`…/Prxy/Tp/{Cd,Prtry}` のオプションのタイプ コード |プライマリ IBAN と一緒に保存されます。 PvP レールをミラーリングするためにプロキシ ハンドル (オプションのタイプ コード付き) を受け入れる場合でも、検証には IBAN が必要です。 |
| **CFI** | ISO 10962 分類法を使用した 6 文字のコード、大文字 |オプションのエンリッチメント。文字が機器クラスと一致することを確認してください。[^iso_cfi] |
| **FISN** |最大 35 文字、大文字の英数字と限定された句読点 |オプション。 ISO 18774 ガイダンスに従って切り詰め/正規化します。[^iso_fisn] |
| **通貨** | ISO 4217 3 文字コード、補助単位によって決定されるスケール |金額は許可された小数点以下に四捨五入する必要があります。 Norito 側で強制します。[^iso_4217] |

#### Crosswalk とデータ保守の義務- **ISIN ↔ Norito アセット ID** および **CUSIP ↔ ISIN** の横断歩道を維持します。から毎晩更新します
  ANNA/DSB フィードと CI で使用されるスナップショットのバージョン管理。[^anna_crosswalk]
- GLEIF パブリック関係ファイルから **BIC ↔ LEI** マッピングを更新して、ブリッジが
  必要に応じて両方を発行します。[^bic_lei]
- **MIC 定義**をブリッジ メタデータと一緒に保存することで、会場の検証が容易になります。
  RA ファイルが日中に変更された場合でも決定的です。[^iso_mic]
- 監査のためにブリッジ メタデータにデータの出所 (タイムスタンプ + ソース) を記録します。を永続化します。
  発行された命令と一緒にスナップショット識別子。
- ロードされた各データセットのコピーを永続化するように `iso_bridge.reference_data.cache_dir` を構成します
  来歴メタデータ (バージョン、ソース、タイムスタンプ、チェックサム) と一緒に。これにより、監査人は
  オペレーターは、上流のスナップショットが回転した後でも、履歴フィードの差分を確認できます。
- ISO クロスウォーク スナップショットは、`iroha_core::iso_bridge::reference_data` によって取り込まれます。
  `iso_bridge.reference_data` 構成ブロック (パス + リフレッシュ間隔)。ゲージ
  `iso_reference_status`、`iso_reference_age_seconds`、`iso_reference_records`、および
  `iso_reference_refresh_interval_secs` は、アラートのためにランタイムの健全性を公開します。 Torii
  ブリッジは、エージェント BIC が構成された BIC にない `pacs.008` 送信を拒否します。
  横断歩道、カウンターパーティが次の場合に決定論的な `InvalidIdentifier` エラーが発生する
  不明。【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN と ISO 4217 バインディングは同じレイヤーで適用されます: pacs.008/pacs.009 フローが適用されるようになりました
  債務者/債権者の IBAN に設定されたエイリアスがない場合、または次の場合に `InvalidIdentifier` エラーが発生します。
  決済通貨が `currency_assets` にないため、不正なブリッジが防止されます
  台帳に到達するまでの指示。 IBAN 検証は国固有にも適用されます
  ISO 7064 mod‑97 に合格する前のチェックデジットの長さと数値は構造的に無効です
  値は早期に拒否されます。【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- CLI 決済ヘルパーは同じガード レールを継承します: pass
  `--iso-reference-crosswalk <path>` と `--delivery-instrument-id` を併用して DvP を実現
  `sese.023` XML スナップショットを発行する前に、プレビューで機器 ID を検証します。【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (および CI ラッパー `ci/check_iso_reference_data.sh`) lint
  横断歩道のスナップショットと備品。このコマンドは、`--isin`、`--bic-lei`、`--mic`、および
  `--fixtures` にフラグが立てられ、実行時に `fixtures/iso_bridge/` のサンプル データセットにフォールバックします。
  引数なし。【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM ヘルパーは、実際の ISO 20022 XML エンベロープ (head.001 + `DataPDU` + `Document`) を取り込むようになりました。
  `head.001` スキーマを介してビジネス アプリケーション ヘッダーを検証するため、`BizMsgIdr`、
  `MsgDefIdr`、`CreDt`、および BIC/ClrSysMmbId エージェントは決定論的に保存されます。 XMLDSig/XAdES
  ブロックは意図的にスキップされたままになります。 

#### 規制と市場構造の考慮事項

- **T+1 決済**: 米国/カナダの株式市場は 2024 年に T+1 に移行します。 Norito を調整します
  スケジュールとそれに応じた SLA アラート。[^sec_t1][^csa_t1]
- **CSDR の罰則**: 決済規律規則により現金による罰則が適用されます。 Norito を確認してください
  メタデータは調整のためのペナルティ参照をキャプチャします。[^csdr]
- **即日決済の試験運用**: インドの規制当局は T0/T+0 決済を段階的に導入しています。保つ
  パイロットの拡大に合わせて橋のカレンダーが更新されました。[^india_t0]
- **担保バイイン/ホールド**: バイインのタイムラインとオプションのホールドに関する ESMA の更新を監視します。
  したがって、条件付き配信 (`HldInd`) は最新のガイダンスに準拠しています。[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### 配送と支払い → `sese.023`| DvP フィールド | ISO 20022 パス |メモ |
|--------------------------------------------------------|--------------------------------------------------------|------|
| `settlement_id` | `TxId` |安定したライフサイクル識別子 |
| `delivery_leg.asset_definition_id` (セキュリティ) | `SctiesLeg/FinInstrmId` |正規識別子 (ISIN、CUSIP、…) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | 10 進数の文字列。資産の精度を尊重 |
| `payment_leg.asset_definition_id` (通貨) | `CashLeg/Ccy` | ISO 通貨コード |
| `payment_leg.quantity` | `CashLeg/Amt` | 10 進数の文字列。数値仕様に従って四捨五入 |
| `delivery_leg.from` (販売者/納品側) | `DlvrgSttlmPties/Pty/Bic` |配信参加者の BIC *(アカウントの正規 ID は現在メタデータでエクスポートされています)* |
| `delivery_leg.from` アカウント識別子 | `DlvrgSttlmPties/Acct` |自由形式。 Norito メタデータには正確なアカウント ID が含まれています。
| `delivery_leg.to` (買い手/受け取り側) | `RcvgSttlmPties/Pty/Bic` |受け入れ参加者のBIC |
| `delivery_leg.to` アカウント識別子 | `RcvgSttlmPties/Acct` |自由形式。受信アカウント ID と一致します |
| `plan.order` | `Plan/ExecutionOrder` |列挙型: `DELIVERY_THEN_PAYMENT` または `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` |列挙型: `ALL_OR_NOTHING`、`COMMIT_FIRST_LEG`、`COMMIT_SECOND_LEG` |
| **メッセージの目的** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (配信) または `RECE` (受信)。提出者がどのレグを実行するかを反映します。 |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (対支払い) または `FREE` (無償)。 |
| `delivery_leg.metadata`、`payment_leg.metadata` | `SctiesLeg/Metadata`、`CashLeg/Metadata` |オプションの Norito JSON は UTF‑8 としてエンコードされます。

> **決済修飾子** – ブリッジは、決済条件コード (`SttlmTxCond`)、部分決済インジケーター (`PrtlSttlmInd`)、およびその他のオプションの修飾子が存在する場合、Norito メタデータから `sese.023/025` にコピーすることで、市場慣行を反映します。 ISO 外部コード リストで公開された列挙を強制して、宛先 CSD が値を認識できるようにします。

### 支払い対支払いの資金調達 → `pacs.009`

PvP 命令に資金を提供する現金対現金レッグは、FI 間クレジットとして発行されます。
転送。ブリッジはこれらの支払いに注釈を付けて、下流システムが認識できるようにします。
彼らは証券決済に資金を提供します。

| PvP 資金調達フィールド | ISO 20022 パス |メモ |
|------------------------------------------------|------------------------------------------------------------|------|
| `primary_leg.quantity` / {金額、通貨} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` |開始者から引き落とされる金額/通貨。 |
|カウンターパーティエージェントの識別子 | `InstgAgt`、`InstdAgt` |送受信エージェントの BIC/LEI。 |
|決済目的 | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` |証券関連の PvP 資金調達の場合は `SECU` に設定します。 |
| Norito メタデータ (アカウント ID、FX データ) | `CdtTrfTxInf/SplmtryData` |完全な AccountId、FX タイムスタンプ、実行計画のヒントを保持します。 |
|命令識別子/ライフサイクルリンク | `CdtTrfTxInf/PmtId/InstrId`、`CdtTrfTxInf/RmtInf` | Norito `settlement_id` と一致するため、キャッシュ レッグは証券側と一致します。 |

JavaScript SDK の ISO ブリッジは、デフォルトで
`pacs.009` カテゴリの目的は `SECU` になります。呼び出し側はそれを別の値でオーバーライドすることができます
証券以外の信用送金を発行する場合は有効な ISO コードですが、無効です
値は事前に拒否されます。

インフラストラクチャが明示的なセキュリティ確認を必要とする場合、ブリッジは
`sese.025` を発行し続けますが、その確認は証券レッグを反映しています
PvP の「目的」ではなく、ステータス (例: `ConfSts = ACCP`)。

### 支払い対支払いの確認 → `sese.025`| PvPフィールド | ISO 20022 パス |メモ |
|-----------------------------------------------|-------------------------------------------|------|
| `settlement_id` | `TxId` |安定したライフサイクル識別子 |
| `primary_leg.asset_definition_id` | `SttlmCcy` |プライマリ レッグの通貨コード |
| `primary_leg.quantity` | `SttlmAmt` |イニシエーターによる配信量 |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON ペイロード) |補足情報に埋め込まれたカウンター通貨コード |
| `counter_leg.quantity` | `SttlmQty` |カウンター金額 |
| `plan.order` | `Plan/ExecutionOrder` | DvP と同じ列挙型セット |
| `plan.atomicity` | `Plan/Atomicity` | DvP と同じ列挙型セット |
| `plan.atomicity` ステータス (`ConfSts`) | `ConfSts` |一致した場合は `ACCP`。ブリッジは拒否時に障害コードを発行します。
|取引相手の識別子 | JSON | `AddtlInf`現在のブリッジは、メタデータ内の完全な AccountId/BIC タプルをシリアル化します。

### レポ担保代替 → `colr.007`

|リポジトリ フィールド / コンテキスト | ISO 20022 パス |メモ |
|-------------------------------------------------|-----------------------------|------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` |レポ契約識別子 |
|担保置換Tx識別子 | `TxId` |置換ごとに生成 |
|元の担保数量 | `Substitution/OriginalAmt` |代替前に差し入れられた担保と一致します。
|元の担保通貨 | `Substitution/OriginalCcy` |通貨コード |
|代替担保数量 | `Substitution/SubstituteAmt` |交換金額 |
|代替担保通貨 | `Substitution/SubstituteCcy` |通貨コード |
|発効日 (ガバナンス・マージン・スケジュール) | `Substitution/EffectiveDt` | ISO 日付 (YYYY-MM-DD) |
|ヘアカットの分類 | `Substitution/Type` |現在、ガバナンス ポリシーに基づいて `FULL` または `PARTIAL` |
|ガバナンスの理由 / 断髪メモ | `Substitution/ReasonCd` |オプション。ガバナンスの理論的根拠を伝える |

### 資金提供と声明

| Iroha コンテキスト | ISO 20022 メッセージ |マッピングの場所 |
|---------------------------------|-------------------|----------------|
|レポ キャッシュ レッグのイグニッション / アンワインド | `pacs.009` | `IntrBkSttlmAmt`、`IntrBkSttlmCcy`、`IntrBkSttlmDt`、`InstgAgt`、`InstdAgt` DvP/PvP レッグから設定 |
|決済後の声明 | `camt.054` |支払いレッグの動きは `Ntfctn/Ntry[*]` に基づいて記録されます。ブリッジは元帳/アカウントのメタデータを `SplmtryData` に挿入します。

### 使用上の注意

* すべての金額は、Norito 数値ヘルパー (`NumericSpec`) を使用してシリアル化されます。
  資産定義全体でスケールの適合性を確保します。
* `TxId` の値は `Max35Text` です — UTF‑8 の長さが 35 文字以下であることを強制します
  ISO 20022 メッセージへのエクスポート。
* BIC は 8 文字または 11 文字の大文字の英数字である必要があります (ISO9362)。拒否する
  Norito 支払いまたは決済を実行する前にこのチェックに失敗したメタデータ
  確認。
* アカウント識別子 (AccountId / ChainId) は補足ファイルにエクスポートされます。
  メタデータを使用して、受信側の参加者がローカル台帳と照合できるようにします。
* `SupplementaryData` は正規の JSON (UTF‑8、ソートキー、JSON ネイティブ) である必要があります
  逃げる）。 SDK ヘルパーはこれを強制するため、署名、テレメトリ ハッシュ、ISO
  ペイロード アーカイブは再構築後も決定的なままです。
* 通貨金額は ISO4217 の小数桁に従います (たとえば、JPY には 0 が付きます)
  小数点、USD には 2)。ブリッジはそれに応じて Norito の数値精度をクランプします。
* CLI 決済ヘルパー (`iroha app settlement ... --atomicity ...`) は現在、
  実行計画が `Plan/ExecutionOrder` に 1:1 マップされる Norito 命令、および
  上記の `Plan/Atomicity`。
* ISO ヘルパー (`ivm::iso20022`) は上記のフィールドを検証し、拒否します
  DvP/PvP レッグが数値仕様またはカウンターパーティの相互関係に違反するメッセージ。

### SDK ビルダー ヘルパー- JavaScript SDK は `buildPacs008Message` / を公開するようになりました。
  `buildPacs009Message` (`javascript/iroha_js/src/isoBridge.js` を参照) なのでクライアント
  自動化により、構造化された決済メタデータ (BIC/LEI、IBAN、
  目的コード、補足 Norito フィールド）を決定論的な pacs XML に変換
  このガイドのマッピング ルールを再実装する必要はありません。
- どちらのヘルパーにも明示的な `creationDateTime` (タイムゾーン付き ISO-8601) が必要です。
  そのため、オペレータは代わりにワークフローから確定的なタイムスタンプをスレッドする必要があります
  SDK のデフォルトを実時間に設定すること。
- `recipes/iso_bridge_builder.mjs` は、これらのヘルパーを接続する方法を示します。
  環境変数または JSON 構成ファイルをマージし、
  生成された XML を再利用し、オプションでそれを Torii (`ISO_SUBMIT=1`) に送信します。
  ISO ブリッジ レシピと同じ待機ケイデンス。


### 参考文献

- `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) および `Pmt` を示す LuxCSD / Clearstream ISO 20022 和解例(`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- 決済修飾子 (`SttlmTxCond`、`PrtlSttlmInd`) をカバーする Clearstream DCP 仕様。<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- SWIFT PMPG ガイダンスでは、証券関連の PvP 資金調達に `pacs.009` と `CtgyPurp/Cd = SECU` を推奨しています。<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- 識別子の長さの制約に関する ISO 20022 メッセージ定義レポート（BIC、Max35Text）。<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ISIN 形式とチェックサム ルールに関する ANNA DSB ガイダンス。<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### 使用上のヒント

- LLM が検査できるように、関連する Norito スニペットまたは CLI コマンドを常に貼り付けてください。
  正確なフィールド名と数値スケール。
- 文書の証跡を残すために引用をリクエスト (`provide clause references`)
  コンプライアンスと監査人のレビュー。
- `docs/source/finance/settlement_iso_mapping.md` で回答の概要をキャプチャします。
  (またはリンクされた付録) なので、将来のエンジニアはクエリを繰り返す必要がありません。

## イベント順序付けハンドブック (ISO 20022 ↔ Norito ブリッジ)

### シナリオ A — 担保代替 (レポ/プレッジ)

**参加者:** 担保提供者/受取人 (および/または代理人)、カストディアン、CSD/T2S  
**タイミング:** 市場カットオフおよび T2S 昼夜サイクルごと。 2 つのレッグを調整して、同じ決済ウィンドウ内で完了するようにします。

#### メッセージ振り付け
1. `colr.010` 担保代替要求 → 担保の提供者/受取人または代理人。  
2. `colr.011` 担保代替応答 → 受諾/拒否 (オプションの拒否理由)。  
3. `colr.012` 担保代替確認 → 代替契約を確認します。  
4. `sese.023` 命令 (2 つの脚):  
   - 元の担保 (`SctiesMvmntTp=DELI`、`Pmt=FREE`、`SctiesTxTp=COLO`) を返却します。  
   - 代替担保（`SctiesMvmntTp=RECE`、`Pmt=FREE`、`SctiesTxTp=COLI`）の提供。  
   ペアをリンクします (下記を参照)。  
5. `sese.024` ステータス アドバイス (受け入れ、一致、保留、失敗、拒否)。  
6. 予約後の `sese.025` 確認。  
7. オプションのキャッシュデルタ (手数料/ヘアカット) → `pacs.009` `CtgyPurp/Cd = SECU` による FI 間クレジット転送。ステータスは `pacs.002` 経由で、`pacs.004` 経由で返されます。

#### 必須の確認応答/ステータス
- トランスポート レベル: ゲートウェイは `admi.007` を発行するか、ビジネス処理の前に拒否する場合があります。  
- 決済ライフサイクル: `sese.024` (処理ステータス + 理由コード)、`sese.025` (最終)。  
- 現金側: `pacs.002` (`PDNG`、`ACSC`、`RJCT` など)、返品用 `pacs.004`。

#### 条件 / アンワインド フィールド
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) は 2 つの命令をチェーンします。  
- `SttlmParams/HldInd` は基準が満たされるまで保持されます。 `sese.030` (`sese.031` ステータス) 経由でリリースします。  
- 部分決済を制御する `SttlmParams/PrtlSttlmInd` (`NPAR`、`PART`、`PARC`、`PARQ`)。  
- 市場固有の条件の場合は `SttlmParams/SttlmTxCond/Cd` (`NOMC` など)。  
- オプションの T2S 条件付き証券配信 (CoSD) ルール (サポートされている場合)。

#### 参考文献
- SWIFT 担保管理 MDR (`colr.010/011/012`)。  
- リンクとステータスに関する CSD/T2S 使用ガイド (DNB、ECB Insights など)。  
- SMPG 決済実務、Clearstream DCP マニュアル、ASX ISO ワークショップ。

### シナリオ B — FX ウィンドウ違反 (PvP 資金調達の失敗)

**参加者:** カウンターパーティおよび現金代理店、証券カストディアン、CSD/T2S  
**タイミング:** FX PvP ウィンドウ (CLS/双方向) および CSD カットオフ。現金が確認されるまで証券レッグを保留しておきます。#### メッセージ振り付け
1. `pacs.009` `CtgyPurp/Cd = SECU` を使用した通貨ごとの FI 間の信用送金。 `pacs.002` 経由のステータス。 `camt.056`/`camt.029` 経由でリコール/キャンセル。すでに決済されている場合は、`pacs.004` が返されます。  
2. `sese.023` `HldInd=true` を使用した DvP 命令により、証券レッグは現金の確認を待ちます。  
3. ライフサイクル `sese.024` 通知 (承認/一致/保留中)。  
4. ウィンドウが期限切れになる前に、両方の `pacs.009` レッグが `ACSC` に達した場合 → `sese.030` で解放 → `sese.031` (mod ステータス) → `sese.025` (確認)。  
5. FX ウィンドウが違反された場合 → 現金をキャンセル/リコール (`camt.056/029` または `pacs.004`)、証券をキャンセルします (`sese.020` + `sese.027`、または市場ルールに従って既に確認されている場合は `sese.026` リバーサル)。

#### 必須の確認応答/ステータス
- 現金: `pacs.002` (`PDNG`、`ACSC`、`RJCT`)、返品の場合は `pacs.004`。  
- 証券: `sese.024` (`NORE`、`ADEA` などの保留/失敗の理由)、`sese.025`。  
- トランスポート: `admi.007` / ビジネス処理の前にゲートウェイが拒否されます。

#### 条件 / アンワインド フィールド
- `SttlmParams/HldInd` + `sese.030` 成功/失敗時のリリース/キャンセル。  
- `Lnkgs` は、有価証券の指示をキャッシュ レッグに結び付けます。  
- 条件付き配信を使用する場合の T2S CoSD ルール。  
- 意図しないパーシャルを防止する `PrtlSttlmInd`。  
- `pacs.009` では、`CtgyPurp/Cd = SECU` は証券関連の資金調達にフラグを立てます。

#### 参考文献
- 証券プロセスにおける支払いに関する PMPG / CBPR+ ガイダンス。  
- SMPG 決済の実践、リンク/ホールドに関する T2S の洞察。  
- Clearstream DCP マニュアル、メンテナンス メッセージに関する ECMS ドキュメント。

### pacs.004 リターン マッピング ノート

- 返品フィクスチャは、`ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) と `TxInf[*]/RtrdRsn/Prtry` として公開される独自の返品理由を正規化するようになりました。そのため、ブリッジの消費者は手数料の帰属コードとオペレータ コードを問題なく再生できます。 XML エンベロープを再解析します。
- `DataPDU` エンベロープ内の AppHdr 署名ブロックは取り込み時に無視されたままになります。監査は、埋め込まれた XMLDSIG フィールドではなく、チャネルの来歴に依存する必要があります。

### 橋の運用チェックリスト
- 上記のコレオグラフィーを強制します (担保: `colr.010/011/012 → sese.023/024/025`、FX 侵害: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`)。  
- `sese.024`/`sese.025` ステータスと `pacs.002` 結果をゲート信号として扱います。 `ACSC` はリリースをトリガーし、`RJCT` はアンワインドを強制します。  
- `HldInd`、`Lnkgs`、`PrtlSttlmInd`、`SttlmTxCond`、およびオプションの CoSD ルールを介して条件付き配信をエンコードします。  
- 必要に応じて、`SupplementaryData` を使用して外部 ID を関連付けます (例: `pacs.009` の UETR)。  
- 市場カレンダー/カットオフによるホールド/アンワインドのタイミングをパラメーター化します。キャンセル期限前に `sese.030`/`camt.056` を発行し、必要に応じて返品にフォールバックします。

### ISO 20022 ペイロードのサンプル (注釈付き)

#### 命令リンケージを備えた担保置換ペア (`sese.023`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

`SctiesMvmntTp=RECE`、`Pmt=FREE`、および `SUBST-2025-04-001-A` を指す `WITH` リンケージを使用して、リンクされた命令 `SUBST-2025-04-001-B` (代替担保の FoP 受信) を送信します。置換が承認されたら、一致する `sese.030` を使用して両方の脚を解放します。

#### 為替確認保留中の証券レッグ (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

両方の `pacs.009` レッグが `ACSC` に達したら解放します。

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` は保留解除を確認し、証券レッグが予約されると `sese.025` が続きます。

#### PvP 資金調達レッグ (証券目的の `pacs.009`)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` は支払いステータスを追跡します (`ACSC` = 確認済み、`RJCT` = 拒否)。ウィンドウを突破した場合は、`camt.056`/`camt.029` 経由でリコールするか、`pacs.004` を送信して決済された資金を返します。