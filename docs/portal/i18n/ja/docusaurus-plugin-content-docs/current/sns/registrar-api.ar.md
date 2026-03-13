---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
تعكس هذه الصفحة `docs/source/sns/registrar_api.md` وتعمل الان كنسخة بوابة
قياسية。あなたのことを忘れないでください。
:::

# واجهة مسجل SNS وhooks الحوكمة (SN-2b)

** الحالة:** 2026-03-24 - قيد مراجعة Nexus コア  
** SN-2b "レジストラ API とガバナンス フック"  
** المطلبات المسبقة:** تعريفات المخطط في [`registry-schema.md`](./registry-schema.md)

تحدد هذه المذكرة نقاط نهاية Torii وخدمات gRPC وDTOات الطلب/الاستجابة واثار
ソーシャルメディア (SNS)。 SDK の開発
ソーシャルメディアのSNS。

## 1. いいえ

|回答 |翻訳 |
|----------|----------|
| और देखें REST は `/v2/sns/*` と gRPC `sns.v1.Registrar` です。 Norito-JSON (`application/json`) と Norito-RPC (`application/x-norito`)。 |
|認証 | `Authorization: Bearer` は mTLS のサフィックス スチュワードです。 `scope=sns.admin` を参照してください。 |
| حدود المعدل |バケット `torii.preauth_scheme_limits` の JSON のバースト: `sns.register`、 `sns.renew`、`sns.controller`、`sns.freeze`。 |
|ああ | Torii يعرض `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المسجل (رشح `scheme="norito_rpc"`); كما تزيد الواجهة `sns_registrar_status_total{result, suffix_id}`。 |

## 2. DTO を使用する

[`registry-schema.md`](./registry-schema.md)。 `NameSelectorV1` + `SuffixId` は、`NameSelectorV1` + `SuffixId` です。

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

## 3. 休息

| और देखें और देखेंああ |ああ |
|-----------|-----------|-----------|------|
| `/v2/sns/registrations` |投稿 | `RegisterNameRequestV1` |ありがとうございます。ログインしてください。 |
| `/v2/sns/registrations/{selector}/renew` |投稿 | `RenewNameRequestV1` |そうです。 يفرض نوافذ 恵み/救い من السياسة. |
| `/v2/sns/registrations/{selector}/transfer` |投稿 | `TransferNameRequestV1` | بعد ارفاق موافقات الحوكمة. |
| `/v2/sns/registrations/{selector}/controllers` |置く | `UpdateControllersRequestV1` |コントローラーححقق من عناوين الحساب الموقعة. |
| `/v2/sns/registrations/{selector}/freeze` |投稿 | `FreezeNameRequestV1` |保護者/評議会。守護者 ومرجع دفتر حوكمة。 |
| `/v2/sns/registrations/{selector}/freeze` |削除 | `GovernanceHookV1` | فك التجميد بعد المعالجة؛をオーバーライドします。 |
| `/v2/sns/reserved/{selector}` |投稿 | `ReservedAssignmentRequestV1` |管理人/評議会。 |
| `/v2/sns/policies/{suffix_id}` |入手 | -- | يجلب `SuffixPolicyV1` الحالي (قابل للكاش)。 |
| `/v2/sns/registrations/{selector}` |入手 | -- | يعيد `NameRecordV1` الحالي + الحالة الفعلية (アクティブ、グレース、الخ)。 |

** セレクタ:** مقطع `{selector}` يقبل I105 او مضغوط او 16 進数 قياسي حسب ADDR-5; Torii يطبعها عبر `NameSelectorV1`。

** 説明:** Norito JSON は `code`、`message`、`details` です。 `sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing`。

### 3.1 مساعدات CLI (متطلب المسجل اليدوي N0)

يمكن لـ stewards البيتا المغلقة الان تشغيل المسجل عبر CLI بدون تجهيز JSON يدويا:

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

- `--owner` يفترض حساب اعدادات CLI؛ `--controller` は、コントローラ اضافية (الافتراضي `[owner]`) です。
- 回答: `PaymentProofV1`; مرر `--payment-json PATH` عندما تكون لديك ايصال منظم.メタデータ (`--metadata-json`) とフック (`--governance-json`) が含まれています。

重要:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

`crates/iroha_cli/src/commands/sns.rs` 意味; DTO ات Norito الموصوفة في هذا المستند بحيث تتطابق مخرجات CLI مع ردود Torii いいえ。

保護者:

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

`--governance-json` يجب ان يحتوي على سجل `GovernanceHookV1` صالح (提案 ID 投票ハッシュ数 تواقيع スチュワード/ガーディアン)。 كل امر يعكس ببساطة نقطة النهاية `/v2/sns/registrations/{selector}/...` المقابلة حتى يتمكن مشغلو البيتا من تمرين اسطح Torii SDK。

## 4. gRPC の使用

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

ワイヤー形式: ハッシュ Norito في وقت الترجمة مسجل تحت
`fixtures/norito_rpc/schema_hashes.json` (`RegisterNameRequestV1`,
`RegisterNameResponseV1`、`NameRecordV1`、記号）。

## 5. フック

كل استدعاء يغير الحالة يجب ان يرفق ادلة مناسبة لاعادة التشغيل:|ああ |ニュース | ニュース
|----------|--------------------------|
|翻訳 / 翻訳 / 翻訳 / 翻訳決済完了管理人はスチュワードです。 |
| تسجيل شريحة プレミアム / تعيين محجوز | `GovernanceHookV1` は、提案 ID + 管理者です。 |
|いいえ |ハッシュ値 + ハッシュ値 DAO؛クリアランスガーディアン عندما ينطلق النقل عبر حل نزاع。 |
|評価/فك 評価 |ガーディアンは المجلس (فك التجميد) をオーバーライドします。 |

Torii يتحقق من الاثباتات عبر فحص:

1. 提案 ID موجود في دفتر الحوكمة (`/v2/governance/proposals/{id}`) وحالته `Approved`。
2. ハッシュ値は、ハッシュ値です。
3. スチュワード/ガーディアン تشير الى المفاتيح العامة المتوقعة من `SuffixPolicyV1`.

`sns_err_governance_missing` です。

## 6. いいえ。

### 6.1 のレビュー

1. `/v2/sns/policies/{suffix_id}` は、グレース والشرائح المتاحة を意味します。
2. `RegisterNameRequestV1`:
   - `selector` مشتق من ラベル I105 (المفضل) او المضغوط (الخيار الثاني)。
   - `term_years` 認証済み。
   - `payment` يشير الى تحويل スプリッター الخزينة/スチュワード。
3. Torii :
   - ラベル + قائمة محجوزة。
   - 期間/総額対 `PriceTierV1`。
   - الثبات الدفع >= السعر المحسوب + الرسوم。
4. Torii:
   - يحفظ `NameRecordV1`。
   - يصدر `RegistryEventV1::NameRegistered`。
   - `RevenueAccrualEventV1`。
   - يعيد السجل الجديد + الاحداث。

### 6.2 グレース

猶予を与えてください:

- Torii `now` `grace_expires_at` 追加料金 `SuffixPolicyV1`。
- 追加料金。 => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` يسجل `expires_at` الجديد。

### 6.3 ガーディアンオーバーライド

1. ガーディアン يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii は `NameStatus::Frozen`、`NameFrozen` です。
3. オーバーライドします。 `/v2/sns/registrations/{selector}/freeze` を削除してください。`GovernanceHookV1` を削除してください。
4. Torii は、`NameUnfrozen` をオーバーライドします。

## 7. いいえ

|ああ |ああ | HTTP |
|------|-------|------|
| `sns_err_reserved` |ありがとうございます。 | 409 |
| `sns_err_policy_violation` |コントローラーを使用してください。 | 422 |
| `sns_err_payment_mismatch` |資産を評価してください。 | 402 |
| `sns_err_governance_missing` |ニュース/ニュース/ニュース/ニュース。 | 403 |
| `sns_err_state_conflict` |ログインしてください。 | 409 |

`X-Iroha-Error-Code` Norito JSON/NRPC を参照してください。

## 8. 大事なこと

- Torii يخزن المزادات المعلقة تحت `NameRecordV1.auction` ويرفض محاولات التسجيل المزاشر بينما الحالة `PendingAuction`。
- ニュース ニュース Norito ニュース重要な API (`/v2/finance/sns/payments`)。
- SDK の開発、開発、開発、開発、開発、開発、開発、開発(`ERR_SNS_RESERVED`, الخ)。

## 9. いいえ

- SN-3 の Torii のバージョン。
- SDK (Rust/JS/Swift) のバージョン。
- توسيع [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) フックをフックします。