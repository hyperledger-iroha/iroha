---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/reports/sf6-security-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b826108d418ac78474c91beab84f07f59be91fbea6548d483bc81e04fc147c0f
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# סקירת אבטחה SF-6

**חלון ההערכה:** 2026-02-10 → 2026-02-18  
**מובילי סקירה:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**היקף:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs של proof streaming, טיפול ב-manifests ב-Torii, אינטגרציית Sigstore/OIDC, ו-CI release hooks.  
**ארטיפקטים:**  
- מקור CLI ובדיקות (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- handlers של manifest/proof ב-Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- אוטומציית release (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harness פריטי דטרמיניסטי (`crates/sorafs_car/tests/sorafs_cli.rs`, [דו"ח פריטי GA של SoraFS Orchestrator](./orchestrator-ga-parity.md))

## מתודולוגיה

1. **סדנאות threat modelling** מיפו יכולות תוקף עבור תחנות עבודה של מפתחים, מערכות CI, ו-node-ים של Torii.  
2. **Code review** התמקד במשטחי הרשאות (OIDC token exchange, keyless signing), אימות Norito manifests ו-back-pressure ב-proof streaming.  
3. **Dynamic testing** שיחזר fixture manifests וסימולציות כשל (token replay, manifest tampering, proof streams מקוצרות) בעזרת parity harness ו-fuzz drives ייעודיים.  
4. **Configuration inspection** אימתה defaults של `iroha_config`, טיפול בדגלי CLI, וסקריפטי release כדי להבטיח ריצות דטרמיניסטיות ונבדקות.  
5. **Process interview** אישרה remediation flow, escalation paths ולכידת audit evidence עם release owners של Tooling WG.

## סיכום ממצאים

| ID | חומרה | תחום | ממצא | פתרון |
|----|----------|------|---------|------------|
| SF6-SR-01 | גבוהה | Keyless signing | ברירות המחדל של audience ל-OIDC tokens היו מרומזות בתבניות CI, עם סיכון ל-cross-tenant replay. | נוספה אכיפה מפורשת של `--identity-token-audience` ב-release hooks ובתבניות CI ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). ה-CI נכשל כעת אם audience חסר. |
| SF6-SR-02 | בינונית | Proof streaming | מסלולי back-pressure קיבלו buffers לא מוגבלים למנויים, מה שאפשר מיצוי זיכרון. | `sorafs_cli proof stream` אוכף גדלי channel מוגבלים עם truncation דטרמיניסטי, רושם Norito summaries ומפסיק את ה-stream; ה-mirror ב-Torii עודכן להגביל response chunks (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | בינונית | שליחת manifests | ה-CLI קיבל manifests בלי לאמת chunk plans משובצים כאשר `--plan` חסר. | `sorafs_cli manifest submit` מחשב ומבצע השוואת CAR digests אלא אם `--expect-plan-digest` סופק, דוחה mismatches ומציג remediation hints. בדיקות מכסות הצלחה/כשל (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | נמוכה | Audit trail | רשימת בדיקות ה-release חסרה יומן אישור חתום לסקירת האבטחה. | נוספה ב-[release process](../developer-releases.md) דרישה לצרף hashes של memo הסקירה ו-URL של כרטיס sign-off לפני GA. |

כל הממצאים high/medium תוקנו במהלך חלון הסקירה ואומתו באמצעות parity harness הקיים. לא נותרו בעיות קריטיות חבויות.

## אימות בקרות

- **היקף הרשאות:** תבניות CI מחייבות audience ו-issuer מפורשים; ה-CLI וה-release helper נכשלים מיד אם `--identity-token-audience` לא מלווה את `--identity-token-provider`.  
- **Replay דטרמיניסטי:** בדיקות מעודכנות מכסות זרימות חיוביות/שליליות של שליחת manifests, ומבטיחות ש-digests לא תואמים נשארים כשגיאות לא דטרמיניסטיות ומזוהים לפני פניה לרשת.  
- **Back-pressure של proof streaming:** Torii משדר כעת פריטי PoR/PoTR בערוצים מוגבלים, וה-CLI שומר רק דגימות latency מקוצרות + חמישה דוגמאות כשל, מונע גדילה לא מוגבלת תוך שמירה על summaries דטרמיניסטיים.  
- **Observability:** מוני proof streaming (`torii_sorafs_proof_stream_*`) ו-CLI summaries לוכדים סיבות abort ומספקים breadcrumbs של audit למפעילים.  
- **Documentation:** מדריכי מפתחים ([developer index](../developer-index.md), [CLI reference](../developer-cli.md)) מציינים דגלים רגישים ואפיקי הסלמה.

## תוספות ל-release checklist

מנהלי release **חייבים** לצרף את הראיות הבאות בעת קידום מועמד GA:

1. Hash של memo סקירת האבטחה האחרון (מסמך זה).  
2. קישור ל-ticket remediation במעקב (לדוגמה, `governance/tickets/SF6-SR-2026.md`).  
3. פלט של `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` שמציג פרמטרים מפורשים audience/issuer.  
4. לוגים של parity harness (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. אישור ש-release notes של Torii כוללים מוני טלמטריה ל-bounded proof streaming.

אי-איסוף הארטיפקטים לעיל חוסם sign-off ל-GA.

**Reference hashes לארטיפקטים (sign-off 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Follow-ups פתוחים

- **רענון threat model:** לחזור על סקירה זו בכל רבעון או לפני הוספת דגלי CLI גדולים.  
- **כיסוי fuzzing:** קידודי ה-transport של proof streaming עוברים fuzzing דרך `fuzz/proof_stream_transport`, כולל payloads identity, gzip, deflate, ו-zstd.  
- **תרגול אירועים:** לתכנן תרגיל מפעילים המדמה token compromise ו-manifest rollback, כדי לוודא שהתיעוד משקף נהלים מתורגלים.

## אישור

- נציג Security Engineering Guild: @sec-eng (2026-02-20)  
- נציג Tooling Working Group: @tooling-wg (2026-02-20)

שמרו את האישורים החתומים לצד bundle של release artefacts.
