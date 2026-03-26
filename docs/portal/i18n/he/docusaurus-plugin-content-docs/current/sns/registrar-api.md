---
id: registrar-api
lang: he
direction: rtl
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sns/registrar_api.md` וכעת משמש כהעתק הפורטל
הקנוני. קובץ המקור נשאר עבור PRs תרגום.
:::

# API של registrar SNS והוקי ממשל (SN-2b)

**סטטוס:** נוסח 2026-03-24 - בבדיקת Nexus Core  
**קישור roadmap:** SN-2b "Registrar API & governance hooks"  
**דרישות מקדימות:** הגדרות סכימה ב-[`registry-schema.md`](./registry-schema.md)

מסמך זה מפרט את נקודות הקצה של Torii, שירותי gRPC, DTOs של בקשה/תגובה וארטיפקטים
של ממשל הדרושים להפעלת registrar של Sora Name Service (SNS). זהו החוזה הסמכותי
עבור SDKs, ארנקים ואוטומציה שצריכים לרשום, לחדש או לנהל שמות SNS.

## 1. תעבורה ואימות

| דרישה | פרט |
|-------|------|
| פרוטוקולים | REST תחת `/v1/sns/*` ושירות gRPC `sns.v1.Registrar`. שניהם מקבלים Norito-JSON (`application/json`) ו-Norito-RPC בינארי (`application/x-norito`). |
| Auth | `Authorization: Bearer` tokens או תעודות mTLS שמונפקות לכל suffix steward. נקודות קצה רגישות לממשל (freeze/unfreeze, הקצאות שמורות) דורשות `scope=sns.admin`. |
| מגבלות קצב | registrars חולקים את ה-buckets `torii.preauth_scheme_limits` עם קוראי JSON וכן מגבלות burst לכל suffix: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| טלמטריה | Torii חושף `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` עבור מטפלי registrar (סינון לפי `scheme="norito_rpc"`); ה-API גם מגדיל `sns_registrar_status_total{result, suffix_id}`. |

## 2. סקירת DTO

השדות מפנים למבנים הקנוניים המוגדרים ב-[`registry-schema.md`](./registry-schema.md). כל המטענים כוללים `NameSelectorV1` + `SuffixId` כדי למנוע ניתוב דו-משמעי.

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

## 3. נקודות קצה REST

| נקודת קצה | שיטה | Payload | תיאור |
|-----------|------|---------|--------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | רישום או פתיחה מחדש של שם. פותר את tier התמחור, מאמת הוכחות תשלום/ממשל, ומייצר אירועי רישום. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | מאריך את התקופה. אוכף חלונות grace/redemption מהמדיניות. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | מעביר בעלות לאחר הצמדת אישורי ממשל. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | מחליף סט controllers; מאמת כתובות חשבון חתומות. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Freeze של guardian/council. דורש כרטיס guardian והפניה לתיק ממשל. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze לאחר תיקון; מוודא ש-override של council נרשם. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | הקצאת שמות שמורים ע"י steward/council. |
| `/v1/sns/policies/{suffix_id}` | GET | -- | מביא את `SuffixPolicyV1` הנוכחי (ניתן לקאש). |
| `/v1/sns/names/{namespace}/{literal}` | GET | -- | מחזיר את `NameRecordV1` הנוכחי + מצב אפקטיבי (Active, Grace, וכו'). |

**קידוד selector:** מקטע הנתיב `{selector}` מקבל i105, דחוס, או hex קנוני לפי ADDR-5; Torii מנרמל אותו דרך `NameSelectorV1`.

**מודל שגיאות:** כל נקודות הקצה מחזירות Norito JSON עם `code`, `message`, `details`. הקודים כוללים `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 עזרי CLI (דרישת registrar ידני N0)

stewards של בטא סגורה יכולים כעת להפעיל את registrar דרך ה-CLI בלי לבנות JSON ידנית:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` ברירת המחדל היא חשבון ה-CLI; חזור על `--controller` כדי לצרף חשבונות controller נוספים (ברירת מחדל `[owner]`).
- דגלי תשלום inline ממופים ישירות ל-`PaymentProofV1`; העבר `--payment-json PATH` כשכבר יש לך קבלה מובנית. Metadata (`--metadata-json`) והוקי ממשל (`--governance-json`) פועלים באותו דפוס.

עזרי קריאה בלבד משלימים חזרות:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

ראו `crates/iroha_cli/src/commands/sns.rs` ליישום; הפקודות ממחזרות את DTOs של Norito שמתוארים במסמך זה כך שהפלט של ה-CLI תואם לתגובות Torii ביט-לביט.

עזרי נוספים מכסים חידושים, העברות ופעולות guardian:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner <i105-account-id> \
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

`--governance-json` חייב להכיל רשומת `GovernanceHookV1` תקפה (proposal id, vote hashes, חתימות steward/guardian). כל פקודה פשוט משקפת את נקודת הקצה `/v1/sns/names/{namespace}/{literal}/...` המתאימה כדי שמפעילי הבטא יוכלו לתרגל בדיוק את משטחי Torii ש-SDKs יקראו.

## 4. שירות gRPC

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

Wire-format: hash סכמת Norito בזמן קומפילציה נרשם תחת
`fixtures/norito_rpc/schema_hashes.json` (שורות `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, וכו').

## 5. הוקי ממשל וראיות

כל קריאה שמשנה מצב חייבת לצרף ראיות המתאימות לשחזור:

| פעולה | נתוני ממשל נדרשים |
|-------|-------------------|
| רישום/חידוש סטנדרטי | הוכחת תשלום שמפנה להוראת settlement; לא נדרש vote council אלא אם tier דורש אישור steward. |
| רישום tier פרימיום / הקצאה שמורה | `GovernanceHookV1` שמפנה ל-proposal id + steward acknowledgement. |
| העברה | hash של vote council + hash של signal DAO; clearance של guardian כשההעברה מופעלת ע"י פתרון סכסוך. |
| Freeze/Unfreeze | חתימת כרטיס guardian בתוספת override של council (unfreeze). |

Torii מאמת הוכחות על ידי בדיקה:

1. proposal id קיים בלדג'ר הממשל (`/v1/governance/proposals/{id}`) והסטטוס `Approved`.
2. ה-hashes תואמים לארטיפקטי ההצבעה הרשומים.
3. חתימות steward/guardian מפנות למפתחות הציבוריים הצפויים מ-`SuffixPolicyV1`.

בדיקות שנכשלו מחזירות `sns_err_governance_missing`.

## 6. דוגמאות זרימת עבודה

### 6.1 רישום סטנדרטי

1. הלקוח שואל `/v1/sns/policies/{suffix_id}` כדי לקבל מחירים, grace ו-tiers זמינים.
2. הלקוח בונה `RegisterNameRequestV1`:
   - `selector` נגזר מתווית i105 (מועדף) או דחוסה (אפשרות שנייה).
   - `term_years` בתוך גבולות המדיניות.
   - `payment` שמפנה להעברת splitter של אוצרות/steward.
3. Torii מאמת:
   - נרמול תווית + רשימה שמורה.
   - Term/gross price מול `PriceTierV1`.
   - סכום הוכחת תשלום >= מחיר מחושב + fees.
4. בהצלחה Torii:
   - שומר `NameRecordV1`.
   - מפיק `RegistryEventV1::NameRegistered`.
   - מפיק `RevenueAccrualEventV1`.
   - מחזיר את הרשומה החדשה + אירועים.

### 6.2 חידוש בתקופת grace

חידושים בתקופת grace כוללים את הבקשה הסטנדרטית בתוספת זיהוי קנס:

- Torii משווה `now` מול `grace_expires_at` ומוסיף טבלאות surcharge מ-`SuffixPolicyV1`.
- הוכחת התשלום חייבת לכסות את הסרצ'רג'. כשל => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` רושם את `expires_at` החדש.

### 6.3 Freeze של guardian ו-override של council

1. guardian מגיש `FreezeNameRequestV1` עם כרטיס שמפנה ל-id תקרית.
2. Torii מעביר את הרשומה ל-`NameStatus::Frozen`, ומפיק `NameFrozen`.
3. לאחר תיקון, council מוציא override; המפעיל שולח DELETE `/v1/sns/names/{namespace}/{literal}/freeze` עם `GovernanceHookV1`.
4. Torii מאמת את ה-override, ומפיק `NameUnfrozen`.

## 7. אימות וקודי שגיאה

| קוד | תיאור | HTTP |
|-----|-------|------|
| `sns_err_reserved` | התווית שמורה או חסומה. | 409 |
| `sns_err_policy_violation` | התקופה, tier או סט controllers מפרים את המדיניות. | 422 |
| `sns_err_payment_mismatch` | אי התאמה של ערך או asset בהוכחת התשלום. | 402 |
| `sns_err_governance_missing` | ארטיפקטי ממשל נדרשים חסרים/לא תקפים. | 403 |
| `sns_err_state_conflict` | הפעולה אינה מותרת במצב מחזור החיים הנוכחי. | 409 |

כל הקודים נחשפים דרך `X-Iroha-Error-Code` ובמעטפות Norito JSON/NRPC מובנות.

## 8. הערות מימוש

- Torii מאחסן מכרזים ממתינים תחת `NameRecordV1.auction` ודוחה נסיונות רישום ישיר בזמן `PendingAuction`.
- הוכחות תשלום ממחזרות קבלות של Norito ledger; שירותי treasury מספקים helper APIs (`/v1/finance/sns/payments`).
- SDKs צריכים לעטוף את נקודות הקצה הללו בעזרים חזקים מבחינת טיפוס כדי שארנקים יציגו סיבות שגיאה ברורות (`ERR_SNS_RESERVED`, וכו').

## 9. השלבים הבאים

- לחבר את מטפלי Torii לחוזה הרישום האמיתי ברגע שמכרזי SN-3 יגיעו.
- לפרסם מדריכי SDK ספציפיים (Rust/JS/Swift) שמפנים ל-API הזה.
- להרחיב את [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) עם קישורים צולבים לשדות הראיות של governance hook.
