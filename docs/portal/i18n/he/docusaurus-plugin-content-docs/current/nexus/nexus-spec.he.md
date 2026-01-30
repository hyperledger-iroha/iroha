---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ce1d8e4711c90db1e5c2cc61cf454555a19263ddfba73f8a85e83d7f8b40f2f
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-spec
title: מפרט טכני של Sora Nexus
description: מראה מלאה של `docs/source/nexus.md`, המכסה את הארכיטקטורה ומגבלות התכנון ל‑Iroha 3 (Sora Nexus).
---

:::note מקור קנוני
העמוד הזה משקף את `docs/source/nexus.md`. שמרו על שתי הגרסאות מסונכרנות עד שהבקלאג של התרגומים יגיע לפורטל.
:::

#! Iroha 3 - Sora Nexus Ledger: מפרט תכנון טכני

המסמך מציע את ארכיטקטורת Sora Nexus Ledger עבור Iroha 3, ומפתח את Iroha 2 אל ledger גלובלי יחיד ומאוחד לוגית המאורגן סביב Data Spaces (DS). Data Spaces מספקים דומיינים חזקים של פרטיות ("private data spaces") והשתתפות פתוחה ("public data spaces"). התכנון שומר על composability לאורך ה‑ledger הגלובלי תוך הבטחת בידוד מחמיר וסודיות לנתוני private‑DS, ומציג סקיילינג של זמינות נתונים באמצעות קידוד מחיקה ב‑Kura (block storage) וב‑WSV (World State View).

אותו ריפו בונה הן את Iroha 2 (רשתות self‑hosted) והן את Iroha 3 (SORA Nexus). הביצוע מונע על ידי Iroha Virtual Machine (IVM) משותף ו‑Kotodama toolchain, ולכן חוזים וארטיפקטים של bytecode נשארים ניידים בין פריסות self‑hosted לבין ה‑ledger הגלובלי של Nexus.

יעדים
- ledger לוגי גלובלי יחיד המורכב ממספר רב של ולידטורים ו‑Data Spaces משתפים פעולה.
- Private Data Spaces לפעולה permissioned (למשל CBDC), עם נתונים שאינם יוצאים מה‑DS הפרטי.
- Public Data Spaces עם השתתפות פתוחה, גישה ללא הרשאות בסגנון Ethereum.
- חוזים חכמים composable בין Data Spaces, בכפוף להרשאות מפורשות לגישה לנכסי private‑DS.
- בידוד ביצועים כך שפעילות ציבורית לא תפגע בעסקאות פנימיות של private‑DS.
- זמינות נתונים בקנה מידה: Kura ו‑WSV עם קידוד מחיקה כדי לתמוך בנתונים כמעט בלתי מוגבלים תוך שמירת נתוני private‑DS פרטיים.

לא מטרות (שלב ראשוני)
- הגדרת כלכלת טוקן או תמריצי ולידטורים; מדיניות scheduling ו‑staking הן pluggable.
- הצגת גרסת ABI חדשה; השינויים מכוונים ל‑ABI v1 עם הרחבות syscalls ו‑pointer‑ABI לפי מדיניות IVM.

מונחים
- Nexus Ledger: ה‑ledger הלוגי הגלובלי שנוצר מהלחמת בלוקים של Data Space (DS) להיסטוריה מסודרת אחת ו‑state commitment.
- Data Space (DS): דומיין ביצוע ואחסון תחום עם ולידטורים, governance, class של פרטיות, מדיניות DA, quotas ומדיניות עמלות משלו. קיימות שתי מחלקות: public DS ו‑private DS.
- Private Data Space: ולידטורים permissioned ובקרת גישה; נתוני טרנזקציה ומצב לא יוצאים מה‑DS. רק commitments/metadata מעוגנים גלובלית.
- Public Data Space: השתתפות ללא הרשאות; כל הנתונים והמצב פומביים.
- Data Space Manifest (DS Manifest): manifest מקודד Norito שמכריז על פרמטרים של DS (validators/QC keys, class פרטיות, מדיניות ISI, פרמטרי DA, retention, quotas, מדיניות ZK, עמלות). hash של ה‑manifest מעוגן ב‑nexus chain. אלא אם צוין אחרת, תעודות quorum של DS משתמשות ב‑ML‑DSA‑87 (מחלקת Dilithium5) כברירת מחדל של חתימה post‑quantum.
- Space Directory: חוזה directory גלובלי on‑chain שעוקב אחר manifests של DS, גרסאות ואירועי governance/rotation לצורך resolvability וביקורות.
- DSID: מזהה ייחודי גלובלי ל‑Data Space. משמש ל‑namespacing של כל האובייקטים וההפניות.
- Anchor: commitment קריפטוגרפי מכותרת/בלוק DS שנכלל ב‑nexus chain כדי לקשור את היסטוריית ה‑DS ל‑ledger הגלובלי.
- Kura: אחסון בלוקים של Iroha. כאן מורחב ל‑erasure‑coded blob storage ו‑commitments.
- WSV: Iroha World State View. כאן מורחב לקטעי מצב ממוינים בגרסאות, עם snapshots, ומקודדים במחיקה.
- IVM: Iroha Virtual Machine לביצוע חוזים חכמים (Kotodama bytecode `.to`).
  - AIR: Algebraic Intermediate Representation. ייצוג אלגברי של חישוב עבור הוכחות בסגנון STARK, המתאר ביצוע כ‑traces מבוססי שדות עם מגבלות מעבר וגבול.

מודל Data Spaces
- זהות: `DataSpaceId (DSID)` מזהה DS ונותן namespacing לכל דבר. ניתן לאתחל DS בשתי רמות:
  - Domain-DS: `ds::domain::<domain_name>` - ביצוע ומצב המוגבלים לדומיין.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ביצוע ומצב המוגבלים להגדרת נכס אחת.
  שתי הצורות מתקיימות; עסקאות יכולות לגעת במספר DSID באופן אטומי.
- מחזור חיי manifest: יצירת DS, עדכונים (רוטציית מפתחות, שינויי מדיניות) ופרישה נרשמים ב‑Space Directory. כל artifact של DS לכל slot מפנה ל‑hash של ה‑manifest האחרון.
- Classes: Public DS (השתתפות פתוחה, DA ציבורי) ו‑Private DS (permissioned, DA חסוי). מדיניות היברידית אפשרית דרך flags ב‑manifest.
- מדיניות לכל DS: הרשאות ISI, פרמטרי DA `(k,m)`, הצפנה, retention, quotas (חלק יחסי מינ/מקס של tx לכל בלוק), מדיניות ZK/optimistic proofs, עמלות.
- Governance: חברות DS ורוטציה של ולידטורים מוגדרות בסעיף ה‑governance של ה‑manifest (הצעות on‑chain, multisig, או governance חיצוני שמעוגן על ידי טרנזקציות nexus ו‑attestations).

Capability manifests ו‑UAID
- Universal accounts: כל משתתף מקבל UAID דטרמיניסטי (`UniversalAccountId` ב‑`crates/iroha_data_model/src/nexus/manifest.rs`) שנמתח על כל dataspaces. Capability manifests (`AssetPermissionManifest`) קושרים UAID ל‑dataspace ספציפי, epochs של הפעלה/תפוגה ורשימה מסודרת של כללי allow/deny `ManifestEntry` שמגבילים `dataspace`, `program_id`, `method`, `asset` ותפקידי AMX אופציונליים. כללי deny תמיד מנצחים; ה‑evaluator מחזיר `ManifestVerdict::Denied` עם סיבת audit או `Allowed` עם allowance metadata תואם.
- Allowances: כל רשומת allow נושאת buckets דטרמיניסטיים של `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) ועוד `max_amount` אופציונלי. Hosts ו‑SDK צורכים את אותו Norito payload, כך שה‑enforcement זהה בין חומרה ו‑SDK.
- Audit telemetry: Space Directory משדר `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) בכל שינוי מצב של manifest. משטח `SpaceDirectoryEventFilter` החדש מאפשר למנויי Torii/data-event לנטר עדכוני UAID manifest, ביטולים והחלטות deny‑wins ללא plumbing מותאם.

עבור ראיות תפעול end‑to‑end, הערות מיגרציה ל‑SDK ורשימות פרסום manifest, שמרו את הסעיף הזה מסונכרן עם Universal Account Guide (`docs/source/universal_accounts_guide.md`). שמרו על שני המסמכים מיושרים כאשר מדיניות UAID או הכלים משתנים.

ארכיטקטורה ברמת‑על
1) שכבת קומפוזיציה גלובלית (Nexus Chain)
- שומרת סדר קנוני יחיד של Nexus Blocks בני 1 שניה שמסיימים טרנזקציות אטומיות שנוגעות ב‑Data Spaces (DS) אחד או יותר. כל טרנזקציה committed מעדכנת את ה‑world state הגלובלי המאוחד (וקטור roots לכל DS).
- כוללת מטא‑דאטה מינימלית plus proofs/QCs מצטברים כדי להבטיח composability, finality ו‑fraud detection (DSIDs שנגעו, roots לכל DS לפני/אחרי, DA commitments, DS validity proofs, ותעודת quorum ל‑DS עם ML‑DSA‑87). אין נתונים פרטיים.
- Consensus: ועדת BFT גלובלית בצינור של 22 (3f+1 עם f=7), נבחרת מתוך עד ~200k מועמדים באמצעות VRF/stake בכל epoch. ועדת nexus מסדרת טרנזקציות ומסיימת את הבלוק תוך 1s.

2) שכבת Data Space (Public/Private)
- מבצעת פרגמנטים לכל DS של טרנזקציות גלובליות, מעדכנת WSV מקומי ומפיקה artifacts של validitiy לכל בלוק (proofs per‑DS מצטברים ו‑DA commitments) שמתגלגלים ל‑Nexus Block של 1s.
- Private DS מצפינים נתונים במנוחה ובתנועה בין ולידטורים מורשים; רק commitments ו‑PQ validity proofs יוצאים מה‑DS.
- Public DS מייצאים bodies מלאים (דרך DA) ו‑PQ validity proofs.

3) טרנזקציות אטומיות cross‑Data‑Space (AMX)
- מודל: כל טרנזקציה יכולה לגעת בכמה DS (למשל domain DS ו‑asset DS). היא committed אטומית ב‑Nexus Block יחיד או מבוטלת; ללא אפקטים חלקיים.
- Prepare‑Commit בתוך 1s: עבור כל טרנזקציה, DS שנוגעים בה מבצעים במקביל על אותו snapshot (roots בתחילת slot) ומפיקים PQ validity proofs לכל DS (FASTPQ‑ISI) ו‑DA commitments. ועדת nexus מחייבת רק אם כל proofs נבדקות ותעודות DA מגיעות בזמן (יעד <=300 ms); אחרת הטרנזקציה תוזמן מחדש ל‑slot הבא.
- Consistency: read/write sets מוכרזים; זיהוי קונפליקט בעת commit מול roots של תחילת slot. ביצוע optimistic ללא locks לכל DS מונע stalls גלובליים; atomicity נאכפת ע"י כלל nexus commit (הכל או כלום בין DS).
- Privacy: Private DS מייצאים רק proofs/commitments המחוברים ל‑pre/post roots. אין יציאה של נתונים פרטיים גולמיים.

4) זמינות נתונים (DA) עם קידוד מחיקה
- Kura שומרת block bodies ו‑WSV snapshots כ‑erasure‑coded blobs. blobs ציבוריים מפוזרים נרחב; blobs פרטיים נשמרים רק אצל private‑DS validators עם chunks מוצפנים.
- DA commitments נרשמים גם ב‑DS artifacts וגם ב‑Nexus Blocks, מה שמאפשר sampling ו‑recovery guarantees בלי לחשוף תוכן פרטי.

מבנה בלוק ו‑commit
- Data Space Proof Artifact (לכל slot של 1s, לכל DS)
  - שדות: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML‑DSA‑87), ds_validity_proof (FASTPQ‑ISI).
  - Private‑DS מייצא artifacts ללא bodies; Public DS מאפשרת שליפה דרך DA.

- Nexus Block (קצב 1s)
  - שדות: block_number, parent_hash, slot_time, tx_list (טרנזקציות אטומיות cross‑DS עם DSIDs), ds_artifacts[], nexus_qc.
  - פונקציה: מסיים את כל הטרנזקציות האטומיות שה‑DS artifacts שלהן נבדקו; מעדכן את וקטור DS roots של world state הגלובלי בצעד אחד.

Consensus ו‑scheduling
- Nexus Chain Consensus: BFT גלובלי בצינור (Sumeragi-class) עם ועדה של 22 (3f+1, f=7) עם יעד blocks 1s ו‑finality 1s. הוועדה נבחרת ב‑epochs דרך VRF/stake מתוך ~200k מועמדים; רוטציה שומרת על ביזור ועמידות לצנזורה.
- Data Space Consensus: כל DS מריץ BFT משלו כדי להפיק per-slot artifacts (proofs, DA commitments, DS QC). committees של lane‑relay נמדדים ב‑`3f+1` לפי `fault_tolerance` של dataspace ומדגמים דטרמיניסטית לכל epoch מתוך מאגר הוולידטורים עם VRF seed הקשור ל‑`(dataspace_id, lane_id)`. Private DS הם permissioned; Public DS מאפשרים liveness פתוח עם מדיניות anti‑Sybil. הוועדה הגלובלית של nexus לא משתנה.
- Transaction Scheduling: משתמשים שולחים טרנזקציות אטומיות עם DSIDs ומערכי read/write. DS מבצעים במקביל בתוך slot; ועדת nexus כוללת את הטרנזקציה בבלוק 1s אם כל DS artifacts מאומתים ותעודות DA בזמן (<=300 ms).
- Performance Isolation: לכל DS יש mempools וביצוע עצמאי. quotas per‑DS מגבילות כמה טרנזקציות הנוגעות ב‑DS ניתן commit לכל בלוק כדי למנוע head‑of‑line blocking ולהגן על latency של private DS.

מודל נתונים ו‑namespacing
- DS‑Qualified IDs: כל היישויות (domains, accounts, assets, roles) מסומנות ב‑`dsid`. דוגמה: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global References: רפרנס גלובלי הוא tuple `(dsid, object_id, version_hint)` וניתן להציבו on‑chain בשכבת nexus או ב‑AMX descriptors לשימוש cross‑DS.
- Norito Serialization: כל הודעות cross‑DS (AMX descriptors, proofs) משתמשות ב‑Norito codecs. לא משתמשים ב‑serde במסלולי production.

Smart Contracts ו‑IVM Extensions
- Execution Context: להוסיף `dsid` להקשר הביצוע של IVM. חוזי Kotodama תמיד מבוצעים בתוך Data Space ספציפי.
- Atomic Cross‑DS Primitives:
  - `amx_begin()` / `amx_commit()` מסמנים טרנזקציה אטומית multi‑DS ב‑IVM host.
  - `amx_touch(dsid, key)` מצהיר על read/write intent לזיהוי קונפליקטים מול snapshot roots של slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (מותר רק אם המדיניות מאפשרת וה‑handle תקין)
- Asset Handles and Fees:
  - פעולות נכסים מאושרות לפי מדיניות ISI/role של ה‑DS; עמלות משולמות ב‑gas token של ה‑DS. capability tokens אופציונליים ומדיניות עשירה יותר (multi‑approver, rate‑limits, geofencing) ניתן להוסיף בהמשך בלי לשנות את המודל האטומי.
- Determinism: כל syscalls החדשות טהורות ודטרמיניסטיות עבור קלטים ומערכי read/write AMX שהוגדרו. אין השפעות חבויות של זמן או סביבה.

Post‑Quantum Validity Proofs (ISI מוכללים)
- FASTPQ‑ISI (PQ, ללא trusted setup): טיעון hash‑based שמכליל את מודל transfer לכל משפחות ISI עם יעד הוכחה תת‑שניה עבור batches בקנה מידה 20k על חומרת GPU.
  - פרופיל תפעולי:
    - Nodes של production בונים prover דרך `fastpq_prover::Prover::canonical`, שמאתחל תמיד את backend ה‑production; mock דטרמיניסטי הוסר. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) ו‑`irohad --fastpq-execution-mode` מאפשרים להצמיד CPU/GPU בצורה דטרמיניסטית, בזמן שה‑observer hook רושם triple של requested/resolved/backend עבור audits. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmetization:
  - KV‑Update AIR: מתייחס ל‑WSV כמפת key‑value טיפוסית שמקבלת commitment באמצעות Poseidon2‑SMT. כל ISI מתרחב למספר קטן של שורות read‑check‑write על מפתחות (accounts, assets, roles, domains, metadata, supply).
  - Opcode‑gated constraints: טבלת AIR יחידה עם selector columns שמיישמת כללי ISI (conservation, monotonic counters, permissions, range checks, עדכוני metadata מוגבלים).
  - Lookup arguments: טבלאות שקופות המתחייבות בהאש עבור permissions/roles, precisions של assets ו‑policy parameters כדי להימנע ממגבלות bitwise כבדות.
- State commitments and updates:
  - Aggregated SMT Proof: כל המפתחות שנגעו בהם (pre/post) מוכחים מול `old_root`/`new_root` עם frontier דחוס ו‑siblings deduped.
  - Invariants: אינוואריאנטים גלובליים (למשל total supply לכל asset) נאכפים באמצעות multiset equality בין effect rows למונים מעוקבים.
- Proof system:
  - FRI‑style polynomial commitments (DEEP‑FRI) עם arity גבוה (8/16) ו‑blow‑up 8‑16; Poseidon2 hashes; Fiat‑Shamir transcript עם SHA‑2/3.
  - Recursion אופציונלי: aggregation מקומי ב‑DS כדי לדחוס micro‑batches להוכחה אחת לכל slot לפי הצורך.
- Scope ודוגמאות:
  - Assets: transfer, mint, burn, register/unregister asset definitions, set precision (bounded), set metadata.
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories (state‑only; בדיקות חתימה מאושרות בידי DS validators ולא בתוך AIR).
  - Roles/Permissions (ISI): grant/revoke roles ו‑permissions; נאכפים דרך lookup tables ובדיקות מדיניות מונוטונית.
  - Contracts/AMX: AMX begin/commit markers, capability mint/revoke אם מופעל; מוכחים כ‑state transitions ומוני policy.
- Out‑of‑AIR checks לשמירת latency:
  - חתימות וקריפטו כבד (למשל ML‑DSA user signatures) נבדקים ע"י DS validators ומאושרים ב‑DS QC; ה‑validity proof מכסה רק עקביות מצב ו‑policy compliance. כך נשמרות proofs PQ ומהירות.
- Performance targets (דוגמה, CPU 32‑core + GPU מודרני):
  - 20k ISIs מעורבים עם key‑touch קטן (<=8 keys/ISI): כ‑0.4‑0.9 s prove, כ‑150‑450 KB proof, כ‑5‑15 ms verify.
  - ISIs כבדים יותר: micro‑batch (למשל 10x2k) + recursion כדי לשמור <1 s לכל slot.
- DS Manifest configuration:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (חתימות נבדקות ב‑DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (ברירת מחדל; חלופות חייבות להיות מוצהרות)
- Fallbacks:
  - ISIs מורכבות/מותאמות יכולות להשתמש ב‑STARK כללי (`zk.policy = "stark_fri_general"`) עם הוכחה דחויה ו‑finality של 1 s דרך QC attestation + slashing על הוכחות שגויות.
  - אפשרויות non‑PQ (כגון Plonk עם KZG) דורשות trusted setup ואינן נתמכות עוד בבילד ברירת מחדל.

AIR Primer (עבור Nexus)
- Execution trace: מטריצה עם רוחב (register columns) ואורך (steps). כל שורה היא שלב לוגי בעיבוד ISI; העמודות מחזיקות ערכי pre/post, selectors ו‑flags.
- Constraints:
  - Transition constraints: כופים יחסים בין שורות (למשל post_balance = pre_balance - amount בשורת חיוב כאשר `sel_transfer = 1`).
  - Boundary constraints: מקבעים public I/O (old_root/new_root, counters) לשורה הראשונה/האחרונה.
  - Lookups/permutations: מבטיחים membership ו‑multiset equality מול טבלאות committed (permissions, asset params) ללא מעגלים כבדים של ביטים.
- Commitment and verification:
  - Prover מתחייב ל‑traces באמצעות hash‑based encodings ובונה פולינומים בדרגה נמוכה התקפים אם המגבלות מתקיימות.
  - Verifier בודק low‑degree דרך FRI (hash‑based, post‑quantum) עם מספר פתיחות Merkle קטן; העלות לוגריתמית במספר ה‑steps.
- Example (Transfer): הרשומות כוללות pre_balance, amount, post_balance, nonce ו‑selectors. המגבלות מבטיחות אי‑שליליות/טווח, שימור ומונוטוניות nonce, בעוד aggregated SMT multi‑proof מחבר leaves pre/post ל‑old/new roots.

ABI and Syscall Evolution (ABI v1)
- Syscalls להוספה (שמות לדוגמה):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Pointer‑ABI Types להוספה:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- עדכונים נדרשים:
  - הוספה ל‑`ivm::syscalls::abi_syscall_list()` (לשמור על סדר), gating לפי policy.
  - מיפוי מספרים לא ידועים ל‑`VMError::UnknownSyscall` ב‑hosts.
  - עדכון tests: syscall list golden, ABI hash, pointer type ID goldens, policy tests.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

מודל פרטיות
- Private Data Containment: bodies של טרנזקציות, state diffs ו‑WSV snapshots של private DS אינם יוצאים מקבוצת ה‑private validators.
- Public Exposure: רק headers, DA commitments ו‑PQ validity proofs מיוצאים.
- Optional ZK Proofs: private DS יכולים להפיק ZK proofs (למשל, יתרה מספקת, עמידה במדיניות) כדי לאפשר פעולות cross‑DS בלי לחשוף מצב פנימי.
- Access Control: הרשאה נאכפת על ידי מדיניות ISI/role בתוך ה‑DS. capability tokens אופציונליים וניתנים להוספה בהמשך.

Performance Isolation and QoS
- consensus, mempools ו‑storage נפרדים לכל DS.
- Nexus scheduling quotas per DS כדי להגביל זמני inclusion של anchors ולמנוע head‑of‑line blocking.
- budgets של משאבי חוזה per DS (compute/memory/IO) נאכפים ע"י IVM host. תחרות ב‑public DS לא יכולה לצרוך budgets של private DS.
- קריאות cross‑DS אסינכרוניות מונעות המתנות סינכרוניות ארוכות בתוך ביצוע private‑DS.

Data Availability and Storage Design
1) Erasure Coding
- שימוש ב‑systematic Reed‑Solomon (לדוגמה GF(2^16)) עבור blob‑level erasure coding של Kura blocks ו‑WSV snapshots: פרמטרים `(k, m)` עם `n = k + m` shards.
- פרמטרים דיפולטיים (public DS): `k=32, m=16` (n=48), מאפשרים שחזור עד 16 shards אבודים עם ~1.5x הרחבה. ל‑private DS: `k=16, m=8` (n=24) בתוך הקבוצה permissioned. שניהם ניתנים להגדרה ב‑DS Manifest.
- Public blobs: shards מפוזרים על פני DA nodes/validators רבים עם בדיקות זמינות באמצעות sampling. DA commitments ב‑headers מאפשרים ל‑light clients לאמת.
- Private blobs: shards מוצפנים ומופצים רק בתוך private‑DS validators (או custodians). השרשרת הגלובלית נושאת רק DA commitments (ללא מיקומי shards או מפתחות).

2) Commitments and Sampling
- עבור כל blob: חישוב Merkle root מעל shards והוספתו ל‑`*_da_commitment`. שמירה על PQ תוך הימנעות מ‑elliptic‑curve commitments.
- DA Attesters: attesters אזוריים שנדגמים ב‑VRF (למשל 64 לכל אזור) מנפיקים תעודת ML‑DSA‑87 המאשרת sampling מוצלח. יעד latency ל‑DA attestation <=300 ms. ועדת nexus מאמתת תעודות במקום למשוך shards.

3) Kura Integration
- בלוקים שומרים bodies של טרנזקציות כ‑erasure‑coded blobs עם Merkle commitments.
- headers נושאים blob commitments; bodies נשלפים דרך DA network עבור public DS ובערוצים פרטיים עבור private DS.

4) WSV Integration
- WSV Snapshotting: תקופתית checkpoint למצב DS ל‑chunked, erasure‑coded snapshots עם commitments ב‑headers. בין snapshots נשמרים change logs. snapshots ציבוריים מפוזרים רחב; private snapshots נשארים אצל private validators.
- Proof‑Carrying Access: חוזים יכולים לספק (או לבקש) הוכחות מצב (Merkle/Verkle) המעוגנות ב‑snapshot commitments. Private DS יכולים לספק attestations של zero‑knowledge במקום proofs גולמיים.

5) Retention and Pruning
- אין pruning עבור public DS: שומרים את כל Kura bodies ו‑WSV snapshots דרך DA (סקיילינג אופקי). Private DS יכולים להגדיר retention פנימי, אך commitments מיוצאים נשארים בלתי ניתנים לשינוי. שכבת nexus שומרת את כל Nexus Blocks ואת commitments של DS artifacts.

Networking and Node Roles
- Global Validators: משתתפים ב‑nexus consensus, מאמתים Nexus Blocks ו‑DS artifacts, ומבצעים DA checks עבור public DS.
- Data Space Validators: מפעילים DS consensus, מריצים חוזים, מנהלים Kura/WSV מקומי, ומטפלים ב‑DA עבור ה‑DS שלהם.
- DA Nodes (אופציונלי): מאחסנים/מפרסמים public blobs ומסייעים ב‑sampling. עבור private DS, nodes של DA ממוקמים עם validators או custodians מהימנים.

System‑Level Improvements and Considerations
- Sequencing/mempool decoupling: אימוץ DAG mempool (למשל Narwhal‑style) להזנת pipelined BFT ב‑nexus layer כדי להוריד latency ולשפר throughput בלי לשנות את המודל הלוגי.
- DS quotas and fairness: quotas per‑DS per‑block ו‑weight caps כדי למנוע head‑of‑line blocking ולהבטיח latency צפויה עבור private DS.
- DS attestation (PQ): DS quorum certificates משתמשים ב‑ML‑DSA‑87 (Dilithium5) כברירת מחדל. זה post‑quantum וגדול יותר מ‑EC signatures אך סביר ל‑QC אחד לכל slot. DS יכולים לבחור ML‑DSA‑65/44 (קטנים יותר) או EC signatures אם הוצהר ב‑DS Manifest; public DS מומלצים לשמור על ML‑DSA‑87.
- DA attesters: עבור public DS, להשתמש ב‑VRF‑sampled regional attesters שמנפיקים DA certificates. ועדת nexus מאמתת תעודות במקום sampling גולמי; private DS שומרים attestations פנימיים.
- Recursion and epoch proofs: אופציונלית, aggregation של micro‑batches בתוך DS להוכחה רקורסיבית אחת לכל slot/epoch כדי לשמור על גודל proof וזמן verify יציבים תחת עומס גבוה.
- Lane scaling (אם צריך): אם ועדה גלובלית יחידה הופכת לצוואר בקבוק, להציג K lanes של sequencing במקביל עם merge דטרמיניסטי. זה שומר על סדר גלובלי יחיד ומאפשר סקיילינג אופקי.
- Deterministic acceleration: לספק kernels SIMD/CUDA עם feature flags עבור hashing/FFT עם fallback CPU bit‑exact כדי לשמור על דטרמיניזם בין חומרות.
- Lane activation thresholds (proposal): להפעיל 2‑4 lanes אם (a) p95 finality עולה על 1.2 s במשך >3 דקות רצופות, או (b) תפוסה per‑block עולה על 85% במשך >5 דקות, או (c) קצב tx נכנס דורש >1.2x מקיבולת הבלוק לאורך זמן. Lanes מחלקות טרנזקציות באופן דטרמיניסטי לפי hash של DSID וממזגות ב‑nexus block.

Fees and Economics (Initial Defaults)
- Gas unit: per‑DS gas token עם compute/IO metering; fees משולמות ב‑gas asset native של ה‑DS. המרה בין DS היא אחריות האפליקציה.
- Inclusion priority: round‑robin בין DS עם quotas לכל DS כדי לשמור fairness ו‑1s SLOs; בתוך DS, fee bidding יכול לשבור שוויון.
- Future: ניתן לשקול שוק fees גלובלי או מדיניות שמפחיתה MEV בלי לשנות atomicity או עיצוב proofs PQ.

Cross‑Data‑Space Workflow (Example)
1) משתמש שולח טרנזקציית AMX הנוגעת public DS P ו‑private DS S: להעביר asset X מ‑S למוטב B שהחשבון שלו ב‑P.
2) בתוך ה‑slot, P ו‑S מבצעים את הפרגמנט שלהם על snapshot. S בודק הרשאה וזמינות, מעדכן מצב פנימי ומייצר PQ validity proof ו‑DA commitment (ללא דליפת נתונים פרטיים). P מכין עדכון מצב תואם (למשל mint/burn/locking ב‑P לפי מדיניות) ואת ההוכחה שלו.
3) ועדת nexus מאמתת את שתי ה‑DS proofs ואת DA certificates; אם שתיהן מאומתות בתוך slot, הטרנזקציה committed אטומית ב‑Nexus Block של 1s ומעדכנת את roots של שני ה‑DS בווקטור ה‑world state הגלובלי.
4) אם proof או DA certificate חסרים/לא תקינים, הטרנזקציה מתבטלת (ללא אפקטים) והלקוח יכול להגיש מחדש ל‑slot הבא. נתונים פרטיים של S לא יוצאים באף שלב.

- Security Considerations
- Deterministic Execution: syscalls של IVM נשארים דטרמיניסטיים; תוצאות cross‑DS נקבעות על ידי AMX commit ו‑finality ולא על ידי שעון או זמן רשת.
- Access Control: הרשאות ISI ב‑private DS מגבילות מי יכול לשלוח טרנזקציות ואילו פעולות מותרות. capability tokens מקודדים זכויות עדינות לשימוש cross‑DS.
- Confidentiality: הצפנה end‑to‑end לנתוני private‑DS, shards מקודדים במחיקה נשמרים רק בקרב חברים מורשים, ו‑ZK proofs אופציונליים ל‑external attestations.
- DoS Resistance: בידוד בשכבות mempool/consensus/storage מונע congestion ציבורי מהשפעה על התקדמות private DS.

שינויים ברכיבי Iroha
- iroha_data_model: להכניס `DataSpaceId`, DS‑qualified identifiers, AMX descriptors (read/write sets), סוגי proof/DA commitments. Serialisation רק Norito.
- ivm: להוסיף syscalls ו‑pointer‑ABI types ל‑AMX (`amx_begin`, `amx_commit`, `amx_touch`) ו‑DA proofs; לעדכן ABI tests/docs לפי מדיניות v1.
