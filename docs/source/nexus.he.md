---
lang: he
direction: rtl
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי עבור docs/source/nexus.md -->

#! Iroha 3 - Sora Nexus Ledger: מפרט תכנון טכני

מסמך זה מציע את ארכיטקטורת Sora Nexus Ledger עבור Iroha 3, ומקדם את Iroha 2 אל ספר חשבונות גלובלי יחיד המאורגן סביב Data Spaces ‏(DS). Data Spaces מספקים דומייני פרטיות חזקים ("private data spaces") והשתתפות פתוחה ("public data spaces"). העיצוב שומר על קומפוזביליות בלדג'ר הגלובלי תוך הבטחת בידוד קפדני וסודיות לנתוני DS פרטיים, ומוסיף סקיילינג לזמינות נתונים באמצעות erasure coding ב-Kura (אחסון בלוקים) וב-WSV (World State View).

אותו ריפוזיטורי בונה גם את Iroha 2 (רשתות בהוסט עצמי) וגם את Iroha 3 (SORA Nexus). ההרצה נשענת על Iroha Virtual Machine ‏(IVM) וכלי Kotodama המשותפים, ולכן חוזים וארטיפקטים של bytecode נשארים ניידים בין פריסות בהוסט עצמי לבין הלדג'ר הגלובלי של Nexus.

מטרות
- לדג'ר לוגי גלובלי אחד המורכב מהרבה ולידטורים משתפי פעולה ומרחבי נתונים.
- Data Spaces פרטיים להפעלה מורשית (למשל CBDC), כאשר הנתונים לא יוצאים מה-DS הפרטי.
- Data Spaces ציבוריים עם השתתפות פתוחה וגישה permissionless בסגנון Ethereum.
- חוזים חכמים קומפוזביליים בין Data Spaces, בכפוף להרשאות מפורשות לגישה לנכסי DS פרטיים.
- בידוד ביצועים כך שפעילות ציבורית לא תפגע בטרנזקציות פנימיות של DS פרטיים.
- זמינות נתונים בקנה מידה: Kura ו-WSV עם erasure coding לתמיכה בנתונים כמעט בלתי מוגבלים תוך שמירה על פרטיות DS פרטיים.

אי-מטרות (שלב התחלתי)
- הגדרת כלכלת טוקנים או תמריצי ולידטורים; תזמון וסטייקינג הם פלג-אין.
- הוספת גרסת ABI חדשה או הרחבת משטחי syscalls/pointer-ABI; ABI v1 קבוע ו-runtime upgrades לא משנים את ABI ה-host.

מונחים
- Nexus Ledger: הלדג'ר הלוגי הגלובלי הנוצר מהרכבת בלוקים של Data Space להיסטוריה יחידה וסדורה ומחויבות מצב.
- Data Space (DS): דומיין הרצה ואחסון תחום עם ולידטורים, ממשל, מחלקת פרטיות, מדיניות DA, מכסות ומדיניות fees משלו. קיימות שתי מחלקות: DS ציבורי ו-DS פרטי.
- Private Data Space: ולידטורים מורשים ובקרת גישה; נתוני טרנזקציה ומצב אינם יוצאים מה-DS. רק commitments/metadata מעוגנים גלובלית.
- Public Data Space: השתתפות permissionless; נתונים מלאים ומצב זמינים לציבור.
- Data Space Manifest (DS Manifest): מניפסט Norito שמצהיר פרמטרים של DS (validators/QC keys, privacy class, ISI policy, DA params, retention, quotas, ZK policy, fees). ה-hash של המניפסט מעוגן בשרשרת nexus. כברירת מחדל DS quorum certificates משתמשים ב-ML-DSA-87 (Dilithium5 class) כחתימה פוסט-קוונטית.
- Space Directory: חוזה רישום on-chain גלובלי שעוקב אחרי מניפסטים, גרסאות ואירועי ממשל/רוטציה לצורך רזולוציה וביקורת.
- DSID: מזהה גלובלי ייחודי ל-Data Space. משמש ל-namespacing של כל האובייקטים וההפניות.
- Anchor: מחויבות קריפטוגרפית מבלוק/כותרת של DS הנכללת בשרשרת nexus לקשירת היסטוריית DS ללדג'ר הגלובלי.
- Kura: אחסון בלוקים של Iroha. מורחב כאן עם blob storage ו-commitments ב-erasure coding.
- WSV: World State View של Iroha. מורחב עם סגמנטים של מצב ממוספרים ותמיכת snapshots ב-erasure coding.
- IVM: Iroha Virtual Machine להרצת חוזים חכמים (Kotodama bytecode `.to`).
 - AIR: Algebraic Intermediate Representation. תיאור אלגברי של חישוב להוכחות בסגנון STARK, המתאר הרצה כ-traces שדות עם אילוצי מעבר וגבול.

מודל Data Spaces
- זהות: `DataSpaceId (DSID)` מזהה DS ומייצר namespace לכל דבר. DS יכול להיווצר בשתי רמות גרנולריות:
  - Domain-DS: `ds::domain::<domain_name>` - הרצה ומצב בתחום הדומיין.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - הרצה ומצב להגדרת נכס יחידה.
  שתי הצורות מתקיימות יחד; טרנזקציות יכולות לגעת במספר DSID באופן אטומי.
- מחזור חיים של מניפסט: יצירת DS, עדכונים (רוטציית מפתחות, שינויי מדיניות) ופרישה נרשמים ב-Space Directory. כל ארטיפקט לכל slot מפנה ל-hash המניפסט העדכני.
- מחלקות: DS ציבורי (participation פתוח, DA ציבורי) ו-DS פרטי (מורשה, DA סודי). מדיניות היברידית אפשרית דרך flags במניפסט.
- מדיניות לכל DS: הרשאות ISI, פרמטרי DA `(k,m)`, הצפנה, retention, מכסות (min/max tx per block), מדיניות ZK/optimistic proofs, fees.
- ממשל: חברות DS ורוטציית ולידטורים מוגדרות בסעיף הממשל של המניפסט (on-chain proposals, multisig, או ממשל חיצוני המעוגן בטרנזקציות nexus וב-attestations).

Gossip מודע dataspace
- קבוצות gossip של טרנזקציות נושאות תג plane (public vs restricted) שמגיע מ-catalog של lanes; קבוצות restricted נשלחות ב-unicast ל-peer-ים אונליין בטופולוגיית commit הנוכחית (בהתאם ל-`transaction_gossip_restricted_target_cap`) בעוד קבוצות public משתמשות ב-`transaction_gossip_public_target_cap` (קבע `null` לשידור). בחירת יעדים מתערבלת לפי הקצב של `transaction_gossip_public_target_reshuffle_ms` ו-`transaction_gossip_restricted_target_reshuffle_ms` (ברירת מחדל: `transaction_gossip_period_ms`). כאשר אין peer-ים אונליין בטופולוגיית commit, אפשר לבחור לסרב או להעביר payloads מוגבלים ל-overlay הציבורי דרך `transaction_gossip_restricted_public_payload` (ברירת מחדל `refuse`); הטלמטריה חושפת ניסיונות fallback, ספירות forward/drop והמדיניות יחד עם בחירת יעדים לפי dataspace.
- dataspaces לא ידועים נבדקים מחדש בתור כשהדגל `transaction_gossip_drop_unknown_dataspace` מופעל; אחרת הם נופלים ל-targeting מוגבל כדי למנוע דליפה.
- אימות בצד קבלה מפיל קבוצות שה-lanes/dataspaces שלהן לא תואמים לקטלוג המקומי או שתג ה-plane לא תואם ל-visibilty הנגזר של dataspace.

Manifests של capabilities ו-UAID
- חשבונות אוניברסליים: לכל משתתף מוקצה UAID דטרמיניסטי (`UniversalAccountId` ב-`crates/iroha_data_model/src/nexus/manifest.rs`) שחוצה את כל ה-dataspaces. manifests של capabilities (`AssetPermissionManifest`) קושרים UAID ל-dataspace ספציפי, epochs של הפעלה/תוקף, ורשימה מסודרת של כללי allow/deny `ManifestEntry` שמגבילים `dataspace`, `program_id`, `method`, `asset` ותפקידי AMX אופציונליים. כללי deny תמיד גוברים; evaluator מחזיר `ManifestVerdict::Denied` עם סיבת ביקורת או grant `Allowed` עם metadata של ההרשאה.
- Snapshots של פורטפוליו UAID נחשפים דרך `GET /v1/accounts/{uaid}/portfolio` (ראו `docs/source/torii/portfolio_api.md`), מגובים באגרגטור דטרמיניסטי ב-`iroha_core::nexus::portfolio`.
- Allowances: לכל allow יש buckets דטרמיניסטיים `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) ועוד `max_amount` אופציונלי. Hosts ו-SDKs צורכים אותו Norito payload כך שהאכיפה זהה בין חומרות ו-SDKs.
- טלמטריית ביקורת: Space Directory משדר `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) בכל שינוי מצב. `SpaceDirectoryEventFilter` מאפשר למנויי Torii/data-event לעקוב אחרי עדכונים, ביטולים והחלטות deny-wins בלי plumbing ידני.

### פעולות מניפסט UAID

פעולות Space Directory זמינות בשתי צורות: CLI מובנה (ל-rollouts סקריפטיים) או שליחות Torii ישירות (ל-CI/CD אוטומטי). שני המסלולים אוכפים את `CanPublishSpaceDirectoryManifest{dataspace}` בתוך executor (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`) ורושמים אירועי מחזור חיים ב-world state (`iroha_core::state::space_directory_manifests`).

#### זרימת CLI (`iroha space-directory manifest ...`)

1. **קידוד JSON של מניפסט** — להמיר טיוטות מדיניות ל-Norito bytes ולהפיק hash שחוזר על עצמו לפני סקירה:

   ```bash
   iroha space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   ה-helper מקבל `--json` (JSON גולמי) או `--manifest` (payload `.to`) ומשקף את הלוגיקה ב-`crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **פרסום/החלפת מניפסטים** — לשגר `PublishSpaceDirectoryManifest` ממקורות Norito או JSON:

   ```bash
   iroha space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` ממלא `entries[*].notes` עבור רשומות בלי הערות מפעיל.

3. **Expirer** מניפסטים שהגיעו לסוף חיים או **Revoke** UAIDs לפי צורך. שני הפקודות מקבלות `--uaid uaid:<hex>` ו-id מספרי של dataspace:

   ```bash
   iroha space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **יצירת bundle לביקורת** — `manifest audit-bundle` כותב את ה-JSON, ה-`.to`, ה-hash, פרופיל dataspace ו-metadata קריא מכונה לתיקיית יציאה:

   ```bash
   iroha space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   ה-bundle מטמיע hooks של `SpaceDirectoryEvent` מהפרופיל כדי להוכיח שה-dataspace חושף webhooks חובה; ראו `docs/space-directory.md` למבנה שדות ודרישות ראיה.

#### Torii APIs

מפעילים ו-SDKs יכולים לבצע את אותן פעולות דרך HTTPS. Torii אוכף את אותן הרשאות וחותם על טרנזקציות עבור הסמכות שסופקה (מפתחות פרטיים נשארים בזיכרון בתוך handler מאובטח של Torii):

- `GET /v1/space-directory/uaids/{uaid}` — לפתור binding נוכחי של dataspace ל-UAID (כתובות מנורמלות, ids של dataspace, program bindings). הוסיפו `address_format=compressed` לפלט Sora Name Service.
- `GET /v1/space-directory/uaids/{uaid}/portfolio` — אגרגטור Norito שממפה ל-`ToriiClient.getUaidPortfolio` כדי שארנקים יציגו נכסים אוניברסליים בלי גרידת מצב לכל dataspace.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` — להביא JSON של המניפסט, metadata של מחזור חיים וה-hash לביקורת.
- `POST /v1/space-directory/manifests` — לשלוח מניפסט חדש או מחליף מ-JSON (`authority`, `private_key`, `manifest`, `reason` אופציונלי). Torii מחזיר `202 Accepted` כשהטרנזקציה בתור.
- `POST /v1/space-directory/manifests/revoke` — לשגר ביטולי חירום עם UAID, dataspace id, epoch אפקטיבי ו-reason אופציונלי (משקף CLI).

SDK JS (`javascript/iroha_js/src/toriiClient.js`) כבר עוטף את ממשקי הקריאה הללו דרך `ToriiClient.getUaidPortfolio`, `.getUaidBindings`, ו-`.getUaidManifests`; גרסאות Swift/Python עתידיות ישתמשו באותם payloads. עיינו ב-`docs/source/torii/portfolio_api.md` לסכמות request/response וב-`docs/space-directory.md` ל-playbook המלא.

עדכוני SDK/AMX אחרונים
- **NX-11 (אימות relay cross-lane):** עוזרי SDK מאמתים כעת את lane relay envelopes מ-`/v1/sumeragi/status`. לקוח Rust מספק `iroha::nexus` helpers לבניה/אימות relay proofs ולדחיית כפילויות `(lane_id, dataspace_id, height)`, ה-binding של Python חושף `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`, ו-SDK JS מספק `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample` כדי שהאופרטורים יוודאו הוכחות cross-lane לפני העברה.
  【crates/iroha/src/nexus.rs:1】【python/iroha_python/iroha_python_rs/src/lib.rs:666】【crates/iroha_js_host/src/lib.rs:640】【javascript/iroha_js/src/nexus.js:1】
- **NX-17 (guardrails לתקציב AMX):** `ivm::analysis::enforce_amx_budget` מעריך עלות ביצוע לפי dataspace וקבוצה באמצעות דוח ניתוח סטטי ומאכף תקציבי 30 ms / 140 ms. ה-helper מדווח הפרות בצורה ברורה ומכוסה בבדיקות יחידה, מה שהופך את תקציב הסלוט ל-דטרמיניסטי עבור scheduler של Nexus וכלי SDK.
  【crates/ivm/src/analysis.rs:142】【crates/ivm/src/analysis.rs:241】

ארכיטקטורה ברמה גבוהה
1) שכבת קומפוזיציה גלובלית (Nexus Chain)
- שומרת סדר קנוני אחד של Nexus Blocks בקצב 1 s שמסיימים טרנזקציות אטומיות על פני DS אחד או יותר. כל טרנזקציה committed מעדכנת את ה-world state הגלובלי (וקטור שורשים לפי DS).
- כוללת metadata מינימלית ועוד proofs/QC מצטברים להבטחת קומפוזביליות, סופיות וזיהוי הונאה (DSIDs שנגעו, שורשי מצב לפני/אחרי, commitments של DA, proofs של DS, ו-DS QC ב-ML-DSA-87). אין נתונים פרטיים.
- קונצנזוס: ועדת BFT גלובלית בקצב 22 (3f+1 עם f=7), נבחרת באפוק באמצעות VRF/stake מתוך עד ~200k מועמדים. הוועדה מסדרת טרנזקציות ומסיימת בלוק בתוך 1 s.

2) שכבת Data Space (Public/Private)
- מבצעת חלקי DS של טרנזקציות גלובליות, מעדכנת WSV מקומי ומפיקה artifacts של תקפות לכל בלוק (proofs של DS ו-DA commitments) שמצטברים ל-Nexus Block בקצב 1 s.
- DS פרטיים מצפינים נתונים במנוחה ובמעבר; רק commitments והוכחות PQ יוצאות החוצה.
- DS ציבוריים מפרסמים את גוף הנתונים (דרך DA) והוכחות PQ.

3) טרנזקציות אטומיות Cross-Data-Space (AMX)
- מודל: טרנזקציה יכולה לגעת במספר DS (למשל domain DS ו-asset DS). היא committed אטומית בבלוק Nexus של 1 s או מתבטלת.
- Prepare-Commit בתוך 1 s: DS נוגעים מריצים במקביל על אותו snapshot ומייצרים proofs PQ (FASTPQ-ISI) ו-DA commitments. ועדת nexus מאשרת רק אם כל proofs ו-DA certificates הגיעו (יעד <=300 ms); אחרת נדחה לסלוט הבא.
- עקביות: read-write sets מצוינים; זיהוי קונפליקטים מתבצע מול שורשי הסלוט. הרצה אופטימית ללא נעילות; האטומיות נאכפת ע"י כלל commit של nexus.
- פרטיות: DS פרטיים מייצאים רק proofs/commitments הקשורים לשורשי pre/post. נתונים פרטיים לא יוצאים.

4) Data Availability (DA) עם erasure coding
- Kura מאחסן גופי בלוקים ו-snapshots של WSV כ-blobs עם erasure coding. Blobs ציבוריים מפוזרים; Blobs פרטיים נשמרים רק אצל ולידטורים פרטיים עם chunks מוצפנים.
- DA commitments נרשמים בארטיפקטים ובבלוקים ומאפשרים דגימה ושחזור בלי חשיפת תוכן פרטי.

מבנה בלוק ו-commit
- Data Space Proof Artifact (לסלוט 1 s, לכל DS)
  - שדות: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - DS פרטיים מייצאים ארטיפקטים ללא גופי נתונים; DS ציבוריים מאפשרים שליפה דרך DA.

- Nexus Block (קצב 1 s)
  - שדות: block_number, parent_hash, slot_time, tx_list (טרנזקציות אטומיות cross-DS עם DSIDs נוגעים), ds_artifacts[], nexus_qc.
  - פונקציה: מסיים טרנזקציות אטומיות שהארטיפקטים שלהן מאומתים; מעדכן את וקטור שורשי ה-DS בצעד אחד.

קונצנזוס ותזמון
- Nexus Chain Consensus: BFT גלובלי ב-pipeline (מחלקת Sumeragi) עם ועדה של 22 (3f+1, f=7) לבלוק 1 s וסופיות 1 s. בחירת חברים באפוק באמצעות VRF/stake מתוך ~200k מועמדים.
- Data Space Consensus: כל DS מריץ BFT משלו ומפיק artifacts לפי slot. ועדות lane-relay נקבעות ל-`3f+1` לפי `fault_tolerance` ומדגימות דטרמיניסטית לפי epoch עם seed של VRF הקשור ל-`(dataspace_id, lane_id)`. DS פרטיים מורשים; DS ציבוריים פתוחים עם אנטי-Sybil. הוועדה הגלובלית נשארת זהה.
- תזמון טרנזקציות: משתמשים מצהירים על DSIDs ו-read/write sets. DS מריצים במקביל; הוועדה מכניסה לבלוק אם ה-artifacts וה-DA certificates בזמן (<=300 ms).
- בידוד ביצועים: mempools והרצה נפרדים לכל DS. מכסות פר-DS מגבילות כמה טרנזקציות לכל בלוק כדי למנוע head-of-line blocking ולהגן על לטנטיות DS פרטיים.

Data Model ו-namespacing
- מזהים מוסמכים DS: דומיינים, חשבונות, נכסים ורולים מסומנים ב-`dsid`. לדוגמה: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global References: טופל `(dsid, object_id, version_hint)` שניתן להניח בשכבת nexus או ב-AMX descriptors.
- Norito Serialization: כל הודעות cross-DS משתמשות ב-Norito. אין serde ב-production.

Smart Contracts והרחבות IVM
- Execution Context: הוספת `dsid` לקונטקסט ההרצה של IVM. חוזי Kotodama רצים בתוך DS ספציפי.
- פרימיטיבים אטומיים cross-DS:
  - `amx_begin()` / `amx_commit()` מסמנים טרנזקציה אטומית multi-DS ב-host של IVM.
  - `amx_touch(dsid, key)` מצהיר כוונת read/write לזיהוי קונפליקטים מול roots של snapshot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (מותר רק אם המדיניות מאפשרת וה-handle תקף)
- Asset Handles ו-fees:
  - פעולות נכס מאושרות לפי ISI/role policies של DS; fees משולמים בגז-טוקן של DS. ניתן להוסיף tokens ו-policy עשיר יותר מאוחר.
- דטרמיניזם: syscalls הם טהורים ודטרמיניסטיים בהינתן הקלטים ו-read/write sets. אין אפקטי זמן או סביבה.

Post-Quantum Validity Proofs (Generalized ISIs)
- FASTPQ-ISI (PQ, ללא trusted setup): ארגומנט מבוסס hash המרחיב את דגם ה-transfer לכל ISI ומכוון להוכחות תת-שניות בקנה מידה 20k על חומרת GPU.
  - פרופיל תפעולי:
    - `fastpq_prover::Prover::canonical` מאתחל backend פרודקשן; ה-mock הדטרמיניסטי הוסר.【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode` ו-`irohad --fastpq-execution-mode` מאפשרים לנעול CPU/GPU דטרמיניסטית; ה-hook מתעד requested/resolved/backend.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- Arithmetization:
  - KV-Update AIR: WSV כמפת KV טיפוסית עם Poseidon2-SMT; כל ISI הופך למספר שורות read-check-write.
  - Opcode-gated constraints: טבלת AIR אחת עם selector columns שמאכפת כללי ISI.
  - Lookup arguments: טבלאות hash-committed לשמירת permissions/roles, precision של assets ופרמטרי policy.
- Commitments ועדכונים:
  - Aggregated SMT Proof: הוכחה לכל המפתחות שנגעו (pre/post) מול `old_root`/`new_root`.
  - Invariants: אינвариנטים גלובליים (למשל supply כולל) נאכפים דרך שוויון multiset.
- Proof system:
  - DEEP-FRI commitments עם arity 8/16 ו-blow-up 8-16; Poseidon2; Fiat-Shamir עם SHA-2/3.
  - Recursion אופציונלית לאגרגציה מקומית.
- Scope:
  - Assets: transfer, mint, burn, register/unregister, set precision, set metadata.
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories.
  - Roles/Permissions: grant/revoke, עם lookup tables ו-policy checks.
  - Contracts/AMX: AMX begin/commit, capability mint/revoke.
- Checks מחוץ ל-AIR:
  - חתימות וקריפטוגרפיה כבדה מאומתות ע"י DS validators ומאושרות ב-DS QC.
- יעדי ביצועים:
  - 20k ISIs: ~0.4-0.9 s, ~150-450 KB proof, ~5-15 ms verify.
  - ISIs כבדים: micro-batch + recursion לשמירה על <1 s לסלוט.
- DS Manifest config:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false`
  - `attestation.qc_signature = "ml_dsa_87"`
- Fallbacks:
  - ISIs מורכבים יכולים להשתמש ב-`zk.policy = "stark_fri_general"` עם הוכחה מושהית.
  - אפשרויות לא-PQ (כמו Plonk עם KZG) דורשות trusted setup ואינן נתמכות בבילד ברירת מחדל.

AIR Primer (ל-Nexus)
- Execution trace: מטריצה של רוחב (עמודות רשומות) ואורך (צעדים). כל שורה היא צעד לוגי.
- Constraints:
  - Transition: יחסי שורה-לשורה (למשל post_balance = pre_balance - amount כאשר `sel_transfer = 1`).
  - Boundary: קישור public I/O (old_root/new_root, counters) לשורה הראשונה/אחרונה.
  - Lookups: חברות ושוויון multiset מול טבלאות commitments.
- Commitment/Verification:
  - Prover מתחייב ל-traces ומבנה פולינומים; Verifier מאמת low-degree עם FRI.
- Example (Transfer): pre_balance, amount, post_balance, nonce, selectors, ומולטי-proofs ל-old/new roots.

יציבות ABI (ABI v1)
- משטח ABI v1 קבוע; לא מוסיפים syscalls או pointer-ABI types חדשים במהדורה זו.
- runtime upgrades חייבים להשאיר `abi_version = 1` עם `added_syscalls`/`added_pointer_types` ריקים.
- ABI goldens (syscall list/ABI hash/IDs) מקובעים ואסור שישתנו.

מודל פרטיות
- Private Data Containment: נתוני טרנזקציה, diffs ו-snapshots של DS פרטי לא יוצאים.
- Public Exposure: רק headers, DA commitments ו-PQ proofs נחשפים.
- ZK Proofs אופציונליים: מאפשרים cross-DS בלי חשיפת מצב פנימי.
- בקרת גישה: ISI/role policies ב-DS. capability tokens אופציונליים.

בידוד ביצועים ו-QoS
- קונצנזוס, mempools ואחסון נפרדים לכל DS.
- מכסות Nexus per-DS כדי להגביל זמן inclusion ולהימנע מ-head-of-line blocking.
- תקציבי משאבים per-DS enforced ב-IVM host; עומס ציבורי לא אוכל תקציבים של DS פרטי.
- קריאות cross-DS אסינכרוניות כדי להימנע מהמתנה סינכרונית ארוכה.

Data Availability ואחסון
1) Erasure Coding
- Reed-Solomon שיטתי לבלוקים ו-snapshots עם `(k, m)` ו-`n = k + m`.
- ברירות מחדל: Public DS `k=32, m=16`, Private DS `k=16, m=8`.
- Public Blobs: shards מבוזרים עם בדיקות זמינות מדגמיות.
- Private Blobs: shards מוצפנים רק אצל ולידטורים פרטיים; בשרשרת נשמרים commitments בלבד.

2) Commitments ו-Sampling
- לכל blob מחושבת root של Merkle ונכללת ב-`*_da_commitment`.
- DA Attesters מונעי VRF מנפיקים ML-DSA-87 certificates.

3) Kura Integration
- גופי טרנזקציות נשמרים כ-blobs עם commitments.
- headers נושאים commitments; שליפה דרך DA או ערוצים פרטיים.

4) WSV Integration
- snapshots תקופתיים עם commitments, ושמירת change logs בין snapshots.
- הוכחות מצב (Merkle/Verkle) או ZK attestations ל-DS פרטיים.

5) Retention/Pruning
- Public DS ללא pruning; Private DS עם retention פנימי, אך commitments בלתי ניתנים לשינוי.

Networking ו-Node Roles
- Global Validators: קונצנזוס nexus, אימות בלוקים ו-DS artifacts, בדיקות DA ציבוריות.
- Data Space Validators: קונצנזוס DS, הרצת חוזים, ניהול Kura/WSV, DA ל-DS.
- DA Nodes: אחסון/שיתוף blobs, דגימה. ב-DS פרטיים הם co-located עם validators.

System-Level Improvements
- DAG mempool + pipeline BFT לשיפור latencies.
- quotas per-DS לצמצום head-of-line blocking.
- PQ attestation כברירת מחדל, עם אפשרות חלופות.
- DA attesters אזוריים ב-VRF.
- Recursion לצמצום proofs.
- Lane scaling עם merge דטרמיניסטי.
- SIMD/CUDA עם fallback דטרמיניסטי.
- Thresholds להפעלת lanes לפי עומס.

Fees וכלכלה
- Gas unit per-DS; fees משולמים ב-gas asset של DS.
- Priority round-robin; בתוך DS fee bidding.
- אפשרות לשוק fees גלובלי בעתיד.

Cross-Data-Space Workflow (Example)
1) משתמש שולח AMX עם DS ציבורי P ו-DS פרטי S.
2) P ו-S מריצים פרגמנטים מול snapshot ומפיקים proofs.
3) ועדת nexus מאשרת; הטרנזקציה committed בבלוק 1 s.
4) אם proof/DA חסרים, הטרנזקציה מתבטלת וניתן לשדר מחדש.

- Considerations של אבטחה
- syscalls דטרמיניסטיים; תוצאות cross-DS נשענות על AMX commit ולא על זמן.
- ISI permissions מגבילות הרשאות; capability tokens אופציונליים.
- הצפנת end-to-end והוכחות ZK אופציונליות.
- בידוד שכבות מגן מפני DoS.

שינויים ברכיבי Iroha
- `iroha_data_model`: הוספת `DataSpaceId`, AMX descriptors, DA commitments.
- `ivm`: משטח ABI v1 קבוע (ללא syscalls/pointer-ABI חדשים); AMX/runtime upgrades משתמשים בפרימיטיבים v1 קיימים; ABI goldens מקובעים.
- `iroha_core`: nexus scheduler, Space Directory, AMX validation, DS artifacts.
- `kura`/`WSV`/`irohad`: עדכוני אחסון, snapshots, רשת ו-config.

Configuration and Determinism
- הכל דרך `iroha_config` ללא env toggles.
- האצות חומרה עם fallback דטרמיניסטי.
- - Post-Quantum default: PQ proofs ו-ML-DSA-87 ברירת מחדל.

### Runtime Lane Lifecycle Control

- **Admin endpoint:** `POST /v1/nexus/lifecycle` עם `additions` ו-`retire` לעדכון lanes ללא restart.
- **Behaviour:** עדכון WSV/Kura, rebuild של queue, תשובה `{ ok: true, lane_count: <u32> }`.
- **Safety:** שימוש ב-lock משותף למניעת מרוצים.
- **TODO:** rebalance, propagation, cleanup טרם הושלם.

Migration Path (Iroha 2 -> Iroha 3)
1) IDs לפי dataspace ו-global state composition.
2) Kura/WSV erasure coding מאחורי feature flags.
3) שמירה על משטח ABI v1 קבוע; יישום AMX ללא syscalls/pointer types חדשים; עדכון tests/docs בלי שינוי ABI.
4) nexus chain מינימלי עם DS ציבורי אחד.
5) הרחבה ל-AMX מלא ו-ML-DSA-87 בכל DS.

Testing Strategy
- Unit tests, IVM tests, integration tests, security tests בהתאם.

### NX-18 Telemetry & Runbook Assets

- Dashboard Grafana, CI gate, evidence bundler, release automation, runbook ו-helpers כפי שמפורט.

Open Questions
1) חתימות טרנזקציה והסכמות capabilities.
2) כלכלת gas ונתיב המרה.
3) DA attesters ו-thresholds.
4) פרמטרי DA ברירת מחדל.
5) גרנולריות DS והיררכיה.
6) ISIs כבדים.
7) read/write sets לעומת inference.

נספח: תאימות למדיניות ריפוזיטורי
- Norito לכל wire/JSON, ABI v1 בלבד; משטח syscalls/pointer-ABI קבוע; דטרמיניזם, ללא serde וללא env config ב-production.

</div>
