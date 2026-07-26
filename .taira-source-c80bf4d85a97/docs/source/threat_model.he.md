<!-- Hebrew translation of docs/source/threat_model.md -->

---
lang: he
direction: rtl
source: docs/source/threat_model.md
status: complete
translator: manual
---

<div dir="rtl">

# מודל האיומים של Hyperledger Iroha v2

_עודכן לאחרונה: 2025-11-07 · סקירה הבאה: 2026-02-05_

קצב תחזוקה: צוות האבטחה יחד עם בעלי רכיבים (≤90 יום). כל עדכון מתועד ב-`status.md` עם קישורי טיקטים.

## הנחות ואי-יעדים

- **הנחות:** רשת ולידטורים permissioned; עמיתים מזדהים הדדית באמצעות mTLS ופינינג מפתחות. לקוחות אינם אמינים. פשרה של אופרטור ו-DoS בקנה מידה אינטרנטי כלולים.
- **לא-יעדים:** שרשרת אספקה מלאה (מכוסה בהקשחת הבילד), עמידות קוונטית, פישינג ממשקי משתמש. שערים הקשורים לשרשרת אספקה שמושפעים מריצה מופיעים כאן.

## תחום הכיסוי

- שדרוגי runtime ומניפסטים
- קונצנזוס Sumeragi (בחירת לידר, הצבעה, RBC, טלמטריית view-change)
- API של Torii (REST/WebSocket, authN/Z, rate-limit)
- מצורפים ו-payloads (טרנזקציות, מניפסטים, BLOB גדולים)
- צנרת ZK (ייצור, אימות, ממשל מעגלים)
- ניהול מפתחות וזהויות (ולידטורים, אופרטורים, חתימות שחרור)
- רשת וטרנספורט (P2P/ingress, ניהול חיבורים, DoS)
- טלמטריה ורישום (יצוא, פרטיות, שלמות)
- חברות וממשל (שינויים בסט הולידטורים, דחייה, סטיות קונפיג)

איום: עמיתים ביזנטיים, לקוחות זדוניים, אופרטורים שנפרצו, תוקפי DoS. איומי שרשרת אספקה/קומפיילר מטופלים בבילד-הקשחה.

## סיווג נכסים

| נכס | תיאור | שלמות | זמינות | סודיות | בעלים |
| --- | --- | --- | --- | --- | --- |
| מצב Ledger | היסטוריה משוכפלת קנונית | Critical | Critical | Moderate | Core WG |
| מניפסטים וארטיפקטים | סקריפטים, בינאריים, attestations | Critical | High | High | Runtime WG |
| מפתחות חתימה לשחרור | מאשרים מניפסטים/שחרורים | Critical | High | High | Security WG |
| מפתחות קונצנזוס | הצבעה/Commit | Critical | High | High | ולידטורים |
| אישורי לקוחות | מפתחות API, הקשרים | High | High | Critical | Torii WG |
| מרשם חברות | רשימת עמיתים לפר epoch | Critical | High | Moderate | Consensus WG |
| אחסון מצורפים | BLOBs וייחוס on-chain | High | High | Moderate | Runtime WG |
| קודקים Norito/Kotodama | כללי סריאליזציה | Critical | High | Moderate | Data Model WG |
| חומר ZK | מעגלים, מפתחות, עדים | High | Moderate | High | ZK WG |
| טלמטריה ולוגים | מדדים, בריאות, אבטחה | Moderate | High | Low/Moderate | Observability WG |
| קונפיג וסודות נוד | mTLS, טוקנים, קונפיג | High | High | High | Ops |

## גבולות אמון

- גבול עמיתים (mTLS; עמיתים עלולים להיות ביזנטיים)
- גבול קליינט (Torii REST/WS)
- גבול שליטה על שדרוגים (מניפסטים/חתימות/קונפיג)
- גבול מצורפים (Torii/גוסיפ)
- גבול עומס ZK (אוף-צ׳יין)
- גבול יצוא טלמטריה
- גבול ממשל חברות

## מדרג סיכון

- **הסתברות:** Rare / Unlikely / Possible / Likely / Almost certain
- **חומרה:** Low / Moderate / High / Critical
- **סיכון:** קריטי/גבוה מחייב הקלה לפני GA (אלא אם התקבל רשמית). גורמים מתועדים ב-`status.md`.
- **יעדי תגובה:** P1 ≤7 ימים, P2 ≤30, P3 ≤90 (אם לא התקבל).

## תרחישי איום

**Runtime Upgrades** – בקרות קיימות: אימות חתימות, אימות `abi_hash`/`code_hash`, ממשל, אכיפת SBOM/SLSA בזמן קבלה. פערים: הפרדת מפתחות חתימה.

**Sumeragi Consensus** – ... (כמו מקור)

**Torii API** – הגנות כניסה מוגבלות; פערים: בקרות DoS לפני אימות.

**מצורפים** – ...

**ZK Pipeline** – ...

**מפתחות וזהויות** – ...

**רשת וטרנספורט** – ...

**טלמטריה ולוגים** – ...

**זמן ואקראיות** – ...

## סיכונים שנותרו

| סיכון | מצב | תוכנית | בעלים | יעד |
| --- | --- | --- | --- | --- |
| Upgrade SBOM provenance gap | סגור | אכיפת SBOM/SLSA וחתימות בזמן קבלה (ראו `docs/source/runtime_upgrades.md`). | Security WG | 2025-11-30 |
| Aggregator fairness audit | פתוח | ביקורת צד ג' לפני GA (`SUM-203`) | Consensus WG | 2025-12-15 |
| Torii operator auth hardening | סגור | WebAuthn/mTLS לאופרטור, התמדה של אישורים, טוקני סשן וטלמטריה | Torii WG | 2025-11-15 |
| Hardware-accelerated hashing | פתוח | Multi-version עם fallback (`RNT-092`) | Runtime WG | 2025-12-01 |
| ZK circuit governance | פתוח | פרוטוקול ממשל (`ZK-077`) | ZK WG | 2025-11-20 |
| Validator key HSM adoption | פתוח | מדיניות HSM (טיקט TBD) | Security WG | 2025-11-15 |
| Release-signing key separation | פתוח | Root אופליין וחתימה סף (TBD) | Security WG | 2025-10-31 |
| Membership registry reconciliation | פתוח | View-hash enforcement (`SUM-203`) | Consensus WG | 2025-10-25 |
| Pre-auth DoS controls | פתוח | gating, cap, tuning | Torii & Core WG | 2025-10-31 |
| Telemetry redaction policy | פתוח | lints/CI | Observability WG | 2025-10-20 |
| Time and NTP hardening | פתוח | NTS/מקורות מרובים | Runtime WG & Ops | 2025-11-10 |
| Membership mismatch telemetry | פתוח | metric + alerting | Consensus WG | 2025-10-15 |
| Attachment sanitisation | סגור | sniffing + subprocess + סניטציה מחדש ביצוא | Runtime WG | 2025-11-30 |
| Witness retention audit | פתוח | בדיקה אוטומטית | ZK WG | 2025-11-05 |
| Peer churn telemetry | בתהליך | Alerts ודשבורדים | Core WG | 2025-10-25 |

## תהליך סקירה

1. סקירת אבטחה ≤90 יום ולפני RC.
2. בעלי רכיבים מעדכנים בקרות.
3. המסמך מעודכן והסיכום ב-`status.md`.
4. אירועים → נספח בתוך 7 ימים.

## חתימות

| WG | POC | מצב | הערות |
| --- | --- | --- | --- |
| Security | security@iroha | בהמתנה | תגובה עד 2025-10-05 |
| Core | core@iroha | בהמתנה | אימות DoS ו-churn |
| Runtime | runtime@iroha | בהמתנה | אישור השלמת סניטיזציה |
| Torii | torii@iroha | בהמתנה | אימות הקשחת auth (`TOR-118`) ותוכנית gating |
| Consensus | consensus@iroha | בהמתנה | Audit + telemetry |
| Data Model | data-model@iroha | בהמתנה | Norito/Kotodama |
| ZK | zk@iroha | בהמתנה | Governance + witness |
| Observability | observability@iroha | בהמתנה | Redaction + logging |

</div>
