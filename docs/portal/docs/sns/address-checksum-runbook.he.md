---
lang: he
direction: rtl
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d15f10764ee430825f4d7caa142c6664016d3934ce7c0da68314a4a4a6f700bd
source_last_modified: "2025-11-15T15:21:14.374476+00:00"
translation_last_reviewed: 2026-01-01
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sns/address_checksum_failure_runbook.md`. עדכנו קודם את קובץ המקור, ואז סנכרנו את העותק הזה.
:::

כשלים של checksum מופיעים כ-`ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) ברחבי Torii, SDKs ולקוחות wallet/explorer. פריטי מפת הדרכים ADDR-6/ADDR-7 דורשים כעת מהמפעילים לפעול לפי runbook זה בכל פעם שמופעלות התראות checksum או נפתחים כרטיסי תמיכה.

## מתי להריץ את ה-play

- **התראות:** `AddressInvalidRatioSlo` (מוגדר ב-`dashboards/alerts/address_ingest_rules.yml`) מופעל וההערות כוללות `reason="ERR_CHECKSUM_MISMATCH"`.
- **סטיה של fixtures:** קובץ הטקסט של Prometheus `account_address_fixture_status` או לוח Grafana מדווח על checksum mismatch עבור כל עותק SDK.
- **הסלמות תמיכה:** צוותי wallet/explorer/SDK מציינים שגיאות checksum, השחתת IME או סריקות לוח גזירה שכבר אינן מתפענחות.
- **תצפית ידנית:** לוגי Torii מציגים שוב ושוב `address_parse_error=checksum_mismatch` עבור נקודות קצה פרודקשן.

אם האירוע קשור במיוחד להתנגשויות Local-8/Local-12, פעלו לפי ה-playbooks `AddressLocal8Resurgence` או `AddressLocal12Collision` במקום זאת.

## רשימת ראיות

| ראיה | פקודה / מיקום | הערות |
|------|----------------|-------|
| Snapshot Grafana | `dashboards/grafana/address_ingest.json` | צלמו חלוקה של סיבות שגיאה ונקודות קצה מושפעות. |
| Payload של התראה | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | כללו תגיות הקשר וחותמות זמן. |
| בריאות fixtures | `artifacts/account_fixture/address_fixture.prom` + Grafana | מוכיח אם עותקי SDK סטו מ-`fixtures/account/address_vectors.json`. |
| שאילתת PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | יצוא CSV למסמך האירוע. |
| לוגים | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (או אגרגצית לוגים) | נקו PII לפני שיתוף. |
| אימות fixture | `cargo xtask address-vectors --verify` | מאשר שהמחולל הקנוני וה-JSON המחויב תואמים. |
| בדיקת תאימות SDK | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | הריצו לכל SDK שדווח בהתראות/כרטיסים. |
| תקינות לוח גזירה/IME | `iroha address inspect <literal>` | מזהה תווים חבויים או שכתובים של IME; ציינו `address_display_guidelines.md`. |

## תגובה מיידית

1. אשרו את ההתראה, קשרו snapshots של Grafana + פלט PromQL בשרשור האירוע, וציינו את הקשרי Torii שנפגעו.
2. הקפיאו הפצות manifest / גרסאות SDK שמשפיעות על ניתוח כתובות.
3. שמרו snapshots של הדשבורד וארטיפקטי Prometheus textfile בתיקית האירוע (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. אספו דגימות לוגים שמציגות payloads של `checksum_mismatch`.
5. עדכנו את בעלי ה-SDK (`#sdk-parity`) עם payloads לדוגמה כדי שיוכלו לבצע triage.

## בידוד גורם שורש

### סטיה של fixture או מחולל

- הריצו שוב `cargo xtask address-vectors --verify`; בצעו יצירה מחדש אם נכשל.
- הריצו `ci/account_fixture_metrics.sh` (או `scripts/account_fixture_helper.py check` בנפרד) לכל SDK ואשרו שה-fixtures הכלולים תואמים ל-JSON הקנוני.

### רגרסיות מקודדים בלקוח / IME

- בדקו literals שסופקו על ידי משתמשים דרך `iroha address inspect` כדי למצוא joins ברוחב אפס, המרות kana או payloads קטועים.
- השוו זרימות wallet/explorer מול `docs/source/sns/address_display_guidelines.md` (יעדי העתקה כפולה, אזהרות, עזרי QR) כדי לוודא שהן עוקבות אחר ה-UX המאושר.

### בעיות manifest או registry

- עקבו אחרי `address_manifest_ops.md` כדי לאמת מחדש את manifest bundle האחרון ולוודא שמסנני Local-8 לא חזרו.

### תעבורה זדונית או פגומה

- נתחו IPs/app IDs פוגעניים דרך לוגי Torii ו-`torii_http_requests_total`.
- שמרו לפחות 24 שעות של לוגים למעקב Security/Governance.

## הקלה ושיקום

| תרחיש | פעולות |
|-------|--------|
| סטיה של fixture | צרו מחדש `fixtures/account/address_vectors.json`, הריצו שוב `cargo xtask address-vectors --verify`, עדכנו חבילות SDK, וצירפו snapshots של `address_fixture.prom` לכרטיס. |
| רגרסיה ב-SDK/לקוח | פתחו issues שמפנות ל-fixture הקנוני + פלט `iroha address inspect`, וחסמו releases באמצעות CI לפריטי SDK (למשל `ci/check_address_normalize.sh`). |
| שליחות זדוניות | הגבילו בקצב או חסמו principals פוגעניים, והסלימו ל-Governance אם נדרש tombstone למסננים. |

לאחר שההקלות הוטמעו, הריצו שוב את שאילתת ה-PromQL שלמעלה כדי לוודא ש-`ERR_CHECKSUM_MISMATCH` נשאר באפס (למעט `/tests/*`) במשך לפחות 30 דקות לפני הורדת האירוע.

## סגירה

1. ארכבו snapshots של Grafana, CSV של PromQL, קטעי לוגים ו-`address_fixture.prom`.
2. עדכנו את `status.md` (סעיף ADDR) ואת שורת ה-roadmap אם השתנו כלים/מסמכים.
3. הוסיפו תובנות לאחר אירוע תחת `docs/source/sns/incidents/` כאשר מתגלות מסקנות חדשות.
4. ודאו שהערות ה-release של ה-SDK מזכירות תיקוני checksum כשזה רלוונטי.
5. אשרו שההתראה נשארת ירוקה 24h ושהבדיקות של fixture נשארות ירוקות לפני הסגירה.
