<!-- Hebrew translation of docs/source/governance_api.md -->

---
lang: he
direction: rtl
source: docs/source/governance_api.md
status: complete
translator: manual
---

<div dir="rtl">

---
title: ממשק אפליקציית הממשל — נקודות קצה (טיוטה)
---

סטטוס: טיוטה לתמיכה במשימות יישום הממשל. המופעים עשויים להשתנות בזמן הפיתוח. דטרמיניזם ומדיניות RBAC הם אילוצים נורמטיביים; כאשר מספקים `authority` ו-`private_key`, Torii חותמת ומגישה את הטרנזקציה. אחרת הלקוחות בונים ושולחים ל-`/transaction`.

## תצוגה כללית

- כל נקודות הקצה מחזירות JSON. בזרימות שמייצרות טרנזקציות, התגובה כוללת `tx_instructions` — מערך של שלד הוראה אחד או יותר:
  - `wire_id`: מזהה הרישום של סוג ההוראה.
  - `payload_hex`: מטען Norito בהקס.
- אם מספקים `authority` ו-`private_key` (או `private_key` ב-DTO של ballot), Torii חותמת ומגישה ועדיין מחזירה `tx_instructions`.
- הלקוח מרכיב SignedTransaction באמצעות הסמכות שלו ו-`chain_id`, חותם ושולח POST אל `/transaction`.

## נקודות קצה

- POST `/v1/gov/proposals/deploy-contract`
  - בקשה (JSON):
    ```json
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
    "code_hash": "blake2b32:…" | "…64hex",
    "abi_hash": "blake2b32:…" | "…64hex",
    "abi_version": "1",
    "window": { "lower": 12345, "upper": 12400 },
    "authority": "alice@wonderland?",
    "private_key": "…?"
  }
    ```
  - תגובה:
    ```json
    { "ok": true, "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "…", "payload_hex": "…" }] }
    ```
  - ולידציה: הצמתים מנרמלים את `abi_hash` בהתאם ל-`abi_version` ודוחים שגיאות. עבור `abi_version = "v1"` הערך הצפוי הוא `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

### Contracts API (Deploy)

- POST `/v1/contracts/deploy`
  - בקשה: `{ "authority": "alice@wonderland", "private_key": "…", "code_b64": "…" }`
  - התנהגות: מחשב `code_hash` מגוף תוכנית ה-IVM ו-`abi_hash` מהכותרת `abi_version`, ואז מגיש `RegisterSmartContractCode` (מניפסט) ו-`RegisterSmartContractBytes` (בייטים מלאים של `.to`) בשם `authority`.
  - תגובה: `{ "ok": true, "code_hash_hex": "…", "abi_hash_hex": "…" }`
  - נקודות קשורות:
    - GET `/v1/contracts/code/{code_hash}` → מחזיר את המניפסט השמור.
    - GET `/v1/contracts/code-bytes/{code_hash}` → מחזיר `{ "code_b64": "…" }`.
- POST `/v1/contracts/instance`
  - בקשה: `{ "authority": "alice@wonderland", "private_key": "…", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "…" }`
  - התנהגות: מבצע דיפלוי מלא ומפעיל מיידית את `(namespace, contract_id)` בטרנזקציה אחת (`RegisterSmartContractCode` + `RegisterSmartContractBytes` + `ActivateContractInstance`).
  - תגובה: `{ "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "…", "abi_hash_hex": "…" }`

### שירות Alias

- POST `/v1/aliases/voprf/evaluate`
  - בקשה: `{ "blinded_element_hex": "…" }`
  - תגובה: `{ "evaluated_element_hex": "…128hex", "backend": "blake2b512-mock" }`
    - `backend` מזהה את מימוש המעריך. נכון לעכשיו: `blake2b512-mock`.
  - הערות: מעריך דטרמיניסטי המדמה VOPRF באמצעות Blake2b512 עם הפרדת דומיין `iroha.alias.voprf.mock.v1`. מיועד לכלי בדיקות עד לחיבור הצינור הפרודקשני.
  - שגיאות: HTTP `400` על קלט הקסהדצימלי שגוי; Torii מחזירה מעטפת Norito מסוג `ValidationFail::QueryFailed::Conversion` עם הודעת המקודד.
- POST `/v1/aliases/resolve`
  - בקשה: `{ "alias": "GB82 WEST 1234 5698 7654 32" }`
  - תגובה: `{ "alias": "GB82WEST12345698765432", "account_id": "…@…", "index": 0, "source": "iso_bridge" }`
  - הערות: דורש הפעלת רנטיים ISO (`[iso_bridge.account_aliases]` ב-`iroha_config`). Torii מנרמל על ידי הסרת רווחים והפיכה לאותיות גדולות. מחזיר 404 אם האליאס לא קיים, ו-503 אם הרנטיים מושבת.
- POST `/v1/aliases/resolve_index`
  - בקשה: `{ "index": 0 }`
  - תגובה: `{ "index": 0, "alias": "GB82WEST12345698765432", "account_id": "…@…", "source": "iso_bridge" }`
  - הערות: אינדקסים מוקצים דטרמיניסטית לפי סדר התצורה (0-based). ניתן לשמור תשובות לאודיט של אירועי attest.

### מגבלת גודל קוד

- פרמטר מותאם: `max_contract_code_bytes` (u64 ב-JSON)
  - שולט בגודל המקסימלי (בבתים) לאחסון קוד חוזה on-chain.
  - ברירת מחדל: ‎16 MiB‎. רישומי `RegisterSmartContractBytes` שחורגים מהקאפ נכשלים עם Invariant Violation.
  - ניתן לעדכן באמצעות `SetParameter(Custom)` עם `id = "max_contract_code_bytes"` ומטען נומרי.

### Ballots

- POST `/v1/gov/ballots/zk`
  - בקשה: `{ "authority": "alice@wonderland", "private_key": "…?", "chain_id": "…", "election_id": "e1", "proof_b64": "…", "public": {…} }`
  - תגובה: `{ "ok": true, "accepted": true, "tx_instructions": [{…}] }`
  - הערות:
    - כאשר בקלט הציבורי קיימים `owner`, ‏`amount`, ‏`duration_blocks` וההוכחה מאומתת מול VK מוגדר, הצומת יוצר או מאריך Governance Lock עבור `election_id` וה-`owner`. הכיוון נשאר `unknown`; רק סכום ותוקף מתעדכנים. הצבעות חוזרות מגדילות בלבד (`max`).
    - הצבעות חוזרות שמנסות להקטין סכום או תוקף נדחות עם `BallotRejected`.
    - מופע חוזה חייב לקרוא ל-`ZK_VOTE_VERIFY_BALLOT` לפני `SubmitBallot`; ה-Host אוכף latch חד-פעמי.

- POST `/v1/gov/ballots/plain`
  - בקשה: `{ "authority": "alice@domain", "private_key": "…?", "chain_id": "…", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }`
  - תגובה: `{ "ok": true, "accepted": true, "tx_instructions": [{…}] }`
  - הערות: הצבעות חוזרות יכולות רק להגדיל סכום או תוקף. `owner` חייב להיות זהה לסמכות הטרנזקציה. מינימום משך הוא `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - בקשה: `{ "referendum_id": "r1", "proposal_id": "…64hex", "authority": "alice@wonderland?", "private_key": "…?" }`
  - תגובה: `{ "ok": true, "tx_instructions": [{…}] }`
  - הערות:
    - כאשר מספקים `authority`/`private_key`, Torii חותמת ומגישה; אחרת מוחזר שלד `FinalizeReferendum` לחתימה.
    - בבחירות ZK, מסלולי חוזה חייבים לקרוא ל-`ZK_VOTE_VERIFY_TALLY` לפני `FinalizeElection`; ה-Host אוכף latch חד-פעמי. `FinalizeReferendum` דוחה רפרנדומים מסוג ZK עד שה-tally מסומן כ-finalized.
    - סגירה אוטומטית ב-`h_end` מפיקה Approved/Rejected רק לרפרנדומים Plain; רפרנדומים ZK נשארים Closed עד שה-tally מסומן finalized ומבוצע `FinalizeReferendum`.
    - בדיקות turnout משתמשות רק ב-approve+reject; abstain לא נספר.

- POST `/v1/gov/enact`
  - בקשה: `{ "proposal_id": "…64hex", "preimage_hash": "…64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "alice@wonderland?", "private_key": "…?" }`
  - תגובה: `{ "ok": true, "tx_instructions": [{…}] }`
  - הערות: כאשר מספקים `authority`/`private_key`, Torii חותמת ומגישה; אחרת מוחזר שלד לחתימה. `preimage_hash` אופציונלי וכרגע אינפורמטיבי בלבד.

### מועצת פרלמנט (טיוטה)

- GET `/v1/gov/parliament/candidates`
  - תגובה: `{ "namespace": "parliament", "epoch": n, "candidate_accounts": ["…", …], "config": { "term_blocks": m, "min_stake": "…", "eligibility_asset_id": "…" } }`
  - הערות: משמש לשיקוף כיוונון מועצה. אם מועצה לא נשמרה, `parliament_term_blocks` מגדיר את אורך האפוק לזריעת PRF (`epoch = floor(height / term_blocks)`), ‏`parliament_min_stake` מכתיב את המינימום (ביחידות הכי קטנות) והנכס נסרק באמצעות `parliament_eligibility_asset_id`.

## RBAC

- הרשאות נדרשות:
  - הצעות: `CanProposeContractDeployment{ contract_id }`
  - Ballots: ‏`CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: ‏`CanEnactGovernance`
  - ניהול מועצה (עתידי): ‏`CanManageParliament`

## מרחבים מוגנים

- פרמטר מותאם `gov_protected_namespaces` (מערך מחרוזות) מפעיל בקרת כניסה לפריסות במרחבים מוגנים.
- על לקוחות לצרף מטא-דאטה לטרנזקציה עבור פריסות:
  - `gov_namespace`: המרחב (למשל `"apps"`)
  - `gov_contract_id`: מזהה הלוגי
- הבקרה מוודאת שקיימת הצעת ממשל מאושרת עבור `(namespace, contract_id, code_hash, abi_hash)` אחרת מתקבלת שגיאת NotPermitted.

### נקודת קצה לנוחות

- POST `/v1/gov/protected-namespaces`
  - בקשה: `{ "namespaces": ["apps", "system"] }`
  - תגובה: `{ "ok": true, "applied": 1 }`
  - הערות: למנהלים/בדיקות; דורש טוקן אם מוגדר. בפרודקשן עדיף טרנזקציה חתומה של `SetParameter(Custom)`.

## עזרי CLI

- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - מאמת:
    - ש-Torii שומר bytecode לכל `code_hash` ול-digest Blake2b-32 תואם.
    - שהמניפסט ב-`/v1/contracts/code/{code_hash}` מדווח `code_hash` / `abi_hash` תואמים.
    - שקיימת הצעת ממשל מאושרת עבור `(namespace, contract_id, code_hash, abi_hash)` לפי ה-hash שהצומת משתמש בו.
  - פולט דוח JSON עם `results[]` לכל חוזה ותקציר שורה אלא אם `--no-summary`.
  - שימושי לבקרה על מרחבים מוגנים או תהליכי פריסה מבוקרי ממשל.

- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - When any lock hint is provided, ZK ballots must supply `owner`, `amount`, and `duration_blocks`; partial hints are rejected. When `min_bond_amount > 0`, lock hints are required. Direction remains optional and is treated as a hint only.
  - הסיכום מוסיף `fingerprint=<hex>` דטרמיניסטי מה-`CastZkBallot` והערכים (`owner`, ‏`amount`, ‏`duration_blocks`, ‏`direction`, ‏`nullifier` בעת הצורך).
  - התשובה מציינת `payload_fingerprint_hex` ושדות מפוענחים כך שכלים חיצוניים לא יצטרכו לממש Norito מחדש.
  - כאשר סיפקו רמזי נעילה, הצומת יפיק אירועי `LockCreated`/`LockExtended` כאשר המעגל חושף ערכים תואמים.

- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - קיימים גם הכינויים `--lock-amount` / `--lock-duration-blocks` לשמירה על תאימות סקריפטים עם מסלול ה-ZK.
  - Summary output mirrors `vote-zk` by including the encoded instruction fingerprint and human-readable ballot fields (`owner`, `amount`, `duration_blocks`, `direction`), providing quick confirmation before signing the skeleton.

## רשימת מופעים

- GET `/v1/gov/instances/{ns}`
  - פרמטרים:
    - `contains`: סינון לפי תת-מחרוזת של `contract_id`.
    - `hash_prefix`: סינון לפי קידומת hex של `code_hash_hex`.
    - `offset` (ברירת מחדל 0), ‏`limit` (ברירת מחדל 100, מקס' 10,000).
    - `order`: ‏`cid_asc` (ברירת מחדל), ‏`cid_desc`, ‏`hash_asc`, ‏`hash_desc`.
  - תגובה: `{ "namespace": "ns", "instances": [{ "contract_id": "…", "code_hash_hex": "…" }, …], "total": N, "offset": n, "limit": m }`.

## Unlock Sweep (מפעיל/בקרה)

- GET `/v1/gov/unlocks/stats`
  - תגובה: `{ "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }`
  - הערות: `last_sweep_height` הוא הגובה האחרון שבו נעשתה סריקת נעילות שפג תוקפן. `expired_locks_now` מחושב ע"י סריקת רשומות עם `expiry_height <= height_current`.

- POST `/v1/gov/ballots/zk-v1`
  - בקשה (DTO בסגנון v1):
    ```json
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex?",
      "owner": "alice@wonderland?",
      "amount": "100?",
      "duration_blocks": 6000?,
      "direction": "Aye|Nay|Abstain?",
      "nullifier": "blake2b32:…64hex?"
    }
    ```
  - תגובה: `{ "ok": true, "accepted": true, "tx_instructions": [{…}] }`

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (פיצ'ר `zk-ballot`)
  - מקבל אובייקט `BallotProof` JSON ומחזיר שלד `CastZkBallot`.
  - ניתן לספק `private_key` כדי ש-Torii תחתום ותשלח; במקרה כזה `reason` יהיה `submitted transaction`.

</div>