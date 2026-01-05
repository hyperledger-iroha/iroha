---
lang: he
direction: rtl
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8990e19977e7f3fb370b9c8f66064542135fa171
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/runtime_upgrades.md -->

# שדרוגי Runtime (IVM + Host) — ללא השבתה, ללא Hardfork

מסמך זה מגדיר מנגנון דטרמיניסטי ומבוקר-ממשל להוספת יכולות IVM/host חדשות (למשל syscalls חדשים וסוגי pointer-ABI) ללא עצירת הרשת או hardfork. הצמתים מפיצים בינארים מראש; ההפעלה מתואמת on-chain בתוך חלון גובה תחום. חוזים ישנים ממשיכים לפעול ללא שינוי; יכולות חדשות מוגנות לפי גרסת ABI ומדיניות.

הערה (מהדורה ראשונה): רק ABI v1 נתמך. מניפסטים של שדרוג runtime לגרסאות ABI אחרות נדחים עד למהדורה עתידית שתוסיף גרסה חדשה.

מטרות
- הפעלה דטרמיניסטית בחלון גובה מתוזמן עם יישום אידמפוטנטי.
- דו-קיום של מספר גרסאות ABI; לעולם לא לשבור בינארים קיימים.
- סייגי קבלה וביצוע כך ש-payloads לפני ההפעלה לא יפעילו התנהגות חדשה.
- פריסה ידידותית למפעילים עם שקיפות יכולות ומצבי כשל ברורים.

לא-מטרות
- שינוי מספרי syscall קיימים או מזהי סוגי pointer (אסור).
- תיקון צמתים בלייב ללא פריסת בינארים מעודכנים.

הגדרות
- גרסת ABI: מספר קטן המוצהר ב-`ProgramMetadata.abi_version` הבוחר `SyscallPolicy` ורשימת allowlist של סוגי pointer.
- ABI Hash: digest דטרמיניסטי של משטח ה-ABI עבור גרסה נתונה: רשימת syscalls (מספרים+מבנים), מזהי סוגי pointer/allowlist ודגלי מדיניות; מחושב על ידי `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: מיפוי host שמחליט האם מספר syscall מותר עבור גרסת ABI ומדיניות host נתונות.
- חלון הפעלה: מקטע גובה בלוק חצי-פתוח `[start, end)` שבו ההפעלה תקפה בדיוק פעם אחת ב-`start`.

אובייקטי מצב (מודל נתונים)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 של bytes Norito קנוניים של manifest.
- שדות `RuntimeUpgradeManifest`:
  - `name: String` — תווית קריאה לבני אדם.
  - `description: String` — תיאור קצר למפעילים.
  - `abi_version: u16` — גרסת ABI יעד להפעלה.
  - `abi_hash: [u8; 32]` — hash ABI קנוני למדיניות היעד.
  - `added_syscalls: Vec<u16>` — מספרי syscall שנעשים תקפים עם גרסה זו.
  - `added_pointer_types: Vec<u16>` — מזהי סוגי pointer שנוספו בשדרוג.
  - `start_height: u64` — גובה הבלוק הראשון שבו ההפעלה מותרת.
  - `end_height: u64` — גבול עליון בלעדי לחלון ההפעלה.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — digests של SBOM לארטיפקטים של שדרוג.
  - `slsa_attestation: Vec<u8>` — bytes גולמיים של SLSA attestation (base64 ב-JSON).
  - `provenance: Vec<ManifestProvenance>` — חתימות על ה-payload הקנוני.
- שדות `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest` — payload הצעה קנוני.
  - `status: RuntimeUpgradeStatus` — מצב מחזור חיים של ההצעה.
  - `proposer: AccountId` — הרשות שהגישה את ההצעה.
  - `created_height: u64` — גובה בלוק שבו ההצעה נכנסה ללדג'ר.
- שדות `RuntimeUpgradeSbomDigest`:
  - `algorithm: String` — מזהה אלגוריתם digest.
  - `digest: Vec<u8>` — bytes גולמיים של digest (base64 ב-JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - אינוואריאנטים: `end_height > start_height`; `abi_version` גדול строго מכל גרסה פעילה; `abi_hash` חייב להיות שווה ל-`ivm::syscalls::compute_abi_hash(policy_for(abi_version))`; `added_*` חייב לרשום בדיוק את הדלתא האדיטיבית בין מדיניות ה-ABI החדשה לקודמת; מספרים/IDs קיימים אסור להסיר או למספר מחדש.

פריסת אחסון
- `world.runtime_upgrades`: מפת MVCC עם מפתח `RuntimeUpgradeId.0` (hash גולמי של 32 בתים) וערכים המקודדים כ-payloads Norito קנוניים מסוג `RuntimeUpgradeRecord`. הרשומות נשמרות בין בלוקים; commits אידמפוטנטיים ובטוחים ל-replay.

הוראות (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - אפקטים: הוספת `RuntimeUpgradeRecord { status: Proposed }` עם מפתח `RuntimeUpgradeId` אם לא קיים.
  - דחייה אם החלון חופף ל- Proposed/Activated אחר או אם האינוואריאנטים נכשלים.
  - אידמפוטנטי: שליחה מחדש של אותם bytes קנוניים היא no-op.
  - קידוד קנוני: bytes של manifest חייבים להתאים ל-`RuntimeUpgradeManifest::canonical_bytes()`; קידודים לא קנוניים נדחים.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - תנאי קדם: קיים Proposed תואם; `current_height` חייב להיות `manifest.start_height`; `current_height < manifest.end_height`.
  - אפקטים: שינוי record ל-`ActivatedAt(current_height)`; הוספת `abi_version` לסט ABI הפעיל.
  - אידמפוטנטי: replays בגובה זהה הם no-op; גבהים אחרים נדחים דטרמיניסטית.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - תנאי קדם: הסטטוס Proposed ו-`current_height < manifest.start_height`.
  - אפקטים: שינוי ל-`Canceled`.

אירועים (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

כללי קבלה
- קבלת חוזים: במהדורה הראשונה מתקבל רק `ProgramMetadata.abi_version = 1`; ערכים אחרים נדחים עם `IvmAdmissionError::UnsupportedAbiVersion`.
  - עבור ABI v1, מחשבים מחדש `abi_hash(1)` ודורשים התאמה ל-payload/manifest כאשר מסופק; אחרת דחייה עם `IvmAdmissionError::ManifestAbiHashMismatch`.
- קבלת טרנזקציות: ההוראות `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` דורשות הרשאות מתאימות (root/sudo); חייבות לעמוד במגבלות חפיפה של חלונות.

אכיפת provenance
- manifests של runtime-upgrade יכולים לשאת digests של SBOM (`sbom_digests`), bytes של SLSA attestation (`slsa_attestation`), ומטא-דאטה של חותמים (חתימות `provenance`). החתימות מכסות את `RuntimeUpgradeManifestSignaturePayload` הקנוני (כל שדות ה-manifest חוץ מרשימת חתימות `provenance`).
- תצורת הממשל שולטת באכיפה תחת `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (מקבל חוסר provenance, בודק אם קיים) או `required` (דוחה אם חסר provenance).
  - `require_sbom`: כאשר `true`, נדרש לפחות digest SBOM אחד.
  - `require_slsa`: כאשר `true`, נדרש SLSA attestation לא ריק.
  - `trusted_signers`: רשימת מפתחות ציבוריים של חותמים מאושרים.
  - `signature_threshold`: מספר מינימלי של חתימות אמינות נדרש.
- דחיות provenance מציגות קודי שגיאה יציבים בכשלי הוראה (קידומת `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- טלמטריה: `runtime_upgrade_provenance_rejections_total{reason}` סופר את סיבות הדחייה.

כללי ביצוע
- מדיניות Host VM: בזמן הרצת תוכנית, גוזרים `SyscallPolicy` מתוך `ProgramMetadata.abi_version`. syscalls לא ידועים לגרסה זו ממופים ל-`VMError::UnknownSyscall`.
- Pointer-ABI: allowlist נגזרת מ-`ProgramMetadata.abi_version`; סוגים מחוץ ל-allowlist עבור גרסה זו נדחים בזמן decode/validation.
- החלפת Host: כל בלוק מחשב מחדש את סט ה-ABI הפעיל; לאחר commit של טרנזקציית הפעלה, טרנזקציות מאוחרות באותו בלוק רואות את המדיניות החדשה (נבדק ע"י `runtime_upgrade_admission::activation_allows_new_abi_in_same_block`).
  - קשירת מדיניות syscall: `CoreHost` קורא את גרסת ה-ABI המוצהרת בטרנזקציה ומיישם `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` מול `SyscallPolicy` פר-בלוק. ה-host ממחזר את מופע ה-VM בטווח הטרנזקציה, ולכן הפעלות באמצע בלוק בטוחות - טרנזקציות מאוחרות רואות מדיניות מעודכנת בעוד הקודמות ממשיכות עם גרסה מקורית.

אינוואריאנטים של דטרמיניזם ובטיחות
- ההפעלה מתרחשת רק ב-`start_height` והיא אידמפוטנטית; reorgs מתחת ל-`start_height` מיישמים מחדש דטרמיניסטית כשהבלוק חוזר.
- גרסאות ABI קיימות נשארות פעילות ללא הגבלה; גרסאות חדשות רק מרחיבות את הסט הפעיל.
- אין משא ומתן דינמי שמשפיע על קונצנזוס או סדר ביצוע; gossip של יכולות הוא מידע בלבד.

רולאאוט מפעיל (ללא השבתה)
1) לפרוס בינארי צומת שתומך ב-ABI החדש (`v+1`) אך אינו מפעיל אותו.
2) לצפות ביכולת הצי באמצעות טלמטריה (אחוז צמתים שמכריזים תמיכה ב-`v+1`).
3) לשלוח `ProposeRuntimeUpgrade` עם חלון שמקדים מספיק (למשל `H+N`).
4) ב-`start_height`, `ActivateRuntimeUpgrade` מופעל אוטומטית כחלק מהבלוק ונועל את סט ה-host הפעיל; צמתים שלא עודכנו ימשיכו לפעול לחוזים ישנים אך ידחו קבלה/ביצוע של תוכניות `v+1`.
5) לאחר ההפעלה, לקמפל/לפרוס חוזים שמכוונים ל-`v+1`.

Torii ו-CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ active_versions: [u16], default_compile_target: u16 }` (ממומש)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (ממומש)
  - `GET /v1/runtime/upgrades` -> רשימת רשומות (ממומש).
  - `POST /v1/runtime/upgrades/propose` -> עוטף את `ProposeRuntimeUpgrade` (מחזיר instruction skeleton; ממומש).
  - `POST /v1/runtime/upgrades/activate/:id` -> עוטף את `ActivateRuntimeUpgrade` (מחזיר instruction skeleton; ממומש).
  - `POST /v1/runtime/upgrades/cancel/:id` -> עוטף את `CancelRuntimeUpgrade` (מחזיר instruction skeleton; ממומש).
- CLI
  - `iroha runtime abi active` (ממומש)
  - `iroha runtime abi hash` (ממומש)
  - `iroha runtime upgrade list` (ממומש)
  - `iroha runtime upgrade propose --file <manifest.json>` (ממומש)
  - `iroha runtime upgrade activate --id <id>` (ממומש)
  - `iroha runtime upgrade cancel --id <id>` (ממומש)

API לשאילתות ליבה
- שאילתת Norito יחידה (חתומה):
  - `FindActiveAbiVersions` מחזיר מבנה Norito `{ active_versions: [u16], default_compile_target: u16 }`.
  - דוגמה: `docs/source/samples/find_active_abi_versions.md` (סוג/שדות ודוגמת JSON).

שינויים נדרשים בקוד (לפי crate)
- iroha_data_model
  - הוספת `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, enums של הוראות, אירועים, ומקודדי JSON/Norito עם בדיקות roundtrip.
- iroha_core
  - WSV: הוספת registry `runtime_upgrades` עם בדיקות חפיפה ו-getters.
  - Executors: מימוש handlers של ISI; פליטת אירועים; אכיפת כללי קבלה.
  - Admission: gating של program manifests לפי פעילות `abi_version` ושוויון `abi_hash`.
  - Syscall policy mapping: העברת סט ABI פעיל ל-VM host constructor; הבטחת דטרמיניזם באמצעות שימוש בגובה הבלוק בתחילת הביצוע.
  - Tests: אידמפוטנטיות חלון ההפעלה, דחיות חפיפה, התנהגות קבלה לפני/אחרי.
- ivm
  - הגדרת `ABI_V2` (דוגמה) עם מדיניות: הרחבת `abi_syscall_list()`; מיפוי `is_syscall_allowed(policy, number)`; הרחבת מדיניות סוגי pointer.
  - חישוב מחדש וקיבוע golden tests: `abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, `pointer_type_ids_golden.rs`.
- iroha_cli / iroha_torii
  - הוספת endpoints ופקודות המפורטים לעיל; helpers של Norito JSON ל-manifests; בדיקות אינטגרציה בסיסיות.
- Kotodama compiler
  - לאפשר targeting של `abi_version = v+1`; לשבץ את `abi_hash` הנכון עבור הגרסה שנבחרה ב-manifests `.to`.

Telemetria
- הוספת gauge `runtime.active_abi_versions` ו-counter `runtime.upgrade_events_total{kind}`.

שיקולי אבטחה
- רק root/sudo רשאים להציע/להפעיל/לבטל; manifests חייבים להיות חתומים כראוי.
- חלונות הפעלה מונעים front-running ומבטיחים יישום דטרמיניסטי.
- `abi_hash` מקבע את משטח הממשק כדי למנוע drift שקט בין בינארים.

קריטריוני קבלה (Conformance)
- לפני ההפעלה, צמתים דוחים דטרמיניסטית קוד עם `abi_version = v+1`.
- לאחר ההפעלה ב-`start_height`, צמתים מקבלים ומבצעים `v+1`; תוכניות ישנות ממשיכות לפעול ללא שינוי.
- golden tests עבור ABI hashes ורשימות syscalls עוברים על x86-64/ARM64.
- ההפעלה אידמפוטנטית ובטוחה תחת reorgs.

</div>
