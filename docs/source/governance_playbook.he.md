---
lang: he
direction: rtl
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd0430f0b524a2190852369c66407faf5b4f8b91bf0b2c65c68bfa367b366d9a
source_last_modified: "2026-01-03T18:07:57.772749+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/governance_playbook.md -->

# מדריך ממשל

מדריך זה מרכז את הטקסים היומיומיים שמיישרים את מועצת הממשל של Sora Network.
הוא מאגד את ההפניות הסמכותיות מתוך המאגר כך שטקסים בודדים יוכלו להישאר תמציתיים,
בעוד שלמפעילים יש נקודת כניסה אחת לכלל התהליך.

## טקסי מועצה

- **ממשל fixtures** – ראו [Sora Parliament Fixture Approval](sorafs/signing_ceremony.md)
  עבור זרימת האישור על-chain שממנה וועדת התשתיות של הפרלמנט פועלת כעת בעת סקירת
  עדכוני chunker של SoraFS.
- **פרסום ספירת קולות** – הפנו ל-[Governance Vote Tally](governance_vote_tally.md)
  עבור זרימת CLI צעד-אחר-צעד ותבנית דיווח.

## Runbooks תפעוליים

- **אינטגרציות API** – [Governance API reference](governance_api.md) מפרט את
  משטחי REST/gRPC המוצגים ע"י שירותי המועצה, כולל דרישות אימות וכללי pagination.
- **דשבורדי טלמטריה** – הגדרות Grafana JSON תחת `docs/source/grafana_*` מגדירות את
  לוחות “Governance Constraints” ו-“Scheduler TEU”. ייצאו את ה-JSON ל-Grafana אחרי
  כל release כדי להישאר מיושרים עם ה-layout הקנוני.

## פיקוח על Data Availability

### מחלקות Retention

פאנלים של הפרלמנט המאשרים manifests של DA חייבים להפנות למדיניות ה-retention
הנאכפת לפני ההצבעה. הטבלה הבאה משקפת את ברירות המחדל הנאכפות דרך
`torii.da_ingest.replication_policy` כדי שסוקרים יוכלו לזהות אי-התאמות בלי
לחפש את מקור ה-TOML.【docs/source/da/replication_policy.md:1】

| תג ממשל | מחלקת Blob | Retention חם | Retention קר | שכפולים נדרשים | מחלקת אחסון |
|----------------|------------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24 h | 14 d | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6 h | 7 d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 h | 180 d | 3 | `cold` |
| `da.default` | _כל המחלקות האחרות_ | 6 h | 30 d | 3 | `warm` |

פאנל התשתיות צריך לצרף את התבנית המלאה מתוך
`docs/examples/da_manifest_review_template.md` לכל הצבעה כדי שה-digest של
manifest, תג ה-retention וארטיפקטי Norito יישארו מקושרים ברישום הממשל.

### Audit trail למניפסט חתום

לפני שהצבעה מגיעה לסדר היום, צוותי המועצה חייבים להוכיח שבייטי ה-manifest
הנסקרים תואמים את מעטפת הפרלמנט ואת ארטיפקט SoraFS. השתמשו בכלים הקיימים כדי
לאסוף את הראיות:

1. משכו את חבילת ה-manifest מ-Torii (`iroha app da get-blob --storage-ticket <hex>`
   או helper שקול ב-SDK) כדי שכולם יגבו hash של אותם בייטים שהגיעו ל-gateways.
2. הריצו את מאמת ה-manifest stub עם המעטפה החתומה:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   זה מחשב מחדש את ה-digest של ה-manifest (BLAKE3), מאמת את
   `chunk_digest_sha3_256`, ובודק כל חתימת Ed25519 המשובצת ב-
   `manifest_signatures.json`. ראו `docs/source/sorafs/manifest_pipeline.md` עבור
   אפשרויות CLI נוספות.
3. העתיקו את ה-digest, `chunk_digest_sha3_256`, ה-handle של הפרופיל ורשימת החותמים
   לתבנית הסקירה. הערה: אם המאמת מדווח על “profile mismatch” או חסר חתימה,
   עצרו את ההצבעה ובקשו מעטפת מתוקנת.
4. אחסנו את פלט המאמת (או ארטיפקט CI מ-`ci/check_sorafs_fixtures.sh`) לצד payload
   Norito `.to` כך שמבקרים יוכלו לשחזר את הראיות ללא גישה ל-gateways פנימיים.

חבילת האודיט המתקבלת אמורה לאפשר לפרלמנט לשחזר כל בדיקת hash וחתימה גם אחרי
שה-manifest סובב החוצה מאחסון חם.

### רשימת בדיקות סקירה

1. משכו את מעטפת ה-manifest שאושרה בפרלמנט (ראו
   `docs/source/sorafs/signing_ceremony.md`) ורשמו את ה-digest מסוג BLAKE3.
2. ודאו שבלוק `RetentionPolicy` של ה-manifest תואם את התג בטבלה לעיל; Torii ידחה
   אי-התאמות, אך המועצה חייבת לתעד את הראיות למבקרים.【docs/source/da/replication_policy.md:32】
3. ודאו שה-payload של Norito שהוגש מפנה לאותו תג retention ולמחלקת blob שמופיעה
   בכרטיס הקליטה.
4. צרפו הוכחת בדיקת המדיניות (פלט CLI, dump של
   `torii.da_ingest.replication_policy`, או ארטיפקט CI) לחבילת הסקירה כדי ש-SRE
   יוכל לשחזר את ההחלטה.
5. רשמו תכניות סבסוד או התאמות שכר דירה כאשר ההצעה תלויה ב-
   `docs/source/sorafs_reserve_rent_plan.md`.

### מטריצת הסלמה

| סוג בקשה | פאנל אחראי | ראיות לצירוף | דדליינים וטלמטריה | הפניות |
|--------------|--------------|--------------------|-----------------------|------------|
| סבסוד / התאמת שכר דירה | תשתיות + אוצר | חבילת DA מלאה, דלתא rent מ-`reserve_rentd`, CSV תחזית רזרבה מעודכן, פרוטוקול הצבעה | לציין את השפעת ה-rent לפני עדכון האוצר; לכלול טלמטריית buffer ל-30 יום כדי שה-Finance יתאם בחלון הסליקה הבא | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| הורדת תוכן / פעולה ציותית | Moderation + Compliance | כרטיס ציות (`ComplianceUpdateV1`), proof tokens, digest של manifest חתום, סטטוס ערעור | לעמוד ב-SLA של תאימות gateway (אישור תוך 24h, הסרה מלאה ≤72h). לצרף קטע `TransparencyReportV1` שמציג את הפעולה. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| הקפאה/rollback חירום | פאנל המודרציה של הפרלמנט | חבילת אישור קודמת, הוראת הקפאה חדשה, digest של manifest rollback, יומן אירוע | לפרסם הודעת הקפאה מיד ולתזמן referendum rollback בסלוט הממשל הבא; לכלול טלמטריית buffer ו-DA replication כדי להצדיק את החירום. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |

השתמשו בטבלה בעת triage של כרטיסי קליטה כדי שכל פאנל יקבל את הארטיפקטים המדויקים
הדרושים למימוש המנדט שלו.

### Deliverables לדיווח

כל החלטת DA-10 חייבת לכלול את הארטיפקטים הבאים (לצרף לרשומת Governance DAG
המופנית בהצבעה):

- חבילת Markdown מלאה מתוך
  `docs/examples/da_manifest_review_template.md` (כולל כעת סעיפי חתימות והסלמה).
- manifest Norito חתום (`.to`) יחד עם מעטפת `manifest_signatures.json` או לוגים של
  מאמת CI שמוכיחים את digest ה-fetch.
- עדכוני שקיפות שנגזרו מהפעולה:
  - דלתא `TransparencyReportV1` עבור הורדות או הקפאות מונעות ציות.
  - דלתא ledger של rent/reserve או snapshot של `ReserveSummaryV1` עבור סבסודים.
- קישורים ל-snapshots של טלמטריה שנאספו במהלך הסקירה (עומק שכפול, buffer headroom,
  backlog של moderation) כדי שמשקיפים יוכלו לאמת תנאים לאחר מכן.

## Moderation & Escalation

הורדות דרך gateway, clawbacks של סבסוד, או הקפאות DA עוקבות אחרי צינור הציות
המפורט ב-`docs/source/sorafs_gateway_compliance_plan.md` וכלי הערעור ב-
`docs/source/sorafs_moderation_panel_plan.md`. הפאנלים צריכים:

1. לרשום את כרטיס הציות המקורי (`ComplianceUpdateV1` או `ModerationAppealV1`) ולצרף
   את proof tokens המשויכים.【docs/source/sorafs_gateway_compliance_plan.md:20】
2. לוודא אם הבקשה מפעילה את מסלול הערעור (הצבעת citizen panel) או הקפאת פרלמנט
   חירום; שני המסלולים חייבים לצטט את digest ה-manifest ואת תג ה-retention שנלכדו
   בתבנית החדשה.【docs/source/sorafs_moderation_panel_plan.md:1】
3. לפרט דדליינים להסלמה (חלונות commit/reveal לערעור, משך הקפאה חירומית) ולציין
   איזו מועצה או פאנל אחראים להמשך.
4. ללכוד את snapshot הטלמטריה (buffer headroom, backlog moderation) ששימש להצדקת
   הפעולה כדי שביקורות עתידיות יוכלו להתאים את ההחלטה למצב בפועל.

פאנלי ציות ומודרציה חייבים לסנכרן את דוחות השקיפות השבועיים עם מפעילי הנתב
הסליקתי כדי שהורדות וסבסודים ישפיעו על אותו סט של manifests.

## תבניות דיווח

כל סקירת DA-10 דורשת חבילת Markdown חתומה. העתיקו את
`docs/examples/da_manifest_review_template.md`, מלאו מטא-דטה של manifest, טבלת
אימות retention וסיכום הצבעת הפאנל, ואז הצמידו את המסמך המלא (עם ארטיפקטי
Norito/JSON המוזכרים) לרשומת Governance DAG. הפאנלים צריכים לקשר את החבילה
בפרוטוקולי הממשל כך שביטולים עתידיים או חידושי סבסוד יוכלו לצטט את digest ה-
manifest המקורי בלי להריץ מחדש את כל הטקס.

## זרימת Incident & Revocation

פעולות חירום מתרחשות כעת on-chain. כאשר צריך לגלגל לאחור שחרור fixture, פתחו
כרטיס ממשל והעלו הצעת revert בפרלמנט שמצביעה על digest ה-manifest שאושר בעבר.
פאנל התשתיות מנהל את ההצבעה, ולאחר סיום ה-Nexus runtime מפרסם את אירוע ה-rollback
שלקוחות downstream צורכים. אין צורך בארטיפקטי JSON מקומיים.

## שמירת המדריך עדכני

- עדכנו קובץ זה בכל פעם ש-runbook חדש שמיועד לממשל מתווסף למאגר.
- הוסיפו כאן קישורים לטקסים חדשים כדי שמפתח המועצה יישאר ניתן לגילוי.
- אם מסמך מוזכר זז (למשל נתיב SDK חדש), עדכנו את הקישור באותו PR כדי למנוע
  הפניות מיושנות.

</div>
