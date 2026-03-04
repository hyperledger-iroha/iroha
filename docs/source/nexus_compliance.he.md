---
lang: he
direction: rtl
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי עבור docs/source/nexus_compliance.md -->

# מנוע תאימות Lane של Nexus ומדיניות רשימת היתרים (NX-12)

סטטוס: 🈴 מיושם — מסמך זה מתאר את מודל המדיניות הפעיל ואת האכיפה הקריטית לקונצנזוס
המוזכרת בפריט הרודמפ **NX-12 — מנוע תאימות Lane ומדיניות רשימת היתרים**.
הוא מסביר את מודל הנתונים, זרימות הממשל, הטלמטריה ואסטרטגיית ההשקה שמיושמות בתוך `crates/iroha_core/src/compliance`
ושמיושמות הן בקבלה ב-Torii והן באימות טרנזקציות של `iroha_core`, כך שכל lane וכל dataspace יוכלו להיות קשורים למדיניות שיפוטית דטרמיניסטית.

## מטרות

- לאפשר לממשל להצמיד כללי allow/deny, דגלי שיפוט, מגבלות העברה של CBDC, ודרישות ביקורת לכל lane manifest.
- להעריך כל טרנזקציה מול הכללים בזמן קבלה ב-Torii ובזמן ביצוע בלוק, כדי להבטיח אכיפה דטרמיניסטית בין צמתים.
- להפיק audit trail קריפטוגרפי ניתן לאימות עם Norito evidence bundles וטלמטריה ניתנת לשאילתה לרגולטורים ולמפעילים.
- לשמור על מודל גמיש: אותו policy engine מכסה lanes פרטיים של CBDC, DS ציבוריים ל-settlement, ו-dataspaces היברידיים של שותפים ללא forks ייעודיים.

## לא-מטרות

- הגדרת נהלי AML/KYC או זרימות הסלמה משפטית. אלה נמצאים ב-compliance playbooks שצורכים את הטלמטריה שמופקת כאן.
- הוספת toggles לכל instruction ב-IVM; המנוע שולט רק באילו accounts/assets/domains יכולים לשלוח טרנזקציות או לתקשר עם lane.
- ביטול Space Directory. manifests נשארים מקור סמכותי למטא-דאטה של DS; מדיניות התאימות רק מפנה לרשומות Space Directory ומשלימה אותן.

## מודל מדיניות

### ישויות ומזהים

מנוע המדיניות פועל על:

- `LaneId` / `DataSpaceId` — מזהה את התחום שבו הכללים חלים.
- `UniversalAccountId (UAID)` — מאפשר לקבץ זהויות cross-lane.
- `JurisdictionFlag` — bitmask שמונה סיווגים רגולטוריים (לדוגמה `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` — מתאר את מי הכללים חלים עליו:
  - `AccountId`, `DomainId` או `UAID`.
  - סלקטורים מבוססי תחילית (`DomainPrefix`, `UaidPrefix`) להתאמת רישומים.
  - `CapabilityTag` עבור manifests של Space Directory (לדוגמה DS שעבר FX-cleared בלבד).
  - gating של `privacy_commitments_any_of` כדי לדרוש שה-lanes יכריזו על Nexus privacy commitments לפני התאמת הכללים
    (משקף את surface של manifest ב-NX-10 ונאכף ב-snapshots של `LanePrivacyRegistry`).

### LaneCompliancePolicy

מדיניות היא מבני Norito שמפורסמים דרך הממשל:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` משלב `ParticipantSelector`, override שיפוטי אופציונלי, capability tags וקודי סיבה.
- `DenyRule` משקף את מבנה allow אך נבחן ראשון (deny wins).
- `TransferLimit` מגדיר תקרות לפי asset/bucket:
  - `max_notional_xor` ו-`max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (לדוגמה CBDC retail לעומת wholesale).
- `AuditControls` מגדיר:
  - האם Torii חייב לשמור כל דחייה ביומן ביקורת.
  - האם החלטות מוצלחות ידגמו לתוך Norito digests.
  - חלון השימור הנדרש עבור `LaneComplianceDecisionRecord`.

### אחסון והפצה

- hashes עדכניים של מדיניות נמצאים ב-manifest של Space Directory לצד מפתחות הוולידטורים.
  `LaneCompliancePolicyReference` (policy id + version + hash) הופך לשדה manifest כדי שוולידטורים ו-SDKs יוכלו למשוך policy blob קנוני.
- `iroha_config` חושף `compliance.policy_cache_dir` לשמירת Norito payload והחתימה המנותקת שלו.
  הצמתים מאמתים חתימות לפני החלת עדכונים כדי להגן מפני מניפולציה.
- המדיניות מוטמעת גם ב-Norito admission manifests שבהם Torii משתמש, כדי ש-CI/SDKs יוכלו לשחזר הערכת מדיניות בלי לדבר עם וולידטורים.

## ממשל ומחזור חיים

1. **הצעה** — הממשל מגיש `ProposeLaneCompliancePolicy` עם Norito payload, הצדקת שיפוט ו-epoch הפעלה.
2. **סקירה** — compliance reviewers חותמים על `LaneCompliancePolicyReviewEvidence`
   (ניתן לביקורת, מאוחסן ב-`governance::ReviewEvidenceStore`).
3. **הפעלה** — לאחר חלון ההשהיה, וולידטורים קולטים את המדיניות באמצעות קריאה ל-`ActivateLaneCompliancePolicy`.
   manifest של Space Directory מתעדכן בצורה אטומית עם reference חדש למדיניות.
4. **תיקון/ביטול** — `AmendLaneCompliancePolicy` נושא diff metadata תוך שמירת הגרסה הקודמת לשחזור פורנזי;
   `RevokeLaneCompliancePolicy` מצמיד את policy id ל-`denied` כך ש-Torii ידחה כל תעבורה שמיועדת ל-lane עד להפעלת חלופה.

Torii חושף:

- `GET /v1/lane-compliance/policies/{lane_id}` — משיכת policy reference העדכני.
- `POST /v1/lane-compliance/policies` — endpoint לממשל בלבד שמשקף את ISI proposal helpers.
- `GET /v1/lane-compliance/decisions` — יומן ביקורת מדורג עם מסננים לפי `lane_id`, `decision`, `jurisdiction`, `reason_code`.

פקודות CLI/SDK עוטפות את משטחי ה-HTTP הללו כדי שמפעילים יוכלו לאוטומט סקירות
ולקבל artefacts (policy blob חתום + reviewer attestations).

## צינור אכיפה

1. **קבלה (Torii)**
   - `Torii` מוריד את המדיניות הפעילה כאשר lane manifest משתנה או כאשר חתימת הקאש פגה.
   - כל טרנזקציה שנכנסת לתור `/v1/pipeline` מתויגת עם `LaneComplianceContext`
     (ids של משתתפים, UAID, metadata של manifest ב-dataspace, policy id, וה-snapshot העדכני של `LanePrivacyRegistry`
     המתואר ב-`crates/iroha_core/src/interlane/mod.rs`).
   - סמכויות עם UAID חייבות להחזיק manifest פעיל של Space Directory עבור ה-dataspace המנותב;
     Torii דוחה טרנזקציות כאשר ה-UAID אינו קשור ל-dataspace לפני הערכת כללי המדיניות.
   - `compliance::Engine` בוחן כללי `deny` ואז `allow`, ולבסוף מחיל limits להעברה.
     טרנזקציות שנכשלו מחזירות שגיאה טיפוסית (`ERR_LANE_COMPLIANCE_DENIED`) עם סיבה ו-policy id לצורכי ביקורת.
   - הקבלה היא סינון מהיר; אימות הקונצנזוס בודק מחדש את אותם כללים באמצעות snapshots של המצב כדי לשמור על דטרמיניזם.
2. **ביצוע (iroha_core)**
   - במהלך בניית בלוק, `iroha_core::tx::validate_transaction_internal` משחזר את אותן בדיקות
     של governance/UAID/privacy/compliance עבור lane באמצעות snapshots של `StateTransaction`
     (`lane_manifests`, `lane_privacy_registry`, `lane_compliance`). כך נשמרת אכיפה קריטית לקונצנזוס גם אם הקאשים של Torii מיושנים.
   - טרנזקציות שמשנות lane manifests או מדיניות תאימות עוברות באותו נתיב אימות; אין bypass רק בקבלה.
3. **Hooks אסינכרוניים**
   - RBC gossip ו-DA fetchers מצמידים policy id לטלמטריה כך שניתן יהיה לשייך החלטות מאוחרות לגרסת הכללים הנכונה.
   - `iroha_cli` ו-SDK helpers חושפים `LaneComplianceDecision::explain()` כדי שאוטומציה תוכל להפיק אבחונים קריאים.

המנוע דטרמיניסטי וטהור; הוא אינו פונה למערכות חיצוניות אחרי הורדת manifest/policy.
זה שומר על CI fixtures ועל שחזור רב-צמתי פשוט.

## ביקורת וטלמטריה

- **מטריקות**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (צריך להישאר < activation delay).
- **לוגים**
  - רשומות מובנות לוכדות `policy_id`, `version`, `participant`, `UAID`, דגלי שיפוט ו-Norito hash של טרנזקציה מפרה.
  - `LaneComplianceDecisionRecord` מקודד ב-Norito ונשמר תחת `world.compliance_logs::<lane_id>::<ts>::<nonce>` כאשר `AuditControls`
    מבקש אחסון עמיד.
- **Evidence bundles**
  - `cargo xtask nexus-lane-audit` מוסיף מצב `--lane-compliance <path>` שממזג את המדיניות, חתימות הסוקרים,
    snapshot מטריקות והלוג האחרון של הביקורת לתוצרי JSON + Parquet. הדגל מצפה ל-JSON payload בצורה:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    ה-CLI מאמת שכל blob של `policy` תואם ל-`lane_id` ברשומה לפני ההטמעה, כדי למנוע ראיות מיושנות או לא תואמות
    בחבילות רגולטוריות וב-dashboardים של הרודמפ.
  - `--markdown-out` (ברירת מחדל `artifacts/nexus_lane_audit.md`) מציג כעת תקציר קריא שמדגיש lanes מפגרים,
    backlog לא אפסי, manifests ממתינים וראיות תאימות חסרות כדי שחבילות annex יכללו גם artefacts machine-readable
    וגם משטח סקירה מהיר.

## תוכנית השקה

1. **P0 — תצפית בלבד**
   - לשחרר את טיפוסי המדיניות, האחסון, endpoints של Torii והמדדים.
   - Torii מעריך מדיניות במצב `audit` (ללא enforcement) כדי לאסוף נתונים.
2. **P1 — אכיפת deny/allow**
   - להפעיל כשל קשיח ב-Torii ובביצוע כאשר כללי deny מופעלים.
   - לדרוש מדיניות לכל lanes של CBDC; DS ציבוריים יכולים להשאר במצב audit.
3. **P2 — מגבלות ו-jurisdiction overrides**
   - להפעיל אכיפה של מגבלות העברה ודגלי שיפוט.
   - להזין טלמטריה ל-`dashboards/grafana/nexus_lanes.json`.
4. **P3 — אוטומציה מלאה של תאימות**
   - לשלב exports של ביקורת עם צרכני `SpaceDirectoryEvent`.
   - לקשור עדכוני מדיניות ל-runbooks של ממשל ולאוטומציית release.

## קבלה ובדיקות

- בדיקות אינטגרציה ב-`integration_tests/tests/nexus/compliance.rs` מכסות:
  - שילובי allow/deny, jurisdiction overrides ומגבלות העברה;
  - מירוצים של הפעלת manifest/policy; ו
  - תאימות החלטות Torii מול `iroha_core` בריצות מרובות צמתים.
- בדיקות יחידה ב-`crates/iroha_core/src/compliance` מאמתות את מנוע ההערכה הטהור, טיימרים לביטול קאש וניתוח מטא-דאטה.
- עדכוני Docs/SDK (Torii + CLI) צריכים להדגים שליפת מדיניות, שליחת הצעות ממשל, פירוש קודי שגיאה ואיסוף ראיות ביקורת.

סגירת NX-12 דורשת את התוצרים לעיל וכן עדכוני סטטוס ב-`status.md`/`roadmap.md` לאחר הפעלת enforcement בכל אשכולות ה-staging.

</div>
