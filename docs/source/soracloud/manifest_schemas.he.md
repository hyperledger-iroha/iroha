<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# סכימות מניפסט SoraCloud V1

דף זה מגדיר את סכימות Norito הדטרמיניסטיות הראשונות עבור SoraCloud
פריסה ב-Iroha 3:

- `SoraContainerManifestV1`
- `SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`
- `AgentApartmentManifestV1`
- `FheParamSetV1`
- `FheExecutionPolicyV1`
- `FheGovernanceBundleV1`
- `FheJobSpecV1`
- `DecryptionAuthorityPolicyV1`
- `DecryptionRequestV1`
- `CiphertextQuerySpecV1`
- `CiphertextQueryResponseV1`
- `SecretEnvelopeV1`
- `CiphertextStateRecordV1`

הגדרות החלודה חיות ב-`crates/iroha_data_model/src/soracloud.rs`.

רשומות זמן ריצה פרטיות של מודל שהועלה הן בכוונה שכבה נפרדת ממנה
גילויי פריסת SCR אלה. הם צריכים להרחיב את מטוס הדגם Soracloud
ושימוש חוזר ב-`SecretEnvelopeV1` / `CiphertextStateRecordV1` עבור בתים מוצפנים
ומצב מקורי של טקסט צופן, במקום להיות מקודד כשירות/מיכל חדש
בא לידי ביטוי. ראה `uploaded_private_models.md`.

## היקף

מניפסטים אלה מיועדים ל-`IVM` + מותאם אישית של Sora Container Runtime
(SCR) כיוון (ללא WASM, ללא תלות Docker בכניסה בזמן ריצה).- `SoraContainerManifestV1` לוכד את זהות החבילה הניתנת להפעלה, סוג זמן ריצה,
  מדיניות יכולת, משאבים, הגדרות בדיקה של מחזור חיים ומפורש
  ייצוא נדרש-config לסביבת זמן הריצה או גרסה מותקנת
  עץ.
- `SoraServiceManifestV1` לוכד את כוונת הפריסה: זהות שירות,
  ה-hash/גרסה של המניפסט המכיל הפניה, ניתוב, מדיניות השקה ו
  כריכות מדינה.
- `SoraStateBindingV1` לוכדת היקף ומגבלות של כתיבה מצב דטרמיניסטי
  (קידומת מרחב שמות, מצב שינוי, מצב הצפנה, פריט/מכסות כוללות).
- `SoraDeploymentBundleV1` זוגות מכולה + מניפסטים ואכיפה של שירות
  בדיקות קבלה דטרמיניסטיות (הצמדת מניפסט-hash, יישור סכימה ו
  יכולת/עקביות מחייבת).
- `AgentApartmentManifestV1` לוכדת מדיניות זמן ריצה מתמשכת של סוכן:
  מכסות כלים, מכסי מדיניות, מגבלות הוצאות, מכסת מדינה, יציאה מרשת ו
  לשדרג התנהגות.
- `FheParamSetV1` לוכד ערכות פרמטרים של FHE בניהול ממשל:
  מזהי קצה/סכמה דטרמיניסטים, פרופיל מודולוס, אבטחה/עומק
  גבולות, וגבהים של מחזור חיים (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` לוכד מגבלות ביצוע של טקסט צופן דטרמיניסטי:
  גדלי מטען מורשים, כניסת קלט/פלט מאוורר, עומק/סיבוב/כובעי אתחול,
  ומצב עיגול קנוני.
- `FheGovernanceBundleV1` מצמיד ערכת פרמטרים ומדיניות לדטרמיניסטית
  אימות קבלה.- `FheJobSpecV1` לוכדת קבלה/ביצוע עבודה בטקסט צופן דטרמיניסטי
  בקשות: מחלקה פעולה, התחייבויות קלט מסודרות, מפתח פלט, ומוגבל
  דרישה לעומק/סיבוב/אתחול מקושרת לקבוצת מדיניות + פרמטרים.
- `DecryptionAuthorityPolicyV1` לוכדת מדיניות גילוי בניהול ממשל:
  מצב סמכות (מוחזקים על ידי לקוח לעומת שירות סף), מניין מאשר/חברים,
  קצבת שבירת זכוכית, תיוג תחום שיפוט, דרישת הסכמה,
  גבולות TTL, ותיוג ביקורת קנוני.
- `DecryptionRequestV1` לוכד ניסיונות חשיפה הקשורים למדיניות:
  הפניה למפתח טקסט צופן (`binding_name` + `state_key` + התחייבות),
  הצדקה, תג שיפוט, hash אופציונלי של הסכמה-ראיה, TTL,
  כוונה/סיבה לשבור זכוכית, והצמדת גיבוב של ממשל.
- `CiphertextQuerySpecV1` לוכד כוונת שאילתה דטרמיניסטית של טקסט צופן בלבד:
  היקף שירות/איגד, מסנן מפתח-קדמת מפתח, מגבלת תוצאות מוגבלת, מטא נתונים
  רמת הקרנה והחלפת הוכחה.
- `CiphertextQueryResponseV1` לוכד פלטי שאילתות ממוזערות בחשיפה:
  הפניות מפתח מוכוונות תקציר, מטא נתונים של טקסט צופן, הוכחות הכללה אופציונליות,
  והקשר של חיתוך/רצף ברמת התגובה.
- `SecretEnvelopeV1` לוכד חומר מטען מוצפן בעצמו:
  מצב הצפנה, מזהה מפתח/גרסה, nonce, בתים של טקסט צופן ו
  התחייבויות יושרה.
- `CiphertextStateRecordV1` לוכד ערכי מצב מקורי של טקסט צופןשלב מטא נתונים ציבוריים (סוג תוכן, תגי מדיניות, מחויבות, גודל מטען)
  עם `SecretEnvelopeV1`.
- חבילות מודלים פרטיות שהועלו על ידי משתמשים צריכות להתבסס על טקסט מוצפן אלה
  רשומות:
  נתחי משקל/תצורה/מעבד מוצפנים חיים במצב, בעוד רישום הדגמים,
  נותרו שושלת משקל, חיבור פרופילים, מפגשי מסקנות ונקודות ביקורת
  תקליטי Soracloud מהשורה הראשונה.

## גירסאות

- `SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
- `AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
- `FHE_PARAM_SET_VERSION_V1 = 1`
- `FHE_EXECUTION_POLICY_VERSION_V1 = 1`
- `FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
- `FHE_JOB_SPEC_VERSION_V1 = 1`
- `DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
- `DECRYPTION_REQUEST_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
- `SECRET_ENVELOPE_VERSION_V1 = 1`
- `CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

אימות דוחה גרסאות לא נתמכות עם
`SoraCloudManifestError::UnsupportedVersion`.

## כללי אימות דטרמיניסטיים (V1)- מניפסט מיכל:
  - `bundle_path` ו-`entrypoint` חייבים להיות לא ריקים.
  - `healthcheck_path` (אם מוגדר) חייב להתחיל עם `/`.
  - `config_exports` עשוי להתייחס רק לתצורות המוצהרות ב
    `required_config_names`.
  - יעדי env config-export חייבים להשתמש בשמות משתני סביבה קנוניים
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - יעדי קובץ config-export חייבים להישאר יחסיים, להשתמש במפרידי `/`, ו
    אסור להכיל מקטעים ריקים, `.` או `..`.
  - ייצוא תצורה אינו יכול למקד לאותו env var או נתיב קובץ יחסי יותר
    מפעם אחת.
- מניפסט שירות:
  - `service_version` חייב להיות לא ריק.
  - `container.expected_schema_version` חייב להתאים לסכימת מכיל v1.
  - `rollout.canary_percent` חייב להיות `0..=100`.
  - `route.path_prefix` (אם מוגדר) חייב להתחיל עם `/`.
  - השמות המחייבים של המדינה חייבים להיות ייחודיים.
- מחייב מדינה:
  - `key_prefix` חייב להיות לא ריק ולהתחיל ב-`/`.
  - `max_item_bytes <= max_total_bytes`.
  - כריכות `ConfidentialState` אינן יכולות להשתמש בהצפנת טקסט רגיל.
- חבילת פריסה:
  - `service.container.manifest_hash` חייב להתאים לקידוד הקנוני
    hash של מניפסט מכיל.
  - `service.container.expected_schema_version` חייב להתאים לסכימת המכולה.
  - כריכות מצב הניתנות לשינוי דורשות `container.capabilities.allow_state_writes=true`.
  - מסלולים ציבוריים דורשים `container.lifecycle.healthcheck_path`.
- מניפסט דירת סוכן:
  - `container.expected_schema_version` חייב להתאים לסכימת קונטיינר v1.
  - שמות יכולות הכלים חייבים להיות לא ריקים וייחודיים.- שמות יכולות המדיניות חייבים להיות ייחודיים.
  - נכסי מגבלת הוצאה חייבים להיות לא ריקים וייחודיים.
  - `max_per_tx_nanos <= max_per_day_nanos` עבור כל מגבלת הוצאה.
  - מדיניות הרשת של רשימת ההיתרים חייבת לכלול מארחים ייחודיים שאינם ריקים.
- ערכת פרמטרים של FHE:
  - `backend` ו-`ciphertext_modulus_bits` חייבים להיות לא ריקים.
  - כל גודל סיביות של מודול טקסט צופן חייב להיות בתוך `2..=120`.
  - סדר שרשרת מודול הטקסט חייב להיות לא עולה.
  - `plaintext_modulus_bits` חייב להיות קטן יותר ממודוס הטקסט המוצפן הגדול ביותר.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - הזמנת גובה מחזור החיים חייבת להיות קפדנית:
    `activation < deprecation < withdraw` כאשר קיים.
  - דרישות סטטוס מחזור חיים:
    - `Proposed` לא מאפשר ביטול/משיכה של גבהים.
    - `Active` דורש `activation_height`.
    - `Deprecated` דורש `activation_height` + `deprecation_height`.
    - `Withdrawn` דורש `activation_height` + `withdraw_height`.
- מדיניות ביצוע FHE:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - הכריכה של ערכת פרמטרים חייבת להתאים לפי `(param_set, version)`.
  - אסור ל-`max_multiplication_depth` לחרוג מעומק מוגדר פרמטר.
  - קבלה לפוליסה דוחה את מחזור החיים של ערכת פרמטרים `Proposed` או `Withdrawn`.
- חבילת ממשל של FHE:
  - מאמת תאימות של מדיניות + ערכת פרמטרים כמטען קבלה דטרמיניסטי אחד.
- מפרט עבודה של FHE:
  - `job_id` ו-`output_state_key` חייבים להיות לא ריקים (`output_state_key` מתחיל ב-`/`).- ערכת הקלט חייבת להיות לא ריקה ומפתחות הקלט חייבים להיות נתיבים קנוניים ייחודיים.
  - אילוצים ספציפיים לפעולה הם קפדניים (`Add`/`Multiply` ריבוי כניסות,
    `RotateLeft`/`Bootstrap` עם קלט יחיד, עם ידיות עומק/סיבוב/רצועת אתחול הבלעדית הדדית).
  - קבלה הקשורה למדיניות אוכפת:
    - מזהי מדיניות/פרמטרים וגרסאות תואמות.
    - מגבלות ספירת קלט/בתים, עומק, סיבוב ומגבלות אתחול נמצאים במסגרת מכסי מדיניות.
    - בתים של פלט מוקרן דטרמיניסטי מתאימים לגבולות טקסט צופן של מדיניות.
- מדיניות רשות פענוח:
  - `approver_ids` חייב להיות לא ריק, ייחודי ומסודר לקסיקוגרפית למהדרין.
  - מצב `ClientHeld` דורש מאשר אחד בדיוק, `approver_quorum=1`,
    ו-`allow_break_glass=false`.
  - מצב `ThresholdService` דורש לפחות שני מאשרים ו
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` חייב להיות לא ריק ואסור להכיל תווי בקרה.
  - `audit_tag` חייב להיות לא ריק ואסור להכיל תווי בקרה.
- בקשת פענוח:
  - `request_id`, `state_key` ו-`justification` חייבים להיות לא ריקים
    (`state_key` מתחיל ב-`/`).
  - `jurisdiction_tag` חייב להיות לא ריק ואסור להכיל תווי בקרה.
  - `break_glass_reason` נדרש כאשר `break_glass=true` יש להשמיט כאשר
    `break_glass=false`.
  - קבלה הקשורה למדיניות אוכפת שוויון שמות מדיניות, בקש TTL לאהעולה על `policy.max_ttl_blocks`, שוויון תג שיפוט, זכוכית שבירה
    שער, ודרישות הוכחות הסכמה מתי
    `policy.require_consent_evidence=true` לבקשות ללא שבירת זכוכית.
- מפרט שאילתת טקסט צופן:
  - `state_key_prefix` חייב להיות לא ריק ולהתחיל ב-`/`.
  - `max_results` מוגבל באופן דטרמיניסטי (`<=256`).
  - הקרנת מטא נתונים היא מפורשת (`Minimal` תקציר בלבד לעומת `Standard` גלוי למפתח).
- תגובת שאילתת צופן:
  - `result_count` חייב להיות שווה לספירת שורות סידורית.
  - הקרנת `Minimal` אסורה לחשוף את `state_key`; `Standard` חייב לחשוף אותו.
  - אסור לשורות להופיע במצב הצפנה של טקסט רגיל.
  - הוכחות הכללה (כאשר קיימות) חייבות לכלול מזהי סכמה לא ריקים ו
    `anchor_sequence >= event_sequence`.
- מעטפה סודית:
  - `key_id`, `nonce` ו-`ciphertext` חייבים להיות לא ריקים.
  - אורך nonce מוגבל (`<=256` בתים).
  - אורך טקסט צופן מוגבל (`<=33554432` בתים).
- רשומת מצב צופן:
  - `state_key` חייב להיות לא ריק ולהתחיל ב-`/`.
  - סוג התוכן של מטא נתונים חייב להיות לא ריק; התגים חייבים להיות מחרוזות ייחודיות שאינן ריקות.
  - `metadata.payload_bytes` חייב להיות שווה ל-`secret.ciphertext.len()`.
  - `metadata.commitment` חייב להיות שווה ל-`secret.commitment`.

## מתקנים קנוניים

אביזרי JSON קנוניקל מאוחסנים ב:- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`
- `fixtures/soracloud/agent_apartment_manifest_v1.json`
- `fixtures/soracloud/fhe_param_set_v1.json`
- `fixtures/soracloud/fhe_execution_policy_v1.json`
- `fixtures/soracloud/fhe_governance_bundle_v1.json`
- `fixtures/soracloud/fhe_job_spec_v1.json`
- `fixtures/soracloud/decryption_authority_policy_v1.json`
- `fixtures/soracloud/decryption_request_v1.json`
- `fixtures/soracloud/ciphertext_query_spec_v1.json`
- `fixtures/soracloud/ciphertext_query_response_v1.json`
- `fixtures/soracloud/secret_envelope_v1.json`
- `fixtures/soracloud/ciphertext_state_record_v1.json`

מבחני מתקן/הלוך ושוב:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`