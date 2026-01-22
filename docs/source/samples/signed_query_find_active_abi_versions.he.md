<!-- Hebrew translation of docs/source/samples/signed_query_find_active_abi_versions.md -->

---
lang: he
direction: rtl
source: docs/source/samples/signed_query_find_active_abi_versions.md
status: complete
translator: manual
---

<div dir="rtl">

# בנייה של `SignedQuery` עבור ‎`FindActiveAbiVersions`

הדוגמה הבאה מדגימה כיצד לבנות, לחתום ולקודד מסגרת Norito מסוג `SignedQuery` שמבצעת את השאילתה הסינגולרית `FindActiveAbiVersions`. ניתן לשלוח את הבייטים שנוצרו ב־`POST /query` או להזין אותם ל־CLI בעזרת `query stdin-raw`.

צעדים
- בנו `SingularQueryBox::FindActiveAbiVersions`.
- עטפו בתוך `QueryRequest::Singular` ולאחר מכן `QueryRequestWithAuthority { authority, request }`.
- חתמו באמצעות `KeyPair` של בעל הסמכות כדי לקבל `SignedQuery`.
- קודדו בעזרת `norito::codec::Encode::encode`.

דוגמת Rust
```rust
use iroha_data_model::prelude::*;
use iroha_data_model::query::{QueryRequest, QueryRequestWithAuthority, SignedQuery};
use iroha_crypto::KeyPair;

fn build_signed_query_find_active_abi(authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    // 1) יצירת תיבת השאילתה הסינגולרית
    let q = iroha_data_model::query::runtime::prelude::FindActiveAbiVersions;
    let box_ = iroha_data_model::query::SingularQueryBox::FindActiveAbiVersions(q);

    // 2) עטיפה כבקשת שאילתה והוספת זהות הסמכות
    let req = QueryRequest::Singular(box_);
    let with_auth = QueryRequestWithAuthority { authority, request: req };

    // 3) חתימה שמייצרת SignedQuery
    let signed = with_auth.sign(kp);

    // 4) קידוד Norito במבנה TLV
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

שליחה
- HTTP גולמי: בצעו `POST /query` עם הבייטים המקודדים כגוף הבקשה.
- CLI: המירו את הבייטים ל־base64 והזינו אל `iroha ledger query stdin-raw`.

פלט
- הצלחה מחזירה מהצומת תגובת Norito מסוג `QueryResponse::Singular(ActiveAbiVersions)`.
- ה־CLI מציג JSON מנותח בעזרת מעטפות ה־Norito JSON.

```json
{
  "active_versions": [1],
  "default_compile_target": 1
}
```

</div>
