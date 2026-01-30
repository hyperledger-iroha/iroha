---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ffe54617f5c5f7c3220bd07a1b3ce7d7cb09eb13d98413c072283d2bb7a8068
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: provider-admission-policy
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> מותאם מ- [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# מדיניות קבלת ספקים וזהות ב-SoraFS (טיוטת SF-2b)

מסמך זה מרכז את התוצרים המעשיים של **SF-2b**: הגדרת מסלול הקבלה, דרישות זהות,
ומטעני attestation עבור ספקי אחסון SoraFS, ואכיפתם. הוא מרחיב את התהליך ברמה גבוהה
שמופיע ב-RFC לארכיטקטורת SoraFS ומפרק את העבודה הנותרת למשימות הנדסיות הניתנות למעקב.

## יעדי המדיניות

- להבטיח שרק מפעילים מאומתים יכולים לפרסם רשומות `ProviderAdvertV1` שהרשת תקבל.
- לקשור כל מפתח פרסום למסמך זהות מאושר ממשל, נקודות קצה מאומתות, ותרומת stake מינימלית.
- לספק כלי אימות דטרמיניסטיים כך ש-Torii, ה-gateways ו-`sorafs-node` יאכפו את אותם בדיקות.
- לתמוך בחידוש ובביטול חירום מבלי לפגוע בדטרמיניזם או בארגונומיה של הכלים.

## דרישות זהות ו-stake

| דרישה | תיאור | תוצר |
|-------|-------|------|
| מקור מפתח פרסום | ספקים חייבים לרשום זוג מפתחות Ed25519 שחותם על כל advert. חבילת הקבלה שומרת את המפתח הציבורי לצד חתימת ממשל. | הרחבת סכמת `ProviderAdmissionProposalV1` עם `advert_key` (32 בתים) והפניה אליו מהרישום (`sorafs_manifest::provider_admission`). |
| מצביע stake | הקבלה דורשת `StakePointer` שאינו אפס ומצביע לבריכת staking פעילה. | להוסיף ולידציה ב-`sorafs_manifest::provider_advert::StakePointer::validate()` ולהציג שגיאות ב-CLI/tests. |
| תגי סמכות שיפוט | ספקים מצהירים על סמכות שיפוט + איש קשר משפטי. | הרחבת סכמת ההצעה עם `jurisdiction_code` (ISO 3166-1 alpha-2) ו-`contact_uri` אופציונלי. |
| Attestation לנקודות קצה | כל נקודת קצה מפורסמת חייבת להיתמך בדוח תעודת mTLS או QUIC. | להגדיר payload Norito בשם `EndpointAttestationV1` ולאחסן לכל נקודת קצה בתוך חבילת הקבלה. |

## תהליך הקבלה

1. **יצירת הצעה**
   - CLI: להוסיף `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     שמייצר `ProviderAdmissionProposalV1` + חבילת attestation.
   - ולידציה: לוודא שדות חובה, stake > 0, ומזהה chunker קנוני ב-`profile_id`.
2. **אישור ממשל**
   - המועצה חותמת על `blake3("sorafs-provider-admission-v1" || canonical_bytes)` באמצעות כלי ה-envelope הקיימים
     (מודול `sorafs_manifest::governance`).
   - ה-envelope נשמר ב-`governance/providers/<provider_id>/admission.json`.
3. **קליטה ברישום**
   - לממש מאמת משותף (`sorafs_manifest::provider_admission::validate_envelope`) ש-Torii/gateways/CLI משתמשים בו מחדש.
   - לעדכן את נתיב הקבלה ב-Torii כדי לדחות adverts ש-digest או תוקף שלהם שונים מה-envelope.
4. **חידוש וביטול**
   - להוסיף `ProviderAdmissionRenewalV1` עם עדכוני endpoint/stake אופציונליים.
   - לחשוף נתיב CLI `--revoke` שמתעד את סיבת הביטול ושולח אירוע ממשל.

## משימות יישום

| תחום | משימה | Owner(s) | סטטוס |
|------|-------|----------|-------|
| סכמות | להגדיר `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) תחת `crates/sorafs_manifest/src/provider_admission.rs`. יושם ב-`sorafs_manifest::provider_admission` עם helpers לולידציה.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ הושלם |
| כלי CLI | להרחיב את `sorafs_manifest_stub` עם תתי-פקודות: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

זרימת ה-CLI מקבלת כעת bundles של תעודות ביניים (`--endpoint-attestation-intermediate`), מפיקה bytes קנוניים להצעה/Envelope ומאמתת חתימות מועצה ב-`sign`/`verify`. מפעילים יכולים לספק גופי advert ישירות או להשתמש מחדש ב-adverts חתומים, וקבצי חתימה יכולים להינתן על ידי שילוב `--council-signature-public-key` עם `--council-signature-file` לטובת אוטומציה.

### רפרנס CLI

הריצו כל פקודה דרך `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.

- `proposal`
  - דגלים נדרשים: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, ולפחות `--endpoint=<kind:host>` אחד.
  - attestation לכל endpoint מצפה ל-`--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, תעודה דרך
    `--endpoint-attestation-leaf=<path>` (בתוספת `--endpoint-attestation-intermediate=<path>` אופציונלי לכל רכיב בשרשרת) וכל מזהי ALPN שנוהלו
    (`--endpoint-attestation-alpn=<token>`). נקודות קצה QUIC יכולות לספק דוחות תעבורה עם
    `--endpoint-attestation-report[-hex]=...`.
  - פלט: bytes קנוניים של הצעת Norito (`--proposal-out`) וסיכום JSON
    (stdout כברירת מחדל או `--json-out`).
- `sign`
  - קלטים: הצעה (`--proposal`), advert חתום (`--advert`), גוף advert אופציונלי
    (`--advert-body`), retention epoch ולפחות חתימת מועצה אחת. ניתן לספק חתימות
    inline (`--council-signature=<signer_hex:signature_hex>`) או דרך קבצים באמצעות
    `--council-signature-public-key` עם `--council-signature-file=<path>`.
  - מפיק envelope מאומת (`--envelope-out`) ודוח JSON שמציין קישורי digest, מספר חותמים ונתיבי קלט.
- `verify`
  - מאמת envelope קיים (`--envelope`), עם בדיקה אופציונלית של ההצעה, ה-advert או גוף ה-advert המתאימים. דוח ה-JSON מדגיש ערכי digest, מצב אימות חתימה ואילו artefacts אופציונליים התאימו.
- `renewal`
  - מקשר envelope מאושר חדש ל-digest שאושר בעבר. נדרש
    `--previous-envelope=<path>` והעוקב `--envelope=<path>` (שניהם payloads של Norito).
    ה-CLI מוודא ש-aliases של פרופיל, יכולות ומפתחות advert נשארים ללא שינוי, תוך מתן אפשרות לעדכוני stake, endpoints ו-metadata. מפיק bytes קנוניים
    `ProviderAdmissionRenewalV1` (`--renewal-out`) יחד עם סיכום JSON.
- `revoke`
  - מוציא חבילת חירום `ProviderAdmissionRevocationV1` עבור ספק שה-envelope שלו חייב להימשך. דורש `--envelope=<path>`, `--reason=<text>`, לפחות
    `--council-signature` אחת, ו-`--revoked-at`/`--notes` אופציונליים. ה-CLI חותם ומאמת את digest הביטול, כותב את ה-payload של Norito דרך `--revocation-out` ומדפיס דוח JSON עם digest וספירת חתימות.
| אימות | לממש מאמת משותף שמשמש את Torii, ה-gateways ו-`sorafs-node`. לספק בדיקות יחידה + אינטגרציית CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ הושלם |
| אינטגרציית Torii | לחבר את המאמת לתהליך ingestion של adverts ב-Torii, לדחות adverts לא תקינים ולהוציא טלמטריה. | Networking TL | ✅ הושלם | Torii טוען כעת envelopes של ממשל (`torii.sorafs.admission_envelopes_dir`), מאמת התאמות digest/חתימה בזמן ingestion ומציג טלמטריה של קבלה.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| חידוש | להוסיף סכמת חידוש/ביטול + helpers CLI, ולפרסם מדריך מחזור חיים ב-docs (ראו runbook להלן ואת פקודות CLI `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ הושלם |
| טלמטריה | להגדיר dashboards/alertים של `provider_admission` (חידוש חסר, תוקף envelope). | Observability | 🟠 בתהליך | המונה `torii_sorafs_admission_total{result,reason}` קיים; dashboards/alertים בהמתנה.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook לחידוש וביטול

#### חידוש מתוזמן (עדכוני stake/topology)
1. בנו את זוג ההצעה/advert העוקב עם `provider-admission proposal` ו-`provider-admission sign`, תוך הגדלת `--retention-epoch` ועדכון stake/endpoints לפי הצורך.
2. הריצו
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   הפקודה מאמתת שדות יכולת/פרופיל ללא שינוי דרך
   `AdmissionRecord::apply_renewal`, מפיקה `ProviderAdmissionRenewalV1` ומדפיסה digests עבור
   יומן הממשל.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. החליפו את ה-envelope הקודם ב-`torii.sorafs.admission_envelopes_dir`, בצעו commit של חידוש Norito/JSON לריפו הממשל, והוסיפו hash חידוש + retention epoch ל-`docs/source/sorafs/migration_ledger.md`.
4. הודיעו למפעילים שה-envelope החדש חי ומעקב אחר `torii_sorafs_admission_total{result="accepted",reason="stored"}` כדי לאשר ingestion.
5. שִחזרו ותקבעו fixtures קנוניים דרך `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) מאמת שהפלטים של Norito נשארים יציבים.

#### ביטול חירום
1. זיהוי ה-envelope שנפרץ והנפקת ביטול:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   ה-CLI חותם על `ProviderAdmissionRevocationV1`, מאמת את סט החתימות דרך
   `verify_revocation_signatures`, ומדווח על digest הביטול.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. הסירו את ה-envelope מ-`torii.sorafs.admission_envelopes_dir`, הפיצו את Norito/JSON הביטול ל-caches של הקבלה, ורשמו את hash הסיבה בפרוטוקולי הממשל.
3. עקבו אחרי `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` כדי לוודא שה-caches מפילים את ה-advert שבוטל; שמרו את artefacts הביטול ברטרוספקטיבות תקרית.

## בדיקות וטלמטריה

- להוסיף fixtures זהובים להצעות/envelopes של קבלה תחת
  `fixtures/sorafs_manifest/provider_admission/`.
- להרחיב CI (`ci/check_sorafs_fixtures.sh`) כדי לחדש הצעות ולאמת envelopes.
- fixtures שנוצרו כוללים `metadata.json` עם digests קנוניים; בדיקות downstream מאשרות
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- לספק בדיקות אינטגרציה:
  - Torii דוחה adverts עם envelopes קבלה חסרים או שפג תוקפם.
  - ה-CLI מבצע round-trip להצעה → envelope → אימות.
  - חידוש הממשל מסובב את attestation של endpoint ללא שינוי מזהה הספק.
- דרישות טלמטריה:
  - להוציא מונים `provider_admission_envelope_{accepted,rejected}` ב-Torii. ✅ `torii_sorafs_admission_total{result,reason}` מציג כעת תוצאות קבלה/דחייה.
  - להוסיף התראות תוקף ללוחות תצפית (חידוש שמועדו בתוך 7 ימים).

## צעדים הבאים

1. ✅ הושלמו שינויי סכמת Norito והוטמעו helpers של ולידציה ב-`sorafs_manifest::provider_admission`. אין צורך ב-feature flags.
2. ✅ זרימות CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) מתועדות ונבדקות בבדיקות אינטגרציה; שמרו את סקריפטי הממשל מסונכרנים עם ה-runbook.
3. ✅ Torii admission/discovery קולט את ה-envelopes ומציג מוני טלמטריה לקבלה/דחייה.
4. מיקוד בתצפית: לסיים dashboards/alertים של קבלה כדי שחידושים שמועדיהם בתוך שבעה ימים יפיקו אזהרות (`torii_sorafs_admission_total`, expiry gauges).
