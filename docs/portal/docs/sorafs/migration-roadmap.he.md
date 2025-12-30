<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bfbe7de97848ee43284d448f9d80b78f68b5e95d36e0f86d2aa12c3633838867
source_last_modified: "2025-11-07T10:28:53.296738+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: "מפת דרכים להגירת SoraFS"
---

> מותאם מ-[`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# מפת דרכים להגירת SoraFS (SF-1)

מסמך זה מממש את הנחיות ההגירה המופיעות ב-
`docs/source/sorafs_architecture_rfc.md`. הוא מרחיב את תוצרי SF-1 לאבני דרך
מוכנות לביצוע, קריטריוני שער, ורשימות בדיקה לבעלים כך שצוותי storage,
governance, DevRel ו-SDK יוכלו לתאם את המעבר מאירוח artefacts legacy
לפרסום הנתמך ב-SoraFS.

מפת הדרכים דטרמיניסטית במכוון: כל אבן דרך מציינת artefacts נדרשים,
קריאות פקודה ושלבי attestation כדי שפייפלייני downstream יפיקו
תוצרים זהים וה-governance תשמור על עקבות שניתנות לביקורת.

## סקירת אבני דרך

| אבן דרך | חלון | מטרות עיקריות | חייבים לספק | בעלים |
|---------|-------|---------------|-------------|-------|
| **M0 - Bootstrap** | שבועות 1-6 | פרסום chunker fixtures דטרמיניסטיים ופרסום כפול של artefacts (legacy + SoraFS). | fixtures של `sorafs_chunker`, שילוב CLI של `sorafs_manifest_stub`, רשומות ביומן ההגירה. | Docs, DevRel, Storage |
| **M1 - Deterministic Enforcement** | שבועות 7-12 | אכיפת fixtures חתומים והכנת alias proofs בזמן שהפייפליינים מאמצים expectation flags. | אימות ליילי של fixtures, manifests חתומים ע"י המועצה, רשומות staging ב-alias registry. | Storage, Governance, SDKs |
| **M2 - Registry First** | שבועות 13-20 | ניתוב pins דרך ה-registry, הקפאת bundles legacy והצגת טלמטריית parity. | חוזה Pin Registry + CLI (`sorafs pin propose/approve`), dashboards של observability, runbooks למפעילים. | Governance, Ops, Observability |
| **M3 - Alias Only** | שבוע 21+ | פירוק אירוח legacy ודרישה ל-alias proofs לצורך שליפה. | Gateways alias-only, parity alerts, עדכון defaults של SDK, הודעת פירוק legacy. | Ops, Networking, SDKs |

סטטוס אבני הדרך מתועד ב-`docs/source/sorafs/migration_ledger.md`. כל שינוי
במפת הדרכים הזו חייב לעדכן את ה-ledger כדי לשמור על סנכרון בין governance
ל-release engineering.

## זרמי עבודה

### 1. עטיפה מחדש של נתוני legacy

| שלב | אבן דרך | תיאור | Owner(s) | פלט |
|------|---------|-------|----------|-----|
| מיפוי ותיוג | M0 | ייצוא SHA3-256 digests של bundles legacy ותיעודם ב-ledger ההגירה (append-only). | Docs, DevRel | רשומות ledger עם `source_path`, `sha3_digest`, `owner`, `planned_manifest_cid`. |
| בניה מחדש דטרמיניסטית | M0-M1 | הפעלת `sorafs_manifest_stub` לכל artefact ריליס ושמירת CAR, manifest, signature envelope ו-fetch plan ב-`artifacts/<team>/<alias>/<timestamp>/`. | Docs, CI | Bundles CAR + manifest ניתנים לשחזור לכל ריליס. |
| לולאת אימות | M1 | להריץ מחדש `sorafs_fetch` מול gateways ב-staging כדי לוודא שגבולות/digests של chunks תואמים fixtures. לרשום pass/fail בהערות ledger. | Governance QA | דוח אימות staging + GitHub issue עבור drift. |
| מעבר ל-registry | M2 | החלפת סטטוס ledger ל-`Pinned` לאחר ש-digest של manifest קיים on-chain; bundle legacy הופך לקריאה בלבד (מגיש אך לא משנה). | Governance, Ops | Hash טרנזקציית registry, כרטיס read-only ל-legacy storage. |
| Decommission | M3 | הסרת רשומות CDN legacy לאחר תקופת חסד של 30 יום, ארכוב אישורי שינוי DNS ופרסום post-mortem. | Ops | checklist decommission, רישום שינוי DNS, סגירת כרטיס incident. |

### 2. אימוץ pinning דטרמיניסטי

| שלב | אבן דרך | תיאור | Owner(s) | פלט |
|------|---------|-------|----------|-----|
| חזרות fixtures | M0 | dry-runs שבועיים המשווים digests מקומיים של chunk מול `fixtures/sorafs_chunker`. לפרסם דוח ב-`docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` עם מטריצת pass/fail. |
| אכיפת חתימות | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` נכשלות אם signatures או manifests נסחפים. overrides לפיתוח דורשים waiver ממשלתי מצורף ל-PR. | Tooling WG | לוג CI, קישור לכרטיס waiver (אם יש). |
| Expectation flags | M1 | פייפליינים קוראים ל-`sorafs_manifest_stub` עם expectations מפורשים כדי לקבע outputs: | Docs CI | סקריפטים מעודכנים שמפנים ל-expectation flags (ראו בלוק פקודה למטה). |
| Registry-first pinning | M2 | `sorafs pin propose` ו-`sorafs pin approve` עוטפים את שליחת ה-manifest; ה-CLI ברירת מחדל משתמש ב-`--require-registry`. | Governance Ops | לוג ביקורת CLI של registry, טלמטריית הצעות שנכשלו. |
| Observability parity | M3 | Dashboards של Prometheus/Grafana מתריעים כאשר מלאי chunks שונה ממסמכי registry; התראות מחוברות ל-ops on-call. | Observability | קישור לדשבורד, IDs של חוקי התראה, תוצאות GameDay. |

#### פקודת פרסום קנונית

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

החליפו את ערכי digest, גודל ו-CID ברפרנסים הצפויים הרשומים ברשומת migration ledger
של ה-artefact.

### 3. מעבר alias ותקשורת

| שלב | אבן דרך | תיאור | Owner(s) | פלט |
|------|---------|-------|----------|-----|
| Alias proofs ב-staging | M1 | רישום claims של alias ב-Pin Registry של staging וצירוף Merkle proofs ל-manifests (`--alias`). | Governance, Docs | bundle הוכחות נשמר לצד manifest + הערת ledger עם שם alias. |
| DNS כפול + הודעה | M1-M2 | הפעלת DNS legacy ו-Torii/SoraDNS במקביל; פרסום הודעות הגירה למפעילים ולערוצי SDK. | Networking, DevRel | פוסט הודעה + כרטיס שינוי DNS. |
| Proof enforcement | M2 | Gateways דוחים manifests ללא headers `Sora-Proof` עדכניים; CI מוסיף שלב `sorafs alias verify` למשיכת proofs. | Networking | פאטץ' קונפיג gateway + פלט CI עם אימות מוצלח. |
| Rollout alias-only | M3 | הסרת DNS legacy, עדכון defaults של SDK להסתמך על Torii/SoraDNS + alias proofs, ותיעוד חלון rollback. | SDK Maintainers, Ops | הערות ריליס SDK, עדכון runbook ops, תכנית rollback. |

### 4. תקשורת וביקורת

- **משמעת ledger:** כל שינוי מצב (fixture drift, registry submission, alias activation)
  חייב להוסיף הערה מתוארכת ב-`docs/source/sorafs/migration_ledger.md`.
- **Minutes של governance:** ישיבות מועצה המאשרות שינויי Pin Registry או מדיניות alias
  חייבות להפנות למפת הדרכים הזו ול-ledger.
- **תקשורת חיצונית:** DevRel מפרסם עדכוני סטטוס בכל אבן דרך (בלוג + קטע changelog)
  עם הדגשת הבטחות דטרמיניסטיות ולוחות זמנים של alias.

## תלויות וסיכונים

| תלות | השפעה | מיתון |
|------|-------|-------|
| זמינות חוזה Pin Registry | חוסם rollout M2 pin-first. | להכין את החוזה לפני M2 עם replay tests; לשמור fallback envelope עד שאין regressions. |
| מפתחות חתימה של המועצה | נדרשים עבור manifest envelopes ואישורי registry. | ceremony חתימה מתועד ב-`docs/source/sorafs/signing_ceremony.md`; לסובב מפתחות עם overlap ולהוסיף הערה ב-ledger. |
| Gateway parity tooling | נדרש לאכיפת alias proofs ו-parity של chunks. | לשחרר עדכוני gateway ב-M1, להשאיר התנהגות legacy מאחורי feature flag עד עמידה בקריטריוני M2. |
| קצב ריליס SDK | לקוחות חייבים לכבד alias proofs לפני M3. | ליישר חלונות ריליס SDK עם milestone gates; להוסיף checklists להגירה לתבניות ריליס. |

הסיכונים הנותרים והמיתונים משוכפלים ב-`docs/source/sorafs_architecture_rfc.md`
וצריכים להיות מוצלבים בעת ביצוע התאמות.

## רשימת קריטריונים ליציאה

| אבן דרך | קריטריונים |
|---------|------------|
| M0 | - כל artefacts היעד נבנו מחדש דרך `sorafs_manifest_stub` עם expectation flags. <br /> - ledger ההגירה מולא לכל משפחת artefact. <br /> - פרסום כפול (legacy + SoraFS) פעיל. |
| M1 | - job ליילי של fixtures ירוק שבעה ימים רצופים. <br /> - alias proofs ב-staging אומתו ב-CI. <br /> - governance מאשר מדיניות expectation flags. |
| M2 | - 100% מה-manifests החדשים עוברים דרך Pin Registry. <br /> - legacy storage מסומן read-only; incident playbook מאושר. <br /> - dashboards observability באוויר עם ספי התראה. |
| M3 | - gateways alias-only בפרודקשן. <br /> - DNS legacy הוסר והופיע בכרטיסי שינוי. <br /> - defaults של SDK עודכנו ופורסמו. <br /> - סטטוס סופי נוסף ל-ledger ההגירה. |

## ניהול שינוי

1. להציע התאמות דרך PR שמעדכן את הקובץ הזה **וגם**
   `docs/source/sorafs/migration_ledger.md`.
2. לקשר minutes של governance וראיות CI בתיאור ה-PR.
3. לאחר merge, להודיע לרשימת storage + DevRel עם סיכום ופעולות צפויות למפעילים.

שמירה על התהליך הזה מבטיחה ש-rollout של SoraFS יישאר דטרמיניסטי, בר-ביקורת ושקוף
בין הצוותים המשתתפים בהשקת Nexus.
