<!-- Hebrew translation of docs/source/samples/signed_query_iterable_find_peers.md -->

---
lang: he
direction: rtl
source: docs/source/samples/signed_query_iterable_find_peers.md
status: complete
translator: manual
---

<div dir="rtl">

# שאילתות איטרביליות — ‎`FindPeers`‎ (בקשות Start / Continue)

הדוגמה הבאה מציגה כיצד להרכיב מסגרות Norito מסוג `SignedQuery` עבור שאילתה איטרבילית שמורכבת מהבקשות `Start` ו־`Continue`.

בקשת Start (אצווה ראשונה)
```rust
use iroha_data_model::prelude::*;
use iroha_data_model::query::{
    builder::QueryExecutor,
    parameters::{FetchSize, Pagination},
    QueryRequest, QueryRequestWithAuthority, SignedQuery,
};
use iroha_crypto::KeyPair;

fn build_start_find_peers(authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    // בניית השאילתה עם תנאי PASS והגדרת עימוד/גודל משיכה אופציונליים
    let mut b = iroha::Client::new(Default::default())
        .query(iroha_data_model::query::peer::prelude::FindPeers);
    b = b.with_pagination(Pagination::new(Some(core::num::NonZeroU64::new(100).unwrap()), 0));
    b = b.with_fetch_size(FetchSize::new(Some(core::num::NonZeroU64::new(100).unwrap())));

    // המרת הבילדר ל־QueryWithParams מסוג מטושטש
    let qwp = b.into_query_with_params();

    // עטיפה כ-Start וחתימה
    let req = QueryRequest::Start(qwp);
    let with_auth = QueryRequestWithAuthority { authority, request: req };
    let signed = with_auth.sign(kp);
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

בקשת Continue
```rust
use iroha_data_model::query::{QueryRequest, QueryRequestWithAuthority, SignedQuery};
use iroha_crypto::KeyPair;

fn build_continue(cursor: iroha_data_model::query::parameters::ForwardCursor, authority: AccountId, kp: &KeyPair) -> Vec<u8> {
    let req = QueryRequest::Continue(cursor);
    let with_auth = QueryRequestWithAuthority { authority, request: req };
    let signed = with_auth.sign(kp);
    norito::codec::Encode::encode(&SignedQuery::from(signed))
}
```

הערות
- תגובת `Start` היא ‏`QueryResponse::Iterable(QueryOutput)` ובתוכה `batch`, ‏`remaining_items` ו־`continue_cursor` אופציונלי.
- השתמשו ב־`continue_cursor` המוחזר כדי לבנות בקשת `Continue` ולשלוף את האצווה הבאה.
- ה־CLI מספק מצב נוח (`query stdin-raw`) לשליחת מסגרות `SignedQuery` מקודדות ב־base64 או בהקסה.

</div>
