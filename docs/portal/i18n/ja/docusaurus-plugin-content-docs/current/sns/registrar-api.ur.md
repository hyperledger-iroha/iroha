---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب پورٹل
٩ی کینونیکل کاپی ہے۔広報活動 広報活動 広報活動 広報活動 広報活動 広報活動 広報活動
:::

# SNS レジストラー API フック (SN-2b)

**評価:** 2026 年 3 月 24 日 - Nexus コアの評価  
**重要事項:** SN-2b「レジストラー API とガバナンス フック」  
**پیشگی شرائط:** اسکیمہ تعریفیں [`registry-schema.md`](./registry-schema.md) میں۔

Torii エンドポイント、gRPC サービス、リクエスト/レスポンス DTO、ガバナンス
アーティファクト ٩ی وضاحت کرتا ہے جو Sora Name Service (SNS) レジストラ چلانے کے لئے
और देखें SDK ウォレットとオートメーションの管理、SNS の管理
جسٹر، 更新 یا 管理 کرنا چاہتے ہیں۔

## 1. ٹرانسپورٹ اور توثیق

| और देखें評価 |
|-----|------|
| پروٹوکولز | REST `/v2/sns/*` の gRPC サービス `sns.v1.Registrar`۔ Norito-JSON (`application/json`) Norito-RPC (`application/x-norito`) قبول کرتے ہیں۔ |
|認証 | `Authorization: Bearer` トークン یا mTLS 証明書 ہر サフィックス スチュワード کی طرف سے جاری۔ガバナンスに敏感なエンドポイント (凍結/凍結解除、予約済み割り当て) |
| और देखेंレジストラ `torii.preauth_scheme_limits` バケット JSON 呼び出し元 کے ساتھ شیئر کرتے ہیں اور ہر 接尾辞 کے لئے バースト キャップ: `sns.register`、`sns.renew`、 `sns.controller`、`sns.freeze`۔ |
| ٹیلیمیٹری | Torii レジストラー ハンドラー (`scheme="norito_rpc"` フィルター) `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` API サポート `sns_registrar_status_total{result, suffix_id}` サポート|

## 2. DTO 認証

[`registry-schema.md`](./registry-schema.md) 正規構造体を参照する ہیں۔ペイロード `NameSelectorV1` + `SuffixId` 処理ルーティング処理

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST エンドポイント

|エンドポイント | और देखेंペイロード |評価 |
|----------|----------|----------|----------|
| `/v2/sns/registrations` |投稿 | `RegisterNameRequestV1` | और देखें価格帯のレベル 支払い/ガバナンスの証明 レベル レジストリ イベントの出力 レベル|
| `/v2/sns/registrations/{selector}/renew` |投稿 | `RenewNameRequestV1` | دت بڑھاتا ہے۔猶予/償還ウィンドウ پالیسی سے 猶予/償還ウィンドウ نافذ کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/transfer` |投稿 | `TransferNameRequestV1` |承認 لگنے کے 所有権 منتقل کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/controllers` |置く | `UpdateControllersRequestV1` |コントローラー署名済みアカウントのアドレス کی توثیق کرتا ہے۔ |
| `/v2/sns/registrations/{selector}/freeze` |投稿 | `FreezeNameRequestV1` |保護者/評議会の凍結۔ガーディアン チケット ガバナンス ドケット حوالہ درکار۔ |
| `/v2/sns/registrations/{selector}/freeze` |削除 | `GovernanceHookV1` |修復 凍結解除評議会オーバーライド ریکارڈ ہونے کو یقینی بناتا ہے۔ |
| `/v2/sns/reserved/{selector}` |投稿 | `ReservedAssignmentRequestV1` |予約された名前 管理人/評議会 割り当て 割り当て|
| `/v2/sns/policies/{suffix_id}` |入手 | -- | `SuffixPolicyV1` موجودہ حاصل کرتا ہے (キャッシュ可能)۔ |
| `/v2/sns/registrations/{selector}` |入手 | -- | موجودہ `NameRecordV1` + موثر حالت (アクティブ、グレース وغیرہ) واپس کرتا ہے۔ |

**セレクター エンコーディング:** `{selector}` パス セグメント I105 圧縮 (`sora`) 標準 16 進数 ADDR-5 の値。 Torii `NameSelectorV1` 正規化する

**エラー モデル:** エンドポイント Norito JSON `code`、`message`、`details` واپس کرتے ہیں۔コードは `sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing` です。

### 3.1 CLI ヘルパー (N0 レジストラ)

クローズドベータ版スチュワード اب CLI کے ذریعے registrar استعمال کر سکتے ہیں بغیر ہاتھ سے JSON بنانے کے:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` CLI 設定アカウントコントローラー アカウントは `--controller` です (デフォルトは `[owner]`)。
- インライン決済フラグ `PaymentProofV1` マップ ہوتے ہیں؛構造化された領収書 `--payment-json PATH` دیں۔メタデータ (`--metadata-json`) ガバナンス フック (`--governance-json`) の説明

読み取り専用ヘルパーのリハーサル:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

実装 `crates/iroha_cli/src/commands/sns.rs` 実装コマンド دستاویز میں بیان کردہ Norito DTO دوبارہ استعمال کرتے ہیں تاکہ CLI 出力 Torii 応答 کےバイト単位のバイト数

追加のヘルパーの更新、移籍とガーディアンのアクションの概要:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner i105... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` میں درست `GovernanceHookV1` ریکارڈ ہونا چاہیے (提案ID、投票ハッシュ、スチュワード/保護者の署名)۔ `/v2/sns/registrations/{selector}/...` エンドポイントのテスト ベータ オペレーターのテスト Torii サーフェスのリハーサルSDK の概要

## 4. gRPC サービス

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

ワイヤ形式: コンパイル時 Norito スキーマ ハッシュ
`fixtures/norito_rpc/schema_hashes.json` میں درج ہے (行 `RegisterNameRequestV1`,
`RegisterNameResponseV1`、`NameRecordV1`、وغیرہ)。

## 5. ガバナンスが証拠を引っ掛ける突然変異呼び出しのリプレイの証拠の確認:

|アクション |必要なガバナンス データ |
|----------|--------------------------|
|標準登録/更新 |決済指示書、支払い証明書を参照してください。議会投票 評価 評価 評価 評価層 評価 管理委員会承認 評価 評価 評価 評価|
|プレミアム層の登録/予約済み割り当て | `GovernanceHookV1` プロポーザル ID + スチュワード承認を参照してください。 |
|転送 |評議会投票ハッシュ + DAO シグナル ハッシュ保護者許可 転送紛争解決 トリガー ہو۔ |
|凍結/凍結解除 |ガーディアンチケットの署名 評議会オーバーライド (凍結解除) |

Torii 証明 ہوئے چیک کرتا ہے:

1. 提案 ID ガバナンス台帳 (`/v2/governance/proposals/{id}`) میں موجود ہے اور ステータス `Approved` ہے۔
2. ハッシュ ハッシュ アーティファクトの投票 マッチ کرتے ہیں۔
3. スチュワード/保護者の署名 `SuffixPolicyV1` 公開鍵を参照してください。

失敗したチェック `sns_err_governance_missing` واپس کرتے ہیں۔

## 6. ワークフローの例

### 6.1 標準登録

1. クライアント `/v2/sns/policies/{suffix_id}` クエリの価格設定、猶予期間の設定、層の確認
2. クライアント `RegisterNameRequestV1` 番号:
   - `selector` I105 の 2 番目に優れた圧縮 (`sora`) ラベルの派生ラベル
   - `term_years` پالیسی حدود میں۔
   - `payment` 財務/スチュワード スプリッター転送、参照してください。
3. Torii を検証します:
   - ラベルの正規化 + 予約済みリスト
   - 期間/総額対 `PriceTierV1`。
   - 支払い証明の金額 >= 計算された価格 + 手数料
4. Torii:
   - `NameRecordV1` محفوظ کرتا ہے۔
   - `RegistryEventV1::NameRegistered` が発声します
   - `RevenueAccrualEventV1` が発声します
   - 記録 + イベント فے۔

### 6.2 猶予期間中の更新

猶予更新の標準リクエストとペナルティ検出の手順:

- Torii `now` と `grace_expires_at` の比較 `SuffixPolicyV1` の追加料金表
- 支払い証明書と追加料金カバー失敗 => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` 勝利 `expires_at` 勝利

### 6.3 ガーディアンのフリーズと評議会のオーバーライド

1. Guardian `FreezeNameRequestV1` 送信 کرتا ہے جس میں Incident id کا حوالہ دینے والا ticket ہوتا ہے۔
2. Torii 記録 `NameStatus::Frozen` منتقل کرتا ہے، `NameFrozen` 放出 کرتا ہے۔
3. 修復評議会による無効化演算子 DELETE `/v2/sns/registrations/{selector}/freeze` کو `GovernanceHookV1` کے ساتھ بھیجتا ہے۔
4. Torii オーバーライド検証 `NameUnfrozen` 発行

## 7. 検証コードとエラーコード

|コード |説明 | HTTP |
|------|---------------|------|
| `sns_err_reserved` |ラベルは予約済みです ブロックされています ہے۔ | 409 |
| `sns_err_policy_violation` |用語、層 コントローラ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` |支払い証明、価値、資産の不一致、 | 402 |
| `sns_err_governance_missing` |必要なガバナンス アーティファクト / 無効なガバナンス アーティファクト| 403 |
| `sns_err_state_conflict` |ライフサイクル状態 操作が許可されました| 409 |

コード `X-Iroha-Error-Code` 構造化 Norito JSON/NRPC エンベロープ 表面 ہوتے ہیں۔

## 8. 実装上の注意事項

- Torii 保留中のオークション کو `NameRecordV1.auction` میں رکھتا ہے اور `PendingAuction` کے دوران 直接登録の試み کو 拒否 کرتا ہے۔
- 支払証明 Norito 元帳領収書 دوبارہ استعمال کرتے ہیں؛財務サービス ヘルパー API (`/v2/finance/sns/payments`)
- SDK エンドポイント 強く型付けされたヘルパー ラップ ウォレット エラー理由 (`ERR_SNS_RESERVED`, وغیرہ) ラップ

## 9. 次のステップ

- SN-3 オークション、Torii ハンドラー、レジストリ契約、ワイヤリング
- SDK 固有のガイド (Rust/JS/Swift) および API を参照してください。
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ガバナンスフック証拠フィールド クロスリンク 拡張 拡張