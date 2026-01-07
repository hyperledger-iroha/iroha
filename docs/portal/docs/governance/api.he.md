---
lang: he
direction: rtl
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77a3a111d5dc132351c92b586389766a2d183c8bb60aed68b18032e10421c92
source_last_modified: "2025-12-04T07:55:53.675646+00:00"
translation_last_reviewed: 2026-01-01
---

סטטוס: טיוטה/סקיצה לליווי משימות מימוש הממשל. המבנים יכולים להשתנות במהלך המימוש. דטרמיניזם ומדיניות RBAC הם אילוצים נורמטיביים; Torii יכולה לחתום/להגיש עסקאות כאשר `authority` ו-`private_key` מסופקים, אחרת הלקוחות בונים ומגישים אל `/transaction`.

סקירה
- כל ה-endpoints מחזירים JSON. בזרימות שמייצרות עסקאות, התשובות כוללות `tx_instructions` - מערך של הוראת שלד אחת או יותר:
  - `wire_id`: מזהה רישום לסוג ההוראה
  - `payload_hex`: בתים של payload Norito (hex)
- אם `authority` ו-`private_key` מסופקים (או `private_key` ב-DTO של ballots), Torii חותמת ומגישה את העסקה ועדיין מחזירה `tx_instructions`.
- אחרת, הלקוחות מרכיבים SignedTransaction עם ה-authority וה-chain_id שלהם, ואז חותמים ושולחים POST אל `/transaction`.
- כיסוי SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` מחזיר `GovernanceProposalResult` (מנרמל שדות status/kind), `ToriiClient.get_governance_referendum_typed` מחזיר `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` מחזיר `GovernanceTally`, `ToriiClient.get_governance_locks_typed` מחזיר `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` מחזיר `GovernanceUnlockStats`, ו-`ToriiClient.list_governance_instances_typed` מחזיר `GovernanceInstancesPage`, תוך אכיפת גישה טיפוסית לכל משטח הממשל עם דוגמאות שימוש ב-README.
- לקוח Python קל (`iroha_torii_client`): `ToriiClient.finalize_referendum` ו-`ToriiClient.enact_proposal` מחזירים חבילות `GovernanceInstructionDraft` טיפוסיות (שעוטפות את שלד `tx_instructions` של Torii), ומונעים parsing ידני של JSON כשסקריפטים מרכיבים זרימות Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` מספק helpers טיפוסיים להצעות, referenda, tallies, locks, unlock stats, וכעת `listGovernanceInstances(namespace, options)` לצד endpoints של council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) כדי שלקוחות Node.js יוכלו למיין `/v1/gov/instances/{ns}` ולהפעיל זרימות מבוססות VRF לצד רשימת אינסטנסים קיימת של חוזים.

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - בקשה (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "alice@wonderland?",
      "private_key": "...?"
    }
  - תשובה (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - ולידציה: הצמתים מקננים `abi_hash` עבור `abi_version` שסופק ודוחים אי התאמה. עבור `abi_version = "v1"`, הערך הצפוי הוא `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API חוזים (deploy)
- POST `/v1/contracts/deploy`
  - בקשה: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - התנהגות: מחשב `code_hash` מגוף תוכנית IVM ו-`abi_hash` מה-header `abi_version`, ואז מגיש `RegisterSmartContractCode` (manifest) ו-`RegisterSmartContractBytes` (בתים מלאים של `.to`) בשם `authority`.
  - תשובה: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - קשור:
    - GET `/v1/contracts/code/{code_hash}` -> מחזיר manifest מאוחסן
    - GET `/v1/contracts/code-bytes/{code_hash}` -> מחזיר `{ code_b64 }`
- POST `/v1/contracts/instance`
  - בקשה: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - התנהגות: מפרס את ה-bytecode שסופק ומפעיל מיד את המיפוי `(namespace, contract_id)` דרך `ActivateContractInstance`.
  - תשובה: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

שירות alias
- POST `/v1/aliases/voprf/evaluate`
  - בקשה: { "blinded_element_hex": "..." }
  - תשובה: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` משקף את מימוש המעריך. הערך הנוכחי: `blake2b512-mock`.
  - הערות: מעריך mock דטרמיניסטי שמיישם Blake2b512 עם הפרדת תחום `iroha.alias.voprf.mock.v1`. מיועד לכלי בדיקה עד שחיבור VOPRF פרודקשן יוחבר ל-Iroha.
  - שגיאות: HTTP `400` ב-input hex פגום. Torii מחזיר מעטפת Norito `ValidationFail::QueryFailed::Conversion` עם הודעת שגיאה מה-decoder.
- POST `/v1/aliases/resolve`
  - בקשה: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - תשובה: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - הערות: דורש ISO bridge runtime staging (`[iso_bridge.account_aliases]` ב-`iroha_config`). Torii מנרמל alias על ידי הסרת רווחים והמרה לאותיות גדולות לפני lookup. מחזיר 404 כאשר ה-alias חסר ו-503 כאשר ה-ISO bridge runtime מושבת.
- POST `/v1/aliases/resolve_index`
  - בקשה: { "index": 0 }
  - תשובה: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - הערות: אינדקסים של alias מוקצים דטרמיניסטית לפי סדר הקונפיגורציה (0-based). לקוחות יכולים לשמור offline כדי לבנות שבילי audit לאירועי attestation של alias.

תקרת גודל קוד
- פרמטר מותאם: `max_contract_code_bytes` (JSON u64)
  - שולט בגודל המקסימלי המותר (בבתים) לאחסון קוד חוזה on-chain.
  - ברירת מחדל: 16 MiB. צמתים דוחים `RegisterSmartContractBytes` כאשר גודל תמונת `.to` חורג מהתקרה עם שגיאת invariant violation.
  - מפעילים יכולים להתאים על ידי `SetParameter(Custom)` עם `id = "max_contract_code_bytes"` ו-payload מספרי.

- POST `/v1/gov/ballots/zk`
  - בקשה: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - תשובה: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות:
    - כאשר ה-inputs הציבוריים של המעגל כוללים `owner`, `amount` ו-`duration_blocks`, וההוכחה מאומתת מול ה-VK המוגדר, הצומת יוצר או מרחיב governance lock עבור `election_id` עם אותו `owner`. הכיוון נשאר מוסתר (`unknown`); רק amount/expiry מתעדכנים. re-votes הם monotonic: amount ו-expiry רק גדלים (הצומת מפעיל max(amount, prev.amount) ו-max(expiry, prev.expiry)).
    - re-votes של ZK שמנסים להקטין amount או expiry נדחים בצד השרת עם אבחונים `BallotRejected`.
    - ביצוע חוזה חייב לקרוא ל-`ZK_VOTE_VERIFY_BALLOT` לפני enqueue של `SubmitBallot`; המארחים אוכפים latch חד פעמי.

- POST `/v1/gov/ballots/plain`
  - בקשה: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - תשובה: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות: re-votes הם extend-only - ballot חדש לא יכול להקטין amount או expiry של lock קיים. ה-`owner` חייב להיות זהה ל-authority של העסקה. משך מינימלי הוא `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - בקשה: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "alice@wonderland?", "private_key": "...?" }
  - תשובה: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - אפקט on-chain (scaffold נוכחי): enact של הצעת deploy מאושרת מוסיף `ContractManifest` מינימלי במפתח `code_hash` עם `abi_hash` הצפוי ומסמן את ההצעה כ-Enacted. אם manifest כבר קיים עבור `code_hash` עם `abi_hash` שונה, ה-enactment נדחה.
  - הערות:
    - עבור בחירות ZK, נתיבי חוזה חייבים לקרוא ל-`ZK_VOTE_VERIFY_TALLY` לפני ביצוע `FinalizeElection`; המארחים אוכפים latch חד פעמי. `FinalizeReferendum` דוחה רפרנדומים מסוג ZK עד שה-tally מסומן כ-finalized.
    - סגירה אוטומטית ב-`h_end` מפיקה Approved/Rejected רק לרפרנדומים Plain; רפרנדומים ZK נשארים Closed עד שה-tally מסומן finalized ומבוצע `FinalizeReferendum`.
    - בדיקות turnout משתמשות רק ב-approve+reject; abstain לא נספר.

- POST `/v1/gov/enact`
  - בקשה: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "alice@wonderland?", "private_key": "...?" }
  - תשובה: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - הערות: Torii מגישה את העסקה החתומה כאשר `authority`/`private_key` מסופקים; אחרת היא מחזירה שלד עבור חתימה והגשה מצד הלקוח. ה-preimage אופציונלי וכרגע אינפורמטיבי.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: מזהה הצעה hex (64 chars)
  - תשובה: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: מחרוזת מזהה referendum
  - תשובה: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - תשובה: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - הערות: מחזיר council שמור כאשר קיים; אחרת גוזר fallback דטרמיניסטי באמצעות נכס ה-stake המוגדר וספים (משקף מפרט VRF עד שיוטמעו הוכחות VRF חיות on-chain).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - בקשה: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - התנהגות: מאמת כל proof של VRF מול input קנוני שמופק מ-`chain_id`, `epoch` ו-beacon של hash הבלוק האחרון; ממיין לפי bytes של הפלט בסדר יורד עם שוברי שוויון; מחזיר את חברי ה-top `committee_size`. לא נשמר.
  - תשובה: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - הערות: Normal = pk ב-G1, proof ב-G2 (96 bytes). Small = pk ב-G2, proof ב-G1 (48 bytes). inputs מופרדים בדומיין וכוללים `chain_id`.

### ברירות מחדל לממשל (iroha_config `gov.*`)

ה-fallback council שמשמש את Torii כאשר אין roster שמור מוגדר דרך `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Overrides סביבתיים שקולים:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` מגביל את מספר חברי ה-fallback כאשר אין council שמור, `parliament_term_blocks` מגדיר את אורך התקופה לנגזרת seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` אוכף מינימום stake (ביחידות הקטנות) על נכס הזכאות, ו-`parliament_eligibility_asset_id` בוחר איזה מאזן נכס נסרק לבניית קבוצת המועמדים.

אימות VK לממשל ללא bypass: אימות ballots תמיד דורש מפתח מאמת `Active` עם bytes inline, והסביבות אינן צריכות להסתמך על toggles בדיקה כדי לדלג על אימות.

RBAC
- ביצוע on-chain דורש הרשאות:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (עתידי): `CanManageParliament`

Namespaces מוגנים
- פרמטר מותאם `gov_protected_namespaces` (JSON array של strings) מפעיל admission gating עבור deploys ל-namespaces שמצוינים.
- לקוחות חייבים לכלול מפתחות metadata בעסקה עבור deploys ל-namespaces מוגנים:
  - `gov_namespace`: ה-namespace היעד (לדוגמה "apps")
  - `gov_contract_id`: מזהה החוזה הלוגי בתוך ה-namespace
- `gov_manifest_approvers`: JSON array אופציונלי של account IDs של מאמתים. כאשר manifest של lane מצהיר על quorum גדול מ-1, admission דורש את ה-authority של העסקה וגם את החשבונות המצוינים כדי לעמוד ב-quorum של ה-manifest.
- טלמטריה חושפת מוני admission דרך `governance_manifest_admission_total{result}` כדי שמפעילים יבדילו בין admits מוצלחים למסלולים `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, ו-`runtime_hook_rejected`.
- טלמטריה חושפת את נתיב האכיפה דרך `governance_manifest_quorum_total{outcome}` (ערכים `satisfied` / `rejected`) כדי שהמפעילים יוכלו לבדוק אישורים חסרים.
- Lanes אוכפים allowlist של namespaces שמפורסם ב-manifests שלהם. כל עסקה שמגדירה `gov_namespace` חייבת לספק `gov_contract_id`, וה-namespace חייב להופיע ב-set `protected_namespaces` של ה-manifest. הגשות `RegisterSmartContractCode` ללא metadata זו נדחות כאשר ההגנה פעילה.
- Admission אוכף שקיים proposal ממשל Enacted עבור tuple `(namespace, contract_id, code_hash, abi_hash)`; אחרת האימות נכשל עם שגיאת NotPermitted.

Hooks לשדרוג runtime
- Manifests של lane יכולים להצהיר על `hooks.runtime_upgrade` כדי לגדר הוראות שדרוג runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- שדות hook:
  - `allow` (bool, ברירת מחדל `true`): כאשר `false`, כל הוראות שדרוג runtime נדחות.
  - `require_metadata` (bool, ברירת מחדל `false`): דורש את רשומת ה-metadata המצוינת על ידי `metadata_key`.
  - `metadata_key` (string): שם ה-metadata הנאכף על ידי hook. ברירת מחדל `gov_upgrade_id` כאשר metadata נדרשת או יש allowlist.
  - `allowed_ids` (array של strings): allowlist אופציונלית של ערכי metadata (לאחר trim). דוחה כאשר הערך שסופק אינו ברשימה.
- כאשר hook קיים, admission של התור אוכף את מדיניות ה-metadata לפני שהעסקה נכנסת לתור. metadata חסרה, ערכים ריקים או ערכים מחוץ ל-allowlist גורמים לשגיאת NotPermitted דטרמיניסטית.
- הטלמטריה עוקבת אחרי outcomes דרך `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- עסקאות שמספקות את ה-hook חייבות לכלול metadata `gov_upgrade_id=<value>` (או המפתח שהוגדר במניפסט) לצד כל אישורי המאמתים שנדרשים על ידי quorum של ה-manifest.

Endpoint נוחות
- POST `/v1/gov/protected-namespaces` - מחיל את `gov_protected_namespaces` ישירות על הצומת.
  - בקשה: { "namespaces": ["apps", "system"] }
  - תשובה: { "ok": true, "applied": 1 }
  - הערות: מיועד לאדמין/בדיקה; דורש API token אם הוגדר. לפרודקשן עדיף לשלוח עסקה חתומה עם `SetParameter(Custom)`.

Helpers ל-CLI
- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - מאחזר אינסטנסים של חוזים ל-namespace ומוודא ש:
    - Torii מאחסנת bytecode לכל `code_hash`, וה-digest Blake2b-32 שלו תואם ל-`code_hash`.
    - ה-manifest המאוחסן תחת `/v1/contracts/code/{code_hash}` מדווח על `code_hash` ו-`abi_hash` תואמים.
    - קיימת הצעת ממשל enacted עבור `(namespace, contract_id, code_hash, abi_hash)` שנגזרת באותו hashing של proposal-id שבו הצומת משתמש.
  - מפיק דוח JSON עם `results[]` לכל חוזה (issues, סיכומי manifest/code/proposal) ועוד סיכום שורה אחת אלא אם הוא מושתק (`--no-summary`).
  - שימושי לאודיט של namespaces מוגנים או לאימות זרימות deploy נשלטות ממשל.
- `iroha gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - מפיק שלד JSON של metadata המשמש בעת שליחת deployments ל-namespaces מוגנים, כולל `gov_manifest_approvers` אופציונלי כדי לעמוד בכללי quorum של ה-manifest.
- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — רמזי lock נדרשים כאשר `min_bond_amount > 0`, וכל סט רמזים שסופק חייב לכלול `owner`, `amount` ו-`duration_blocks`.
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - סיכום שורה אחת מציג כעת `fingerprint=<hex>` דטרמיניסטי שנגזר מ-`CastZkBallot` המוצפן יחד עם hints מפוענחים (`owner`, `amount`, `duration_blocks`, `direction` כאשר סופקו).
  - תשובות CLI מסמנות `tx_instructions[]` עם `payload_fingerprint_hex` ועוד שדות מפוענחים כדי שכלי downstream יוכלו לאמת את השלד בלי ליישם שוב Norito decoding.
  - מתן hints של lock מאפשר לצומת לשדר אירועים `LockCreated`/`LockExtended` עבור ballots של ZK כאשר המעגל יחשוף את אותם ערכים.
- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - ה-aliases `--lock-amount`/`--lock-duration-blocks` משקפים את שמות הדגלים של ZK לשם פריטיות בסקריפטים.
  - פלט הסיכום משקף `vote-zk` בכך שהוא כולל את ה-fingerprint של ההוראה המקודדת ושדות ballot קריאים (`owner`, `amount`, `duration_blocks`, `direction`), ומספק אישור מהיר לפני חתימת השלד.

Listing של אינסטנסים
- GET `/v1/gov/instances/{ns}` - מציג אינסטנסים פעילים של חוזים עבור namespace.
  - Query params:
    - `contains`: סינון לפי substring של `contract_id` (case-sensitive)
    - `hash_prefix`: סינון לפי prefix hex של `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: אחד מ-`cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - תשובה: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) או `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Unlock sweep (מפעיל/ביקורת)
- GET `/v1/gov/unlocks/stats`
  - תשובה: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - הערות: `last_sweep_height` משקף את גובה הבלוק האחרון שבו locks שפג תוקפם נסרקו ונשמרו. `expired_locks_now` מחושב בסריקת רשומות lock עם `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - בקשה (DTO בסגנון v1):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "owner": "alice@wonderland?",
      "nullifier": "blake2b32:...64hex?"
    }
  - תשובה: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - מקבל JSON של `BallotProof` ישירות ומחזיר שלד `CastZkBallot`.
  - בקשה:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 של מיכל ZK1 או H2*
        "root_hint": null,                // optional 32-byte hex string (eligibility root)
        "owner": null,                    // AccountId אופציונלי כאשר המעגל מחייב owner
        "nullifier": null                 // optional 32-byte hex string (nullifier hint)
      }
    }
  - תשובה:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - הערות:
    - השרת ממפה `root_hint`/`owner`/`nullifier` אופציונליים מה-ballot אל `public_inputs_json` עבור `CastZkBallot`.
    - bytes של ה-envelope מקודדים מחדש ב-base64 עבור payload ההוראה.
    - התשובה `reason` משתנה ל-`submitted transaction` כאשר Torii מגישה את ה-ballot.
    - endpoint זה זמין רק כאשר feature `zk-ballot` מופעל.

מסלול אימות CastZkBallot
- `CastZkBallot` מפענח את ה-proof הבסיס64 שסופק ודוחה payloads ריקים או פגומים (`BallotRejected` עם `invalid or empty proof`).
- ה-host פותר את מפתח האימות של ballot מתוך referendum (`vk_ballot`) או ברירות המחדל של הממשל ודורש שהרשומה תתקיים, תהיה `Active`, ותכיל bytes inline.
- bytes של מפתח אימות מאוחסן עוברים hash מחדש עם `hash_vk`; כל אי התאמה ב-commitment עוצרת את הביצוע לפני אימות כדי להגן על רשומות registry משובשות (`BallotRejected` עם `verifying key commitment mismatch`).
- bytes של proof נשלחים ל-backend הרשום דרך `zk::verify_backend`; תמלילים לא תקינים מופיעים כ-`BallotRejected` עם `invalid proof` וההוראה נכשלת דטרמיניסטית.
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- proofs מוצלחים פולטים `BallotAccepted`; nullifiers כפולים, שורשי זכאות מיושנים או רגרסיות lock ממשיכים להניב את סיבות הדחיה הקיימות שתוארו לעיל.

## התנהגות שגויה של מאמתים וקונצנזוס משותף

### זרימת Slashing ו-Jailing

הקונצנזוס מפיק `Evidence` בקידוד Norito כאשר מאמת מפר את הפרוטוקול. כל payload נכנס ל-`EvidenceStore` בזיכרון ואם טרם נראה הוא מתממש למפת `consensus_evidence` הנתמכת ב-WSV. רשומות ישנות מ-`sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת מחדל `7200` בלוקים) נדחות כדי להשאיר את הארכיון מוגבל, אך הדחיה נרשמת עבור המפעילים.

עבירות מוכרות ממופות אחד-לאחד ל-`EvidenceKind`; המבדילים יציבים ומאולצים על ידי מודל הנתונים:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - המאמת חתם על hashes מתנגשים עבור אותו tuple `(phase,height,view,epoch)`.
- **InvalidQc** - aggregate פרסם commit certificate שהמבנה שלו נכשל בבדיקות דטרמיניסטיות (למשל bitmap חתימות ריק).
- **InvalidProposal** - מנהיג הציע בלוק שנכשל באימות מבני (למשל מפר את כלל locked-chain).
- **Censorship** — signed submission receipts show a transaction that was never proposed/committed.

מפעילים וכלים יכולים לבדוק ולשדר מחדש payloads דרך:

- Torii: `GET /v1/sumeragi/evidence` ו-`GET /v1/sumeragi/evidence/count`.
- CLI: `iroha sumeragi evidence list`, `... count`, ו-`... submit --evidence-hex <payload>`.

הממשל חייב להתייחס ל-bytes של evidence כהוכחה קנונית:

1. **לאסוף את ה-payload** לפני שהוא מתיישן. לארכב את bytes Norito הגולמיים לצד metadata של height/view.
2. **להכין את הענישה** על ידי הטמעת ה-payload במשאל עם או הוראת sudo (למשל `Unregister::peer`). הביצוע מאמת מחדש את ה-payload; evidence פגום או stale נדחה דטרמיניסטית.
3. **לקבוע טופולוגיית המשך** כך שהמאמת הפוגע לא יוכל להצטרף מיד מחדש. זרימות טיפוסיות משרשרות `SetParameter(Sumeragi::NextMode)` ו-`SetParameter(Sumeragi::ModeActivationHeight)` עם roster מעודכן.
4. **לאמת תוצאות** דרך `/v1/sumeragi/evidence` ו-`/v1/sumeragi/status` כדי לוודא שמונה ה-evidence התקדם והממשל ביצע את ההסרה.

### רצף קונצנזוס משותף

קונצנזוס משותף מבטיח שקבוצת המאמתים היוצאת מסיימת את בלוק הגבול לפני שהקבוצה החדשה מתחילה להציע. ה-runtime אוכף את הכלל באמצעות פרמטרים מזווגים:

- `SumeragiParameter::NextMode` ו-`SumeragiParameter::ModeActivationHeight` חייבים להתחייב באותו **בלוק**. `mode_activation_height` חייב להיות גדול באופן מוחלט מגובה הבלוק שנשא את העדכון, וכך מספק לפחות בלוק lag אחד.
- `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) הוא guard קונפיגורציה שמונע hand-offs ללא lag:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- ה-runtime וה-CLI מציגים פרמטרים staged דרך `/v1/sumeragi/params` ו-`iroha sumeragi params --summary`, כדי שמפעילים יאמתו גבהי הפעלה ורוסטרים של מאמתים.
- אוטומצית הממשל חייבת תמיד:
  1. לסיים החלטת הסרה (או השבה) שמגובה ב-evidence.
  2. לשגר reconfiguration המשך עם `mode_activation_height = h_current + activation_lag_blocks`.
  3. לנטר `/v1/sumeragi/status` עד ש-`effective_consensus_mode` מתחלף בגובה הצפוי.

כל סקריפט שמבצע רוטציות מאמתים או slashing **לא חייב** לנסות הפעלה ללא lag או להשמיט את פרמטרי ה-hand-off; עסקאות כאלה נדחות ומשאירות את הרשת במצב הקודם.

## משטחי טלמטריה

- מדדי Prometheus מייצאים פעילות ממשל:
  - `governance_proposals_status{status}` (gauge) עוקב אחרי ספירות proposals לפי status.
  - `governance_protected_namespace_total{outcome}` (counter) עולה כאשר admission של namespaces מוגנים מאפשר או דוחה deploy.
  - `governance_manifest_activations_total{event}` (counter) רושם הוספות manifest (`event="manifest_inserted"`) ו-bindings של namespace (`event="instance_bound"`).
- `/status` כולל אוביקט `governance` שמשקף ספירות proposals, מדווח על totals של namespaces מוגנים ומציג activations אחרונים של manifest (namespace, contract id, code/ABI hash, block height, activation timestamp). מפעילים יכולים לסקור שדה זה כדי לאשר שה-enactments עדכנו manifests וששערי namespaces מוגנים נאכפים.
- תבנית Grafana (`docs/source/grafana_governance_constraints.json`) וה-runbook של טלמטריה ב-`telemetry.md` מראים איך לחבר התראות עבור proposals תקועות, חסרים של manifest activations, או דחיות לא צפויות של namespaces מוגנים בזמן runtime upgrades.