---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 49d68f883ec4fb0dbbe6f1ebe6a6fd1405dadf4279b86bc988a0f39f0362ee23
source_last_modified: "2025-11-07T12:19:17.315191+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
דף זה משקף את תכנית ה-rollout של SNNet-10 ב-`docs/source/soranet/testnet_rollout_plan.md`. שמרו על שתי הגרסאות מסונכרנות עד שהדוקס הישנים יפרשו.
:::

SNNet-10 מתאם את ההפעלה המדורגת של שכבת האנונימיות של SoraNet ברחבי הרשת. השתמשו בתכנית הזו כדי לתרגם את הבולט של ה-roadmap ל-deliverables קונקרטיים, runbooks ושערי telemetry כך שכל operator יבין את הציפיות לפני ש-SoraNet הופכת לברירת המחדל של התעבורה.

## שלבי השקה

| שלב | ציר זמן (יעד) | היקף | artefacts נדרשים |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet סגור** | Q4 2026 | 20-50 relays על >=3 ASNs המופעלים על ידי תורמים ליבה. | Testnet onboarding kit, smoke suite ל-guard pinning, baseline ל-latency + metrics של PoW, ו-log של brownout drill. |
| **T1 - Public Beta** | Q1 2027 | >=100 relays, guard rotation מופעל, exit bonding נאכף, ו-SDK betas ברירת מחדל ל-SoraNet עם `anon-guard-pq`. | Onboarding kit מעודכן, checklist לאימות מפעילים, SOP לפרסום directory, חבילת dashboards ל-telemetry, ודוחות rehearsal לתקריות. |
| **T2 - Mainnet ברירת מחדל** | Q2 2027 (מותנה בהשלמת SNNet-6/7/9) | רשת הייצור עוברת לברירת מחדל SoraNet; transport מסוג obfs/MASQUE ואכיפת PQ ratchet מופעלים. | פרוטוקולי אישור governance, נוהל rollback direct-only, אזעקות downgrade, ודוח metrics חתום להצלחה. |

אין **מסלול דילוג** - כל שלב חייב לספק את ה-telemetry וה-artefacts של governance מהשלב הקודם לפני הקידום.

## Kit הצטרפות ל-testnet

כל operator relay מקבל חבילה דטרמיניסטית עם הקבצים הבאים:

| Artefact | תיאור |
|----------|-------------|
| `01-readme.md` | סקירה, נקודות קשר, וציר זמן. |
| `02-checklist.md` | Checklist לפני העליה (חומרה, reachability רשתית, אימות guard policy). |
| `03-config-example.toml` | תצורת relay + orchestrator מינימלית של SoraNet המיושרת לבלוקי compliance של SNNet-9, כולל בלוק `guard_directory` שמצמיד את hash של guard snapshot האחרון. |
| `04-telemetry.md` | הוראות לחיבור dashboards של SoraNet privacy metrics וספי alert. |
| `05-incident-playbook.md` | נוהל תגובה ל-brownout/downgrade עם מטריצת הסלמה. |
| `06-verification-report.md` | תבנית שמפעילים ממלאים ומחזירים לאחר מעבר smoke tests. |

עותק מרונדר נמצא ב-`docs/examples/soranet_testnet_operator_kit/`. כל קידום מרענן את ה-kit; מספרי גרסה עוקבים אחרי השלב (למשל, `testnet-kit-vT0.1`).

למפעילי public-beta (T1), ה-brief התמציתי ב-`docs/source/soranet/snnet10_beta_onboarding.md` מסכם דרישות מקדימות, deliverables של telemetry וזרימת ההגשה תוך הפניה ל-kit הדטרמיניסטי ול-helpers של אימות.

`cargo xtask soranet-testnet-feed` מייצר JSON feed שמאגד את חלון הקידום, roster של relays, דוח metrics, evidence של drills, ו-hashes של קבצים מצורפים שמוזכרים בתבנית stage-gate. חתמו על logs של drill ועל attachments עם `cargo xtask soranet-testnet-drill-bundle` קודם כדי שה-feed ירשום `drill_log.signed = true`.

## מדדי הצלחה

הקידום בין שלבים מותנה ב-telemetry הבאה, שנאספת לפחות שבועיים:

- `soranet_privacy_circuit_events_total`: 95% מה-circuits מסתיימים ללא אירועי brownout או downgrade; 5% הנותרים מוגבלים על ידי אספקת PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: פחות מ-1% מסשני fetch ביום מפעילים brownout מחוץ ל-drills מתוכננים.
- `soranet_privacy_gar_reports_total`: סטיה בתוך +/-10% מתמהיל קטגוריות GAR הצפוי; קפיצות חייבות להיות מוסברות בעדכוני policy מאושרים.
- שיעור הצלחת PoW tickets: >=99% בתוך חלון יעד של 3 שניות; מדווח דרך `soranet_privacy_throttles_total{scope="congestion"}`.
- latency (95th percentile) לכל אזור: <200 ms לאחר שה-circuits הושלמו, נאסף דרך `soranet_privacy_rtt_millis{percentile="p95"}`.

תבניות dashboards ו-alerts נמצאות ב-`dashboard_templates/` וב-`alert_templates/`; שיקפו אותן במאגר telemetry והוסיפו אותן לבדיקות lint של CI. השתמשו ב-`cargo xtask soranet-testnet-metrics` כדי להפיק את הדוח עבור governance לפני בקשת הקידום.

הגשות stage-gate חייבות לעקוב אחרי `docs/source/soranet/snnet10_stage_gate_template.md`, שמקשר לטופס Markdown המוכן להעתקה תחת `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist לאימות

המפעילים חייבים לאשר את הבאים לפני כניסה לכל שלב:

- ✅ Relay advert חתום עם admission envelope הנוכחי.
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) עובר.
- ✅ `guard_directory` מצביע על ה-artefact האחרון של `GuardDirectorySnapshotV2` ו-`expected_directory_hash_hex` תואם ל-committee digest (הפעלת relay רושמת את ה-hash המאומת).
- ✅ מדדי PQ ratchet (`sorafs_orchestrator_pq_ratio`) נשארים מעל ספי היעד לשלב המבוקש.
- ✅ תצורת compliance של GAR תואמת ל-tag האחרון (ראו קטלוג SNNet-9).
- ✅ סימולציית אזעקת downgrade (כיבוי collectors, ציפיה להתראה בתוך 5 דקות).
- ✅ Drill של PoW/DoS בוצע עם צעדי מיתון מתועדים.

תבנית מוכנה מראש כלולה ב-kit ההצטרפות. המפעילים מגישים את הדוח המלא למוקד התמיכה של governance לפני קבלת אישורי ייצור.

## Governance ודיווח

- **Change control:** קידומים דורשים אישור Governance Council המתועד בפרוטוקולי המועצה ומצורף לעמוד הסטטוס.
- **Status digest:** לפרסם עדכונים שבועיים המסכמים את מספר ה-relays, יחס PQ, תקריות brownout ו-action items פתוחים (מאוחסן ב-`docs/source/status/soranet_testnet_digest.md` לאחר התחלת הקצב).
- **Rollbacks:** לשמור על תכנית rollback חתומה שמחזירה את הרשת לשלב הקודם בתוך 30 דקות, כולל פסילת DNS/guard cache ותבניות תקשורת ל-clients.

## נכסים תומכים

- `cargo xtask soranet-testnet-kit [--out <dir>]` מייצר את kit ההצטרפות מתוך `xtask/templates/soranet_testnet/` אל ספריית היעד (ברירת מחדל `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` מעריך את מדדי ההצלחה של SNNet-10 ומפיק דוח pass/fail מובנה המתאים לסקירות governance. snapshot לדוגמה נמצא ב-`docs/examples/soranet_testnet_metrics_sample.json`.
- תבניות Grafana ו-Alertmanager נמצאות תחת `dashboard_templates/soranet_testnet_overview.json` ו-`alert_templates/soranet_testnet_rules.yml`; העתיקו אותן למאגר telemetry או חברו אותן לבדיקות lint ב-CI.
- תבנית תקשורת downgrade עבור הודעות SDK/portal נמצאת ב-`docs/source/soranet/templates/downgrade_communication_template.md`.
- digests שבועיים של status צריכים להשתמש ב-`docs/source/status/soranet_testnet_weekly_digest.md` כטופס קנוני.

Pull requests צריכים לעדכן את הדף הזה לצד כל שינוי ב-artefacts או telemetry כדי שתכנית ה-rollout תישאר קנונית.
